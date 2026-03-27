//! # Rate Limiting Utilities
//!
//! Provides a gas-efficient, fixed-window rate limiter to protect contract
//! entry points from spam or resource exhaustion.
//!
//! ### Features
//! * **Granularity**: Supports per-address and per-function limits.
//! * **Flexibility**: Configurable windows and call maximums.
//! * **Exemptions**: Optional allowlist for trusted addresses or contracts.
//!
//! ### Storage Layout
//! * `(RL_CFG, function)`: `(window_seconds, max_calls)` in instance storage.
//! * `(RL_ST, address, function)`: `(window_start, count)` in instance storage.
//! * `(RL_EX, address)`: `bool` exemption flag.

use soroban_sdk::{Address, Env, Symbol};

use crate::time::TimeUtils;

/// Internal storage key prefixes for rate limiting state and config.
mod keys {
    use soroban_sdk::{symbol_short, Symbol};

    /// Prefix for the `(window_seconds, max_calls)` configuration.
    pub const RATE_LIMIT_CONFIG: Symbol = symbol_short!("RL_CFG");
    /// Prefix for the per-user `(window_start, count)` state.
    pub const RATE_LIMIT_STATE: Symbol = symbol_short!("RL_ST");
    /// Prefix for the boolean exemption flag for an address.
    pub const RATE_LIMIT_EXEMPT: Symbol = symbol_short!("RL_EX");
}

/// Helper for enforcing rate limits on contract calls.
pub struct RateLimiter;

impl RateLimiter {
    /// Defines a rate limit for a specific function symbol.
    ///
    /// ### Parameters
    /// * `e` - The Soroban environment.
    /// * `function` - The symbol identifying the limited function.
    /// * `window_seconds` - Duration of the rolling fixed window.
    /// * `max_calls` - Maximum allowed calls per window.
    ///
    /// ### Errors
    /// * Panics if `window_seconds` or `max_calls` is zero.
    ///
    /// ### Security
    /// * This function does not perform auth; the caller MUST be an admin.
    pub fn set_limit(e: &Env, function: &Symbol, window_seconds: u64, max_calls: u32) {
        if window_seconds == 0 || max_calls == 0 {
            panic!("Invalid rate limit configuration");
        }

        let key = (keys::RATE_LIMIT_CONFIG, function.clone());
        e.storage()
            .instance()
            .set(&key, &(window_seconds, max_calls));
    }

    /// Removes the rate limit configuration for a function.
    pub fn clear_limit(e: &Env, function: &Symbol) {
        let key = (keys::RATE_LIMIT_CONFIG, function.clone());
        e.storage().instance().remove(&key);
    }

    /// Marks an address as exempt from all rate limiting checks.
    ///
    /// Useful for trusted oracle contracts, relayers, or the admin.
    pub fn set_exempt(e: &Env, address: &Address, exempt: bool) {
        let key = (keys::RATE_LIMIT_EXEMPT, address.clone());
        if exempt {
            e.storage().instance().set(&key, &true);
        } else {
            e.storage().instance().remove(&key);
        }
    }

    /// Checks if a given address is in the exemption list.
    pub fn is_exempt(e: &Env, address: &Address) -> bool {
        let key = (keys::RATE_LIMIT_EXEMPT, address.clone());
        e.storage().instance().get::<_, bool>(&key).unwrap_or(false)
    }

