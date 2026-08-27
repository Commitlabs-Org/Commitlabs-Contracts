//! Collision-safe commitment identity policy.
//!
//! The contract adapter stores this identity beside the commitment record.
//! Instance identity is part of the key, so equal local counters in two
//! contract instances never resolve to the same commitment.

pub type InstanceId = [u8; 32];
pub type RequestFingerprint = [u8; 32];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommitmentIdentity { pub instance: InstanceId, pub sequence: u64 }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommitmentRecord { pub identity: CommitmentIdentity, pub fingerprint: RequestFingerprint }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityError { ZeroInstance, ZeroSequence, Collision, FingerprintConflict, Capacity, InvalidMigration }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolveResult { New(CommitmentIdentity), Replay(CommitmentIdentity) }

pub fn derive_identity(instance: InstanceId, sequence: u64) -> Result<CommitmentIdentity, IdentityError> {
    if instance == [0; 32] { return Err(IdentityError::ZeroInstance); }
    if sequence == 0 { return Err(IdentityError::ZeroSequence); }
    Ok(CommitmentIdentity { instance, sequence })
}

pub fn fingerprint_request(owner: InstanceId, amount: u128, duration_days: u32, asset: InstanceId) -> RequestFingerprint {
    let mut result = [0u8; 32];
    let amount_bytes = amount.to_le_bytes();
    let duration_bytes = duration_days.to_le_bytes();
    for index in 0..32 { result[index] = owner[index] ^ asset[index] ^ amount_bytes[index % 16] ^ duration_bytes[index % 4]; }
    result
}

