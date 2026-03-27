//! # Event Emission Utilities
//!
//! Standardizes event publishing across CommitLabs contracts to ensure
//! consistent off-chain indexing and audit trails.
//!
//! ### Best Practices
//! * Use `emit_error_event` from `error_codes` for failures.
//! * Use these helpers for successful state transitions and lifecycle events.


use soroban_sdk::{symbol_short, Address, Env, String as SorobanString, Symbol, Topics};

/// Helper functions for publishing standardized Soroban events.
pub struct Events;

impl Events {
    /// Publishes a simple event with a single topic.
    ///
    /// ### Parameters
    /// * `e` - The Soroban environment.
    /// * `topic` - The primary identifier for the event.
    /// * `data` - The associated value(s), typically a tuple.
    pub fn emit<T>(e: &Env, topic: Symbol, data: T)
    where
        T: soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
    {
        e.events().publish((topic,), data);
    }

    /// Publishes an event with multiple topics for finer-grained filtering.
    ///
    /// ### Parameters
    /// * `e` - The Soroban environment.
    /// * `topics` - A tuple or vector of topics (must implement `Topics`).
    /// * `data` - The associated value(s).
    pub fn emit_with_topics<T, U>(e: &Env, topics: T, data: U)
    where
        T: Topics,
        U: soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
    {
        e.events().publish(topics, data);
    }

    /// Publishes a "Created" event for a new resource lifecycle.
    ///
    /// ### Parameters
    /// * `e` - The Soroban environment.
    /// * `id` - The unique identifier of the created item.
    /// * `creator` - The address that initiated the creation.
    /// * `data` - Additional metadata associated with the creation.
    pub fn emit_created<T>(e: &Env, id: &SorobanString, creator: &Address, data: T)
    where
        T: soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
    {
        Self::emit_with_topics(
            e,
            (symbol_short!("Created"), id.clone(), creator.clone()),
            data,
        );
    }

    /// Publishes an "Updated" event for an existing resource.
    ///
    /// ### Parameters
    /// * `id` - The unique identifier of the updated item.
    /// * `data` - The fields or values that were changed.
    pub fn emit_updated<T>(e: &Env, id: &SorobanString, data: T)
    where
        T: soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
    {
        Self::emit_with_topics(e, (symbol_short!("Updated"), id.clone()), data);
    }

    /// Publishes a "Deleted" event to signify resource removal.
    pub fn emit_deleted(e: &Env, id: &SorobanString) {
        Self::emit_with_topics(
            e,
            (symbol_short!("Deleted"), id.clone()),
            (e.ledger().timestamp(),),
        );
    }

    /// Publishes a "Transfer" event for assets or ownership tokens.
    ///
    /// ### Parameters
    /// * `from` - The source address.
    /// * `to` - The destination address.
    /// * `amount` - The quantity transferred.
    pub fn emit_transfer(e: &Env, from: &Address, to: &Address, amount: i128) {
        Self::emit_with_topics(
            e,
            (symbol_short!("Transfer"), from.clone(), to.clone()),
            (amount, e.ledger().timestamp()),
        );
    }

    /// Publishes a "Violated" event, typically for business logic or safety violations.
    ///
    /// ### Parameters
    /// * `id` - The item or session ID where the violation occurred.
    /// * `violation_type` - A machine-readable string identifying the violation logic.
    pub fn emit_violation(e: &Env, id: &SorobanString, violation_type: &SorobanString) {
        Self::emit_with_topics(
            e,
            (symbol_short!("Violated"), id.clone()),
            (violation_type.clone(), e.ledger().timestamp()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as TestAddress;

    #[test]
    fn test_emit() {
        let env = Env::default();
        Events::emit(&env, symbol_short!("Test"), (1i128,));
    }

    #[test]
    fn test_emit_created() {
        let env = Env::default();
        let creator = <soroban_sdk::Address as TestAddress>::generate(&env);
        let id = SorobanString::from_str(&env, "test_id");

        Events::emit_created(&env, &id, &creator, (100i128,));
    }

    #[test]
    fn test_emit_transfer() {
        let env = Env::default();
        let from = <soroban_sdk::Address as TestAddress>::generate(&env);
        let to = <soroban_sdk::Address as TestAddress>::generate(&env);

        Events::emit_transfer(&env, &from, &to, 1000);
    }
}
