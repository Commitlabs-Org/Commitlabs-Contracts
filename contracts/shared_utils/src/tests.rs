//! Integration tests for shared utilities

#[cfg(all(test, not(target_family = "wasm")))]
mod integration_tests {
    use crate::access_control::AccessControl;
    use crate::events::Events;
    use crate::math::SafeMath;
    use crate::pausable::Pausable;
    use crate::storage::Storage;
    use crate::time::TimeUtils;
    use crate::validation::Validation;
    use soroban_sdk::testutils::{Events as _, Ledger};
    use soroban_sdk::{
        contract, contractimpl, symbol_short, vec, Env, IntoVal, String as SorobanString, Symbol,
    };

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

    #[test]
    fn test_penalty_and_validation_integration() {
        let amount = 2_000i128;
        let penalty_percent = 15u32;

        Validation::require_positive(amount);
        Validation::require_valid_percent(penalty_percent);

        let penalty = SafeMath::penalty_amount(amount, penalty_percent);
        let remaining = SafeMath::apply_penalty(amount, penalty_percent);

        Validation::require_non_negative(remaining);
        assert_eq!(penalty, 300);
        assert_eq!(remaining, 1_700);
        assert_eq!(SafeMath::add(remaining, penalty), amount);
    }

    #[test]
    fn test_checked_expiration_and_storage_round_trip() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.ledger().with_mut(|ledger| {
            ledger.timestamp = 5_000;
        });

        env.as_contract(&contract_id, || {
            Storage::set_initialized(&env);

            let expiration_key = symbol_short!("EXPIRY");
            let expiration = TimeUtils::checked_calculate_expiration(&env, 7).unwrap();

            Storage::set(&env, &expiration_key, &expiration);

            assert!(Storage::has(&env, &expiration_key));
            assert_eq!(Storage::get::<u64>(&env, &expiration_key), Some(expiration));
            assert_eq!(
                TimeUtils::seconds_to_days(expiration - TimeUtils::now(&env)),
                7
            );
        });
    }

    #[test]
    fn test_time_validity_changes_with_ledger_progress() {
        let env = Env::default();

        env.ledger().with_mut(|ledger| {
            ledger.timestamp = 10_000;
        });

        let expiration = TimeUtils::calculate_expiration(&env, 2);
        assert!(TimeUtils::is_valid(&env, expiration));
        assert_eq!(
            TimeUtils::time_remaining(&env, expiration),
            TimeUtils::days_to_seconds(2)
        );

        env.ledger().with_mut(|ledger| {
            ledger.timestamp = expiration;
        });

        assert!(TimeUtils::is_expired(&env, expiration));
        assert_eq!(TimeUtils::time_remaining(&env, expiration), 0);
    }

    #[test]
    fn test_access_control_with_authorized_user() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = <soroban_sdk::Address as soroban_sdk::testutils::Address>::generate(&env);
        let authorized_user =
            <soroban_sdk::Address as soroban_sdk::testutils::Address>::generate(&env);
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            Storage::set_initialized(&env);
            Storage::set_admin(&env, &admin);

            let authorized_key: Symbol = symbol_short!("AUTHUSR");
            env.storage()
                .instance()
                .set(&(authorized_key.clone(), authorized_user.clone()), &true);

            AccessControl::require_admin_or_authorized(&env, &authorized_user, &authorized_key);
            assert!(!AccessControl::is_admin(&env, &authorized_user));
        });
    }

    #[test]
    fn test_transfer_event_contains_expected_topics_and_data() {
        let env = Env::default();
        let from = <soroban_sdk::Address as soroban_sdk::testutils::Address>::generate(&env);
        let to = <soroban_sdk::Address as soroban_sdk::testutils::Address>::generate(&env);
        let contract_id = env.register_contract(None, TestContract);

        env.ledger().with_mut(|ledger| {
            ledger.timestamp = 77_777;
        });

        env.as_contract(&contract_id, || {
            Events::emit_transfer(&env, &from, &to, 750);
        });

        let events = env.events().all();
        let last_event = events.last().unwrap();

        assert_eq!(last_event.0, contract_id);
        assert_eq!(
            last_event.1,
            vec![
                &env,
                symbol_short!("Transfer").into_val(&env),
                from.into_val(&env),
                to.into_val(&env)
            ]
        );
        let data: (i128, u64) = last_event.2.into_val(&env);
        assert_eq!(data, (750i128, 77_777u64));
    }

    #[test]
    fn test_pausable_key_alignment_and_state_reads() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            // Ensure constant and helper key resolve to the same symbol.
            assert_eq!(Pausable::PAUSED_KEY, Pausable::paused_key(&env));

            // Simulate contracts that set the key directly on init.
            env.storage()
                .instance()
                .set(&Pausable::PAUSED_KEY, &true);
            assert!(Pausable::is_paused(&env));

            // Updating via helper key should reflect the same storage slot.
            env.storage()
                .instance()
                .set(&Pausable::paused_key(&env), &false);
            assert!(!Pausable::is_paused(&env));
        });
    }

    #[test]
    fn test_pause_unpause_toggles_state_and_emits_events() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            assert!(!Pausable::is_paused(&env));
            Pausable::require_not_paused(&env);

            Pausable::pause(&env);
            assert!(Pausable::is_paused(&env));
            Pausable::require_paused(&env);

            Pausable::unpause(&env);
            assert!(!Pausable::is_paused(&env));
            Pausable::require_not_paused(&env);
        });

        let events = env.events().all();
        assert_eq!(events.len(), 2);

        let pause_event = events.first().unwrap();
        assert_eq!(pause_event.0, contract_id);
        assert_eq!(
            pause_event.1,
            vec![&env, symbol_short!("Pause").into_val(&env)]
        );

        let unpause_event = events.last().unwrap();
        assert_eq!(unpause_event.0, contract_id);
        assert_eq!(
            unpause_event.1,
            vec![&env, symbol_short!("Unpause").into_val(&env)]
        );
    }

    #[test]
    #[should_panic(expected = "Contract is already paused")]
    fn test_pause_when_already_paused_panics() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            Pausable::pause(&env);
            Pausable::pause(&env);
        });
    }

    #[test]
    #[should_panic(expected = "Contract is already unpaused")]
    fn test_unpause_when_unpaused_panics() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            Pausable::unpause(&env);
        });
    }

    #[test]
    #[should_panic(expected = "Contract is paused - operation not allowed")]
    fn test_require_not_paused_panics_when_paused() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            Pausable::pause(&env);
            Pausable::require_not_paused(&env);
        });
    }

    #[test]
    #[should_panic(expected = "Contract is not paused")]
    fn test_require_paused_panics_when_unpaused() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            Pausable::require_paused(&env);
        });
    }
}


