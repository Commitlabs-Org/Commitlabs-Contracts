//! Pausable contract functionality for emergency stops

use soroban_sdk::{symbol_short, Env, Symbol};

use super::events::Events;

/// Pausable contract functionality
pub struct Pausable;

impl Pausable {
    /// Storage key for the paused state
    pub const PAUSED_KEY: Symbol = symbol_short!("paused");

    pub fn paused_key(env: &Env) -> Symbol {
        Symbol::new(env, "paused")
    }

    /// Check if the contract is currently paused
    ///
    /// # Arguments
    /// * `e` - The environment
    ///
    /// # Returns
    /// `true` if paused, `false` otherwise
    pub fn is_paused(e: &Env) -> bool {
        let paused_key = symbol_short!("paused");
        e.storage()
            .instance()
            .get::<_, bool>(&Self::paused_key(e))
            .unwrap_or(false)
    }

    /// Pause the contract
    ///
    /// # Arguments
    /// * `e` - The environment
    ///
    /// # Panics
    /// Panics if contract is already paused
    pub fn pause(e: &Env) {
        if Self::is_paused(e) {
            panic!("Contract is already paused");
        }

        // Set paused state
        e.storage().instance().set(&Self::paused_key(e), &true);

        // Emit pause event
        Events::emit(e, symbol_short!("Pause"), ());
    }

    /// Unpause the contract
    ///
    /// # Arguments
    /// * `e` - The environment
    ///
    /// # Panics
    /// Panics if contract is already unpaused
    pub fn unpause(e: &Env) {
        if !Self::is_paused(e) {
            panic!("Contract is already unpaused");
        }

        // Clear paused state
        e.storage().instance().set(&Self::paused_key(e), &false);

        // Emit unpause event
        Events::emit(e, symbol_short!("Unpause"), ());
    }

    /// Modifier to require that the contract is not paused
    ///
    /// # Arguments
    /// * `e` - The environment
    ///
    /// # Panics
    /// Panics if contract is paused
    pub fn require_not_paused(e: &Env) {
        if Self::is_paused(e) {
            panic!("Contract is paused - operation not allowed");
        }
    }

    /// Modifier to require that the contract is paused
    ///
    /// # Arguments
    /// * `e` - The environment
    ///
    /// # Panics
    /// Panics if contract is not paused
    pub fn require_paused(e: &Env) {
        if !Self::is_paused(e) {
            panic!("Contract is not paused");
        }
    }
}
