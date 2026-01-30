//! Emergency control utilities
use super::events::Events;
use soroban_sdk::{symbol_short, Env};

pub mod keys {
    use soroban_sdk::{symbol_short, Symbol};
    pub const EMERGENCY_MODE: Symbol = symbol_short!("EMG_MODE");
}

pub struct EmergencyControl;

impl EmergencyControl {
    /// Check if the contract is in emergency mode
    pub fn is_emergency_mode(e: &Env) -> bool {
        e.storage()
            .instance()
            .get::<_, bool>(&keys::EMERGENCY_MODE)
            .unwrap_or(false)
    }

    /// Require that the contract is NOT in emergency mode
    pub fn require_not_emergency(e: &Env) {
        if Self::is_emergency_mode(e) {
            panic!("Action not allowed in emergency mode");
        }
    }

    /// Require that the contract IS in emergency mode
    pub fn require_emergency(e: &Env) {
        if !Self::is_emergency_mode(e) {
            panic!("Action only allowed in emergency mode");
        }
    }

    /// Set emergency mode status
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
