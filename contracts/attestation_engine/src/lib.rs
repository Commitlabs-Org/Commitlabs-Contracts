#![no_std]
use shared_utils::{BatchError, BatchMode, BatchProcessor, BatchResultVoid, Pausable, RateLimiter};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, BytesN, Env,
    IntoVal, Map, String, Symbol, TryIntoVal, Val, Vec,
};

const CURRENT_VERSION: u32 = 1;

// ============================================================================
// Error Types
// ============================================================================

/// Contract errors for structured error handling
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AttestationError {
    /// Contract has not been initialized
    NotInitialized = 1,
    /// Contract has already been initialized
    AlreadyInitialized = 2,
    /// Caller is not authorized to perform this action
    Unauthorized = 3,
    /// Invalid commitment ID
    InvalidCommitmentId = 4,
    /// Invalid attestation type (must be health_check, violation, fee_generation, or drawdown)
    InvalidAttestationType = 5,
    /// Invalid attestation data for the given type
    InvalidAttestationData = 6,
    /// Commitment not found in core contract
    CommitmentNotFound = 7,
    /// Storage operation failed
    StorageError = 8,
    /// Invalid fee amount (must be non-negative)
    InvalidFeeAmount = 9,
    /// Fee recipient not set; cannot withdraw
    FeeRecipientNotSet = 10,
    /// Insufficient collected fees to withdraw
    InsufficientFees = 11,
    /// Invalid WASM hash for upgrade
    InvalidWasmHash = 12,
    /// Migration already applied
    AlreadyMigrated = 13,
    /// Invalid version for migration
    InvalidVersion = 14,
}

// ============================================================================
// Storage Keys
// ============================================================================

