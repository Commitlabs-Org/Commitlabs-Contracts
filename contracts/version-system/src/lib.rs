//! Version management contract for the CommitLabs protocol.
//!
//! Tracks semantic versions on-chain, enforces monotonic upgrades,
//! manages compatibility between versions, and provides integrators
//! with a stable query surface for version negotiation.
//!
//! # Trust boundaries
//! - `initialize`: callable once by the deployer (`require_auth`).
//! - `update_version`, `update_minimum_version`: authorized caller only (`require_auth`).
//! - `deprecate_version`, `set_compatibility`: admin-authorized (`require_auth`).
//! - `start_migration`, `complete_migration`: initiator/executor (`require_auth`); no state mutation.
//! - All read functions (`get_*`, `compare_*`, `is_*`, `meets_*`, `check_*`): permissionless.
//!
//! # Storage keys mutated by write functions
//! | Function | Keys written |
//! |---|---|
//! | `initialize` | `CurrentVersion`, `MinimumVersion`, `VersionMetadata(v)`, `VersionHistory`, `VersionCount`, `Initialized` |
//! | `update_version` | `CurrentVersion`, `VersionMetadata(v)`, `VersionHistory`, `VersionCount` |
//! | `update_minimum_version` | `MinimumVersion` |
//! | `deprecate_version` | `VersionMetadata(v)` |
//! | `set_compatibility` | `Compatibility(v1,v2)`, `Compatibility(v2,v1)` |
#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, String, Vec};

#[derive(Clone, PartialEq, Eq, Debug)]
#[contracttype]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

#[derive(Clone)]
#[contracttype]
pub struct VersionMetadata {
    pub version: Version,
    pub timestamp: u64,
    pub description: String,
    pub deployed_by: Address,
    pub deprecated: bool,
}

#[derive(Clone)]
#[contracttype]
pub struct CompatibilityInfo {
    pub is_compatible: bool,
    pub notes: String,
    pub checked_at: u64,
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
    /// Initialize the contract with the first semantic version.
    ///
    /// Sets both `current` and `minimum` to the same initial version.
    /// Can only be called once — subsequent calls panic.
    ///
    /// # Parameters
    /// - `deployer`: Address authorizing the deployment; stored in version metadata.
    /// - `major`, `minor`, `patch`: Initial semantic version components.
    /// - `description`: Human-readable release notes for this version.
    ///
    /// # Errors
    /// - Panics `"Already initialized"` if called more than once.
    ///
    /// # Security
    /// - `deployer.require_auth()` — only the deployer can initialize.
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

    /// Bump the contract to a new semantic version.
    ///
    /// The new version must be strictly greater than the current one —
    /// regressions and same-version updates are rejected.
    ///
    /// # Parameters
    /// - `updater`: Address authorizing the update.
    /// - `major`, `minor`, `patch`: Target version (must be > current).
    /// - `description`: Release notes for this version.
    ///
    /// # Errors
    /// - Panics `"Contract not initialized"` if `initialize` was not called.
    /// - Panics `"Invalid version increment"` if new ≤ current.
    ///
    /// # Security
    /// - `updater.require_auth()` — no open upgrade path.
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

    /// Returns the current deployed version.
    ///
    /// # Errors
    /// - Panics `"Contract not initialized"` if called before `initialize`.
    pub fn get_current_version(env: Env) -> Version {
        Self::require_initialized(&env);
        env.storage()
            .instance()
            .get(&DataKey::CurrentVersion)
            .unwrap()
    }

    /// Returns the minimum version still considered supported.
    ///
    /// Versions below this threshold should be treated as end-of-life by integrators.
    ///
    /// # Errors
    /// - Panics `"Contract not initialized"` if called before `initialize`.
    pub fn get_minimum_version(env: Env) -> Version {
        Self::require_initialized(&env);
        env.storage()
            .instance()
            .get(&DataKey::MinimumVersion)
            .unwrap()
    }

