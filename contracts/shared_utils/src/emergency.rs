//! # Emergency Control Utilities
//!
//! Provides a global circuit breaker (Emergency Mode) to halt sensitive contract
//! operations during incidents.
//!
//! ### Security
//! * When enabled, `require_not_emergency` will panic, effectively freezing state transitions.
//! * Only authorized administrators should be able to toggle emergency mode.

use super::events::Events;
use soroban_sdk::{symbol_short, Env};

/// Storage keys for emergency state.
pub mod keys {
    use soroban_sdk::{symbol_short, Symbol};
    /// Key for the boolean emergency mode flag.
    pub const EMERGENCY_MODE: Symbol = symbol_short!("EMG_MODE");
}

/// Helper for managing and enforcing emergency state.
pub struct EmergencyControl;

impl EmergencyControl {
    /// Checks if the contract is currently in emergency mode.
    ///
    /// ### Returns
    /// * `true` if emergency mode is active, `false` otherwise.
    pub fn is_emergency_mode(e: &Env) -> bool {
        e.storage()
            .instance()
            .get::<_, bool>(&keys::EMERGENCY_MODE)
            .unwrap_or(false)
    }

    /// Asserts that emergency mode is NOT active.
    ///
    /// ### Errors
    /// * Panics if the contract is in emergency mode.
    pub fn require_not_emergency(e: &Env) {
        if Self::is_emergency_mode(e) {
            panic!("Action not allowed in emergency mode");
        }
    }

    /// Asserts that emergency mode IS active.
    ///
    /// Useful for recovery functions that should only run during a freeze.
    pub fn require_emergency(e: &Env) {
        if !Self::is_emergency_mode(e) {
            panic!("Action only allowed in emergency mode");
        }
    }

    /// Toggles the emergency mode flag and emits a diagnostic event.
    ///
    /// ### Parameters
    /// * `e` - The Soroban environment.
    /// * `enabled` - The new state for emergency mode.
    ///
    /// ### Security
    /// * This function does not perform auth; the caller MUST be an admin.
    pub fn set_emergency_mode(e: &Env, enabled: bool) {
        e.storage().instance().set(&keys::EMERGENCY_MODE, &enabled);

        // Emit event for emergency mode change
        let event_type = if enabled {
            symbol_short!("EMG_ON")
        } else {
            symbol_short!("EMG_OFF")
        };
        Events::emit(
            e,
            symbol_short!("EmgMode"),
            (event_type, e.ledger().timestamp()),
        );
    }
}