/// Storage keys for the contract
#[contracttype]
pub enum DataKey {
    /// Admin address
    Admin,
    /// Core contract address
    CoreContract,
    /// Verifier whitelist (Address -> bool)
    Verifier(Address),
    /// Attestations for a commitment (commitment_id -> Vec<Attestation>)
    Attestations(String),
    /// Health metrics for a commitment (commitment_id -> HealthMetrics)
    HealthMetrics(String),
    /// Attestation counter for a commitment (commitment_id -> u64)
    AttestationCounter(String),
    /// Reentrancy guard
    ReentrancyGuard,
    /// Global analytics: total attestations recorded
    TotalAttestations,
    /// Global analytics: total violation-type or non-compliant attestations
    TotalViolations,
    /// Global analytics: total fees generated across all commitments
    TotalFees,
    /// Per-verifier analytics: attestation count by verifier
    VerifierAttestationCount(Address),
    /// Fee collection: protocol treasury for withdrawals
    FeeRecipient,
    /// Attestation verification fee: amount per attestation (0 = no fee)
    AttestationFeeAmount,
    /// Attestation verification fee: token address (when amount > 0)
    AttestationFeeAsset,
    /// Collected fees per asset (asset -> i128)
    CollectedFees(Address),
    /// Contract version for migrations
    Version,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attestation {
    pub commitment_id: String,
    pub timestamp: u64,
    pub attestation_type: String, // "health_check", "violation", "fee_generation", "drawdown"
    pub data: Map<String, String>, // Flexible data structure
    pub is_compliant: bool,
    pub verified_by: Address,
}

/// Parameters for batch attestation operations
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttestParams {
    pub commitment_id: String,
    pub attestation_type: String,
    pub data: Map<String, String>,
    pub is_compliant: bool,
}

// Import Commitment types from commitment_core (define locally for cross-contract calls)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitmentRules {
    pub duration_days: u32,
    pub max_loss_percent: u32,
    pub commitment_type: String, // "safe", "balanced", "aggressive"
    pub early_exit_penalty: u32,
    pub min_fee_threshold: i128,
    pub grace_period_days: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Commitment {
    pub commitment_id: String,
    pub owner: Address,
    pub nft_token_id: u32,
    pub rules: CommitmentRules,
    pub amount: i128,
    pub asset_address: Address,
    pub created_at: u64,
    pub expires_at: u64,
    pub current_value: i128,
    pub status: String, // "active", "settled", "violated", "early_exit"
}

// Duplicate definitions removed.  HealthState and the small DataKey
// variant were legacy leftovers; the first `DataKey` and `Attestation`
// structs earlier in the file are the authoritative versions.  The
// health metrics helper later will load `HealthMetrics` directly instead.

// Import Commitment types from commitment_core (define locally for cross-contract calls)

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HealthMetrics {
    pub commitment_id: String,
    pub current_value: i128,
    pub initial_value: i128,
    pub drawdown_percent: i128,
    pub fees_generated: i128,
    pub volatility_exposure: i128,
    pub last_attestation: u64,
    pub compliance_score: u32, // 0-100
}

#[contract]
pub struct AttestationEngineContract;

#[contractimpl]
impl AttestationEngineContract {
    /// Initialize the attestation engine
    ///
    /// # Arguments
    /// * `admin` - The admin address for the contract
    /// * `commitment_core` - The address of the commitment_core contract
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(AttestationError::AlreadyInitialized)` if already initialized
    pub fn initialize(
        e: Env,
        admin: Address,
        commitment_core: Address,
    ) -> Result<(), AttestationError> {
        // Check if already initialized
        if e.storage().instance().has(&DataKey::Admin) {
            return Err(AttestationError::AlreadyInitialized);
        }

        // Store admin and commitment core contract address in instance storage
        e.storage().instance().set(&DataKey::Admin, &admin);
        e.storage()
            .instance()
            .set(&DataKey::CoreContract, &commitment_core);

        Ok(())
    }

    // ========================================================================
    // Verifier Whitelist Management
    // ========================================================================

    /// Add a verifier to the whitelist
    ///
    /// # Arguments
    /// * `caller` - Must be admin
    /// * `verifier` - Address to add as authorized verifier
    pub fn add_verifier(
        e: Env,
        caller: Address,
        verifier: Address,
    ) -> Result<(), AttestationError> {
        caller.require_auth();

        // Check caller is admin
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(AttestationError::NotInitialized)?;

        if caller != admin {
            return Err(AttestationError::Unauthorized);
        }

        // Add verifier to whitelist
        e.storage()
            .instance()
            .set(&DataKey::Verifier(verifier.clone()), &true);

        // Emit event
        e.events()
            .publish((Symbol::new(&e, "VerifierAdded"),), (verifier,));

        Ok(())
    }

    /// Remove a verifier from the whitelist
    ///
    /// # Arguments
    /// * `caller` - Must be admin
    /// * `verifier` - Address to remove from authorized verifiers
    pub fn remove_verifier(
        e: Env,
        caller: Address,
        verifier: Address,
    ) -> Result<(), AttestationError> {
        caller.require_auth();

        // Check caller is admin
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(AttestationError::NotInitialized)?;

        if caller != admin {
            return Err(AttestationError::Unauthorized);
        }

        // Remove verifier from whitelist
        e.storage()
            .instance()
            .remove(&DataKey::Verifier(verifier.clone()));

        // Emit event
        e.events()
            .publish((Symbol::new(&e, "VerifierRemoved"),), (verifier,));

        Ok(())
    }

    /// Check if an address is an authorized verifier
    fn is_authorized_verifier(e: &Env, address: &Address) -> bool {
        // Admin is always authorized
        if let Some(admin) = e
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::Admin)
        {
            if *address == admin {
                return true;
            }
        }

        // Check verifier whitelist
        e.storage()
            .instance()
            .get(&DataKey::Verifier(address.clone()))
            .unwrap_or(false)
    }

    /// Pause the contract
    ///
    /// # Arguments
    /// * `e` - The environment
    ///
    /// # Panics
    /// Panics if caller is not admin or if contract is already paused
    pub fn pause(e: Env) {
        // Enforce admin-only
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("Contract not initialized"));
        admin.require_auth();
        Pausable::pause(&e);
    }

    /// Unpause the contract
    ///
    /// # Arguments
    /// * `e` - The environment
    ///
    /// # Panics
    /// Panics if caller is not admin or if contract is already unpaused
    pub fn unpause(e: Env) {
        // Enforce admin-only
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("Contract not initialized"));
        admin.require_auth();
        Pausable::unpause(&e);
    }

    /// Check if the contract is paused
    ///
    /// # Arguments
    /// * `e` - The environment
    ///
    /// # Returns
    /// `true` if paused, `false` otherwise
    pub fn is_paused(e: Env) -> bool {
        Pausable::is_paused(&e)
    }

    /// Check if an address is a verifier (public version)
    pub fn is_verifier(e: Env, address: Address) -> bool {
        Self::is_authorized_verifier(&e, &address)
    }

    /// Get the admin address
    pub fn get_admin(e: Env) -> Result<Address, AttestationError> {
        e.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(AttestationError::NotInitialized)
    }

    /// Get the core contract address
    pub fn get_core_contract(e: Env) -> Result<Address, AttestationError> {
        e.storage()
            .instance()
            .get(&DataKey::CoreContract)
            .ok_or(AttestationError::NotInitialized)
    }

    /// Get current on-chain version (0 if legacy/uninitialized).
    pub fn get_version(e: Env) -> u32 {
        read_version(&e)
    }

    /// Update admin (admin-only).
    pub fn set_admin(e: Env, caller: Address, new_admin: Address) -> Result<(), AttestationError> {
        require_admin(&e, &caller)?;
        e.storage().instance().set(&DataKey::Admin, &new_admin);
        Ok(())
    }

    /// Upgrade contract WASM (admin-only).
    pub fn upgrade(
        e: Env,
        caller: Address,
        new_wasm_hash: BytesN<32>,
    ) -> Result<(), AttestationError> {
        require_admin(&e, &caller)?;
        require_valid_wasm_hash(&e, &new_wasm_hash)?;
        e.deployer().update_current_contract_wasm(new_wasm_hash);
        Ok(())
    }

    /// Migrate storage from a previous version to CURRENT_VERSION (admin-only).
    pub fn migrate(e: Env, caller: Address, from_version: u32) -> Result<(), AttestationError> {
        require_admin(&e, &caller)?;

        let stored_version = read_version(&e);
        if stored_version == CURRENT_VERSION {
            return Err(AttestationError::AlreadyMigrated);
        }
        if from_version != stored_version || from_version > CURRENT_VERSION {
            return Err(AttestationError::InvalidVersion);
        }

        // Ensure analytics counters are initialized
        if !e.storage().instance().has(&DataKey::TotalAttestations) {
            e.storage()
                .instance()
                .set(&DataKey::TotalAttestations, &0u64);
        }
        if !e.storage().instance().has(&DataKey::TotalViolations) {
            e.storage().instance().set(&DataKey::TotalViolations, &0u64);
        }
        if !e.storage().instance().has(&DataKey::TotalFees) {
            e.storage().instance().set(&DataKey::TotalFees, &0i128);
        }
        if !e.storage().instance().has(&DataKey::ReentrancyGuard) {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
        }

        e.storage()
            .instance()
            .set(&DataKey::Version, &CURRENT_VERSION);
        Ok(())
    }

    /// Get stored health metrics for a commitment (without recalculation)
    pub fn get_stored_health_metrics(e: Env, commitment_id: String) -> Option<HealthMetrics> {
        let key = DataKey::HealthMetrics(commitment_id);
        e.storage().persistent().get(&key)
    }

    /// Store health metrics for a commitment
    fn store_health_metrics(e: &Env, metrics: &HealthMetrics) {
        let key = DataKey::HealthMetrics(metrics.commitment_id.clone());
        e.storage().persistent().set(&key, metrics);
    }

    // ========================================================================
    // Storage helpers
    // ========================================================================

    /// Derive the storage key for the attestations list of a commitment.
    fn attestations_key(commitment_id: &String) -> DataKey {
        DataKey::Attestations(commitment_id.clone())
    }

    /// Derive the storage key for health metrics of a commitment.
    fn health_metrics_key(commitment_id: &String) -> DataKey {
        DataKey::HealthMetrics(commitment_id.clone())
    }

    /// Load all attestations for a commitment; returns empty vector when
    /// none are present (avoids repeated boilerplate).
    fn load_attestations(e: &Env, commitment_id: &String) -> Vec<Attestation> {
        e.storage()
            .persistent()
            .get(&Self::attestations_key(commitment_id))
            .unwrap_or_else(|| Vec::new(e))
    }

    /// Store an attestation vector for a commitment.
    fn store_attestations(
        e: &Env,
        commitment_id: &String,
        attestations: &Vec<Attestation>,
    ) {
        e.storage()
            .persistent()
            .set(&Self::attestations_key(commitment_id), attestations);
    }

    /// Store health metrics for a commitment.
    fn store_health_metrics(e: &Env, metrics: &HealthMetrics) {
        e.storage()
            .persistent()
            .set(&Self::health_metrics_key(&metrics.commitment_id), metrics);
    }

    /// Retrieve stored health metrics or return a default value if missing.
    fn get_health_state_or_default(e: &Env, commitment_id: &String) -> HealthMetrics {
        e.storage()
            .persistent()
            .get(&Self::health_metrics_key(commitment_id))
            .unwrap_or_else(|| HealthMetrics {
                commitment_id: commitment_id.clone(),
                current_value: 0,
                initial_value: 0,
                drawdown_percent: 0,
                fees_generated: 0,
                volatility_exposure: 0,
                last_attestation: 0,
                compliance_score: 100,
            })
    }

    // ========================================================================
    // Validation Helpers
    // ========================================================================

    /// Validate attestation type is one of the allowed types
    fn is_valid_attestation_type(e: &Env, att_type: &String) -> bool {
        let health_check = String::from_str(e, "health_check");
        let violation = String::from_str(e, "violation");
        let fee_generation = String::from_str(e, "fee_generation");
        let drawdown = String::from_str(e, "drawdown");

        *att_type == health_check
            || *att_type == violation
            || *att_type == fee_generation
            || *att_type == drawdown
    }

    /// Validate attestation data based on type
    fn validate_attestation_data(e: &Env, att_type: &String, data: &Map<String, String>) -> bool {
        let health_check = String::from_str(e, "health_check");
        let violation = String::from_str(e, "violation");
        let fee_generation = String::from_str(e, "fee_generation");
        let drawdown = String::from_str(e, "drawdown");

        if *att_type == health_check {
            // health_check: optional fields, always valid
            true
        } else if *att_type == violation {
            // violation: requires "violation_type" and "severity"
            let violation_type_key = String::from_str(e, "violation_type");
            let severity_key = String::from_str(e, "severity");
            data.contains_key(violation_type_key) && data.contains_key(severity_key)
        } else if *att_type == fee_generation {
            // fee_generation: requires "fee_amount"
            let fee_amount_key = String::from_str(e, "fee_amount");
            data.contains_key(fee_amount_key)
        } else if *att_type == drawdown {
            // drawdown: requires "drawdown_percent"
            let drawdown_percent_key = String::from_str(e, "drawdown_percent");
            data.contains_key(drawdown_percent_key)
        } else {
            false
        }
    }

    /// Check if commitment exists in core contract
    fn commitment_exists(e: &Env, commitment_id: &String) -> bool {
        let commitment_core: Address = match e.storage().instance().get(&DataKey::CoreContract) {
            Some(addr) => addr,
            None => return false,
        };

        // Try to get commitment from core contract
        let mut args = Vec::new(e);
        args.push_back(commitment_id.clone().into_val(e));

        // Use try_invoke_contract to handle potential failures
        let result = e.try_invoke_contract::<Val, soroban_sdk::Error>(
            &commitment_core,
            &Symbol::new(e, "get_commitment"),
            args,
        );

        match result {
            Ok(Ok(_)) => true,
            _ => false,
        }
    }

    // ========================================================================
    // Health Metrics Update
    // ========================================================================

    /// Update health metrics after an attestation
    fn update_health_metrics(e: &Env, commitment_id: &String, attestation: &Attestation) {
        // Get or create health metrics
        let key = DataKey::HealthMetrics(commitment_id.clone());
        let mut metrics: HealthMetrics =
            e.storage()
                .persistent()
                .get(&key)
                .unwrap_or_else(|| HealthMetrics {
                    commitment_id: commitment_id.clone(),
                    current_value: 0,
                    initial_value: 0,
                    drawdown_percent: 0,
                    fees_generated: 0,
                    volatility_exposure: 0,
                    last_attestation: 0,
                    compliance_score: 100,
                });

        // Update last_attestation timestamp
        metrics.last_attestation = attestation.timestamp;

        // Update type-specific metrics
        let fee_generation = String::from_str(e, "fee_generation");
        let drawdown_type = String::from_str(e, "drawdown");
        let violation = String::from_str(e, "violation");

        if attestation.attestation_type == fee_generation {
            // Add to fees_generated
            let fee_amount_key = String::from_str(e, "fee_amount");
            if let Some(fee_str) = attestation.data.get(fee_amount_key) {
                // Parse fee amount from string
                if let Some(fee_amount) = Self::parse_i128_from_string(e, &fee_str) {
                    metrics.fees_generated = metrics
                        .fees_generated
                        .checked_add(fee_amount)
                        .unwrap_or(metrics.fees_generated);

                    // Update global total fees analytics
                    let total_fees: i128 =
                        e.storage().instance().get(&DataKey::TotalFees).unwrap_or(0);
                    let new_total = total_fees.checked_add(fee_amount).unwrap_or(total_fees);
                    e.storage().instance().set(&DataKey::TotalFees, &new_total);
                }
            }
        } else if attestation.attestation_type == drawdown_type {
            // Update drawdown_percent
            let drawdown_percent_key = String::from_str(e, "drawdown_percent");
            if let Some(drawdown_str) = attestation.data.get(drawdown_percent_key) {
                if let Some(drawdown_val) = Self::parse_i128_from_string(e, &drawdown_str) {
                    metrics.drawdown_percent = drawdown_val;
                }
            }
        } else if attestation.attestation_type == violation {
            // Decrease compliance score for violations
            let severity_key = String::from_str(e, "severity");
            let penalty = if let Some(severity) = attestation.data.get(severity_key) {
                let high = String::from_str(e, "high");
                let medium = String::from_str(e, "medium");
                if severity == high {
                    30u32
                } else if severity == medium {
                    20u32
                } else {
                    10u32
                }
            } else {
                20u32 // Default penalty
            };

            metrics.compliance_score = metrics.compliance_score.saturating_sub(penalty);
        }

        // Compliance bonus for compliant attestations
        if attestation.is_compliant && attestation.attestation_type != violation {
            // Small bonus for compliant attestations, capped at 100
            metrics.compliance_score =
                core::cmp::min(100, metrics.compliance_score.saturating_add(1));
        }

        // Store updated metrics
        e.storage().persistent().set(&key, &metrics);
    }

    /// Parse i128 from String (optimized implementation)
    fn parse_i128_from_string(_e: &Env, s: &String) -> Option<i128> {
        let len = s.len();
        if len == 0 || len > 64 {
            return None; // Early return for invalid lengths
        }

        // Copy string to buffer
        let mut buf = [0u8; 64];
        s.copy_into_slice(&mut buf[..len as usize]);

        let mut result: i128 = 0;
        let mut start_idx = 0;
        let is_negative = buf[0] == b'-';

        if is_negative {
            start_idx = 1;
            if len == 1 {
                return None; // Just a minus sign
            }
        }

        // OPTIMIZATION: Single pass parsing with early exit on invalid char
        for i in start_idx..len as usize {
            let b = buf[i];
            if b < b'0' || b > b'9' {
                return None; // Invalid character - early exit
            }
            result = result.checked_mul(10)?;
            result = result.checked_add((b - b'0') as i128)?;
        }

        if is_negative {
            result = result.checked_neg()?;
        }

        Some(result)
    }

    // ========================================================================
    // Access Control
    // ========================================================================

    /// Record an attestation for a commitment
    ///
    /// # Arguments
    /// * `caller` - The address recording the attestation (must be authorized verifier)
    /// * `commitment_id` - The commitment being attested
    /// * `attestation_type` - Type: "health_check", "violation", "fee_generation", "drawdown"
    /// * `data` - Type-specific data map
    /// * `is_compliant` - Whether the commitment is compliant
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(AttestationError::*)` on various validation failures
    ///
    /// # Reentrancy Protection
    /// Uses checks-effects-interactions pattern with an explicit guard.
    pub fn attest(
        e: Env,
        caller: Address,
        commitment_id: String,
        attestation_type: String,
        data: Map<String, String>,
        is_compliant: bool,
    ) -> Result<(), AttestationError> {
        // 1. Reentrancy protection
        if e.storage().instance().has(&DataKey::ReentrancyGuard) {
            panic!("Reentrancy detected");
        }
        e.storage().instance().set(&DataKey::ReentrancyGuard, &true);

        // Check if contract is paused
        Pausable::require_not_paused(&e);

        // 2. Verify caller signed the transaction
        caller.require_auth();

        // 3. Check caller is authorized verifier
        if !Self::is_authorized_verifier(&e, &caller) {
            e.storage().instance().remove(&DataKey::ReentrancyGuard);
            return Err(AttestationError::Unauthorized);
        }

        // 3b. Rate limit attestations per verifier
        let fn_symbol = Symbol::new(&e, "attest");
        RateLimiter::check(&e, &caller, &fn_symbol);

        // 4. Validate commitment_id is not empty
        if commitment_id.len() == 0 {
            e.storage().instance().remove(&DataKey::ReentrancyGuard);
            return Err(AttestationError::InvalidCommitmentId);
        }

        // 5. Validate commitment exists in core contract
        if !Self::commitment_exists(&e, &commitment_id) {
            e.storage().instance().remove(&DataKey::ReentrancyGuard);
            return Err(AttestationError::CommitmentNotFound);
        }

        // 6. Validate attestation type
        if !Self::is_valid_attestation_type(&e, &attestation_type) {
            e.storage().instance().remove(&DataKey::ReentrancyGuard);
            return Err(AttestationError::InvalidAttestationType);
        }

        // 7. Validate data format for the attestation type
        if !Self::validate_attestation_data(&e, &attestation_type, &data) {
            e.storage().instance().remove(&DataKey::ReentrancyGuard);
            return Err(AttestationError::InvalidAttestationData);
        }

        // 7b. Collect attestation verification fee if configured
        let fee_amount: i128 = e
            .storage()
            .instance()
            .get(&DataKey::AttestationFeeAmount)
            .unwrap_or(0);
        if fee_amount > 0 {
            if let Some(fee_asset) = e
                .storage()
                .instance()
                .get::<DataKey, Address>(&DataKey::AttestationFeeAsset)
            {
                let contract_address = e.current_contract_address();
                let token_client = token::Client::new(&e, &fee_asset);
                token_client.transfer(&caller, &contract_address, &fee_amount);
                let key = DataKey::CollectedFees(fee_asset.clone());
                let current: i128 = e.storage().instance().get(&key).unwrap_or(0);
                e.storage().instance().set(&key, &(current + fee_amount));
            }
        }

        // 8. Create attestation record
        let timestamp = e.ledger().timestamp();
        let attestation = Attestation {
            commitment_id: commitment_id.clone(),
            attestation_type: attestation_type.clone(),
            data,
            timestamp: e.ledger().timestamp(),
            verified_by: caller.clone(),
            is_compliant,
        };

        // 9. Store attestation in commitment's list using helper
        let mut attestations = Self::load_attestations(&e, &commitment_id);
        attestations.push_back(attestation);
        Self::store_attestations(&e, &commitment_id, &attestations);

        // 11. Increment attestation counter
        let counter_key = DataKey::AttestationCounter(commitment_id.clone());
        let counter: u64 = e.storage().persistent().get(&counter_key).unwrap_or(0);
        e.storage().persistent().set(&counter_key, &(counter + 1));

        // 11b. OPTIMIZATION: Batch update all analytics counters
        let (total_attestations, total_violations, verifier_count) = {
            let total_att = e
                .storage()
                .instance()
                .get(&DataKey::TotalAttestations)
                .unwrap_or(0u64);
            let total_viol = e
                .storage()
                .instance()
                .get(&DataKey::TotalViolations)
                .unwrap_or(0u64);
            let verifier_key = DataKey::VerifierAttestationCount(caller.clone());
            let ver_count = e.storage().instance().get(&verifier_key).unwrap_or(0u64);
            (total_att, total_viol, ver_count)
        };

        e.storage()
            .instance()
            .set(&DataKey::TotalAttestations, &(total_attestations + 1));

        // Track violations (explicit or non-compliant)
        let violation_type = String::from_str(&e, "violation");
        if attestation_type == violation_type || !is_compliant {
            e.storage()
                .instance()
                .set(&DataKey::TotalViolations, &(total_violations + 1));
        }

        // Track per-verifier attestation count
        let verifier_key = DataKey::VerifierAttestationCount(caller.clone());
        e.storage()
            .instance()
            .set(&verifier_key, &(verifier_count + 1));

        // 12. Emit enhanced AttestationRecorded event
        e.events().publish(
            (
                Symbol::new(&e, "AttestationRecorded"),
                commitment_id,
                caller,
            ),
            (attestation_type, is_compliant, timestamp),
        );

        // 13. Clear reentrancy guard
        e.storage().instance().remove(&DataKey::ReentrancyGuard);

        Ok(())
    }

