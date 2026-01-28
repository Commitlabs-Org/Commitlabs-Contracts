//! Integration tests for shared utilities

#[cfg(test)]
mod integration_tests {
    use crate::math::SafeMath;
    use crate::validation::Validation;
    use crate::time::TimeUtils;
    use crate::storage::Storage;
    use crate::access_control::AccessControl;
    use crate::events::Events;
    use soroban_sdk::{Env, String as SorobanString, contract, contractimpl};

    // Dummy contract used to provide a valid contract context for integration tests
    #[contract]
    pub struct TestContract;

    #[contractimpl]
    impl TestContract {
        pub fn stub() {}
    }

    #[test]
    fn test_math_and_validation_integration() {
        // Test that math utilities work with validation
        let amount = 1000i128;
        Validation::require_positive(amount);
        
        let percent = SafeMath::percent(amount, 10);
        assert_eq!(percent, 100);
        
        Validation::require_valid_percent(10);
    }

    #[test]
    fn test_time_and_storage_integration() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            // Set up storage
            Storage::set_initialized(&env);
            let admin = <soroban_sdk::Address as soroban_sdk::testutils::Address>::generate(&env);
            Storage::set_admin(&env, &admin);

            // Use time utilities
            let expiration = TimeUtils::calculate_expiration(&env, 30);
            assert!(expiration > TimeUtils::now(&env));
        });
    }

    #[test]
    fn test_access_control_and_storage() {
        let env = Env::default();
        let admin = <soroban_sdk::Address as soroban_sdk::testutils::Address>::generate(&env);

        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            Storage::set_initialized(&env);
            Storage::set_admin(&env, &admin);

            assert!(AccessControl::is_admin(&env, &admin));
        });
    }

    #[test]
    fn test_events_and_validation() {
        let env = Env::default();
        let creator = <soroban_sdk::Address as soroban_sdk::testutils::Address>::generate(&env);
        let id = SorobanString::from_str(&env, "test_id");
        
        Validation::require_non_empty_string(&id, "id");
        Events::emit_created(&env, &id, &creator, (100i128,));
    }
}
