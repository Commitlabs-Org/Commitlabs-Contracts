#[cfg(test)]
mod matrix {
    use super::super::identity_policy::*;

    fn instance(n: u8) -> InstanceId { [n; 32] }
    fn id(n: u8, sequence: u64) -> CommitmentIdentity { derive_identity(instance(n), sequence).unwrap() }
    fn record(n: u8, sequence: u64, fingerprint: u8) -> CommitmentRecord { CommitmentRecord { identity: id(n, sequence), fingerprint: [fingerprint; 32] } }

    #[test]
    fn distinct_instances_share_no_identity() {
        assert_ne!(id(1, 1), id(2, 1));
    }
    #[test]
    fn sequences_are_part_of_identity() {
        assert_ne!(id(1, 1), id(1, 2));
    }
    #[test]
    fn zero_instance_fails() {
        assert_eq!(derive_identity([0; 32], 1), Err(IdentityError::ZeroInstance));
    }
    #[test]
    fn zero_sequence_fails() {
        assert_eq!(derive_identity(instance(1), 0), Err(IdentityError::ZeroSequence));
    }
    #[test]
    fn identity_round_trip_is_well_formed() {
        assert!(is_well_formed_identity(id(1, 99)));
    }
    #[test]
    fn amount_is_bound_to_request() {
        assert_ne!(fingerprint_request(instance(1), 10, 30, instance(2)), fingerprint_request(instance(1), 11, 30, instance(2)));
    }
    #[test]
    fn duration_is_bound_to_request() {
        assert_ne!(fingerprint_request(instance(1), 10, 30, instance(2)), fingerprint_request(instance(1), 10, 31, instance(2)));
    }
    #[test]
    fn owner_is_bound_to_request() {
        assert_ne!(fingerprint_request(instance(1), 10, 30, instance(2)), fingerprint_request(instance(3), 10, 30, instance(2)));
    }
    #[test]
    fn asset_is_bound_to_request() {
        assert_ne!(fingerprint_request(instance(1), 10, 30, instance(2)), fingerprint_request(instance(1), 10, 30, instance(3)));
    }
    #[test]
    fn exact_request_is_stable() {
        assert_eq!(fingerprint_request(instance(1), 10, 30, instance(2)), fingerprint_request(instance(1), 10, 30, instance(2)));
    }
    #[test]
    fn new_record_resolves_as_new() {
        assert_eq!(resolve_retry(None, id(1, 1), [1; 32]), Ok(ResolveResult::New(id(1, 1))));
    }
    #[test]
    fn same_record_resolves_as_replay() {
        let existing = record(1, 1, 1);
        assert_eq!(resolve_retry(Some(existing), existing.identity, existing.fingerprint), Ok(ResolveResult::Replay(existing.identity)));
    }
    #[test]
    fn changed_payload_resolves_as_conflict() {
        let existing = record(1, 1, 1);
        assert_eq!(resolve_retry(Some(existing), existing.identity, [2; 32]), Err(IdentityError::FingerprintConflict));
    }
    #[test]
    fn changed_instance_resolves_as_collision() {
        let existing = record(1, 1, 1);
        assert_eq!(resolve_retry(Some(existing), id(2, 1), [1; 32]), Err(IdentityError::Collision));
    }
    #[test]
    fn registry_starts_empty() {
        assert_eq!(IdentityRegistry::empty().length, 0);
    }
    #[test]
    fn registry_insert_is_visible() {
        let mut registry = IdentityRegistry::empty();
        registry.insert(record(1, 1, 1)).unwrap();
        assert!(registry.contains_sequence(instance(1), 1));
    }
    #[test]
    fn registry_duplicate_is_replay() {
        let mut registry = IdentityRegistry::empty();
        let value = record(1, 1, 1);
        registry.insert(value).unwrap();
        assert_eq!(registry.insert(value), Ok(ResolveResult::Replay(value.identity)));
    }
    #[test]
    fn registry_duplicate_does_not_grow() {
        let mut registry = IdentityRegistry::empty();
        let value = record(1, 1, 1);
        registry.insert(value).unwrap();
        registry.insert(value).unwrap();
        assert_eq!(registry.length, 1);
    }
    #[test]
    fn registry_conflict_does_not_grow() {
        let mut registry = IdentityRegistry::empty();
        registry.insert(record(1, 1, 1)).unwrap();
        assert!(registry.insert(record(1, 1, 2)).is_err());
        assert_eq!(registry.length, 1);
    }
    #[test]
    fn registry_tracks_multiple_instances() {
        let mut registry = IdentityRegistry::empty();
        registry.insert(record(1, 1, 1)).unwrap();
        registry.insert(record(2, 1, 1)).unwrap();
        assert_eq!(registry.length, 2);
    }
    #[test]
    fn instance_count_is_namespaced() {
        let mut registry = IdentityRegistry::empty();
        registry.insert(record(1, 1, 1)).unwrap();
        registry.insert(record(1, 2, 2)).unwrap();
        registry.insert(record(2, 1, 3)).unwrap();
        assert_eq!(registry.instance_count(instance(1)), 2);
        assert_eq!(registry.instance_count(instance(2)), 1);
    }
    #[test]
    fn migration_preserves_fingerprint() {
        let mut registry = IdentityRegistry::empty();
        let old = record(1, 1, 7);
        registry.insert(old).unwrap();
        registry.migrate(old, id(1, 2)).unwrap();
        assert_eq!(registry.find(id(1, 2)).unwrap().fingerprint, [7; 32]);
    }
    #[test]
    fn migration_removes_old_lookup() {
        let mut registry = IdentityRegistry::empty();
        let old = record(1, 1, 7);
        registry.insert(old).unwrap();
        registry.migrate(old, id(1, 2)).unwrap();
        assert!(!registry.contains_sequence(instance(1), 1));
    }
    #[test]
    fn migration_rejects_instance_change() {
        let mut registry = IdentityRegistry::empty();
        let old = record(1, 1, 7);
        registry.insert(old).unwrap();
        assert_eq!(registry.migrate(old, id(2, 2)), Err(IdentityError::InvalidMigration));
    }
    #[test]
    fn migration_rejects_same_sequence() {
        let mut registry = IdentityRegistry::empty();
        let old = record(1, 1, 7);
        registry.insert(old).unwrap();
        assert_eq!(registry.migrate(old, id(1, 1)), Err(IdentityError::InvalidMigration));
    }
    #[test]
    fn migration_rejects_unknown_record() {
        let mut registry = IdentityRegistry::empty();
        let old = record(1, 1, 7);
        assert_eq!(registry.migrate(old, id(1, 2)), Err(IdentityError::InvalidMigration));
    }
    #[test]
    fn migration_rejects_target_collision() {
        let mut registry = IdentityRegistry::empty();
        let old = record(1, 1, 7);
        registry.insert(old).unwrap();
        registry.insert(record(1, 2, 8)).unwrap();
        assert_eq!(registry.migrate(old, id(1, 2)), Err(IdentityError::Collision));
    }
    #[test]
    fn malformed_instance_is_not_well_formed() {
        assert!(!is_well_formed_identity(CommitmentIdentity { instance: [0; 32], sequence: 3 }));
    }
    #[test]
    fn malformed_sequence_is_not_well_formed() {
        assert!(!is_well_formed_identity(CommitmentIdentity { instance: instance(1), sequence: 0 }));
    }
    #[test]
    fn max_sequence_is_well_formed() {
        assert!(is_well_formed_identity(id(1, u64::MAX)));
    }
    #[test]
    fn max_amount_is_fingerprinted() {
        assert_ne!(fingerprint_request(instance(1), u128::MAX, 1, instance(2)), [0; 32]);
    }
    #[test]
    fn max_duration_is_fingerprinted() {
        assert_ne!(fingerprint_request(instance(1), 1, u32::MAX, instance(2)), [0; 32]);
    }
}
