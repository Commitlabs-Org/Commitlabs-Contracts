#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, String, Vec};

#[derive(Clone, PartialEq, Eq)]
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
    /// Initialize the contract with initial version
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

    /// Update to a new version
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

    /// Get current version
    pub fn get_current_version(env: Env) -> Version {
        Self::require_initialized(&env);
        env.storage()
            .instance()
            .get(&DataKey::CurrentVersion)
            .unwrap()
    }

    /// Get minimum supported version
    pub fn get_minimum_version(env: Env) -> Version {
        Self::require_initialized(&env);
        env.storage()
            .instance()
            .get(&DataKey::MinimumVersion)
            .unwrap()
    }

    /// Get version history count
    pub fn get_version_count(env: Env) -> u32 {
        Self::require_initialized(&env);
        env.storage()
            .instance()
            .get(&DataKey::VersionCount)
            .unwrap()
    }

    /// Get version metadata
    pub fn get_version_metadata(env: Env, version: Version) -> VersionMetadata {
        Self::require_initialized(&env);
        env.storage()
            .persistent()
            .get(&DataKey::VersionMetadata(version))
            .unwrap_or_else(|| panic!("Version not found"))
    }

    /// Get version history
    pub fn get_version_history(env: Env) -> Vec<Version> {
        Self::require_initialized(&env);
        env.storage()
            .persistent()
            .get(&DataKey::VersionHistory)
            .unwrap()
    }

    /// Compare two versions (-1: v1 < v2, 0: v1 == v2, 1: v1 > v2)
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

    /// Check if version is supported
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

    /// Check if current version meets minimum requirement
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

    /// Update minimum supported version
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

    /// Deprecate a version
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

    /// Check if version is deprecated
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

    /// Set compatibility between versions
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

    /// Check compatibility between versions
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
        Self::default_compatibility_check(v1, v2)
    }

    /// Check if client is compatible with current version
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

    /// Start migration
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

    /// Complete migration
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

    fn default_compatibility_check(v1: Version, v2: Version) -> (bool, String) {
        // Same major version = compatible (for version > 0)
        if v1.major == v2.major && v1.major > 0 {
            return (
                true,
                String::from_str(&Env::default(), "Same major version - backward compatible"),
            );
        }

        // Different major versions = not compatible
        if v1.major != v2.major {
            return (
                false,
                String::from_str(
                    &Env::default(),
                    "Different major versions - breaking changes",
                ),
            );
        }

        // Major version 0 - same minor is compatible
        if v1.major == 0 && v2.major == 0 {
            if v1.minor == v2.minor {
                return (
                    true,
                    String::from_str(&Env::default(), "Version 0.x.x - same minor version"),
                );
            } else {
                return (
                    false,
                    String::from_str(&Env::default(), "Version 0.x.x - different minor versions"),
                );
            }
        }

        (
            false,
            String::from_str(&Env::default(), "Unknown compatibility"),
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
}
