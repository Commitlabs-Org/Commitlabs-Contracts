//! Pausable contract functionality for emergency stops

use super::events::Events;
use soroban_sdk::{symbol_short, Env, Symbol};

/// Pausable contract functionality
pub struct Pausable;

impl Pausable {
    /// Storage key for the paused state
    pub const PAUSED_KEY: Symbol = symbol_short!("paused");

    /// Check if the contract is currently paused
    ///
    /// # Arguments
    /// * `e` - The environment
    ///
    /// # Returns
    /// `true` if paused, `false` otherwise
    pub fn is_paused(e: &Env) -> bool {
        e.storage()
            .instance()
            .get::<_, bool>(&Self::PAUSED_KEY)
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
        e.storage().instance().set(&Self::PAUSED_KEY, &true);

        // Emit pause event
        Events::emit(e, Symbol::new(e, "Pause"), ());
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
        e.storage().instance().set(&Self::PAUSED_KEY, &false);

        // Emit unpause event
        Events::emit(e, Symbol::new(e, "Unpause"), ());
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
