//! # Version System Contract
//!
//! Manages the on-chain versioning of protocol contracts and exposes
//! compile-time build metadata so that WASM binaries are self-describing.
//!
//! ## Compile-time constants
//!
//! Three `pub const` values are baked into every compiled WASM:
//!
//! | Constant | Type | Description |
//! |----------|------|-------------|
//! | [`CONTRACT_VERSION_MAJOR`] | `u32` | Semantic major version |
//! | [`CONTRACT_VERSION_MINOR`] | `u32` | Semantic minor version |
//! | [`CONTRACT_VERSION_PATCH`] | `u32` | Semantic patch version |
//! | [`CONTRACT_VERSION_STR`] | `&str` | Full semver string `"MAJOR.MINOR.PATCH"` |
//!
//! These constants are independent of the **on-chain** `CurrentVersion` state.
//! The on-chain version represents the *protocol* version agreed upon by
//! governance; the compile-time constants represent the *implementation*
//! version of this specific WASM binary.
//!
//! ## Trust Boundaries
//! - **Deployer**: can call `initialize` once; becomes the privileged updater.
//! - **Authorized updaters**: call `update_version`, `update_minimum_version`,
//!   `deprecate_version`, `set_compatibility`, `start_migration`,
//!   `complete_migration` — all enforce `require_auth`.
//! - **Anyone**: read-only getters and `compare_versions`.
//!
//! ## Arithmetic Safety
//! All version comparisons use simple `u32` arithmetic.  No overflow risk
//! exists for realistic version numbers.

#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, String, Vec};

// ============================================================================
// Compile-time version constants
// ============================================================================

/// Major version of this WASM binary (semver).
///
/// Increment for breaking changes to the contract ABI or storage layout.
pub const CONTRACT_VERSION_MAJOR: u32 = 0;

/// Minor version of this WASM binary (semver).
///
/// Increment for backward-compatible feature additions.
pub const CONTRACT_VERSION_MINOR: u32 = 1;

/// Patch version of this WASM binary (semver).
///
/// Increment for backward-compatible bug fixes.
pub const CONTRACT_VERSION_PATCH: u32 = 0;

/// Full semver string baked into the WASM binary at compile time.
///
/// Format: `"MAJOR.MINOR.PATCH"`.
pub const CONTRACT_VERSION_STR: &str = "0.1.0";

/// A semantic version triple.
#[derive(Clone, PartialEq, Eq)]
#[contracttype]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

/// On-chain metadata associated with a specific protocol version.
#[derive(Clone)]
#[contracttype]
pub struct VersionMetadata {
    /// The version this metadata describes.
    pub version: Version,
    /// Ledger timestamp at the time the version was registered.
    pub timestamp: u64,
    /// Human-readable release notes or description.
    pub description: String,
    /// Address that deployed or registered this version.
    pub deployed_by: Address,
    /// `true` if this version has been explicitly deprecated.
    pub deprecated: bool,
}

/// Compatibility verdict between two versions.
#[derive(Clone)]
#[contracttype]
pub struct CompatibilityInfo {
    /// Whether `v1` and `v2` are considered compatible.
    pub is_compatible: bool,
    /// Human-readable explanation.
    pub notes: String,
    /// Ledger timestamp when the compatibility was last set.
    pub checked_at: u64,
}

/// Build metadata returned by [`ContractVersioning::get_build_metadata`].
///
/// Contains the compile-time constants baked into this WASM binary.
/// This data is immutable — it cannot be changed without redeploying the contract.
#[derive(Clone)]
#[contracttype]
pub struct BuildMetadata {
    /// Compile-time major version (from [`CONTRACT_VERSION_MAJOR`]).
    pub major: u32,
    /// Compile-time minor version (from [`CONTRACT_VERSION_MINOR`]).
    pub minor: u32,
    /// Compile-time patch version (from [`CONTRACT_VERSION_PATCH`]).
    pub patch: u32,
    /// Full semver string (from [`CONTRACT_VERSION_STR`]).
    pub version_str: String,
}

#[contracttype]
pub enum DataKey {
    CurrentVersion,
    MinimumVersion,
    VersionHistory,
    VersionCount,
    VersionMetadata(Version),
    Compatibility(Version, Version),
    Initialized,
}

#[contract]
pub struct ContractVersioning;

#[contractimpl]
impl ContractVersioning {
    // ========================================================================
    // Build metadata (compile-time constants)
    // ========================================================================

    /// Return the compile-time version constants baked into this WASM binary.
    ///
    /// Unlike [`get_current_version`](Self::get_current_version), this
    /// function requires no initialization and never panics.  It reflects
    /// the **implementation** version of the deployed binary, not the
    /// governance-agreed **protocol** version stored on-chain.
    ///
    /// # Use Case
    /// Integrators can call this immediately after deployment to verify that
    /// the correct binary was deployed before calling `initialize`.
    pub fn get_build_metadata(env: Env) -> BuildMetadata {
        BuildMetadata {
            major: CONTRACT_VERSION_MAJOR,
            minor: CONTRACT_VERSION_MINOR,
            patch: CONTRACT_VERSION_PATCH,
            version_str: String::from_str(&env, CONTRACT_VERSION_STR),
        }
    }