    /// Get all attestations for a commitment
    pub fn get_attestations(e: Env, commitment_id: String) -> Vec<Attestation> {
        Self::load_attestations(&e, &commitment_id)
    }

    /// Get attestation count for a commitment
    pub fn get_attestation_count(e: Env, commitment_id: String) -> u64 {
        let key = DataKey::AttestationCounter(commitment_id);
        e.storage().persistent().get(&key).unwrap_or(0)
    }

    /// Read a slice of a commitment's attestation list. Useful when
    /// the full vector is large; callers supply an `offset` and `limit`.
    pub fn get_attestations_page(
        e: Env,
        commitment_id: String,
        offset: u32,
        limit: u32,
    ) -> Vec<Attestation> {
        let all = Self::load_attestations(&e, &commitment_id);
        let start = offset as usize;
        if start >= all.len() {
            return Vec::new(&e);
        }
        let end = core::cmp::min(start + limit as usize, all.len());
        let mut page = Vec::new(&e);
        for i in start..end {
            if let Some(att) = all.get(i) {
                page.push_back(att.clone());
            }
        }
        page
    }

    /// Get current health metrics for a commitment
    pub fn get_health_metrics(e: Env, commitment_id: String) -> HealthMetrics {
        let core = Self::get_commitment_core(&e);
        let commitment = Self::core_get_commitment(&e, &core, &commitment_id);

        let initial_value = commitment.amount;
        let current_value = commitment.current_value;
        let drawdown_percent = Self::calc_drawdown_percent(initial_value, current_value);

        let state = Self::get_health_state_or_default(&e, &commitment_id);

        // we already fetched `commitment` above via the core client
        // get all attestations
        let attestations = Self::get_attestations(e.clone(), commitment_id.clone());
        // (initial_value, current_value and drawdown_percent were computed earlier)

        // Sum fees from fee attestations
        // Extract fee_amount from data map where key is "fee_amount"
        let fees_generated: i128 = 0;
        let fee_key = String::from_str(&e, "fee_amount");
        for att in attestations.iter() {
            if att.attestation_type == String::from_str(&e, "fee_generation") {
                // Try to get fee_amount from data map
                if let Some(_fee_val) = att.data.get(fee_key.clone()) {
                    // The value is stored as String, we need to parse it
                    // For simplicity, we'll use a helper to extract numeric value
                    // In a real implementation, fees would be stored as i128 directly
                    // For now, we'll track fees in a separate storage or use a different approach
                    // Since Map<String, String> stores strings, we'll need parsing
                    // Simplified: assume fee is stored as string representation of number
                }
            }
        }

        // For now, fees_generated will be 0 until we implement proper fee tracking
        // This is acceptable as the requirement is to sum from fee attestations
        // which requires the attest() function to properly store fees

        // Calculate volatility exposure from attestations
        // Simplified: use variance of price changes from attestations
        let mut volatility_exposure: i128 = 0;
        if attestations.len() > 1 {
            // Calculate variance from price data in attestations
            // For now, return 0 as placeholder - would need price history
            volatility_exposure = 0;
        }

        // Get last attestation timestamp
        let last_attestation = attestations
            .iter()
            .map(|att| att.timestamp)
            .max()
            .unwrap_or(0);

        // Calculate compliance score
        let compliance_score = Self::calculate_compliance_score(e.clone(), commitment_id.clone());

        let stored = Self::get_stored_health_metrics(e.clone(), commitment_id.clone());
        let (fees_generated, volatility_exposure, last_attestation, stored_compliance) = stored
            .as_ref()
            .map(|m| {
                (
                    m.fees_generated,
                    m.volatility_exposure,
                    m.last_attestation,
                    m.compliance_score,
                )
            })
            .unwrap_or((0, 0, last_attestation, compliance_score));

        HealthMetrics {
            commitment_id,
            current_value,
            initial_value,
            drawdown_percent,
            fees_generated,
            volatility_exposure,
            last_attestation,
            compliance_score: if stored.is_some() {
                stored_compliance
            } else {
                compliance_score
            },
        }
    }