    /// Returns the total number of versions registered (including the initial one).
    ///
    /// # Errors
    /// - Panics `"Contract not initialized"` if called before `initialize`.
    pub fn get_version_count(env: Env) -> u32 {
        Self::require_initialized(&env);
        env.storage()
            .instance()
            .get(&DataKey::VersionCount)
            .unwrap()
    }

    /// Returns the metadata stored for a specific version.
    ///
    /// # Parameters
    /// - `version`: The exact version to look up.
    ///
    /// # Errors
    /// - Panics `"Contract not initialized"` if called before `initialize`.
    /// - Panics `"Version not found"` if the version was never registered.
    pub fn get_version_metadata(env: Env, version: Version) -> VersionMetadata {
        Self::require_initialized(&env);
        env.storage()
            .persistent()
            .get(&DataKey::VersionMetadata(version))
            .unwrap_or_else(|| panic!("Version not found"))
    }

    /// Returns the full ordered list of versions since initialization.
    ///
    /// # Errors
    /// - Panics `"Contract not initialized"` if called before `initialize`.
    pub fn get_version_history(env: Env) -> Vec<Version> {
        Self::require_initialized(&env);
        env.storage()
            .persistent()
            .get(&DataKey::VersionHistory)
            .unwrap()
    }

    /// Compares two versions using standard semver ordering.
    ///
    /// Returns `-1` if `v1 < v2`, `0` if equal, `1` if `v1 > v2`.
    /// Comparison is major → minor → patch.
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

    /// Returns `true` if `version` falls within `[minimum, current]` (inclusive).
    ///
    /// Note: deprecated versions are still considered supported — deprecation
    /// is an advisory signal, not an access gate.
    ///
    /// # Errors
    /// - Panics `"Contract not initialized"` if called before `initialize`.
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

    /// Returns `true` if the current deployed version is ≥ the required version.
    ///
    /// Useful for feature-gating: callers can assert a minimum capability level
    /// before invoking newer contract functions.
    ///
    /// # Errors
    /// - Panics `"Contract not initialized"` if called before `initialize`.
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

    /// Raises the minimum supported version floor.
    ///
    /// Versions below the new minimum will be considered end-of-life.
    /// The new minimum cannot exceed the current version.
    ///
    /// # Parameters
    /// - `updater`: Address authorizing the change.
    /// - `major`, `minor`, `patch`: New minimum (must be ≤ current).
    ///
    /// # Errors
    /// - Panics `"Contract not initialized"` if called before `initialize`.
    /// - Panics `"Minimum version cannot exceed current version"` if new_min > current.
    ///
    /// # Security
    /// - `updater.require_auth()` — prevents unauthorized EOL declarations.
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

    /// Marks a version as deprecated (end-of-life signal for integrators).
    ///
    /// Deprecated ≠ unsupported: `is_version_supported` is unaffected.
    /// Deprecation is a one-way flag — it cannot be reversed.
    ///
    /// # Parameters
    /// - `admin`: Address authorizing the deprecation.
    /// - `version`: The version to deprecate (must exist in metadata).
    /// - `reason`: Human-readable explanation for integrators.
    ///
    /// # Errors
    /// - Panics `"Contract not initialized"` if called before `initialize`.
    /// - Panics `"Version not found"` if the version was never registered.
    /// - Panics `"Already deprecated"` if called twice on the same version.
    ///
    /// # Security
    /// - `admin.require_auth()` — only authorized admin can deprecate.
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

    /// Returns `true` if the version has been deprecated.
    ///
    /// Returns `false` for versions that were never registered (not a panic).
    ///
    /// # Errors
    /// - Panics `"Contract not initialized"` if called before `initialize`.
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

    /// Records an explicit compatibility relationship between two versions.
    ///
    /// Stored bidirectionally: `set_compatibility(v1, v2, ...)` also answers
    /// `check_compatibility(v2, v1, ...)`. Overrides the default heuristic.
    ///
    /// # Parameters
    /// - `admin`: Address authorizing the record.
    /// - `v1`, `v2`: The two versions being related.
    /// - `is_compatible`: Whether they are compatible.
    /// - `notes`: Explanation for integrators (e.g. migration steps required).
    ///
    /// # Errors
    /// - Panics `"Contract not initialized"` if called before `initialize`.
    ///
    /// # Security
    /// - `admin.require_auth()` — prevents unauthorized compatibility overrides.
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

