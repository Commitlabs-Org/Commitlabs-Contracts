#![allow(unused)]
use soroban_sdk::{contracttype, Address, Env, IntoVal, Map, String, TryFromVal, Val, Vec};

/// Batch processing mode for handling multiple operations
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BatchMode {
    /// All operations must succeed or entire batch fails (rollback)
    Atomic,
    /// Process all operations, continue on failures, return detailed report
    BestEffort,
}

/// Error details for a specific operation in a batch
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchError {
    /// Index of the failed operation in the batch
    pub index: u32,
    /// Error code from the operation
    pub error_code: u32,
    /// Contextual information about the error
    pub context: String,
}

/// Result of a batch operation returning Strings (e.g., commitment IDs)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchResultString {
    /// Overall success status (true if all succeeded, false if any failed)
    pub success: bool,
    /// Results from each operation (commitment IDs, etc.)
    pub results: Vec<String>,
    /// List of errors encountered (empty if all succeeded)
    pub errors: Vec<BatchError>,
}

/// Result of a batch operation with no return values (just success/failure)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchResultVoid {
    /// Overall success status (true if all succeeded, false if any failed)
    pub success: bool,
    /// Number of successful operations
    pub success_count: u32,
    /// List of errors encountered (empty if all succeeded)
    pub errors: Vec<BatchError>,
}

impl BatchResultString {
    /// Create a new successful batch result
    pub fn success(e: &Env, results: Vec<String>) -> Self {
        BatchResultString {
            success: true,
            results,
            errors: Vec::new(e),
        }
    }

    /// Create a new failed batch result
    pub fn failure(e: &Env, errors: Vec<BatchError>) -> Self {
        BatchResultString {
            success: false,
            results: Vec::new(e),
            errors,
        }
    }

    /// Create a partial result (BestEffort mode)
    pub fn partial(results: Vec<String>, errors: Vec<BatchError>) -> Self {
        let success = errors.is_empty();
        BatchResultString {
            success,
            results,
            errors,
        }
    }
}

impl BatchResultVoid {
    /// Create a new successful batch result
    pub fn success(e: &Env, count: u32) -> Self {
        BatchResultVoid {
            success: true,
            success_count: count,
            errors: Vec::new(e),
        }
    }

    /// Create a new failed batch result
    pub fn failure(e: &Env, errors: Vec<BatchError>) -> Self {
        BatchResultVoid {
            success: false,
            success_count: 0,
            errors,
        }
    }

    /// Create a partial result (BestEffort mode)
    pub fn partial(count: u32, errors: Vec<BatchError>) -> Self {
        let success = errors.is_empty();
        BatchResultVoid {
            success,
            success_count: count,
            errors,
        }
    }
}

/// Detailed operation report for BestEffort mode
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchOperationReport {
    /// Total number of operations attempted
    pub total: u32,
    /// Number of successful operations
    pub succeeded: u32,
    /// Number of failed operations
    pub failed: u32,
    /// List of successful operation indices or IDs
    pub successful_indices: Vec<u32>,
    /// Detailed error information for failed operations
    pub failed_operations: Vec<DetailedBatchError>,
}

/// Detailed error information for BestEffort mode
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetailedBatchError {
    /// Index of the failed operation
    pub index: u32,
    /// Error code
    pub error_code: u32,
    /// Error message
    pub message: String,
    /// Additional context (e.g., commitment_id, amount)
    pub context: String,
}

/// Configuration for batch size limits
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchConfig {
    /// Maximum number of operations allowed in a single batch
    pub max_batch_size: u32,
    /// Whether batch operations are enabled
    pub enabled: bool,
}

impl Default for BatchConfig {
    fn default() -> Self {
        BatchConfig {
            max_batch_size: 50,
            enabled: true,
        }
    }
}

/// Storage key for batch configuration
#[contracttype]
pub enum BatchDataKey {
    /// Batch configuration (global)
    Config,
    /// Per-contract batch size limit override
    ContractBatchLimit(String),
}

/// State snapshot for atomic batch operations
/// Tracks changes that can be rolled back if batch fails
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateSnapshot {
    /// Commitment state changes: (commitment_id, old_state_as_string)
    /// Using String to serialize commitment state for flexibility
    pub commitment_changes: Vec<(String, String)>,
    /// Counter changes: (counter_name, old_value)
    pub counter_changes: Vec<(String, i128)>,
    /// Owner commitment list changes: (owner, old_commitment_ids)
    pub owner_list_changes: Vec<(String, Vec<String>)>,
}

impl StateSnapshot {
    /// Create a new empty snapshot
    pub fn new(e: &Env) -> Self {
        StateSnapshot {
            commitment_changes: Vec::new(e),
            counter_changes: Vec::new(e),
            owner_list_changes: Vec::new(e),
        }
    }