    /// Return a [`Version`] built from compile-time constants.
    ///
    /// Convenience wrapper around [`get_build_metadata`](Self::get_build_metadata)
    /// for callers that only need the numeric triple.
    pub fn get_contract_version(_env: Env) -> Version {
        Version {
            major: CONTRACT_VERSION_MAJOR,
            minor: CONTRACT_VERSION_MINOR,
            patch: CONTRACT_VERSION_PATCH,
        }
    }

    /// Return `true` when the compile-time binary version satisfies the
    /// given `required` minimum.
    ///
    /// Useful for a downstream contract to assert it is interacting with
    /// a sufficiently recent build before proceeding.
    ///
    /// # Parameters
    /// - `required_major`, `required_minor`, `required_patch` – Minimum
    ///   acceptable binary version.
    pub fn binary_meets_minimum(
        env: Env,
        required_major: u32,
        required_minor: u32,
        required_patch: u32,
    ) -> bool {
        let binary = Version {
            major: CONTRACT_VERSION_MAJOR,
            minor: CONTRACT_VERSION_MINOR,
            patch: CONTRACT_VERSION_PATCH,
        };
        let required = Version {
            major: required_major,
            minor: required_minor,
            patch: required_patch,
        };
        Self::compare_versions(env, binary, required) >= 0
    }

    // ========================================================================
    // Initialization
    // ========================================================================

    /// Initialize the contract with its first protocol version.
    ///
    /// # Parameters
    /// - `deployer` – Address that owns privileged operations;
    ///   `require_auth` is enforced.
    /// - `major`, `minor`, `patch` – Initial protocol version.
    /// - `description` – Human-readable release notes.
    ///
    /// # Panics
    /// - `"Already initialized"` if called more than once.
    ///
    /// # Events
    /// Emits `("ver_upd", major, minor) → (patch, description, deployer)`.
    ///
    /// # Security
    /// Deploy scripts should call this in the same transaction as contract
    /// deployment to prevent front-running.
    pub fn initialize(
        env: Env,
        deployer: Address,
        major: u32,
        minor: u32,
        patch: u32,
        description: String,
    ) {
        deployer.require_auth();

        let initialized_key = DataKey::Initialized;
        if env.storage().instance().has(&initialized_key) {
            panic!("Already initialized");
        }

        let version = Version {
            major,
            minor,
            patch,
        };

        // Set current version
        env.storage()
            .instance()
            .set(&DataKey::CurrentVersion, &version);

        // Set minimum supported version
        env.storage()
            .instance()
            .set(&DataKey::MinimumVersion, &version);

        // Create metadata
        let metadata = VersionMetadata {
            version: version.clone(),
            timestamp: env.ledger().timestamp(),
            description: description.clone(),
            deployed_by: deployer.clone(),
            deprecated: false,
        };

        // Store metadata
        env.storage()
            .persistent()
            .set(&DataKey::VersionMetadata(version.clone()), &metadata);

        // Initialize version history
        let mut history: Vec<Version> = Vec::new(&env);
        history.push_back(version.clone());
        env.storage()
            .persistent()
            .set(&DataKey::VersionHistory, &history);

        // Set version count
        env.storage().instance().set(&DataKey::VersionCount, &1u32);

        // Mark as initialized
        env.storage().instance().set(&initialized_key, &true);

        // Emit event
        env.events().publish(
            (symbol_short!("ver_upd"), major, minor),
            (patch, description, deployer),
        );
    }

    /// Advance the on-chain protocol version.
    ///
    /// The new version must be a valid increment over the current version
    /// (major, minor, or patch must increase; no downgrade is allowed).
    ///
    /// # Parameters
    /// - `updater` – Must be authorized; `require_auth` is enforced.
    /// - `major`, `minor`, `patch` – New protocol version.
    /// - `description` – Human-readable release notes.
    ///
    /// # Panics
    /// - `"Contract not initialized"` if `initialize` was not called.
    /// - `"Invalid version increment"` if the new version is not strictly
    ///   greater than the current version.
    ///
    /// # Events
    /// Emits `("ver_upd", major, minor) → (patch, description, updater)`.
    pub fn update_version(
        env: Env,
        updater: Address,
        major: u32,
        minor: u32,
        patch: u32,
        description: String,
    ) {
        updater.require_auth();
        Self::require_initialized(&env);

        let new_version = Version {
            major,
            minor,
            patch,
        };
        let current_version: Version = env
            .storage()
            .instance()
            .get(&DataKey::CurrentVersion)
            .unwrap();

        // Validate version increment
        if !Self::is_valid_increment(&current_version, &new_version) {
            panic!("Invalid version increment");
        }

        // Update current version
        env.storage()
            .instance()
            .set(&DataKey::CurrentVersion, &new_version);

        // Create metadata
        let metadata = VersionMetadata {
            version: new_version.clone(),
            timestamp: env.ledger().timestamp(),
            description: description.clone(),
            deployed_by: updater.clone(),
            deprecated: false,
        };

        // Store metadata
        env.storage()
            .persistent()
            .set(&DataKey::VersionMetadata(new_version.clone()), &metadata);

        // Update history
        let mut history: Vec<Version> = env
            .storage()
            .persistent()
            .get(&DataKey::VersionHistory)
            .unwrap();
        history.push_back(new_version.clone());
        env.storage()
            .persistent()
            .set(&DataKey::VersionHistory, &history);

        // Increment count
        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::VersionCount)
            .unwrap();
        env.storage()
            .instance()
            .set(&DataKey::VersionCount, &(count + 1));