    /// Returns the compatibility status between two versions.
    ///
    /// Checks explicit records first; falls back to the default heuristic
    /// (same major ≥ 1 → compatible; different major → incompatible;
    /// major 0 → same minor required).
    ///
    /// Returns `(is_compatible, notes)`.
    ///
    /// # Errors
    /// - Panics `"Contract not initialized"` if called before `initialize`.
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

    /// Returns `true` if `client_version` is compatible with the current deployed version.
    ///
    /// Delegates to `check_compatibility`; uses the same explicit + default rules.
    ///
    /// # Errors
    /// - Panics `"Contract not initialized"` if called before `initialize`.
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

    /// Emits a migration-start event for off-chain tooling.
    ///
    /// No state is mutated — this is a coordination signal only.
    ///
    /// # Errors
    /// - Panics `"Contract not initialized"` if called before `initialize`.
    ///
    /// # Security
    /// - `initiator.require_auth()` — prevents spurious migration signals.
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

    /// Emits a migration-complete event for off-chain tooling.
    ///
    /// No state is mutated. `success = false` signals a failed migration
    /// so monitors can alert without requiring a separate error path.
    ///
    /// # Errors
    /// - Panics `"Contract not initialized"` if called before `initialize`.
    ///
    /// # Security
    /// - `executor.require_auth()` — only the migration executor can close the signal.
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
        let cmp = if old.major != new.major {
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
        };

        cmp
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

        (
            false,
            String::from_str(env, "Unknown compatibility"),
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env, String};

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

    // ── helpers ──────────────────────────────────────────────────────────────

    fn v(major: u32, minor: u32, patch: u32) -> Version {
        Version { major, minor, patch }
    }

    /// Deploy + initialize at 1.0.0, return (client, deployer).
    fn setup(e: &Env) -> (ContractVersioningClient, Address) {
        let id = e.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(e, &id);
        let deployer = Address::generate(e);
        e.mock_all_auths();
        client.initialize(&deployer, &1, &0, &0, &String::from_str(e, "v1"));
        (client, deployer)
    }

    // ── (a) error paths ───────────────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "Already initialized")]
    fn test_initialize_twice_panics() {
        let e = Env::default();
        let (client, deployer) = setup(&e);
        client.initialize(&deployer, &2, &0, &0, &String::from_str(&e, "dup"));
    }

    #[test]
    #[should_panic(expected = "Invalid version increment")]
    fn test_update_version_regression_panics() {
        let e = Env::default();
        let (client, deployer) = setup(&e);
        // 1.0.0 → 0.9.0 is a regression
        client.update_version(&deployer, &0, &9, &0, &String::from_str(&e, "bad"));
    }

    #[test]
    #[should_panic(expected = "Invalid version increment")]
    fn test_update_version_same_panics() {
        let e = Env::default();
        let (client, deployer) = setup(&e);
        // same version is not a valid increment
        client.update_version(&deployer, &1, &0, &0, &String::from_str(&e, "same"));
    }