    /// Verifies that the caller has not exceeded the permitted call frequency.
    ///
    /// Increments the local call counter or resets it if the window has expired.
    ///
    /// ### Parameters
    /// * `e` - The Soroban environment.
    /// * `address` - The address identifying the caller.
    /// * `function` - The identifier of the protected logic.
    ///
    /// ### Errors
    /// * Panics with "Rate limit exceeded" if the caller exceeds `max_calls` in the window.
    ///
    /// ### Security
    /// * Uses a **fixed window** algorithm. Users can theoretically burst at the
    ///   end of one window and the start of the next.
    /// * Updates instance storage balance on every successful check.
    pub fn check(e: &Env, address: &Address, function: &Symbol) {
        // Exempt addresses bypass rate limits
        if Self::is_exempt(e, address) {
            return;
        }

        // Load configuration; if none, do nothing
        let cfg_key = (keys::RATE_LIMIT_CONFIG, function.clone());
        let config = e.storage().instance().get::<_, (u64, u32)>(&cfg_key);

        let (window_seconds, max_calls) = match config {
            Some(cfg) => cfg,
            None => return,
        };

        let now = TimeUtils::now(e);

        // Load current state
        let state_key = (keys::RATE_LIMIT_STATE, address.clone(), function.clone());
        let (mut window_start, mut count) = e
            .storage()
            .instance()
            .get::<_, (u64, u32)>(&state_key)
            .unwrap_or((now, 0u32));

        // Reset window if expired
        if now.saturating_sub(window_start) >= window_seconds {
            window_start = now;
            count = 0;
        }

        // Enforce count
        let new_count = count.saturating_add(1);
        if new_count > max_calls {
            panic!("Rate limit exceeded");
        }

        // Persist updated state
        e.storage()
            .instance()
            .set(&state_key, &(window_start, new_count));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        contract, contractimpl, symbol_short,
        testutils::{Address as TestAddress, Ledger},
        Address, Env, Symbol,
    };

    #[contract]
    pub struct TestRateLimitContract;

    #[contractimpl]
    impl TestRateLimitContract {
        pub fn limited_call(e: Env, caller: Address) {
            let fn_symbol = symbol_short!("limited");
            RateLimiter::check(&e, &caller, &fn_symbol);
        }

        pub fn configure_limit(e: Env, function: Symbol, window_seconds: u64, max_calls: u32) {
            RateLimiter::set_limit(&e, &function, window_seconds, max_calls);
        }

        pub fn set_exempt(e: Env, who: Address, exempt: bool) {
            RateLimiter::set_exempt(&e, &who, exempt);
        }
    }

    #[test]
    fn test_rate_limit_allows_within_limit() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestRateLimitContract);
        let client = TestRateLimitContractClient::new(&env, &contract_id);

        let caller = <Address as TestAddress>::generate(&env);

        // Configure: 2 calls per 60 seconds
        client.configure_limit(&symbol_short!("limited"), &60u64, &2u32);

        // First and second calls should succeed
        client.limited_call(&caller);
        client.limited_call(&caller);
    }

    #[test]
    #[should_panic(expected = "Rate limit exceeded")]
    fn test_rate_limit_blocks_on_exceed() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestRateLimitContract);
        let client = TestRateLimitContractClient::new(&env, &contract_id);

        let caller = <Address as TestAddress>::generate(&env);

        // Configure: 1 call per 60 seconds
        client.configure_limit(&symbol_short!("limited"), &60u64, &1u32);

        client.limited_call(&caller);
        // Second call within same window should panic
        client.limited_call(&caller);
    }

    #[test]
    fn test_rate_limit_resets_after_window() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestRateLimitContract);
        let client = TestRateLimitContractClient::new(&env, &contract_id);

        let caller = <Address as TestAddress>::generate(&env);

        // Configure: 1 call per 60 seconds
        client.configure_limit(&symbol_short!("limited"), &60u64, &1u32);

        // Set timestamp to 100
        env.ledger().with_mut(|l| {
            l.timestamp = 100;
        });
        client.limited_call(&caller);

        // Advance beyond window and call again
        env.ledger().with_mut(|l| {
            l.timestamp = 200;
        });
        client.limited_call(&caller);
    }

    #[test]
    fn test_exempt_address_bypasses_limits() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestRateLimitContract);
        let client = TestRateLimitContractClient::new(&env, &contract_id);

        let caller = <Address as TestAddress>::generate(&env);

        // Configure: 1 call per 60 seconds
        client.configure_limit(&symbol_short!("limited"), &60u64, &1u32);

        // Mark as exempt
        client.set_exempt(&caller, &true);

        // Multiple calls should succeed
        client.limited_call(&caller);
        client.limited_call(&caller);
        client.limited_call(&caller);
    }
}
