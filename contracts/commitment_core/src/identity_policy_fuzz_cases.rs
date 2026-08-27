#[cfg(test)]
mod additional_cases {
    use super::super::identity_policy::*;
    fn id(n: u8, s: u64) -> CommitmentIdentity { derive_identity([n; 32], s).unwrap() }
    fn rec(n: u8, s: u64, f: u8) -> CommitmentRecord { CommitmentRecord { identity: id(n, s), fingerprint: [f; 32] } }

    #[test] fn case_01_rejects_zero_sequence() { assert!(derive_identity([1; 32], 0).is_err()); }
    #[test] fn case_02_rejects_zero_instance() { assert!(derive_identity([0; 32], 1).is_err()); }
    #[test] fn case_03_accepts_sequence_one() { assert!(derive_identity([1; 32], 1).is_ok()); }
    #[test] fn case_04_accepts_large_sequence() { assert!(derive_identity([1; 32], u64::MAX).is_ok()); }
    #[test] fn case_05_instance_a_differs_from_b() { assert_ne!(id(1, 1), id(2, 1)); }
    #[test] fn case_06_sequence_a_differs_from_b() { assert_ne!(id(1, 1), id(1, 2)); }
    #[test] fn case_07_equal_keys_replay() { let r = rec(1, 1, 1); assert!(matches!(resolve_retry(Some(r), r.identity, r.fingerprint), Ok(ResolveResult::Replay(_)))); }
    #[test] fn case_08_changed_fingerprint_conflicts() { let r = rec(1, 1, 1); assert_eq!(resolve_retry(Some(r), r.identity, [2; 32]), Err(IdentityError::FingerprintConflict)); }
    #[test] fn case_09_changed_instance_collides() { let r = rec(1, 1, 1); assert_eq!(resolve_retry(Some(r), id(2, 1), r.fingerprint), Err(IdentityError::Collision)); }
    #[test] fn case_10_missing_is_new() { assert!(matches!(resolve_retry(None, id(1, 1), [1; 32]), Ok(ResolveResult::New(_)))); }
    #[test] fn case_11_registry_find_missing() { assert_eq!(IdentityRegistry::empty().find(id(1, 1)), None); }
    #[test] fn case_12_registry_insert_find() { let mut x = IdentityRegistry::empty(); x.insert(rec(1, 1, 1)).unwrap(); assert!(x.find(id(1, 1)).is_some()); }
    #[test] fn case_13_registry_insert_two() { let mut x = IdentityRegistry::empty(); x.insert(rec(1, 1, 1)).unwrap(); x.insert(rec(1, 2, 2)).unwrap(); assert_eq!(x.length, 2); }
    #[test] fn case_14_registry_instance_count_zero() { assert_eq!(IdentityRegistry::empty().instance_count([1; 32]), 0); }
    #[test] fn case_15_registry_sequence_missing() { let mut x = IdentityRegistry::empty(); x.insert(rec(1, 1, 1)).unwrap(); assert!(!x.contains_sequence([1; 32], 2)); }
    #[test] fn case_16_registry_sequence_present() { let mut x = IdentityRegistry::empty(); x.insert(rec(1, 1, 1)).unwrap(); assert!(x.contains_sequence([1; 32], 1)); }
    #[test] fn case_17_migration_forward() { let mut x = IdentityRegistry::empty(); let old = rec(1, 1, 1); x.insert(old).unwrap(); assert!(x.migrate(old, id(1, 3)).is_ok()); }
    #[test] fn case_18_migration_back_fails() { let mut x = IdentityRegistry::empty(); let old = rec(1, 3, 1); x.insert(old).unwrap(); assert!(x.migrate(old, id(1, 2)).is_err()); }
    #[test] fn case_19_migration_cross_instance_fails() { let mut x = IdentityRegistry::empty(); let old = rec(1, 1, 1); x.insert(old).unwrap(); assert!(x.migrate(old, id(2, 2)).is_err()); }
    #[test] fn case_20_migration_missing_fails() { let mut x = IdentityRegistry::empty(); assert!(x.migrate(rec(1, 1, 1), id(1, 2)).is_err()); }
    #[test] fn case_21_migration_collision_fails() { let mut x = IdentityRegistry::empty(); let old = rec(1, 1, 1); x.insert(old).unwrap(); x.insert(rec(1, 2, 2)).unwrap(); assert!(x.migrate(old, id(1, 2)).is_err()); }
    #[test] fn case_22_fingerprint_binds_amount() { assert_ne!(fingerprint_request([1; 32], 1, 1, [2; 32]), fingerprint_request([1; 32], 2, 1, [2; 32])); }
    #[test] fn case_23_fingerprint_binds_duration() { assert_ne!(fingerprint_request([1; 32], 1, 1, [2; 32]), fingerprint_request([1; 32], 1, 2, [2; 32])); }
    #[test] fn case_24_fingerprint_binds_asset() { assert_ne!(fingerprint_request([1; 32], 1, 1, [2; 32]), fingerprint_request([1; 32], 1, 1, [3; 32])); }
    #[test] fn case_25_fingerprint_binds_owner() { assert_ne!(fingerprint_request([1; 32], 1, 1, [2; 32]), fingerprint_request([3; 32], 1, 1, [2; 32])); }
    #[test] fn case_26_identity_well_formed() { assert!(is_well_formed_identity(id(1, 1))); }
    #[test] fn case_27_identity_zero_instance_bad() { assert!(!is_well_formed_identity(CommitmentIdentity { instance: [0; 32], sequence: 1 })); }
    #[test] fn case_28_identity_zero_sequence_bad() { assert!(!is_well_formed_identity(CommitmentIdentity { instance: [1; 32], sequence: 0 })); }
    #[test] fn case_29_registry_is_copyable() { let x = IdentityRegistry::empty(); let y = x; assert_eq!(x, y); }
    #[test] fn case_30_record_is_copyable() { let x = rec(1, 1, 1); let y = x; assert_eq!(x, y); }
    #[test] fn case_31_registry_capacity_starts_at_zero() { assert_eq!(IdentityRegistry::empty().length, 0); }
    #[test] fn case_32_registry_counts_one_instance() { let mut x = IdentityRegistry::empty(); x.insert(rec(7, 1, 1)).unwrap(); assert_eq!(x.instance_count([7; 32]), 1); }
    #[test] fn case_33_registry_excludes_other_instance() { let mut x = IdentityRegistry::empty(); x.insert(rec(7, 1, 1)).unwrap(); assert_eq!(x.instance_count([8; 32]), 0); }
    #[test] fn case_34_replay_keeps_identity() { let r = rec(9, 3, 4); assert_eq!(resolve_retry(Some(r), r.identity, r.fingerprint), Ok(ResolveResult::Replay(id(9, 3)))); }
    #[test] fn case_35_new_keeps_identity() { assert_eq!(resolve_retry(None, id(9, 3), [4; 32]), Ok(ResolveResult::New(id(9, 3)))); }
    #[test] fn case_36_conflict_is_not_replay() { let r = rec(9, 3, 4); assert_ne!(resolve_retry(Some(r), r.identity, [5; 32]), Ok(ResolveResult::Replay(r.identity))); }
}