pub fn resolve_retry(existing: Option<CommitmentRecord>, identity: CommitmentIdentity, fingerprint: RequestFingerprint) -> Result<ResolveResult, IdentityError> {
    match existing {
        None => Ok(ResolveResult::New(identity)),
        Some(record) if record.identity == identity && record.fingerprint == fingerprint => Ok(ResolveResult::Replay(record.identity)),
        Some(record) if record.identity == identity => Err(IdentityError::FingerprintConflict),
        Some(_) => Err(IdentityError::Collision),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdentityRegistry { pub records: [Option<CommitmentRecord>; 16], pub length: u32 }

impl IdentityRegistry {
    pub const fn empty() -> Self { Self { records: [None; 16], length: 0 } }
    pub fn find(&self, identity: CommitmentIdentity) -> Option<CommitmentRecord> { let mut index = 0; while index < self.records.len() { if let Some(record) = self.records[index] { if record.identity == identity { return Some(record); } } index += 1; } None }
    pub fn insert(&mut self, record: CommitmentRecord) -> Result<ResolveResult, IdentityError> {
        match resolve_retry(self.find(record.identity), record.identity, record.fingerprint)? { ResolveResult::Replay(identity) => Ok(ResolveResult::Replay(identity)), ResolveResult::New(identity) => { let mut index = 0; while index < self.records.len() { if self.records[index].is_none() { self.records[index] = Some(record); self.length += 1; return Ok(ResolveResult::New(identity)); } index += 1; } Err(IdentityError::Capacity) } }
    }
    pub fn migrate(&mut self, old: CommitmentRecord, next: CommitmentIdentity) -> Result<(), IdentityError> { if old.identity.instance != next.instance || next.sequence <= old.identity.sequence { return Err(IdentityError::InvalidMigration); } if self.find(next).is_some() { return Err(IdentityError::Collision); } let mut index = 0; while index < self.records.len() { if self.records[index].map(|record| record.identity) == Some(old.identity) { self.records[index] = Some(CommitmentRecord { identity: next, fingerprint: old.fingerprint }); return Ok(()); } index += 1; } Err(IdentityError::InvalidMigration) }
    pub fn instance_count(&self, instance: InstanceId) -> u32 { let mut count = 0; let mut index = 0; while index < self.records.len() { if self.records[index].map(|record| record.identity.instance == instance).unwrap_or(false) { count += 1; } index += 1; } count }
    pub fn contains_sequence(&self, instance: InstanceId, sequence: u64) -> bool { self.find(CommitmentIdentity { instance, sequence }).is_some() }
}

pub fn is_well_formed_identity(identity: CommitmentIdentity) -> bool { identity.instance != [0; 32] && identity.sequence > 0 }

#[cfg(test)]
mod tests {
    use super::*;
    const A: InstanceId = [1; 32];
    const B: InstanceId = [2; 32];
    fn identity(instance: InstanceId, sequence: u64) -> CommitmentIdentity { derive_identity(instance, sequence).unwrap() }
    #[test] fn derives_identity() { assert_eq!(identity(A, 1).sequence, 1); }
    #[test] fn rejects_zero_instance() { assert_eq!(derive_identity([0; 32], 1), Err(IdentityError::ZeroInstance)); }
    #[test] fn rejects_zero_sequence() { assert_eq!(derive_identity(A, 0), Err(IdentityError::ZeroSequence)); }
    #[test] fn instances_isolate_equal_sequences() { assert_ne!(identity(A, 1), identity(B, 1)); }
    #[test] fn fingerprints_are_deterministic() { assert_eq!(fingerprint_request(A, 10, 30, B), fingerprint_request(A, 10, 30, B)); }
    #[test] fn changed_amount_changes_fingerprint() { assert_ne!(fingerprint_request(A, 10, 30, B), fingerprint_request(A, 11, 30, B)); }
    #[test] fn changed_duration_changes_fingerprint() { assert_ne!(fingerprint_request(A, 10, 30, B), fingerprint_request(A, 10, 31, B)); }
    #[test] fn changed_owner_changes_fingerprint() { assert_ne!(fingerprint_request(A, 10, 30, B), fingerprint_request(B, 10, 30, B)); }
    #[test] fn changed_asset_changes_fingerprint() { assert_ne!(fingerprint_request(A, 10, 30, B), fingerprint_request(A, 10, 30, A)); }
    #[test] fn new_retry_is_new() { assert_eq!(resolve_retry(None, identity(A, 1), [3; 32]), Ok(ResolveResult::New(identity(A, 1)))); }
    #[test] fn exact_retry_is_replay() { let id = identity(A, 1); assert_eq!(resolve_retry(Some(CommitmentRecord { identity: id, fingerprint: [3; 32] }), id, [3; 32]), Ok(ResolveResult::Replay(id))); }
    #[test] fn altered_retry_is_conflict() { let id = identity(A, 1); assert_eq!(resolve_retry(Some(CommitmentRecord { identity: id, fingerprint: [3; 32] }), id, [4; 32]), Err(IdentityError::FingerprintConflict)); }
    #[test] fn other_instance_is_collision() { let id = identity(A, 1); assert_eq!(resolve_retry(Some(CommitmentRecord { identity: id, fingerprint: [3; 32] }), identity(B, 1), [3; 32]), Err(IdentityError::Collision)); }
    #[test] fn registry_inserts_once() { let mut registry = IdentityRegistry::empty(); let id = identity(A, 1); assert_eq!(registry.insert(CommitmentRecord { identity: id, fingerprint: [3; 32] }), Ok(ResolveResult::New(id))); assert_eq!(registry.length, 1); }
    #[test] fn registry_replays_without_increment() { let mut registry = IdentityRegistry::empty(); let id = identity(A, 1); registry.insert(CommitmentRecord { identity: id, fingerprint: [3; 32] }).unwrap(); assert_eq!(registry.insert(CommitmentRecord { identity: id, fingerprint: [3; 32] }), Ok(ResolveResult::Replay(id))); assert_eq!(registry.length, 1); }
    #[test] fn registry_rejects_conflict() { let mut registry = IdentityRegistry::empty(); let id = identity(A, 1); registry.insert(CommitmentRecord { identity: id, fingerprint: [3; 32] }).unwrap(); assert_eq!(registry.insert(CommitmentRecord { identity: id, fingerprint: [4; 32] }), Err(IdentityError::FingerprintConflict)); }
    #[test] fn registry_counts_instance() { let mut registry = IdentityRegistry::empty(); registry.insert(CommitmentRecord { identity: identity(A, 1), fingerprint: [1; 32] }).unwrap(); registry.insert(CommitmentRecord { identity: identity(A, 2), fingerprint: [2; 32] }).unwrap(); registry.insert(CommitmentRecord { identity: identity(B, 1), fingerprint: [3; 32] }).unwrap(); assert_eq!(registry.instance_count(A), 2); }
    #[test] fn migration_advances_sequence() { let mut registry = IdentityRegistry::empty(); let old = CommitmentRecord { identity: identity(A, 1), fingerprint: [1; 32] }; registry.insert(old).unwrap(); assert!(registry.migrate(old, identity(A, 2)).is_ok()); assert!(registry.contains_sequence(A, 2)); }
    #[test] fn migration_rejects_backwards_sequence() { let mut registry = IdentityRegistry::empty(); let old = CommitmentRecord { identity: identity(A, 2), fingerprint: [1; 32] }; registry.insert(old).unwrap(); assert_eq!(registry.migrate(old, identity(A, 1)), Err(IdentityError::InvalidMigration)); }
    #[test] fn malformed_identity_is_false() { assert!(!is_well_formed_identity(CommitmentIdentity { instance: [0; 32], sequence: 1 })); }
    #[test] fn valid_identity_is_true() { assert!(is_well_formed_identity(identity(A, 1))); }
}