    /// Record a commitment state change
    pub fn record_commitment_change(&mut self, commitment_id: String, old_state: String) {
        self.commitment_changes
            .push_back((commitment_id, old_state));
    }

    /// Record a counter change
    pub fn record_counter_change(&mut self, counter_name: String, old_value: i128) {
        self.counter_changes.push_back((counter_name, old_value));
    }

    /// Record an owner list change
    pub fn record_owner_list_change(&mut self, owner_key: String, old_list: Vec<String>) {
        self.owner_list_changes.push_back((owner_key, old_list));
    }

    /// Check if snapshot is empty (no changes recorded)
    pub fn is_empty(&self) -> bool {
        self.commitment_changes.is_empty()
            && self.counter_changes.is_empty()
            && self.owner_list_changes.is_empty()
    }
}

/// Rollback helper for atomic batch operations
pub struct RollbackHelper;

impl RollbackHelper {
    /// Restore state from snapshot
    /// This is a marker - actual restoration must be done by the contract
    /// using the snapshot data with contract-specific storage keys
    pub fn needs_rollback(snapshot: &StateSnapshot) -> bool {
        !snapshot.is_empty()
    }

    /// Create an error indicating rollback is needed
    pub fn create_rollback_error(
        e: &Env,
        index: u32,
        error_code: u32,
        context: &str,
    ) -> BatchError {
        BatchError {
            index,
            error_code,
            context: String::from_str(e, context),
        }
    }
}

/// Batch processing helpers
pub struct BatchProcessor;

impl BatchProcessor {
    /// Validate batch size against limits
    pub fn validate_batch_size(e: &Env, batch_size: u32, max_size: u32) -> Result<(), u32> {
        if batch_size == 0 {
            return Err(1); // Error code: Empty batch
        }
        if batch_size > max_size {
            return Err(2); // Error code: Batch too large
        }
        Ok(())
    }

    /// Get batch configuration
    pub fn get_config(e: &Env) -> BatchConfig {
        e.storage()
            .instance()
            .get::<BatchDataKey, BatchConfig>(&BatchDataKey::Config)
            .unwrap_or_default()
    }

    /// Set batch configuration (admin only)
    pub fn set_config(e: &Env, config: BatchConfig) {
        e.storage().instance().set(&BatchDataKey::Config, &config);
    }

    /// Check if batch operations are enabled
    pub fn is_enabled(e: &Env) -> bool {
        Self::get_config(e).enabled
    }

    /// Get maximum batch size
    pub fn max_batch_size(e: &Env) -> u32 {
        Self::get_config(e).max_batch_size
    }

    /// Set contract-specific batch limit
    pub fn set_contract_limit(e: &Env, contract_name: String, limit: u32) {
        e.storage()
            .instance()
            .set(&BatchDataKey::ContractBatchLimit(contract_name), &limit);
    }

    /// Get contract-specific batch limit (falls back to global limit)
    pub fn get_contract_limit(e: &Env, contract_name: String) -> u32 {
        e.storage()
            .instance()
            .get::<BatchDataKey, u32>(&BatchDataKey::ContractBatchLimit(contract_name))
            .unwrap_or_else(|| Self::max_batch_size(e))
    }

    /// Validate and enforce batch size limits
    /// Returns Ok(()) if valid, Err(error_code) if invalid
    pub fn enforce_batch_limits(
        e: &Env,
        batch_size: u32,
        contract_name: Option<String>,
    ) -> Result<(), u32> {
        // Check if batch operations are enabled
        if !Self::is_enabled(e) {
            return Err(3); // Error code: Batch operations disabled
        }

        // Get the appropriate limit
        let max_size = if let Some(name) = contract_name {
            Self::get_contract_limit(e, name)
        } else {
            Self::max_batch_size(e)
        };

        // Validate batch size
        Self::validate_batch_size(e, batch_size, max_size)
    }

    /// Initialize batch configuration with default values
    pub fn initialize_batch_config(e: &Env) {
        if !e.storage().instance().has(&BatchDataKey::Config) {
            let default_config = BatchConfig::default();
            Self::set_config(e, default_config);
        }
    }

    /// Disable all batch operations (emergency circuit breaker)
    pub fn disable_batch_operations(e: &Env) {
        let mut config = Self::get_config(e);
        config.enabled = false;
        Self::set_config(e, config);
    }

    /// Enable batch operations
    pub fn enable_batch_operations(e: &Env) {
        let mut config = Self::get_config(e);
        config.enabled = true;
        Self::set_config(e, config);
    }

