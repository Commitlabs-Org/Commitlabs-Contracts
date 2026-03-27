//! # Pausable Contract Functionality
//!
//! Provides a mechanism to temporarily halt specific contract operations.
//! This is typically used for maintenance or during low-level security incidents
//! where a full "Emergency Freeze" might be overkill.
//!
//! ### Security
//! * Toggling the paused state MUST be restricted to authorized administrators.
//! * `require_not_paused` should be added to the start of sensitive public functions.


use soroban_sdk::{symbol_short, Env, Symbol};

use super::events::Events;

/// Pausable contract functionality
/// Helper for managing and enforcing a contract's paused state.
pub struct Pausable;

impl Pausable {
    /// Storage key identifier for the boolean paused state.
    pub const PAUSED_KEY: Symbol = symbol_short!("paused");

    /// Accessor for the paused storage key Symbol.
    pub fn paused_key(env: &Env) -> Symbol {
        Symbol::new(env, "paused")
    }

    /// Checks if the contract is currently in a paused state.
    ///
    /// ### Returns
    /// * `true` if paused, `false` otherwise.
    pub fn is_paused(e: &Env) -> bool {
        e.storage()
            .instance()
            .get::<_, bool>(&Self::paused_key(e))
            .unwrap_or(false)
    }

    /// Transitions the contract to a paused state.
    ///
    /// ### Errors
    /// * Panics if the contract is already paused.
    pub fn pause(e: &Env) {
        if Self::is_paused(e) {
            panic!("Contract is already paused");
        }

        // Set paused state
        e.storage().instance().set(&Self::paused_key(e), &true);

        // Emit pause event
        Events::emit(e, symbol_short!("Pause"), ());
    }

    /// Transitions the contract to an unpaused (active) state.
    ///
    /// ### Errors
    /// * Panics if the contract is not currently paused.
    pub fn unpause(e: &Env) {
        if !Self::is_paused(e) {
            panic!("Contract is already unpaused");
        }

        // Clear paused state
        e.storage().instance().set(&Self::paused_key(e), &false);

        // Emit unpause event
        Events::emit(e, symbol_short!("Unpause"), ());
    }

    /// Asserts that the contract is NOT paused.
    ///
    /// ### Errors
    /// * Panics with "Contract is paused" if `is_paused` is true.
    pub fn require_not_paused(e: &Env) {
        if Self::is_paused(e) {
            panic!("Contract is paused - operation not allowed");
        }
    }

    /// Asserts that the contract IS paused.
    ///
    /// ### Errors
    /// * Panics if the contract is active.
    pub fn require_paused(e: &Env) {
        if !Self::is_paused(e) {
            panic!("Contract is not paused");
        }
    }
}