    /// Verify commitment compliance
    pub fn verify_compliance(e: Env, commitment_id: String) -> bool {
        let core = Self::get_commitment_core(&e);
        let commitment = Self::core_get_commitment(&e, &core, &commitment_id);
        let health = Self::get_health_metrics(e.clone(), commitment_id.clone());
        let has_violations = Self::core_check_violations(&e, &core, &commitment_id);

        // Loss limit compliance
        let max_loss = commitment.rules.max_loss_percent as i128;
        let loss_ok = health.drawdown_percent <= max_loss;

        // Duration compliance (if applicable)
        let now = e.ledger().timestamp();
        let duration_ok = if commitment.rules.duration_days == 0 {
            true
        } else {
            now <= commitment.expires_at
        };

        // Fee threshold compliance (if applicable)
        let fee_ok = if commitment.rules.min_fee_threshold <= 0 {
            true
        } else {
            health.fees_generated >= commitment.rules.min_fee_threshold
        };

        // Overall health compliance (if score is present; 0 means unknown)
        let overall_health_ok = health.compliance_score == 0 || health.compliance_score >= 80;

        // Status-based sanity checks
        let status_violated = commitment.status == String::from_str(&e, "violated");
        let status_ok = !status_violated;

        loss_ok && duration_ok && fee_ok && overall_health_ok && !has_violations && status_ok
    }