    /// Update maximum batch size
    pub fn update_max_batch_size(e: &Env, new_max: u32) {
        let mut config = Self::get_config(e);
        config.max_batch_size = new_max;
        Self::set_config(e, config);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl, Env, String, Vec};

    // Test contract for batch operations
    #[contract]
    pub struct TestBatchContract;

    #[contractimpl]
    impl TestBatchContract {
        pub fn test_get_config(e: Env) -> BatchConfig {
            BatchProcessor::get_config(&e)
        }

        pub fn test_set_config(e: Env, config: BatchConfig) {
            BatchProcessor::set_config(&e, config);
        }

        pub fn test_get_contract_limit(e: Env, name: String) -> u32 {
            BatchProcessor::get_contract_limit(&e, name)
        }

        pub fn test_set_contract_limit(e: Env, name: String, limit: u32) {
            BatchProcessor::set_contract_limit(&e, name, limit);
        }
    }

    #[test]
    fn test_batch_result_string_success() {
        let e = Env::default();
        let mut results = Vec::new(&e);
        results.push_back(String::from_str(&e, "result1"));
        results.push_back(String::from_str(&e, "result2"));

        let batch_result = BatchResultString::success(&e, results.clone());
        assert!(batch_result.success);
        assert_eq!(batch_result.results.len(), 2);
        assert_eq!(batch_result.errors.len(), 0);
    }

    #[test]
    fn test_batch_result_string_failure() {
        let e = Env::default();
        let mut errors = Vec::new(&e);
        errors.push_back(BatchError {
            index: 0,
            error_code: 1,
            context: String::from_str(&e, "test error"),
        });

        let batch_result = BatchResultString::failure(&e, errors.clone());
        assert!(!batch_result.success);
        assert_eq!(batch_result.results.len(), 0);
        assert_eq!(batch_result.errors.len(), 1);
    }

    #[test]
    fn test_batch_result_string_partial() {
        let e = Env::default();
        let mut results = Vec::new(&e);
        results.push_back(String::from_str(&e, "result1"));

        let mut errors = Vec::new(&e);
        errors.push_back(BatchError {
            index: 1,
            error_code: 1,
            context: String::from_str(&e, "test error"),
        });

        let batch_result = BatchResultString::partial(results, errors);
        assert!(!batch_result.success);
        assert_eq!(batch_result.results.len(), 1);
        assert_eq!(batch_result.errors.len(), 1);
    }

    #[test]
    fn test_batch_result_void_success() {
        let e = Env::default();
        let batch_result = BatchResultVoid::success(&e, 5);
        assert!(batch_result.success);
        assert_eq!(batch_result.success_count, 5);
        assert_eq!(batch_result.errors.len(), 0);
    }

    #[test]
    fn test_batch_result_void_partial() {
        let e = Env::default();
        let mut errors = Vec::new(&e);
        errors.push_back(BatchError {
            index: 2,
            error_code: 1,
            context: String::from_str(&e, "test error"),
        });

        let batch_result = BatchResultVoid::partial(3, errors);
        assert!(!batch_result.success);
        assert_eq!(batch_result.success_count, 3);
        assert_eq!(batch_result.errors.len(), 1);
    }

    #[test]
    fn test_validate_batch_size() {
        let e = Env::default();

        // Valid batch size
        assert!(BatchProcessor::validate_batch_size(&e, 10, 50).is_ok());

        // Empty batch
        assert_eq!(BatchProcessor::validate_batch_size(&e, 0, 50), Err(1));

        // Batch too large
        assert_eq!(BatchProcessor::validate_batch_size(&e, 51, 50), Err(2));
    }

    #[test]
    fn test_batch_config() {
        let e = Env::default();
        let contract_id = e.register_contract(None, TestBatchContract);
        let client = TestBatchContractClient::new(&e, &contract_id);

        let config = client.test_get_config();
        assert_eq!(config.max_batch_size, 50);
        assert!(config.enabled);

        let new_config = BatchConfig {
            max_batch_size: 100,
            enabled: true,
        };
        client.test_set_config(&new_config);

        let retrieved_config = client.test_get_config();
        assert_eq!(retrieved_config.max_batch_size, 100);
        assert!(retrieved_config.enabled);
    }

    #[test]
    fn test_contract_specific_limit() {
        let e = Env::default();
        let contract_id = e.register_contract(None, TestBatchContract);
        let client = TestBatchContractClient::new(&e, &contract_id);

        let contract_name = String::from_str(&e, "commitment_core");

        // Should use global limit initially
        assert_eq!(client.test_get_contract_limit(&contract_name), 50);

        // Set contract-specific limit
        client.test_set_contract_limit(&contract_name, &25);
        assert_eq!(client.test_get_contract_limit(&contract_name), 25);
    }
}
