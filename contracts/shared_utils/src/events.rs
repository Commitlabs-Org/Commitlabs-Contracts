//! Event emission patterns and utilities

use soroban_sdk::{Env, symbol_short, Symbol, Address, String as SorobanString, Topics};

/// Event emission helper functions
pub struct Events;

impl Events {
    /// Emit a simple event with topic and data
    /// 
    /// # Arguments
    /// * `e` - The environment
    /// * `topic` - The event topic (Symbol)
    /// * `data` - The event data (tuple)
    pub fn emit<T>(e: &Env, topic: Symbol, data: T)
    where
        T: soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
    {
        e.events().publish((topic,), data);
    }

    /// Emit an event with multiple topics
    /// 
    /// # Arguments
    /// * `e` - The environment
    /// * `topics` - Tuple of topics (must implement Topics)
    /// * `data` - The event data (tuple)
    pub fn emit_with_topics<T, U>(e: &Env, topics: T, data: U)
    where
        T: Topics,
        U: soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
    {
        e.events().publish(topics, data);
    }

    /// Emit a creation event
    /// 
    /// # Arguments
    /// * `e` - The environment
    /// * `id` - The created item ID
    /// * `creator` - The creator address
    /// * `data` - Additional event data
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

    /// Emit an update event
    /// 
    /// # Arguments
    /// * `e` - The environment
    /// * `id` - The updated item ID
    /// * `data` - The update data
    pub fn emit_updated<T>(e: &Env, id: &SorobanString, data: T)
    where
        T: soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
    {
        Self::emit_with_topics(e, (symbol_short!("Updated"), id.clone()), data);
    }

    /// Emit a deletion event
    /// 
    /// # Arguments
    /// * `e` - The environment
    /// * `id` - The deleted item ID
    pub fn emit_deleted(e: &Env, id: &SorobanString) {
        Self::emit_with_topics(
            e,
            (symbol_short!("Deleted"), id.clone()),
            (e.ledger().timestamp(),),
        );
    }

    /// Emit a transfer event
    /// 
    /// # Arguments
    /// * `e` - The environment
    /// * `from` - The sender address
    /// * `to` - The recipient address
    /// * `amount` - The transfer amount
    pub fn emit_transfer(e: &Env, from: &Address, to: &Address, amount: i128) {
        Self::emit_with_topics(
            e,
            (symbol_short!("Transfer"), from.clone(), to.clone()),
            (amount, e.ledger().timestamp()),
        );
    }

    /// Emit a violation event
    /// 
    /// # Arguments
    /// * `e` - The environment
    /// * `id` - The item ID with violation
    /// * `violation_type` - The type of violation
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