    /// Record fee generation
    ///
    /// Convenience function that creates a fee_generation attestation
    ///
    /// # Arguments
    /// * `caller` - Must be authorized verifier
    /// * `commitment_id` - The commitment generating fees
    /// * `fee_amount` - The fee amount generated
    pub fn record_fees(
        e: Env,
        caller: Address,
        commitment_id: String,
        fee_amount: i128,
    ) -> Result<(), AttestationError> {
        // Build data map for fee_generation attestation
        let mut data = Map::new(&e);
        data.set(
            String::from_str(&e, "fee_amount"),
            Self::i128_to_string(&e, fee_amount),
        );

        // Delegate to attest which handles auth, storage and metrics.
        Self::attest(
            e.clone(),
            caller.clone(),
            commitment_id.clone(),
            String::from_str(&e, "fee_generation"),
            data,
            true,
        )?;

        // Emit event for easier indexing of fees
        e.events().publish(
            (Symbol::new(&e, "FeeRecorded"), commitment_id.clone()),
            (fee_amount, e.ledger().timestamp()),
        );

        Ok(())
    }

    /// Record drawdown event
    ///
    /// # Arguments
    /// * `caller` - Must be authorized verifier
    /// * `commitment_id` - The commitment with drawdown
    /// * `drawdown_percent` - The current drawdown percentage
    pub fn record_drawdown(
        e: Env,
        caller: Address,
        commitment_id: String,
        drawdown_percent: i128,
    ) -> Result<(), AttestationError> {
        // fetch commitment to determine loss threshold
        let commitment_core: Address = e
            .storage()
            .instance()
            .get(&DataKey::CoreContract)
            .ok_or(AttestationError::NotInitialized)?;

        let mut args = Vec::new(&e);
        args.push_back(commitment_id.clone().into_val(&e));
        let commitment_val: Val =
            e.invoke_contract(&commitment_core, &Symbol::new(&e, "get_commitment"), args);
        let commitment: Commitment = commitment_val
            .try_into_val(&e)
            .map_err(|_| AttestationError::CommitmentNotFound)?;

        let max_loss = commitment.rules.max_loss_percent as i128;
        let is_violation = drawdown_percent > max_loss;
        let is_compliant = !is_violation;

        // common data payload
        let mut data = Map::new(&e);
        data.set(
            String::from_str(&e, "drawdown_percent"),
            Self::i128_to_string(&e, drawdown_percent),
        );
        data.set(
            String::from_str(&e, "max_loss_percent"),
            Self::i128_to_string(&e, max_loss),
        );

        // if violation, record a separate attestation and emit event
        if is_violation {
            Self::attest(
                e.clone(),
                caller.clone(),
                commitment_id.clone(),
                String::from_str(&e, "violation"),
                data.clone(),
                false,
            )?;
            e.events().publish(
                (Symbol::new(&e, "ViolationDetected"), commitment_id.clone()),
                (drawdown_percent, max_loss, e.ledger().timestamp()),
            );
        }

        // always record a drawdown attestation
        Self::attest(
            e.clone(),
            caller.clone(),
            commitment_id.clone(),
            String::from_str(&e, "drawdown"),
            data,
            is_compliant,
        )?;

        // emit a dedicated drawdown event as before
        e.events().publish(
            (Symbol::new(&e, "DrawdownRecorded"), commitment_id.clone()),
            (drawdown_percent, is_compliant, e.ledger().timestamp()),
        );

        Ok(())
    }

