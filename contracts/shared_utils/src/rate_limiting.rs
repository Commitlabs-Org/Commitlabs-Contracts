//! Rate limiting utilities for Soroban contracts.
//!
//! This module provides a lightweight, gas-conscious rate limiting helper
//! that can be reused across contracts. It supports:
//! - Per-address limits
//! - Per-function limits
//! - Time window based limits (fixed windows)
//! - Optional exemption list
//!
//! Storage layout (instance storage, per contract):
//! - (RL_CFG, function_symbol) -> (window_seconds: u64, max_calls: u32)
//! - (RL_STATE, address, function_symbol) -> (window_start: u64, count: u32)
//! - (RL_EX, address) -> bool

use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::time::TimeUtils;

/// Internal storage key prefixes for rate limiting
mod keys {
    use soroban_sdk::{symbol_short, Symbol};

    // Configuration for a function: (window_seconds, max_calls)
    pub const RATE_LIMIT_CONFIG: Symbol = symbol_short!("RL_CFG");
    // Per-address per-function state: (window_start, count)
    pub const RATE_LIMIT_STATE: Symbol = symbol_short!("RL_ST");
    // Exemption flag for an address
    pub const RATE_LIMIT_EXEMPT: Symbol = symbol_short!("RL_EX");
}

/// Rate limiting helper
pub struct RateLimiter;

impl RateLimiter {
    /// Configure a rate limit for a given function.
    ///
    /// This stores a fixed-window configuration:
    /// - `window_seconds`: length of the window in seconds
    /// - `max_calls`: maximum calls allowed within that window
    ///
    /// Passing zero for either argument is rejected to avoid silent misconfig.
    pub fn set_limit(e: &Env, function: &Symbol, window_seconds: u64, max_calls: u32) {
        if window_seconds == 0 || max_calls == 0 {
            panic!("Invalid rate limit configuration");
        }

        let key = (keys::RATE_LIMIT_CONFIG, function.clone());
        e.storage()
            .instance()
            .set(&key, &(window_seconds, max_calls));
    }

    /// Clear the rate limit configuration for a function.
    pub fn clear_limit(e: &Env, function: &Symbol) {
        let key = (keys::RATE_LIMIT_CONFIG, function.clone());
        e.storage().instance().remove(&key);
    }

    /// Set or clear exemption for an address.
    ///
    /// When `exempt == true`, the address is not subject to rate limits.
    pub fn set_exempt(e: &Env, address: &Address, exempt: bool) {
        let key = (keys::RATE_LIMIT_EXEMPT, address.clone());
        if exempt {
            e.storage().instance().set(&key, &true);
        } else {
            e.storage().instance().remove(&key);
        }
    }

    /// Check if an address is exempt from rate limits.
    pub fn is_exempt(e: &Env, address: &Address) -> bool {
        let key = (keys::RATE_LIMIT_EXEMPT, address.clone());
        e.storage()
            .instance()
            .get::<_, bool>(&key)
            .unwrap_or(false)
    }

    /// Enforce a rate limit for a given address & function.
    ///
    /// Behavior:
    /// - If no config exists for `function`, this is a no-op.
    /// - If `address` is exempt, this is a no-op.
    /// - Otherwise, maintains a fixed time window based on ledger timestamp.
    /// - Panics with `"Rate limit exceeded"` when limit is hit.
    pub fn check(e: &Env, address: &Address, function: &Symbol) {
        // Exempt addresses bypass rate limits
        if Self::is_exempt(e, address) {
            return;
        }

        // Load configuration; if none, do nothing
        let cfg_key = (keys::RATE_LIMIT_CONFIG, function.clone());
        let config = e
            .storage()
            .instance()
            .get::<_, (u64, u32)>(&cfg_key);

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
        contract, contractimpl,
        testutils::{Address as TestAddress, Ledger},
        Env, Address,
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