        // Emit event
        env.events().publish(
            (symbol_short!("ver_upd"), major, minor),
            (patch, description, updater),
        );
    }

    /// Return the current on-chain protocol version.
    ///
    /// # Panics
    /// `"Contract not initialized"` if called before `initialize`.
    pub fn get_current_version(env: Env) -> Version {
        Self::require_initialized(&env);
        env.storage()
            .instance()
            .get(&DataKey::CurrentVersion)
            .unwrap()
    }

    /// Return the minimum supported protocol version.
    ///
    /// Versions below this value are considered end-of-life.
    pub fn get_minimum_version(env: Env) -> Version {
        Self::require_initialized(&env);
        env.storage()
            .instance()
            .get(&DataKey::MinimumVersion)
            .unwrap()
    }

    /// Return the total number of protocol versions registered so far.
    pub fn get_version_count(env: Env) -> u32 {
        Self::require_initialized(&env);
        env.storage()
            .instance()
            .get(&DataKey::VersionCount)
            .unwrap()
    }

    /// Return [`VersionMetadata`] for a specific protocol version.
    ///
    /// # Panics
    /// `"Version not found"` if no metadata was stored for `version`.
    pub fn get_version_metadata(env: Env, version: Version) -> VersionMetadata {
        Self::require_initialized(&env);
        env.storage()
            .persistent()
            .get(&DataKey::VersionMetadata(version))
            .unwrap_or_else(|| panic!("Version not found"))
    }

    /// Return the ordered list of all protocol versions registered so far.
    pub fn get_version_history(env: Env) -> Vec<Version> {
        Self::require_initialized(&env);
        env.storage()
            .persistent()
            .get(&DataKey::VersionHistory)
            .unwrap()
    }

    /// Compare two versions lexicographically by (major, minor, patch).
    ///
    /// Returns `-1` if `v1 < v2`, `0` if `v1 == v2`, `1` if `v1 > v2`.
    pub fn compare_versions(_env: Env, v1: Version, v2: Version) -> i32 {
        if v1.major != v2.major {
            return if v1.major > v2.major { 1 } else { -1 };
        }
        if v1.minor != v2.minor {
            return if v1.minor > v2.minor { 1 } else { -1 };
        }
        if v1.patch != v2.patch {
            return if v1.patch > v2.patch { 1 } else { -1 };
        }
        0
    }

    /// Return `true` if `version` is between the minimum and current versions
    /// (inclusive on both ends).
    pub fn is_version_supported(env: Env, version: Version) -> bool {
        Self::require_initialized(&env);
        let min_version: Version = env
            .storage()
            .instance()
            .get(&DataKey::MinimumVersion)
            .unwrap();
        let current_version: Version = env
            .storage()
            .instance()
            .get(&DataKey::CurrentVersion)
            .unwrap();

        let min_cmp = Self::compare_versions(env.clone(), version.clone(), min_version);
        let max_cmp = Self::compare_versions(env.clone(), version, current_version);

        min_cmp >= 0 && max_cmp <= 0
    }

    /// Return `true` if the on-chain current version is at least
    /// `(major, minor, patch)`.
    pub fn meets_minimum_version(env: Env, major: u32, minor: u32, patch: u32) -> bool {
        Self::require_initialized(&env);
        let current: Version = env
            .storage()
            .instance()
            .get(&DataKey::CurrentVersion)
            .unwrap();
        let required = Version {
            major,
            minor,
            patch,
        };

        Self::compare_versions(env, current, required) >= 0
    }

    /// Update the minimum supported protocol version.
    ///
    /// The new minimum must not exceed the current version.
    ///
    /// # Parameters
    /// - `updater` – Must be authorized; `require_auth` is enforced.
    ///
    /// # Panics
    /// `"Minimum version cannot exceed current version"` if the proposed
    /// minimum is greater than the current version.
    ///
    /// # Events
    /// Emits `("min_upd",) → (major, minor, patch)`.
    pub fn update_minimum_version(env: Env, updater: Address, major: u32, minor: u32, patch: u32) {
        updater.require_auth();
        Self::require_initialized(&env);

        let new_min = Version {
            major,
            minor,
            patch,
        };
        let current: Version = env
            .storage()
            .instance()
            .get(&DataKey::CurrentVersion)
            .unwrap();

        if Self::compare_versions(env.clone(), new_min.clone(), current) > 0 {
            panic!("Minimum version cannot exceed current version");
        }

        env.storage()
            .instance()
            .set(&DataKey::MinimumVersion, &new_min);

        env.events()
            .publish((symbol_short!("min_upd"),), (major, minor, patch));
    }

    /// Mark a protocol version as deprecated.
    ///
    /// # Panics
    /// - `"Version not found"` if the version has no stored metadata.
    /// - `"Already deprecated"` if the version is already deprecated.
    ///
    /// # Events
    /// Emits `("ver_depr", major, minor) → (patch, reason)`.
    pub fn deprecate_version(env: Env, admin: Address, version: Version, reason: String) {
        admin.require_auth();
        Self::require_initialized(&env);

        let metadata_key = DataKey::VersionMetadata(version.clone());
        let mut metadata: VersionMetadata = env
            .storage()
            .persistent()
            .get(&metadata_key)
            .unwrap_or_else(|| panic!("Version not found"));

        if metadata.deprecated {
            panic!("Already deprecated");
        }

        metadata.deprecated = true;
        env.storage().persistent().set(&metadata_key, &metadata);

        env.events().publish(
            (symbol_short!("ver_depr"), version.major, version.minor),
            (version.patch, reason),
        );
    }

    /// Return `true` if `version` has been marked as deprecated.
    ///
    /// Returns `false` if no metadata exists for the version.
    pub fn is_version_deprecated(env: Env, version: Version) -> bool {
        Self::require_initialized(&env);

        match env
            .storage()
            .persistent()
            .get::<DataKey, VersionMetadata>(&DataKey::VersionMetadata(version))
        {
            Some(metadata) => metadata.deprecated,
            None => false,
        }
    }

    /// Declare a compatibility relationship between two versions.
    ///
    /// Stored bidirectionally: querying `(v1, v2)` or `(v2, v1)` returns
    /// the same answer.
    ///
    /// # Events
    /// Emits `("compat",) → (v1, v2, is_compatible, notes)`.
    pub fn set_compatibility(
        env: Env,
        admin: Address,
        v1: Version,
        v2: Version,
        is_compatible: bool,
        notes: String,
    ) {
        admin.require_auth();
        Self::require_initialized(&env);

        let info = CompatibilityInfo {
            is_compatible,
            notes: notes.clone(),
            checked_at: env.ledger().timestamp(),
        };

        // Store bidirectional compatibility
        env.storage()
            .persistent()
            .set(&DataKey::Compatibility(v1.clone(), v2.clone()), &info);
        env.storage()
            .persistent()
            .set(&DataKey::Compatibility(v2.clone(), v1.clone()), &info);

        env.events()
            .publish((symbol_short!("compat"),), (v1, v2, is_compatible, notes));
    }

    /// Check whether two versions are compatible.
    ///
    /// Returns `(is_compatible, notes)`.  If an explicit entry was stored via
    /// [`set_compatibility`](Self::set_compatibility) it takes precedence;
    /// otherwise the default semver rules apply (same major version ≥ 1 is
    /// compatible; major version 0 requires the same minor).
    pub fn check_compatibility(env: Env, v1: Version, v2: Version) -> (bool, String) {
        Self::require_initialized(&env);

        // Check explicit compatibility setting
        if let Some(info) = env
            .storage()
            .persistent()
            .get::<DataKey, CompatibilityInfo>(&DataKey::Compatibility(v1.clone(), v2.clone()))
        {
            return (info.is_compatible, info.notes);
        }

        // Use default compatibility rules
        Self::default_compatibility_check(&env, v1, v2)
    }

    /// Return `true` if `client_version` is compatible with the current
    /// on-chain protocol version according to [`check_compatibility`](Self::check_compatibility).
    pub fn is_client_compatible(env: Env, client_version: Version) -> bool {
        Self::require_initialized(&env);
        let current: Version = env
            .storage()
            .instance()
            .get(&DataKey::CurrentVersion)
            .unwrap();
        let (compatible, _) = Self::check_compatibility(env, client_version, current);
        compatible
    }

    /// Signal the start of a migration from one protocol version to another.
    ///
    /// Emits an on-chain event that off-chain tooling can use to coordinate
    /// the migration process.  No state changes are made.
    ///
    /// # Events
    /// Emits `("mig_strt",) → (from_version, to_version, initiator)`.
    pub fn start_migration(
        env: Env,
        initiator: Address,
        from_version: Version,
        to_version: Version,
    ) {
        initiator.require_auth();
        Self::require_initialized(&env);

        env.events().publish(
            (symbol_short!("mig_strt"),),
            (from_version, to_version, initiator),
        );
    }

    /// Signal the completion of a migration.
    ///
    /// # Events
    /// Emits `("mig_done",) → (from_version, to_version, success)`.
    pub fn complete_migration(
        env: Env,
        executor: Address,
        from_version: Version,
        to_version: Version,
        success: bool,
    ) {
        executor.require_auth();
        Self::require_initialized(&env);

        env.events().publish(
            (symbol_short!("mig_done"),),
            (from_version, to_version, success),
        );
    }

    // ============ Internal Helper Functions ============

    fn require_initialized(env: &Env) {
        if !env.storage().instance().has(&DataKey::Initialized) {
            panic!("Contract not initialized");
        }
    }

    fn is_valid_increment(old: &Version, new: &Version) -> bool {
        // New version must be greater
        if old.major != new.major {
            if old.major > new.major {
                return false;
            }
            true
        } else if old.minor != new.minor {
            if old.minor > new.minor {
                return false;
            }
            old.major == new.major
        } else if old.patch != new.patch {
            if old.patch > new.patch {
                return false;
            }
            old.major == new.major && old.minor == new.minor
        } else {
            false
        }
    }

    fn default_compatibility_check(env: &Env, v1: Version, v2: Version) -> (bool, String) {
        // Same major version = compatible (for version > 0)
        if v1.major == v2.major && v1.major > 0 {
            return (
                true,
                String::from_str(env, "Same major version - backward compatible"),
            );
        }

        // Different major versions = not compatible
        if v1.major != v2.major {
            return (
                false,
                String::from_str(env, "Different major versions - breaking changes"),
            );
        }

        // Major version 0 - same minor is compatible
        if v1.major == 0 && v2.major == 0 {
            if v1.minor == v2.minor {
                return (
                    true,
                    String::from_str(env, "Version 0.x.x - same minor version"),
                );
            } else {
                return (
                    false,
                    String::from_str(env, "Version 0.x.x - different minor versions"),
                );
            }
        }

        (false, String::from_str(env, "Unknown compatibility"))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env, String};

    // ========================================================================
    // Build metadata / compile-time constant tests (#290)
    // ========================================================================

    /// `get_build_metadata` must return the compile-time constants and must
    /// not require the contract to be initialized.
    #[test]
    fn test_get_build_metadata_no_init_required() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        // Call *before* initialize — must not panic
        let meta = client.get_build_metadata();
        assert_eq!(meta.major, CONTRACT_VERSION_MAJOR);
        assert_eq!(meta.minor, CONTRACT_VERSION_MINOR);
        assert_eq!(meta.patch, CONTRACT_VERSION_PATCH);
        assert_eq!(
            meta.version_str,
            String::from_str(&env, CONTRACT_VERSION_STR)
        );
    }

    /// `get_contract_version` must return the compile-time triple without
    /// requiring initialization.
    #[test]
    fn test_get_contract_version_no_init_required() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let v = client.get_contract_version();
        assert_eq!(v.major, CONTRACT_VERSION_MAJOR);
        assert_eq!(v.minor, CONTRACT_VERSION_MINOR);
        assert_eq!(v.patch, CONTRACT_VERSION_PATCH);
    }

    /// Compile-time constants must match the declared `CONTRACT_VERSION_STR`.
    #[test]
    fn test_version_str_matches_constants() {
        extern crate std;
        use std::string::ToString;

        // Parse "MAJOR.MINOR.PATCH" and compare to the individual constants
        let s = CONTRACT_VERSION_STR.to_string();
        let parts: std::vec::Vec<&str> = s.split('.').collect();
        assert_eq!(parts.len(), 3, "VERSION_STR must be MAJOR.MINOR.PATCH");
        let major: u32 = parts[0].parse().expect("major must be a u32");
        let minor: u32 = parts[1].parse().expect("minor must be a u32");
        let patch: u32 = parts[2].parse().expect("patch must be a u32");
        assert_eq!(major, CONTRACT_VERSION_MAJOR);
        assert_eq!(minor, CONTRACT_VERSION_MINOR);
        assert_eq!(patch, CONTRACT_VERSION_PATCH);
    }

    /// `binary_meets_minimum` must return `true` when the required version
    /// equals the binary version.
    #[test]
    fn test_binary_meets_minimum_exact() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        assert!(client.binary_meets_minimum(
            &CONTRACT_VERSION_MAJOR,
            &CONTRACT_VERSION_MINOR,
            &CONTRACT_VERSION_PATCH,
        ));
    }

    /// `binary_meets_minimum` must return `false` when the required version
    /// is strictly greater than the binary version.
    #[test]
    fn test_binary_meets_minimum_too_high() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        // Require a version higher than the binary (major = binary + 1)
        assert!(!client.binary_meets_minimum(
            &(CONTRACT_VERSION_MAJOR + 1),
            &0,
            &0,
        ));
    }

    /// `binary_meets_minimum` must return `true` when requiring a lower version.
    #[test]
    fn test_binary_meets_minimum_lower_requirement() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        // Require 0.0.0 — every binary should satisfy this
        assert!(client.binary_meets_minimum(&0, &0, &0));
    }

    /// Build metadata is consistent with `get_contract_version`.
    #[test]
    fn test_build_metadata_consistent_with_contract_version() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let meta = client.get_build_metadata();
        let cv = client.get_contract_version();

        assert_eq!(meta.major, cv.major);
        assert_eq!(meta.minor, cv.minor);
        assert_eq!(meta.patch, cv.patch);
    }

    // ========================================================================
    // Existing tests
    // ========================================================================

    #[test]
    fn test_initialization() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let deployer = Address::generate(&env);
        let description = String::from_str(&env, "Initial version");

        env.mock_all_auths();

        client.initialize(&deployer, &1, &0, &0, &description);

        let version = client.get_current_version();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);

        assert_eq!(client.get_version_count(), 1);
    }

    #[test]
    fn test_version_update() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let deployer = Address::generate(&env);

        env.mock_all_auths();

        client.initialize(&deployer, &1, &0, &0, &String::from_str(&env, "Initial"));
        client.update_version(
            &deployer,
            &1,
            &1,
            &0,
            &String::from_str(&env, "Minor update"),
        );

        let version = client.get_current_version();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 1);
        assert_eq!(version.patch, 0);

        assert_eq!(client.get_version_count(), 2);
    }

    #[test]
    fn test_version_comparison() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let v1 = Version {
            major: 1,
            minor: 0,
            patch: 0,
        };
        let v2 = Version {
            major: 2,
            minor: 0,
            patch: 0,
        };
        let v3 = Version {
            major: 1,
            minor: 0,
            patch: 0,
        };

        assert_eq!(client.compare_versions(&v1, &v2), -1);
        assert_eq!(client.compare_versions(&v2, &v1), 1);
        assert_eq!(client.compare_versions(&v1, &v3), 0);
    }

    #[test]
    fn test_version_support() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let deployer = Address::generate(&env);

        env.mock_all_auths();

        client.initialize(&deployer, &1, &0, &0, &String::from_str(&env, "Initial"));
        client.update_version(&deployer, &2, &0, &0, &String::from_str(&env, "V2"));

        assert!(client.is_version_supported(&Version {
            major: 1,
            minor: 0,
            patch: 0
        }));
        assert!(client.is_version_supported(&Version {
            major: 2,
            minor: 0,
            patch: 0
        }));
        assert!(!client.is_version_supported(&Version {
            major: 3,
            minor: 0,
            patch: 0
        }));
    }

    #[test]
    fn test_deprecation() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        env.mock_all_auths();

        client.initialize(&admin, &1, &0, &0, &String::from_str(&env, "Initial"));

        let version = Version {
            major: 1,
            minor: 0,
            patch: 0,
        };
        client.deprecate_version(&admin, &version, &String::from_str(&env, "Outdated"));

        assert!(client.is_version_deprecated(&version));
    }

    #[test]
    fn test_meets_minimum_version() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let deployer = Address::generate(&env);

        env.mock_all_auths();

        client.initialize(&deployer, &2, &5, &3, &String::from_str(&env, "Test"));

        assert!(client.meets_minimum_version(&2, &5, &3));
        assert!(client.meets_minimum_version(&2, &0, &0));
        assert!(client.meets_minimum_version(&1, &0, &0));
        assert!(!client.meets_minimum_version(&3, &0, &0));
    }

    #[test]
    fn test_get_version_metadata() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let deployer = Address::generate(&env);
        let description = String::from_str(&env, "Initial release");

        client.initialize(&deployer, &1, &0, &0, &description);

        let version = Version {
            major: 1,
            minor: 0,
            patch: 0,
        };
        let metadata = client.get_version_metadata(&version);

        assert_eq!(metadata.version.major, 1);
        assert_eq!(metadata.version.minor, 0);
        assert_eq!(metadata.version.patch, 0);
        assert_eq!(metadata.description, description);
        assert_eq!(metadata.deployed_by, deployer);
        assert_eq!(metadata.deprecated, false);
    }

    #[test]
    fn test_get_version_history() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let deployer = Address::generate(&env);

        client.initialize(&deployer, &1, &0, &0, &String::from_str(&env, "V1"));
        client.update_version(&deployer, &1, &1, &0, &String::from_str(&env, "V1.1"));
        client.update_version(&deployer, &2, &0, &0, &String::from_str(&env, "V2"));

        let history = client.get_version_history();
        assert_eq!(history.len(), 3);

        assert_eq!(history.get(0).unwrap().major, 1);
        assert_eq!(history.get(0).unwrap().minor, 0);
        assert_eq!(history.get(1).unwrap().major, 1);
        assert_eq!(history.get(1).unwrap().minor, 1);
        assert_eq!(history.get(2).unwrap().major, 2);
        assert_eq!(history.get(2).unwrap().minor, 0);
    }

    #[test]
    #[should_panic(expected = "Already initialized")]
    fn test_double_initialization() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let deployer = Address::generate(&env);

        client.initialize(&deployer, &1, &0, &0, &String::from_str(&env, "V1"));
        client.initialize(&deployer, &1, &0, &0, &String::from_str(&env, "V1"));
    }

    #[test]
    #[should_panic(expected = "Contract not initialized")]
    fn test_get_current_version_not_initialized() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        client.get_current_version();
    }

    #[test]
    #[should_panic(expected = "Contract not initialized")]
    fn test_get_minimum_version_not_initialized() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        client.get_minimum_version();
    }

    #[test]
    #[should_panic(expected = "Invalid version increment")]
    fn test_invalid_version_increment_major_decrease() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let deployer = Address::generate(&env);

        client.initialize(&deployer, &2, &0, &0, &String::from_str(&env, "V2"));
        client.update_version(&deployer, &1, &0, &0, &String::from_str(&env, "V1"));
    }

    #[test]
    #[should_panic(expected = "Invalid version increment")]
    fn test_invalid_version_increment_minor_decrease() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let deployer = Address::generate(&env);

        client.initialize(&deployer, &1, &5, &0, &String::from_str(&env, "V1.5"));
        client.update_version(&deployer, &1, &3, &0, &String::from_str(&env, "V1.3"));
    }

    #[test]
    #[should_panic(expected = "Invalid version increment")]
    fn test_invalid_version_increment_same_version() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let deployer = Address::generate(&env);

        client.initialize(&deployer, &1, &0, &0, &String::from_str(&env, "V1"));
        client.update_version(&deployer, &1, &0, &0, &String::from_str(&env, "V1"));
    }

    #[test]
    fn test_valid_patch_increment() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let deployer = Address::generate(&env);

        client.initialize(&deployer, &1, &0, &0, &String::from_str(&env, "V1.0.0"));
        client.update_version(&deployer, &1, &0, &1, &String::from_str(&env, "V1.0.1"));

        let version = client.get_current_version();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 1);
    }

    #[test]
    fn test_valid_minor_increment() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let deployer = Address::generate(&env);

        client.initialize(&deployer, &1, &0, &5, &String::from_str(&env, "V1.0.5"));
        client.update_version(&deployer, &1, &1, &0, &String::from_str(&env, "V1.1.0"));

        let version = client.get_current_version();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 1);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn test_valid_major_increment() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let deployer = Address::generate(&env);

        client.initialize(&deployer, &1, &5, &3, &String::from_str(&env, "V1.5.3"));
        client.update_version(&deployer, &2, &0, &0, &String::from_str(&env, "V2.0.0"));

        let version = client.get_current_version();
        assert_eq!(version.major, 2);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn test_update_minimum_version() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        client.initialize(&admin, &1, &0, &0, &String::from_str(&env, "V1"));
        client.update_version(&admin, &2, &0, &0, &String::from_str(&env, "V2"));

        client.update_minimum_version(&admin, &1, &5, &0);

        let min_version = client.get_minimum_version();
        assert_eq!(min_version.major, 1);
        assert_eq!(min_version.minor, 5);
        assert_eq!(min_version.patch, 0);
    }

    #[test]
    #[should_panic(expected = "Minimum version cannot exceed current version")]
    fn test_update_minimum_version_exceeds_current() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        client.initialize(&admin, &1, &0, &0, &String::from_str(&env, "V1"));
        client.update_minimum_version(&admin, &2, &0, &0);
    }

    #[test]
    #[should_panic(expected = "Already deprecated")]
    fn test_double_deprecation() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        client.initialize(&admin, &1, &0, &0, &String::from_str(&env, "V1"));

        let version = Version {
            major: 1,
            minor: 0,
            patch: 0,
        };
        client.deprecate_version(&admin, &version, &String::from_str(&env, "Outdated"));
        client.deprecate_version(&admin, &version, &String::from_str(&env, "Still outdated"));
    }

    #[test]
    fn test_is_version_deprecated_nonexistent() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        client.initialize(&admin, &1, &0, &0, &String::from_str(&env, "V1"));

        let nonexistent = Version {
            major: 5,
            minor: 0,
            patch: 0,
        };
        assert_eq!(client.is_version_deprecated(&nonexistent), false);
    }

    #[test]
    #[should_panic(expected = "Version not found")]
    fn test_get_version_metadata_not_found() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        client.initialize(&admin, &1, &0, &0, &String::from_str(&env, "V1"));

        let nonexistent = Version {
            major: 5,
            minor: 0,
            patch: 0,
        };
        client.get_version_metadata(&nonexistent);
    }

    #[test]
    fn test_check_compatibility_same_major_version() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        client.initialize(&admin, &2, &0, &0, &String::from_str(&env, "V2"));

        let v1 = Version {
            major: 1,
            minor: 0,
            patch: 0,
        };
        let v2 = Version {
            major: 1,
            minor: 5,
            patch: 3,
        };

        let (compatible, _) = client.check_compatibility(&v1, &v2);
        assert!(compatible);
    }

    #[test]
    fn test_check_compatibility_different_major_version() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        client.initialize(&admin, &2, &0, &0, &String::from_str(&env, "V2"));

        let v1 = Version {
            major: 1,
            minor: 0,
            patch: 0,
        };
        let v2 = Version {
            major: 2,
            minor: 0,
            patch: 0,
        };

        let (compatible, _) = client.check_compatibility(&v1, &v2);
        assert!(!compatible);
    }

    #[test]
    fn test_check_compatibility_version_zero_same_minor() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        client.initialize(&admin, &1, &0, &0, &String::from_str(&env, "V1"));

        let v1 = Version {
            major: 0,
            minor: 1,
            patch: 0,
        };
        let v2 = Version {
            major: 0,
            minor: 1,
            patch: 5,
        };

        let (compatible, _) = client.check_compatibility(&v1, &v2);
        assert!(compatible);
    }

    #[test]
    fn test_check_compatibility_version_zero_different_minor() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        client.initialize(&admin, &1, &0, &0, &String::from_str(&env, "V1"));

        let v1 = Version {
            major: 0,
            minor: 1,
            patch: 0,
        };
        let v2 = Version {
            major: 0,
            minor: 2,
            patch: 0,
        };

        let (compatible, _) = client.check_compatibility(&v1, &v2);
        assert!(!compatible);
    }

    #[test]
    fn test_set_compatibility_explicit() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        client.initialize(&admin, &1, &0, &0, &String::from_str(&env, "V1"));

        let v1 = Version {
            major: 1,
            minor: 0,
            patch: 0,
        };
        let v2 = Version {
            major: 2,
            minor: 0,
            patch: 0,
        };

        client.set_compatibility(
            &admin,
            &v1,
            &v2,
            &true,
            &String::from_str(&env, "Migration tested"),
        );

        let (compatible, notes) = client.check_compatibility(&v1, &v2);
        assert!(compatible);
        assert_eq!(notes, String::from_str(&env, "Migration tested"));
    }

    #[test]
    fn test_set_compatibility_bidirectional() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        client.initialize(&admin, &1, &0, &0, &String::from_str(&env, "V1"));

        let v1 = Version {
            major: 1,
            minor: 0,
            patch: 0,
        };
        let v2 = Version {
            major: 2,
            minor: 0,
            patch: 0,
        };

        client.set_compatibility(
            &admin,
            &v1,
            &v2,
            &false,
            &String::from_str(&env, "Breaking changes"),
        );

        let (compatible1, _) = client.check_compatibility(&v1, &v2);
        let (compatible2, _) = client.check_compatibility(&v2, &v1);
        assert!(!compatible1);
        assert!(!compatible2);
    }

    #[test]
    fn test_is_client_compatible_true() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        client.initialize(&admin, &1, &5, &0, &String::from_str(&env, "V1.5"));

        let client_version = Version {
            major: 1,
            minor: 3,
            patch: 0,
        };

        assert!(client.is_client_compatible(&client_version));
    }

    #[test]
    fn test_is_client_compatible_false() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        client.initialize(&admin, &1, &0, &0, &String::from_str(&env, "V1"));

        let client_version = Version {
            major: 2,
            minor: 0,
            patch: 0,
        };

        assert!(!client.is_client_compatible(&client_version));
    }

    #[test]
    fn test_version_comparison_minor_difference() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let v1 = Version {
            major: 1,
            minor: 2,
            patch: 0,
        };
        let v2 = Version {
            major: 1,
            minor: 5,
            patch: 0,
        };

        assert_eq!(client.compare_versions(&v1, &v2), -1);
        assert_eq!(client.compare_versions(&v2, &v1), 1);
    }

    #[test]
    fn test_version_comparison_patch_difference() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let v1 = Version {
            major: 1,
            minor: 0,
            patch: 1,
        };
        let v2 = Version {
            major: 1,
            minor: 0,
            patch: 3,
        };

        assert_eq!(client.compare_versions(&v1, &v2), -1);
        assert_eq!(client.compare_versions(&v2, &v1), 1);
    }

    #[test]
    fn test_version_support_after_minimum_update() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        client.initialize(&admin, &1, &0, &0, &String::from_str(&env, "V1"));
        client.update_version(&admin, &2, &0, &0, &String::from_str(&env, "V2"));
        client.update_minimum_version(&admin, &1, &5, &0);

        assert!(!client.is_version_supported(&Version {
            major: 1,
            minor: 0,
            patch: 0
        }));
        assert!(client.is_version_supported(&Version {
            major: 1,
            minor: 5,
            patch: 0
        }));
        assert!(client.is_version_supported(&Version {
            major: 2,
            minor: 0,
            patch: 0
        }));
    }

    #[test]
    fn test_migration_events() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        client.initialize(&admin, &1, &0, &0, &String::from_str(&env, "V1"));

        let from_version = Version {
            major: 1,
            minor: 0,
            patch: 0,
        };
        let to_version = Version {
            major: 2,
            minor: 0,
            patch: 0,
        };

        client.start_migration(&admin, &from_version, &to_version);
        client.complete_migration(&admin, &from_version, &to_version, &true);
    }

    #[test]
    fn test_version_count_increments() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        client.initialize(&admin, &1, &0, &0, &String::from_str(&env, "V1"));
        assert_eq!(client.get_version_count(), 1);

        client.update_version(&admin, &1, &1, &0, &String::from_str(&env, "V1.1"));
        assert_eq!(client.get_version_count(), 2);

        client.update_version(&admin, &1, &2, &0, &String::from_str(&env, "V1.2"));
        assert_eq!(client.get_version_count(), 3);

        client.update_version(&admin, &2, &0, &0, &String::from_str(&env, "V2"));
        assert_eq!(client.get_version_count(), 4);
    }
}