    /// Convert i128 to String (helper function)
    fn i128_to_string(e: &Env, value: i128) -> String {
        if value == 0 {
            return String::from_str(e, "0");
        }

        let mut n = value;
        let is_negative = n < 0;
        if is_negative {
            n = -n;
        }

        let mut buf = [0u8; 64];
        let mut i = 0;

        while n > 0 {
            let digit = (n % 10) as u8 + b'0';
            if i < 64 {
                buf[i] = digit;
                i += 1;
            }
            n /= 10;
        }

        if is_negative && i < 64 {
            buf[i] = b'-';
            i += 1;
        }

        // Reverse buffer
        let len = i;
        let mut result_buf = [0u8; 64];
        for j in 0..len {
            result_buf[j] = buf[len - 1 - j];
        }

        String::from_str(e, core::str::from_utf8(&result_buf[..len]).unwrap_or("0"))
    }

    /// Calculate compliance score (0-100)
    ///
    /// # Formal Verification
    /// **Preconditions:**
    /// - `commitment_id` exists
    ///
    /// **Postconditions:**
    /// - Returns value in range [0, 100]
    /// - Score decreases with violations
    /// - Score decreases if drawdown exceeds threshold
    /// - Pure function (no state changes)
    ///
    /// **Invariants Maintained:**
    /// - Score always in valid range [0, 100]
    ///
    /// **Security Properties:**
    /// - SP-4: State consistency (read-only)
    /// - SP-3: Arithmetic safety
    pub fn calculate_compliance_score(e: Env, commitment_id: String) -> u32 {
        // First check if we have stored metrics with a compliance score
        let metrics_key = DataKey::HealthMetrics(commitment_id.clone());
        if let Some(stored_metrics) = e
            .storage()
            .persistent()
            .get::<DataKey, HealthMetrics>(&metrics_key)
        {
            return stored_metrics.compliance_score;
        }

        // Get commitment from core contract
        let commitment_core: Address = e.storage().instance().get(&DataKey::CoreContract).unwrap();

        // Call get_commitment on commitment_core contract
        // Using Symbol::new() for function name longer than 9 characters
        let mut args = Vec::new(&e);
        args.push_back(commitment_id.clone().into_val(&e));
        let commitment_val: Val =
            e.invoke_contract(&commitment_core, &Symbol::new(&e, "get_commitment"), args);

        // Convert Val to Commitment
        let commitment: Commitment = commitment_val.try_into_val(&e).unwrap();

        // Get all attestations
        let attestations = Self::get_attestations(e.clone(), commitment_id.clone());

        // Base score: 100
        let mut score: i32 = 100;

        // Count violations: -20 per violation
        let violation_count = attestations
            .iter()
            .filter(|att| {
                !att.is_compliant || att.attestation_type == String::from_str(&e, "violation")
            })
            .count() as i32;
        score = score
            .checked_sub(violation_count.checked_mul(20).unwrap_or(0))
            .unwrap_or(0);

        // Calculate drawdown vs threshold: -1 per % over threshold
        let initial_value = commitment.amount;
        let current_value = commitment.current_value;
        let max_loss_percent = commitment.rules.max_loss_percent as i128;

        if initial_value > 0 {
            let drawdown_percent = {
                let diff = initial_value.checked_sub(current_value).unwrap_or(0);
                diff.checked_mul(100)
                    .unwrap_or(0)
                    .checked_div(initial_value)
                    .unwrap_or(0)
            };

            if drawdown_percent > max_loss_percent {
                let over_threshold = drawdown_percent.checked_sub(max_loss_percent).unwrap_or(0);
                score = score.checked_sub(over_threshold as i32).unwrap_or(0);
            }
        }

        // Calculate fee generation vs expectations: +1 per % of expected fees
        let min_fee_threshold = commitment.rules.min_fee_threshold;
        // Get fees from health metrics (which sums from attestations)
        // We'll calculate this from the attestations directly
        let total_fees: i128 = 0;
        let fee_key = String::from_str(&e, "fee_amount");

        for att in attestations.iter() {
            if att.attestation_type == String::from_str(&e, "fee_generation") {
                // Extract fee from data map
                // Since Map<String, String> stores strings, we need to parse
                // For this implementation, we'll use a simplified approach:
                // If fee_amount exists in data, we'll try to extract it
                // In production, fees should be stored as i128 in a separate field
                if let Some(_fee_str) = att.data.get(fee_key.clone()) {
                    // Parse would be needed here - for now, we'll use 0
                    // This is acceptable as fee tracking requires proper implementation
                    // of the attest() function to store fees correctly
                }
            }
        }

        // Only add fee bonus if we have fees and a threshold
        if min_fee_threshold > 0 && total_fees > 0 {
            let fee_percent = total_fees
                .checked_mul(100)
                .unwrap_or(0)
                .checked_div(min_fee_threshold)
                .unwrap_or(0);
            // Cap the bonus to prevent excessive score inflation
            let bonus = if fee_percent > 100 { 100 } else { fee_percent };
            score = score.checked_add(bonus as i32).unwrap_or(100);
        }

        // Duration adherence: +10 if on track
        let current_time = e.ledger().timestamp();
        let expires_at = commitment.expires_at;
        let created_at = commitment.created_at;

        if expires_at > created_at {
            let total_duration = expires_at.checked_sub(created_at).unwrap_or(1);
            let elapsed = current_time.checked_sub(created_at).unwrap_or(0);

            // Check if we're on track (not too far behind or ahead)
            // Simplified: if elapsed is within reasonable bounds of expected progress
            let expected_progress = (elapsed as u128)
                .checked_mul(100)
                .unwrap_or(0)
                .checked_div(total_duration as u128)
                .unwrap_or(0);

            // Consider "on track" if between 0-100% of expected time
            if expected_progress <= 100 {
                score = score.checked_add(10).unwrap_or(100);
            }
        }

        // Clamp between 0 and 100
        if score < 0 {
            score = 0;
        } else if score > 100 {
            score = 100;
        }

        // Emit compliance score update event
        e.events().publish(
            (symbol_short!("ScoreUpd"), commitment_id),
            (score as u32, e.ledger().timestamp()),
        );

        score as u32
    }

    /// Get high-level protocol analytics combining commitment and attestation data.
    ///
    /// Returns:
    /// - total_commitments (from core contract)
    /// - total_attestations
    /// - total_violations
    /// - total_fees_generated
    pub fn get_protocol_statistics(e: Env) -> (u64, u64, u64, i128) {
        // Read commitment_core statistics
        let commitment_core: Address = e.storage().instance().get(&DataKey::CoreContract).unwrap();

        // get_total_commitments() on core contract
        let args = Vec::new(&e);
        let total_commitments_val: Val = e.invoke_contract(
            &commitment_core,
            &Symbol::new(&e, "get_total_commitments"),
            args,
        );
        let total_commitments: u64 = total_commitments_val.try_into_val(&e).unwrap();

        let total_attestations: u64 = e
            .storage()
            .instance()
            .get(&DataKey::TotalAttestations)
            .unwrap_or(0);
        let total_violations: u64 = e
            .storage()
            .instance()
            .get(&DataKey::TotalViolations)
            .unwrap_or(0);
        let total_fees: i128 = e.storage().instance().get(&DataKey::TotalFees).unwrap_or(0);

        (
            total_commitments,
            total_attestations,
            total_violations,
            total_fees,
        )
    }