    #[test]
    #[should_panic(expected = "Contract not initialized")]
    fn test_update_version_not_initialized_panics() {
        let e = Env::default();
        let id = e.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&e, &id);
        let caller = Address::generate(&e);
        e.mock_all_auths();
        client.update_version(&caller, &1, &0, &0, &String::from_str(&e, "x"));
    }

    #[test]
    #[should_panic(expected = "Contract not initialized")]
    fn test_get_current_version_not_initialized_panics() {
        let e = Env::default();
        let id = e.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&e, &id);
        client.get_current_version();
    }

    #[test]
    #[should_panic(expected = "Version not found")]
    fn test_get_version_metadata_not_found_panics() {
        let e = Env::default();
        let (client, _) = setup(&e);
        client.get_version_metadata(&v(9, 9, 9));
    }

    #[test]
    #[should_panic(expected = "Version not found")]
    fn test_deprecate_version_not_found_panics() {
        let e = Env::default();
        let (client, admin) = setup(&e);
        client.deprecate_version(&admin, &v(9, 9, 9), &String::from_str(&e, "ghost"));
    }

    #[test]
    #[should_panic(expected = "Already deprecated")]
    fn test_deprecate_version_twice_panics() {
        let e = Env::default();
        let (client, admin) = setup(&e);
        let reason = String::from_str(&e, "old");
        client.deprecate_version(&admin, &v(1, 0, 0), &reason);
        client.deprecate_version(&admin, &v(1, 0, 0), &reason);
    }

    #[test]
    #[should_panic(expected = "Minimum version cannot exceed current version")]
    fn test_update_minimum_version_exceeds_current_panics() {
        let e = Env::default();
        let (client, deployer) = setup(&e);
        // current is 1.0.0 — setting min to 2.0.0 must panic
        client.update_minimum_version(&deployer, &2, &0, &0);
    }

    // ── (b) getters without coverage ─────────────────────────────────────────

    #[test]
    fn test_get_minimum_version() {
        let e = Env::default();
        let (client, _) = setup(&e);
        assert_eq!(client.get_minimum_version(), v(1, 0, 0));
    }

    #[test]
    fn test_get_version_metadata_success() {
        let e = Env::default();
        let (client, deployer) = setup(&e);
        let meta = client.get_version_metadata(&v(1, 0, 0));
        assert_eq!(meta.version, v(1, 0, 0));
        assert_eq!(meta.deployed_by, deployer);
        assert!(!meta.deprecated);
    }

    #[test]
    fn test_get_version_history() {
        let e = Env::default();
        let (client, deployer) = setup(&e);
        client.update_version(&deployer, &1, &1, &0, &String::from_str(&e, "v1.1"));
        client.update_version(&deployer, &1, &2, &0, &String::from_str(&e, "v1.2"));
        let history = client.get_version_history();
        assert_eq!(history.len(), 3);
        assert_eq!(history.get(0).unwrap(), v(1, 0, 0));
        assert_eq!(history.get(1).unwrap(), v(1, 1, 0));
        assert_eq!(history.get(2).unwrap(), v(1, 2, 0));
    }

    #[test]
    fn test_update_minimum_version_success() {
        let e = Env::default();
        let (client, deployer) = setup(&e);
        client.update_version(&deployer, &2, &0, &0, &String::from_str(&e, "v2"));
        // set min to 1.5.0 — below current 2.0.0
        client.update_minimum_version(&deployer, &1, &5, &0);
        assert_eq!(client.get_minimum_version(), v(1, 5, 0));
    }

    #[test]
    fn test_set_and_check_compatibility_explicit() {
        let e = Env::default();
        let (client, admin) = setup(&e);
        let notes = String::from_str(&e, "migration required");
        client.set_compatibility(&admin, &v(1, 0, 0), &v(2, 0, 0), &true, &notes);
        let (ok, _) = client.check_compatibility(&v(1, 0, 0), &v(2, 0, 0));
        assert!(ok);
        // bidirectional — set_compatibility stores both directions
        let (ok2, _) = client.check_compatibility(&v(2, 0, 0), &v(1, 0, 0));
        assert!(ok2);
    }

    #[test]
    fn test_check_compatibility_default_same_major() {
        let e = Env::default();
        let (client, _) = setup(&e);
        // same major ≥ 1 → compatible by default heuristic
        let (ok, _) = client.check_compatibility(&v(1, 0, 0), &v(1, 5, 0));
        assert!(ok);
    }

    #[test]
    fn test_check_compatibility_default_diff_major() {
        let e = Env::default();
        let (client, _) = setup(&e);
        let (ok, _) = client.check_compatibility(&v(1, 0, 0), &v(2, 0, 0));
        assert!(!ok);
    }

    #[test]
    fn test_check_compatibility_default_v0() {
        let e = Env::default();
        let id = e.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&e, &id);
        let deployer = Address::generate(&e);
        e.mock_all_auths();
        client.initialize(&deployer, &0, &1, &0, &String::from_str(&e, "pre-release"));

        // same minor under major 0 → compatible
        let (ok, _) = client.check_compatibility(&v(0, 1, 0), &v(0, 1, 5));
        assert!(ok);

        // different minor under major 0 → incompatible
        let (ok2, _) = client.check_compatibility(&v(0, 1, 0), &v(0, 2, 0));
        assert!(!ok2);
    }

    #[test]
    fn test_is_client_compatible() {
        let e = Env::default();
        let (client, _) = setup(&e);
        // client at same major as current (1.x.x) → compatible
        assert!(client.is_client_compatible(&v(1, 0, 0)));
        // client at different major → incompatible
        assert!(!client.is_client_compatible(&v(2, 0, 0)));
    }

    #[test]
    fn test_start_and_complete_migration() {
        let e = Env::default();
        let (client, deployer) = setup(&e);
        // both are event-only — just verify they don't panic
        client.start_migration(&deployer, &v(1, 0, 0), &v(2, 0, 0));
        client.complete_migration(&deployer, &v(1, 0, 0), &v(2, 0, 0), &true);
        client.complete_migration(&deployer, &v(1, 0, 0), &v(2, 0, 0), &false);
    }

    // ── (c) version semantics edge cases ─────────────────────────────────────

    #[test]
    fn test_compare_versions_minor_diff() {
        let e = Env::default();
        let id = e.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&e, &id);
        assert_eq!(client.compare_versions(&v(1, 1, 0), &v(1, 2, 0)), -1);
        assert_eq!(client.compare_versions(&v(1, 2, 0), &v(1, 1, 0)), 1);
    }

    #[test]
    fn test_compare_versions_patch_diff() {
        let e = Env::default();
        let id = e.register_contract(None, ContractVersioning);
        let client = ContractVersioningClient::new(&e, &id);
        assert_eq!(client.compare_versions(&v(1, 0, 1), &v(1, 0, 2)), -1);
        assert_eq!(client.compare_versions(&v(1, 0, 2), &v(1, 0, 1)), 1);
    }

    #[test]
    fn test_update_version_patch_increment() {
        let e = Env::default();
        let (client, deployer) = setup(&e);
        client.update_version(&deployer, &1, &0, &1, &String::from_str(&e, "patch"));
        assert_eq!(client.get_current_version(), v(1, 0, 1));
        assert_eq!(client.get_version_count(), 2);
    }

    #[test]
    fn test_update_version_major_increment() {
        let e = Env::default();
        let (client, deployer) = setup(&e);
        client.update_version(&deployer, &2, &0, &0, &String::from_str(&e, "major"));
        assert_eq!(client.get_current_version(), v(2, 0, 0));
    }

    #[test]
    fn test_is_version_supported_boundary() {
        let e = Env::default();
        let (client, deployer) = setup(&e);
        client.update_version(&deployer, &2, &0, &0, &String::from_str(&e, "v2"));
        // min=1.0.0, current=2.0.0
        assert!(!client.is_version_supported(&v(0, 9, 0))); // below min
        assert!(client.is_version_supported(&v(1, 0, 0)));  // at min
        assert!(client.is_version_supported(&v(2, 0, 0)));  // at current
        assert!(!client.is_version_supported(&v(2, 0, 1))); // above current
    }

    #[test]
    fn test_deprecation_does_not_affect_support() {
        let e = Env::default();
        let (client, admin) = setup(&e);
        client.deprecate_version(&admin, &v(1, 0, 0), &String::from_str(&e, "eol"));
        // deprecated but still within [min, current] range
        assert!(client.is_version_supported(&v(1, 0, 0)));
        assert!(client.is_version_deprecated(&v(1, 0, 0)));
    }

    #[test]
    fn test_metadata_deprecated_flag() {
        let e = Env::default();
        let (client, admin) = setup(&e);
        client.deprecate_version(&admin, &v(1, 0, 0), &String::from_str(&e, "old"));
        let meta = client.get_version_metadata(&v(1, 0, 0));
        assert!(meta.deprecated);
        assert_eq!(meta.version, v(1, 0, 0));
        assert_eq!(meta.deployed_by, admin);
    }
}
