// Allocation Strategies Contract
#![no_std]

use shared_utils::RateLimiter;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env, Map, Symbol, Vec,
};

pub const CURRENT_VERSION: u32 = 1;

// ============================================================================
// ERROR CODES - Error Handling
// ============================================================================
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InvalidAmount = 4,
    PoolNotFound = 5,
    PoolInactive = 6,
    PoolCapacityExceeded = 7,
    NoSuitablePools = 8,
    AllocationNotFound = 9,
    InvalidPoolId = 10,
    InvalidAPY = 11,
    InvalidCapacity = 12,
    ArithmeticOverflow = 13,
    ReentrancyDetected = 14,
    InvalidWasmHash = 15,
    InvalidVersion = 16,
    AlreadyMigrated = 17,
}

// ============================================================================
// DATA STRUCTURES
// ============================================================================

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Strategy {
    Safe,
    Balanced,
    Aggressive,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Pool {
    pub pool_id: u32,
    pub risk_level: RiskLevel,
    pub apy: u32,
    pub total_liquidity: i128,
    pub max_capacity: i128,
    pub active: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Allocation {
    pub commitment_id: u64,
    pub pool_id: u32,
    pub amount: i128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct AllocationSummary {
    pub commitment_id: u64,
    pub strategy: Strategy,
    pub total_allocated: i128,
    pub allocations: Vec<Allocation>,
}

// ============================================================================
// STORAGE KEYS
// ============================================================================

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Pool(u32),
    Allocations(u64),
    Strategy(u64),
    CommitmentCore,
    Admin,
    Initialized,
    ReentrancyGuard,
    PoolRegistry,          // Vec<u32> of all pool IDs
    TotalAllocated(u64),   // Total amount allocated per commitment
    AllocationOwner(u64),  // Track allocation ownership
    Version,               // Contract version
}

// ============================================================================
// MAIN CONTRACT
// ============================================================================

#[contract]
pub struct AllocationStrategiesContract;

#[contractimpl]
impl AllocationStrategiesContract {
    // ========================================================================
    // INITIALIZATION
    // ========================================================================

    pub fn initialize(env: Env, admin: Address, commitment_core: Address) -> Result<(), Error> {
        // Check if already initialized
        if env.storage().instance().has(&DataKey::Initialized) {
            return Err(Error::AlreadyInitialized);
        }

        // Validate addresses
        admin.require_auth();

        // Set storage
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::CommitmentCore, &commitment_core);
        env.storage().instance().set(&DataKey::Initialized, &true);
        env.storage().instance().set(&DataKey::PoolRegistry, &Vec::<u32>::new(&env));
        env.storage().instance().set(&DataKey::Version, &CURRENT_VERSION);
        
        // Emit initialization event
        env.events()
            .publish((symbol_short!("init"), symbol_short!("alloc")), admin);

        Ok(())
    }

    // ========================================================================
    // ADMIN FUNCTIONS
    // ========================================================================

    pub fn register_pool(
        env: Env,
        admin: Address,
        pool_id: u32,
        risk_level: RiskLevel,
        apy: u32,
        max_capacity: i128,
    ) -> Result<(), Error> {
        admin.require_auth();
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &admin)?;
        Self::require_no_reentrancy(&env)?;

        // Input validation
        if max_capacity <= 0 {
            return Err(Error::InvalidCapacity);
        }

        if apy > 100_000 {
            // Max 1000% APY (10000 basis points = 100%)
            return Err(Error::InvalidAPY);
        }

        // Check if pool already exists
        if env.storage().persistent().has(&DataKey::Pool(pool_id)) {
            return Err(Error::InvalidPoolId);
        }

        let pool = Pool {
            pool_id,
            risk_level,
            apy,
            total_liquidity: 0,
            max_capacity,
            active: true,
            created_at: env.ledger().timestamp(),
            updated_at: env.ledger().timestamp(),
        };

        // Store pool
        env.storage()
            .persistent()
            .set(&DataKey::Pool(pool_id), &pool);

        // Add to registry
        let mut registry: Vec<u32> = env
            .storage()
            .instance()
            .get(&DataKey::PoolRegistry)
            .unwrap_or(Vec::new(&env));
        registry.push_back(pool_id);
        env.storage()
            .instance()
            .set(&DataKey::PoolRegistry, &registry);

        // Emit event
        env.events()
            .publish((symbol_short!("pool_reg"), pool_id), risk_level);

        Ok(())
    }

    pub fn update_pool_status(
        env: Env,
        admin: Address,
        pool_id: u32,
        active: bool,
    ) -> Result<(), Error> {
        admin.require_auth();
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &admin)?;
        Self::require_no_reentrancy(&env)?;

        let mut pool = Self::get_pool_internal(&env, pool_id)?;
        pool.active = active;
        pool.updated_at = env.ledger().timestamp();

        env.storage()
            .persistent()
            .set(&DataKey::Pool(pool_id), &pool);

        env.events()
            .publish((symbol_short!("pool_upd"), pool_id), active);

        Ok(())
    }

    pub fn update_pool_capacity(
        env: Env,
        admin: Address,
        pool_id: u32,
        new_capacity: i128,
    ) -> Result<(), Error> {
        admin.require_auth();
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &admin)?;

        if new_capacity <= 0 {
            return Err(Error::InvalidCapacity);
        }

        let mut pool = Self::get_pool_internal(&env, pool_id)?;

        // Ensure new capacity is not less than current liquidity
        if new_capacity < pool.total_liquidity {
            return Err(Error::PoolCapacityExceeded);
        }

        pool.max_capacity = new_capacity;
        pool.updated_at = env.ledger().timestamp();

        env.storage()
            .persistent()
            .set(&DataKey::Pool(pool_id), &pool);

        Ok(())
    }

    // ========================================================================
    // CORE ALLOCATION FUNCTIONS
    // ========================================================================

    /// Allocate funds according to strategy
    ///
    /// # Formal Verification
    /// **Preconditions:**
    /// - Contract is initialized
    /// - `amount > 0`
    /// - `reentrancy_guard == false`
    /// - No existing allocation for `commitment_id`
    ///
    /// **Postconditions:**
    /// - `get_allocation(commitment_id).total_allocated == amount`
    /// - For all pools P: `P.total_liquidity <= P.max_capacity`
    /// - `reentrancy_guard == false`
    ///
    /// **Invariants Maintained:**
    /// - INV-4: Reentrancy guard invariant
    /// - Pool capacity never exceeded
    ///
    /// **Security Properties:**
    /// - SP-1: Reentrancy protection
    /// - SP-3: Arithmetic safety (overflow checks)
    pub fn allocate(
        env: Env,
        caller: Address,
        commitment_id: u64,
        amount: i128,
        strategy: Strategy,
    ) -> Result<AllocationSummary, Error> {
        caller.require_auth();
        Self::require_initialized(&env)?;
        Self::require_no_reentrancy(&env)?;

        // Rate limit allocations per caller address
        let fn_symbol = symbol_short!("alloc");
        RateLimiter::check(&env, &caller, &fn_symbol);

        // Set reentrancy guard
        Self::set_reentrancy_guard(&env, true);

        // Input validation
        if amount <= 0 {
            Self::set_reentrancy_guard(&env, false);
            return Err(Error::InvalidAmount);
        }

        // Check for existing allocation (prevent double allocation)
        if env
            .storage()
            .persistent()
            .has(&DataKey::Allocations(commitment_id))
        {
            Self::set_reentrancy_guard(&env, false);
            return Err(Error::AlreadyInitialized);
        }

        // Store allocation ownership
        env.storage()
            .persistent()
            .set(&DataKey::AllocationOwner(commitment_id), &caller);

        // Store the strategy
        env.storage()
            .persistent()
            .set(&DataKey::Strategy(commitment_id), &strategy);

        // Get pools based on strategy
        let pools = Self::select_pools(&env, strategy)?;

        if pools.is_empty() {
            Self::set_reentrancy_guard(&env, false);
            return Err(Error::NoSuitablePools);
        }

        // Calculate allocation amounts with overflow protection
        let allocation_plan = Self::calculate_allocation(&env, amount, &pools, strategy)?;

        // Execute allocations
        let mut allocations = Vec::new(&env);
        let mut total_allocated = 0i128;

        for (pool_id, alloc_amount) in allocation_plan.iter() {
            if alloc_amount <= 0 {
                continue;
            }

            // Update pool liquidity with overflow check
            let mut pool = Self::get_pool_internal(&env, pool_id)?;

            // Check pool is active
            if !pool.active {
                Self::set_reentrancy_guard(&env, false);
                return Err(Error::PoolInactive);
            }

            // Safe addition with overflow check
            let new_liquidity = pool
                .total_liquidity
                .checked_add(alloc_amount)
                .ok_or(Error::ArithmeticOverflow)?;

            if new_liquidity > pool.max_capacity {
                Self::set_reentrancy_guard(&env, false);
                return Err(Error::PoolCapacityExceeded);
            }

            pool.total_liquidity = new_liquidity;
            pool.updated_at = env.ledger().timestamp();
            env.storage()
                .persistent()
                .set(&DataKey::Pool(pool_id), &pool);

            // Record allocation
            let allocation = Allocation {
                commitment_id,
                pool_id,
                amount: alloc_amount,
                timestamp: env.ledger().timestamp(),
            };

            allocations.push_back(allocation);

            // Safe addition
            total_allocated = total_allocated
                .checked_add(alloc_amount)
                .ok_or(Error::ArithmeticOverflow)?;
        }

        // Verify total matches requested amount
        if total_allocated != amount {
            Self::set_reentrancy_guard(&env, false);
            return Err(Error::ArithmeticOverflow);
        }

        // Store allocations
        env.storage()
            .persistent()
            .set(&DataKey::Allocations(commitment_id), &allocations);
        env.storage()
            .persistent()
            .set(&DataKey::TotalAllocated(commitment_id), &total_allocated);

        // Clear reentrancy guard
        Self::set_reentrancy_guard(&env, false);

        // Emit event
        env.events().publish(
            (symbol_short!("allocate"), commitment_id),
            (strategy, amount),
        );

        Ok(AllocationSummary {
            commitment_id,
            strategy,
            total_allocated,
            allocations,
        })
    }

    pub fn rebalance(
        env: Env,
        caller: Address,
        commitment_id: u64,
    ) -> Result<AllocationSummary, Error> {
        caller.require_auth();
        Self::require_initialized(&env)?;
        Self::require_no_reentrancy(&env)?;

        // Rate limit rebalancing per caller address
        let fn_symbol = symbol_short!("rebal");
        RateLimiter::check(&env, &caller, &fn_symbol);

        // Verify ownership
        let owner: Address = env
            .storage()
            .persistent()
            .get(&DataKey::AllocationOwner(commitment_id))
            .ok_or(Error::AllocationNotFound)?;

        if owner != caller {
            return Err(Error::Unauthorized);
        }

        Self::set_reentrancy_guard(&env, true);

        // Get current allocations
        let current_allocations: Vec<Allocation> = env
            .storage()
            .persistent()
            .get(&DataKey::Allocations(commitment_id))
            .ok_or(Error::AllocationNotFound)?;

        // Get strategy
        let strategy: Strategy = env
            .storage()
            .persistent()
            .get(&DataKey::Strategy(commitment_id))
            .ok_or(Error::AllocationNotFound)?;

        let mut total_amount = 0i128;

        // Remove old allocations from pools with overflow protection
        for allocation in current_allocations.iter() {
            total_amount = total_amount
                .checked_add(allocation.amount)
                .ok_or(Error::ArithmeticOverflow)?;

            let mut pool = Self::get_pool_internal(&env, allocation.pool_id)?;
            pool.total_liquidity = pool
                .total_liquidity
                .checked_sub(allocation.amount)
                .ok_or(Error::ArithmeticOverflow)?;
            pool.updated_at = env.ledger().timestamp();
            env.storage()
                .persistent()
                .set(&DataKey::Pool(allocation.pool_id), &pool);
        }

        // Reallocate with current strategy
        let pools = Self::select_pools(&env, strategy)?;
        let allocation_plan = Self::calculate_allocation(&env, total_amount, &pools, strategy)?;

        let mut new_allocations = Vec::new(&env);
        let mut new_total = 0i128;

        for (pool_id, alloc_amount) in allocation_plan.iter() {
            if alloc_amount <= 0 {
                continue;
            }

            let mut pool = Self::get_pool_internal(&env, pool_id)?;

            if !pool.active {
                continue; // Skip inactive pools during rebalancing
            }

            let new_liquidity = pool
                .total_liquidity
                .checked_add(alloc_amount)
                .ok_or(Error::ArithmeticOverflow)?;

            if new_liquidity <= pool.max_capacity {
                pool.total_liquidity = new_liquidity;
                pool.updated_at = env.ledger().timestamp();
                env.storage()
                    .persistent()
                    .set(&DataKey::Pool(pool_id), &pool);

                let allocation = Allocation {
                    commitment_id,
                    pool_id,
                    amount: alloc_amount,
                    timestamp: env.ledger().timestamp(),
                };

                new_allocations.push_back(allocation);
                new_total = new_total
                    .checked_add(alloc_amount)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
        }

        env.storage()
            .persistent()
            .set(&DataKey::Allocations(commitment_id), &new_allocations);
        env.storage()
            .persistent()
            .set(&DataKey::TotalAllocated(commitment_id), &new_total);

        Self::set_reentrancy_guard(&env, false);

        env.events()
            .publish((symbol_short!("rebalance"), commitment_id), new_total);

        Ok(AllocationSummary {
            commitment_id,
            strategy,
            total_allocated: new_total,
            allocations: new_allocations,
        })
    }

    // ========================================================================
    // VIEW FUNCTIONS
    // ========================================================================

    pub fn get_allocation(env: Env, commitment_id: u64) -> AllocationSummary {
        let allocations: Vec<Allocation> = env
            .storage()
            .persistent()
            .get(&DataKey::Allocations(commitment_id))
            .unwrap_or(Vec::new(&env));

        let strategy: Strategy = env
            .storage()
            .persistent()
            .get(&DataKey::Strategy(commitment_id))
            .unwrap_or(Strategy::Balanced);

        let total = env
            .storage()
            .persistent()
            .get(&DataKey::TotalAllocated(commitment_id))
            .unwrap_or(0i128);

        AllocationSummary {
            commitment_id,
            strategy,
            total_allocated: total,
            allocations,
        }
    }

    pub fn get_pool(env: Env, pool_id: u32) -> Result<Pool, Error> {
        Self::get_pool_internal(&env, pool_id)
    }

    pub fn get_all_pools(env: Env) -> Vec<Pool> {
        let registry: Vec<u32> = env
            .storage()
            .instance()
            .get(&DataKey::PoolRegistry)
            .unwrap_or(Vec::new(&env));

        let mut pools = Vec::new(&env);
        for pool_id in registry.iter() {
            if let Ok(pool) = Self::get_pool_internal(&env, pool_id) {
                pools.push_back(pool);
            }
        }
        pools
    }

    pub fn is_initialized(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Initialized)
            .unwrap_or(false)
    }

    /// Get current on-chain version (0 if legacy/uninitialized).
    pub fn get_version(env: Env) -> u32 {
        read_version(&env)
    }

    /// Update admin (admin-only).
    pub fn set_admin(
        env: Env,
        caller: Address,
        new_admin: Address,
    ) -> Result<(), Error> {
        caller.require_auth();
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &caller)?;
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        Ok(())
    }

    /// Upgrade contract WASM (admin-only).
    pub fn upgrade(
        env: Env,
        caller: Address,
        new_wasm_hash: BytesN<32>,
    ) -> Result<(), Error> {
        caller.require_auth();
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &caller)?;
        require_valid_wasm_hash(&env, &new_wasm_hash)?;
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        Ok(())
    }

    /// Migrate storage from a previous version to CURRENT_VERSION (admin-only).
    pub fn migrate(
        env: Env,
        caller: Address,
        from_version: u32,
    ) -> Result<(), Error> {
        caller.require_auth();
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &caller)?;

        let stored_version = read_version(&env);
        if stored_version == CURRENT_VERSION {
            return Err(Error::AlreadyMigrated);
        }
        if from_version != stored_version || from_version > CURRENT_VERSION {
            return Err(Error::InvalidVersion);
        }

        // Ensure registry exists
        if !env.storage().instance().has(&DataKey::PoolRegistry) {
            env.storage()
                .instance()
                .set(&DataKey::PoolRegistry, &Vec::<u32>::new(&env));
        }
        if !env.storage().instance().has(&DataKey::ReentrancyGuard) {
            env.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
        }

        env.storage().instance().set(&DataKey::Version, &CURRENT_VERSION);
        Ok(())
    }

    // ========================================================================
    // INTERNAL HELPER FUNCTIONS
    // ========================================================================

    fn require_initialized(env: &Env) -> Result<(), Error> {
        if !env
            .storage()
            .instance()
            .get(&DataKey::Initialized)
            .unwrap_or(false)
        {
            return Err(Error::NotInitialized);
        }
        Ok(())
    }

    fn require_admin(env: &Env, address: &Address) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;

        if admin != *address {
            return Err(Error::Unauthorized);
        }
        Ok(())
    }

    /// Configure rate limits for this contract's core functions.
    ///
    /// Restricted to admin.
    pub fn set_rate_limit(
        env: Env,
        admin: Address,
        function: Symbol,
        window_seconds: u64,
        max_calls: u32,
    ) -> Result<(), Error> {
        admin.require_auth();
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &admin)?;

        RateLimiter::set_limit(&env, &function, window_seconds, max_calls);
        Ok(())
    }

    /// Set or clear exemption from rate limits for an address.
    ///
    /// Restricted to admin.
    pub fn set_rate_limit_exempt(
        env: Env,
        admin: Address,
        address: Address,
        exempt: bool,
    ) -> Result<(), Error> {
        admin.require_auth();
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &admin)?;

        RateLimiter::set_exempt(&env, &address, exempt);
        Ok(())
    }

    fn require_no_reentrancy(env: &Env) -> Result<(), Error> {
        let guard: bool = env
            .storage()
            .instance()
            .get(&DataKey::ReentrancyGuard)
            .unwrap_or(false);

        if guard {
            return Err(Error::ReentrancyDetected);
        }
        Ok(())
    }

    fn set_reentrancy_guard(env: &Env, value: bool) {
        env.storage()
            .instance()
            .set(&DataKey::ReentrancyGuard, &value);
    }

    fn get_pool_internal(env: &Env, pool_id: u32) -> Result<Pool, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Pool(pool_id))
            .ok_or(Error::PoolNotFound)
    }

    fn select_pools(env: &Env, strategy: Strategy) -> Result<Vec<Pool>, Error> {
        let mut pools = Vec::new(env);

        let registry: Vec<u32> = env
            .storage()
            .instance()
            .get(&DataKey::PoolRegistry)
            .unwrap_or(Vec::new(env));

        for pool_id in registry.iter() {
            if let Ok(pool) = Self::get_pool_internal(env, pool_id) {
                if !pool.active {
                    continue;
                }

                let include = match strategy {
                    Strategy::Safe => matches!(pool.risk_level, RiskLevel::Low),
                    Strategy::Balanced => true,
                    Strategy::Aggressive => {
                        matches!(pool.risk_level, RiskLevel::High | RiskLevel::Medium)
                    }
                };

                if include {
                    pools.push_back(pool);
                }
            }
        }

        Ok(pools)
    }

    fn calculate_allocation(
        env: &Env,
        total_amount: i128,
        pools: &Vec<Pool>,
        strategy: Strategy,
    ) -> Result<Map<u32, i128>, Error> {
        let mut allocation_map = Map::new(env);
        let pool_count = pools.len();

        if pool_count == 0 {
            return Ok(allocation_map);
        }

        match strategy {
            Strategy::Safe => {
                let amount_per_pool = total_amount / pool_count as i128;
                for pool in pools.iter() {
                    allocation_map.set(pool.pool_id, amount_per_pool);
                }
            }
            Strategy::Balanced => {
                let mut low_risk_pools = Vec::new(env);
                let mut medium_risk_pools = Vec::new(env);
                let mut high_risk_pools = Vec::new(env);

                for pool in pools.iter() {
                    match pool.risk_level {
                        RiskLevel::Low => low_risk_pools.push_back(pool),
                        RiskLevel::Medium => medium_risk_pools.push_back(pool),
                        RiskLevel::High => high_risk_pools.push_back(pool),
                    }
                }

                // Safe percentage calculations with checked operations
                let low_amount = total_amount
                    .checked_mul(40)
                    .and_then(|x| x.checked_div(100))
                    .ok_or(Error::ArithmeticOverflow)?;

                let medium_amount = total_amount
                    .checked_mul(40)
                    .and_then(|x| x.checked_div(100))
                    .ok_or(Error::ArithmeticOverflow)?;

                let high_amount = total_amount
                    .checked_mul(20)
                    .and_then(|x| x.checked_div(100))
                    .ok_or(Error::ArithmeticOverflow)?;

                Self::distribute_to_pools(env, &mut allocation_map, &low_risk_pools, low_amount)?;
                Self::distribute_to_pools(
                    env,
                    &mut allocation_map,
                    &medium_risk_pools,
                    medium_amount,
                )?;
                Self::distribute_to_pools(env, &mut allocation_map, &high_risk_pools, high_amount)?;
            }
            Strategy::Aggressive => {
                let mut medium_risk_pools = Vec::new(env);
                let mut high_risk_pools = Vec::new(env);

                for pool in pools.iter() {
                    match pool.risk_level {
                        RiskLevel::Medium => medium_risk_pools.push_back(pool),
                        RiskLevel::High => high_risk_pools.push_back(pool),
                        _ => {}
                    }
                }

                let high_amount = total_amount
                    .checked_mul(70)
                    .and_then(|x| x.checked_div(100))
                    .ok_or(Error::ArithmeticOverflow)?;

                let medium_amount = total_amount
                    .checked_mul(30)
                    .and_then(|x| x.checked_div(100))
                    .ok_or(Error::ArithmeticOverflow)?;

                Self::distribute_to_pools(env, &mut allocation_map, &high_risk_pools, high_amount)?;
                Self::distribute_to_pools(
                    env,
                    &mut allocation_map,
                    &medium_risk_pools,
                    medium_amount,
                )?;
            }
        }

        Ok(allocation_map)
    }

    fn distribute_to_pools(
        _env: &Env,
        allocation_map: &mut Map<u32, i128>,
        pools: &Vec<Pool>,
        amount: i128,
    ) -> Result<(), Error> {
        let pool_count = pools.len();
        if pool_count == 0 {
            return Ok(());
        }

        let amount_per_pool = amount / pool_count as i128;

        for pool in pools.iter() {
            let available_capacity = pool
                .max_capacity
                .checked_sub(pool.total_liquidity)
                .ok_or(Error::ArithmeticOverflow)?;

            let alloc_amount = if amount_per_pool > available_capacity {
                available_capacity
            } else {
                amount_per_pool
            };

            if alloc_amount > 0 {
                allocation_map.set(pool.pool_id, alloc_amount);
            }
        }

        Ok(())
    }
}

fn read_version(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get::<_, u32>(&DataKey::Version)
        .unwrap_or(0)
}

fn require_valid_wasm_hash(env: &Env, wasm_hash: &BytesN<32>) -> Result<(), Error> {
    let zero = BytesN::from_array(env, &[0; 32]);
    if *wasm_hash == zero {
        return Err(Error::InvalidWasmHash);
    }
    Ok(())
}


// ============================================================================
// TESTS MODULE
// ============================================================================

#[cfg(test)]
mod tests;

#[cfg(all(test, feature = "benchmark"))]
mod benchmarks;