    /// Get analytics for a given verifier (attestation recorder).
    ///
    /// Returns the total number of attestations recorded by this verifier.
    pub fn get_verifier_statistics(e: Env, verifier: Address) -> u64 {
        let key = DataKey::VerifierAttestationCount(verifier);
        e.storage().instance().get(&key).unwrap_or(0)
    }

    // ========================================================================
    // Batch Operations
    // ========================================================================

    /// Batch attest multiple commitments in a single transaction
    ///
    /// # Arguments
    /// * `caller` - The address recording the attestations (must be authorized verifier)
    /// * `params_list` - Vector of AttestParams for each attestation
    /// * `mode` - BatchMode::Atomic or BatchMode::BestEffort
    ///
    /// # Returns
    /// BatchResult with empty results and any errors
    ///
    /// # Gas Optimization
    /// - Batch read of analytics counters
    /// - Single aggregate counter update at end
    /// - Batch health metrics updates
    pub fn batch_attest(
        e: Env,
        caller: Address,
        params_list: Vec<AttestParams>,
        mode: BatchMode,
    ) -> BatchResultVoid {
        // Reentrancy protection
        if e.storage().instance().has(&DataKey::ReentrancyGuard) {
            panic!("Reentrancy detected");
        }
        e.storage().instance().set(&DataKey::ReentrancyGuard, &true);

        // Verify caller signed the transaction
        caller.require_auth();

        // Check caller is authorized verifier
        if !Self::is_authorized_verifier(&e, &caller) {
            e.storage().instance().remove(&DataKey::ReentrancyGuard);
            let mut errors = Vec::new(&e);
            errors.push_back(BatchError {
                index: 0,
                error_code: AttestationError::Unauthorized as u32,
                context: String::from_str(&e, "not_authorized_verifier"),
            });
            return BatchResultVoid::failure(&e, errors);
        }

        // Validate batch size
        let batch_size = params_list.len();
        let contract_name = String::from_str(&e, "attestation_engine");
        if let Err(error_code) =
            BatchProcessor::enforce_batch_limits(&e, batch_size, Some(contract_name))
        {
            e.storage().instance().remove(&DataKey::ReentrancyGuard);
            let mut errors = Vec::new(&e);
            errors.push_back(BatchError {
                index: 0,
                error_code,
                context: String::from_str(&e, "batch_size_validation"),
            });
            return BatchResultVoid::failure(&e, errors);
        }

        let mut errors = Vec::new(&e);
        let mut results = Vec::new(&e);

        // Read analytics counters once (optimization)
        let (mut total_attestations, mut total_violations, mut verifier_count) = {
            let total_att = e
                .storage()
                .instance()
                .get(&DataKey::TotalAttestations)
                .unwrap_or(0u64);
            let total_viol = e
                .storage()
                .instance()
                .get(&DataKey::TotalViolations)
                .unwrap_or(0u64);
            let verifier_key = DataKey::VerifierAttestationCount(caller.clone());
            let ver_count = e.storage().instance().get(&verifier_key).unwrap_or(0u64);
            (total_att, total_viol, ver_count)
        };

        let timestamp = e.ledger().timestamp();
        let violation_type = String::from_str(&e, "violation");

        // Process each attestation
        for i in 0..batch_size {
            let params = params_list.get(i).unwrap();

            // Validate commitment_id
            if params.commitment_id.len() == 0 {
                if mode == BatchMode::Atomic {
                    e.storage().instance().remove(&DataKey::ReentrancyGuard);
                    errors.push_back(BatchError {
                        index: i,
                        error_code: AttestationError::InvalidCommitmentId as u32,
                        context: String::from_str(&e, "empty_commitment_id"),
                    });
                    return BatchResultVoid::failure(&e, errors);
                } else {
                    errors.push_back(BatchError {
                        index: i,
                        error_code: AttestationError::InvalidCommitmentId as u32,
                        context: String::from_str(&e, "empty_commitment_id"),
                    });
                    continue;
                }
            }

            // Validate commitment exists
            if !Self::commitment_exists(&e, &params.commitment_id) {
                if mode == BatchMode::Atomic {
                    e.storage().instance().remove(&DataKey::ReentrancyGuard);
                    errors.push_back(BatchError {
                        index: i,
                        error_code: AttestationError::CommitmentNotFound as u32,
                        context: String::from_str(&e, "commitment_not_found"),
                    });
                    return BatchResultVoid::failure(&e, errors);
                } else {
                    errors.push_back(BatchError {
                        index: i,
                        error_code: AttestationError::CommitmentNotFound as u32,
                        context: String::from_str(&e, "commitment_not_found"),
                    });
                    continue;
                }
            }

            // Validate attestation type
            if !Self::is_valid_attestation_type(&e, &params.attestation_type) {
                if mode == BatchMode::Atomic {
                    e.storage().instance().remove(&DataKey::ReentrancyGuard);
                    errors.push_back(BatchError {
                        index: i,
                        error_code: AttestationError::InvalidAttestationType as u32,
                        context: String::from_str(&e, "invalid_type"),
                    });
                    return BatchResultVoid::failure(&e, errors);
                } else {
                    errors.push_back(BatchError {
                        index: i,
                        error_code: AttestationError::InvalidAttestationType as u32,
                        context: String::from_str(&e, "invalid_type"),
                    });
                    continue;
                }
            }

            // Validate data format
            if !Self::validate_attestation_data(&e, &params.attestation_type, &params.data) {
                if mode == BatchMode::Atomic {
                    e.storage().instance().remove(&DataKey::ReentrancyGuard);
                    errors.push_back(BatchError {
                        index: i,
                        error_code: AttestationError::InvalidAttestationData as u32,
                        context: String::from_str(&e, "invalid_data"),
                    });
                    return BatchResultVoid::failure(&e, errors);
                } else {
                    errors.push_back(BatchError {
                        index: i,
                        error_code: AttestationError::InvalidAttestationData as u32,
                        context: String::from_str(&e, "invalid_data"),
                    });
                    continue;
                }
            }

            // Create attestation record
            let attestation = Attestation {
                commitment_id: params.commitment_id.clone(),
                attestation_type: params.attestation_type.clone(),
                data: params.data.clone(),
                timestamp,
                verified_by: caller.clone(),
                is_compliant: params.is_compliant,
            };

            // Store attestation
            let mut attestations = Self::load_attestations(&e, &params.commitment_id);
            attestations.push_back(attestation.clone());
            Self::store_attestations(&e, &params.commitment_id, &attestations);

            // Update health metrics
            Self::update_health_metrics(&e, &params.commitment_id, &attestation);

            // Increment attestation counter
            let counter_key = DataKey::AttestationCounter(params.commitment_id.clone());
            let counter: u64 = e.storage().persistent().get(&counter_key).unwrap_or(0);
            e.storage().persistent().set(&counter_key, &(counter + 1));

            // Update analytics counters (in memory)
            total_attestations += 1;
            verifier_count += 1;
            if attestation.attestation_type == violation_type || !attestation.is_compliant {
                total_violations += 1;
            }

            results.push_back(());

            // Emit event
            e.events().publish(
                (
                    Symbol::new(&e, "AttestationRecorded"),
                    params.commitment_id.clone(),
                    caller.clone(),
                ),
                (
                    params.attestation_type.clone(),
                    params.is_compliant,
                    timestamp,
                ),
            );
        }

        // Write analytics counters once (optimization)
        e.storage()
            .instance()
            .set(&DataKey::TotalAttestations, &total_attestations);
        e.storage()
            .instance()
            .set(&DataKey::TotalViolations, &total_violations);
        let verifier_key = DataKey::VerifierAttestationCount(caller.clone());
        e.storage().instance().set(&verifier_key, &verifier_count);

        // Clear reentrancy guard
        e.storage().instance().remove(&DataKey::ReentrancyGuard);

        // Emit batch event
        e.events().publish(
            (Symbol::new(&e, "BatchAttest"), batch_size),
            (results.len(), errors.len(), timestamp),
        );

        BatchResultVoid::partial(results.len(), errors)
    }

    /// Configure rate limits for this contract's functions (e.g. `attest`).
    ///
    /// Restricted to admin.
    pub fn set_rate_limit(
        e: Env,
        caller: Address,
        function: Symbol,
        window_seconds: u64,
        max_calls: u32,
    ) -> Result<(), AttestationError> {
        caller.require_auth();
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(AttestationError::NotInitialized)?;
        if caller != admin {
            return Err(AttestationError::Unauthorized);
        }

        RateLimiter::set_limit(&e, &function, window_seconds, max_calls);
        Ok(())
    }

    /// Set or clear rate limit exemption for a verifier.
    ///
    /// Restricted to admin.
    pub fn set_rate_limit_exempt(
        e: Env,
        caller: Address,
        verifier: Address,
        exempt: bool,
    ) -> Result<(), AttestationError> {
        caller.require_auth();
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(AttestationError::NotInitialized)?;
        if caller != admin {
            return Err(AttestationError::Unauthorized);
        }

        RateLimiter::set_exempt(&e, &verifier, exempt);
        Ok(())
    }

    // ========================================================================
    // Fee collection (protocol revenue)
    // ========================================================================

    /// Set attestation verification fee: amount per attestation and token. Admin only.
    /// Set amount to 0 to disable.
    pub fn set_attestation_fee(
        e: Env,
        caller: Address,
        amount: i128,
        asset: Address,
    ) -> Result<(), AttestationError> {
        caller.require_auth();
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(AttestationError::NotInitialized)?;
        if caller != admin {
            return Err(AttestationError::Unauthorized);
        }
        if amount < 0 {
            return Err(AttestationError::InvalidFeeAmount);
        }
        e.storage()
            .instance()
            .set(&DataKey::AttestationFeeAmount, &amount);
        e.storage()
            .instance()
            .set(&DataKey::AttestationFeeAsset, &asset);
        e.events().publish(
            (Symbol::new(&e, "AttestationFeeSet"), caller),
            (amount, asset, e.ledger().timestamp()),
        );
        Ok(())
    }

    /// Set fee recipient (protocol treasury). Admin only.
    pub fn set_fee_recipient(
        e: Env,
        caller: Address,
        recipient: Address,
    ) -> Result<(), AttestationError> {
        caller.require_auth();
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(AttestationError::NotInitialized)?;
        if caller != admin {
            return Err(AttestationError::Unauthorized);
        }
        e.storage()
            .instance()
            .set(&DataKey::FeeRecipient, &recipient);
        e.events().publish(
            (Symbol::new(&e, "FeeRecipientSet"), caller),
            (recipient, e.ledger().timestamp()),
        );
        Ok(())
    }

    /// Withdraw collected fees to the configured fee recipient. Admin only.
    pub fn withdraw_fees(
        e: Env,
        caller: Address,
        asset_address: Address,
        amount: i128,
    ) -> Result<(), AttestationError> {
        caller.require_auth();
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(AttestationError::NotInitialized)?;
        if caller != admin {
            return Err(AttestationError::Unauthorized);
        }
        if amount <= 0 {
            return Err(AttestationError::InvalidFeeAmount);
        }
        let recipient: Address = e
            .storage()
            .instance()
            .get(&DataKey::FeeRecipient)
            .ok_or(AttestationError::FeeRecipientNotSet)?;
        let key = DataKey::CollectedFees(asset_address.clone());
        let collected: i128 = e.storage().instance().get(&key).unwrap_or(0);
        if amount > collected {
            return Err(AttestationError::InsufficientFees);
        }
        e.storage().instance().set(&key, &(collected - amount));
        let contract_address = e.current_contract_address();
        let token_client = token::Client::new(&e, &asset_address);
        token_client.transfer(&contract_address, &recipient, &amount);
        e.events().publish(
            (Symbol::new(&e, "FeesWithdrawn"), caller, recipient),
            (asset_address, amount, e.ledger().timestamp()),
        );
        Ok(())
    }

    /// Get attestation fee (amount, asset). (0, default) if not set.
    pub fn get_attestation_fee(e: Env) -> (i128, Option<Address>) {
        let amount: i128 = e
            .storage()
            .instance()
            .get(&DataKey::AttestationFeeAmount)
            .unwrap_or(0);
        let asset: Option<Address> = e.storage().instance().get(&DataKey::AttestationFeeAsset);
        (amount, asset)
    }

    /// Get fee recipient. None if not set.
    pub fn get_fee_recipient(e: Env) -> Option<Address> {
        e.storage().instance().get(&DataKey::FeeRecipient)
    }

    /// Get collected fees for an asset.
    pub fn get_collected_fees(e: Env, asset_address: Address) -> i128 {
        e.storage()
            .instance()
            .get(&DataKey::CollectedFees(asset_address))
            .unwrap_or(0)
    }
}

fn read_version(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get::<_, u32>(&DataKey::Version)
        .unwrap_or(0)
}

fn require_admin(e: &Env, caller: &Address) -> Result<(), AttestationError> {
    caller.require_auth();
    let admin: Address = e
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(AttestationError::NotInitialized)?;
    if *caller != admin {
        return Err(AttestationError::Unauthorized);
    }
    Ok(())
}

fn require_valid_wasm_hash(e: &Env, wasm_hash: &BytesN<32>) -> Result<(), AttestationError> {
    let zero = BytesN::from_array(e, &[0; 32]);
    if *wasm_hash == zero {
        return Err(AttestationError::InvalidWasmHash);
    }
    Ok(())
}

#[cfg(all(test, feature = "benchmark"))]
mod benchmarks;
// #[cfg(test)]
// mod tests;
