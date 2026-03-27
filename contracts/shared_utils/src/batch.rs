//! # Batch Processing Utilities
//!
//! Provides structures and helpers for executing multiple operations in a single
//! transaction. Supports two modes:
//! * **Atomic**: All operations must succeed; any failure rolls back the entire batch.
//! * **BestEffort**: All operations are attempted; individual failures are reported.
//!
//! Includes state snapshotting for manual rollback logic in non-atomic contexts.

use soroban_sdk::{contracttype, Env, String, Vec};

/// Defines how a batch of operations should be handled.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BatchMode {
    /// Strict mode: any failure causes the whole transaction to roll back.
    Atomic,
    /// Resilient mode: continues processing remaining items if one fails.
    BestEffort,
}

/// Basic error information for a failed operation within a batch.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchError {
    /// 0-based index of the operation in the original batch vector.
    pub index: u32,
    /// System or contract-specific error code.
    pub error_code: u32,
    /// Short text describing the failure or providing context (e.g., ID).
    pub context: String,
}

/// Container for batch results where each operation returns a identifier (String).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchResultString {
    /// `true` only if every operation in the batch succeeded.
    pub success: bool,
    /// Collection of return values (e.g., commitment IDs).
    pub results: Vec<String>,
    /// List of errors encountered during processing.
    pub errors: Vec<BatchError>,
}

/// Container for batch results for operations that do not return a value.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchResultVoid {
    /// `true` only if every operation in the batch succeeded.
    pub success: bool,
    /// Total count of operations that were successfully processed.
    pub success_count: u32,
    /// List of errors encountered during processing.
    pub errors: Vec<BatchError>,
}

impl BatchResultString {
    /// Generates a successful batch result.
    ///
    /// ### Parameters
    /// * `e` - The Soroban environment.
    /// * `results` - The list of successful return values.
    pub fn success(e: &Env, results: Vec<String>) -> Self {
        BatchResultString {
            success: true,
            results,
            errors: Vec::new(e),
        }
    }

    /// Generates a failed batch result with the provided errors.
    ///
    /// ### Parameters
    /// * `e` - The Soroban environment.
    /// * `errors` - The collection of failures to report.
    pub fn failure(e: &Env, errors: Vec<BatchError>) -> Self {
        BatchResultString {
            success: false,
            results: Vec::new(e),
            errors,
        }
    }

    /// Generates a partial result, typically for BestEffort mode.
    ///
    /// The `success` flag is automatically calculated based on the presence of errors.
    ///
    /// ### Parameters
    /// * `results` - Successful results.
    /// * `errors` - Encountered errors.
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
    pub fn failure(_e: &Env, errors: Vec<BatchError>) -> Self {
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

/// Comprehensive report for BestEffort batch processing.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchOperationReport {
    /// Total operations submitted.
    pub total: u32,
    /// Number of successful operations.
    pub succeeded: u32,
    /// Number of failed operations.
    pub failed: u32,
    /// Indices of operations that finished successfully.
    pub successful_indices: Vec<u32>,
    /// Detailed diagnostic information for failures.
    pub failed_operations: Vec<DetailedBatchError>,
}

/// Detailed diagnostic information for a failed operation in BestEffort mode.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetailedBatchError {
    /// 0-based index of the failed operation.
    pub index: u32,
    /// System or contract-specific error code (e.g., balance check failure).
    pub error_code: u32,
    /// Human-readable error message.
    pub message: String,
    /// Additional investigative context (e.g., the specific ID or amount that failed).
    pub context: String,
}

/// System-wide configuration for batch processing constraints.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchConfig {
    /// Maximum allowed operations in a single batch (prevents resource exhaustion).
    pub max_batch_size: u32,
    /// Kill-switch for batch operations.
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

/// Storage keys used for batch configuration and overrides.
#[contracttype]
pub enum BatchDataKey {
    /// Global system configuration (`BatchConfig`).
    Config,
    /// Contract-specific batch size overrides (keyed by contract name).
    ContractBatchLimit(String),
}

/// Captures a point-in-time state of commitment data for rollback purposes.
///
/// ### Security
/// * Snapshots are stored in memory/storage during execution.
/// * Ensure that all sensitive state changes are captured if Atomic mode is required.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateSnapshot {
    /// Recorded changes to commitments: `(id, previous_state_serialized)`.
    pub commitment_changes: Vec<(String, String)>,
    /// Recorded changes to global/local counters: `(name, previous_value)`.
    pub counter_changes: Vec<(String, i128)>,
    /// Recorded changes to ownership lists: `(owner, previous_list)`.
    pub owner_list_changes: Vec<(String, Vec<String>)>,
}

impl StateSnapshot {
    /// Initializes a new, empty state snapshot.
    pub fn new(e: &Env) -> Self {
        StateSnapshot {
            commitment_changes: Vec::new(e),
            counter_changes: Vec::new(e),
            owner_list_changes: Vec::new(e),
        }
    }

    /// Logs a commitment state prior to modification.
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

    /// Returns `true` if no state changes have been recorded in this snapshot.
    pub fn is_empty(&self) -> bool {
        self.commitment_changes.is_empty()
            && self.counter_changes.is_empty()
            && self.owner_list_changes.is_empty()
    }
}

/// Provides logic for identifying when a rollback is necessary.
pub struct RollbackHelper;

impl RollbackHelper {
    /// Checks whether the snapshot contains data requiring a state restoration.
    pub fn needs_rollback(snapshot: &StateSnapshot) -> bool {
        !snapshot.is_empty()
    }

    /// Creates a formalized `BatchError` used to trigger or report a rollback.
    ///
    /// ### Parameters
    /// * `e` - The Soroban environment.
    /// * `index` - The index of the operation causing the rollback.
    /// * `error_code` - The underlying error code.
    /// * `context` - Diagnostic context string.
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

/// Central logic for managing and enforcing batch processing limits and configuration.
pub struct BatchProcessor;

impl BatchProcessor {
    /// Validates that the batch size is within acceptable bounds.
    ///
    /// ### Parameters
    /// * `batch_size` - Number of operations in the current request.
    /// * `max_size` - Maximum allowed operations.
    ///
    /// ### Errors
    /// * Returns `Err(1)` if the batch is empty.
    /// * Returns `Err(2)` if the batch exceeds `max_size`.
    pub fn validate_batch_size(_e: &Env, batch_size: u32, max_size: u32) -> Result<(), u32> {
        if batch_size == 0 {
            return Err(1); // Error code: Empty batch
        }
        if batch_size > max_size {
            return Err(2); // Error code: Batch too large
        }
        Ok(())
    }

    /// Retrieves the current system-wide batch configuration.
    pub fn get_config(e: &Env) -> BatchConfig {
        e.storage()
            .instance()
            .get::<BatchDataKey, BatchConfig>(&BatchDataKey::Config)
            .unwrap_or_default()
    }

    /// Persists a new batch configuration.
    ///
    /// ### Security
    /// * This function itself does not check auth; the calling contract MUST ensure
    ///   only authorized admins can call this.
    pub fn set_config(e: &Env, config: BatchConfig) {
        e.storage().instance().set(&BatchDataKey::Config, &config);
    }

    /// Checks if batch operations are globally enabled.
    pub fn is_enabled(e: &Env) -> bool {
        Self::get_config(e).enabled
    }

    /// Returns the global maximum batch size.
    pub fn max_batch_size(e: &Env) -> u32 {
        Self::get_config(e).max_batch_size
    }

    /// Sets a size limit override for a specific contract.
    pub fn set_contract_limit(e: &Env, contract_name: String, limit: u32) {
        e.storage()
            .instance()
            .set(&BatchDataKey::ContractBatchLimit(contract_name), &limit);
    }

    /// Retrieves the batch limit for a specific contract, falling back to the global limit if unset.
    pub fn get_contract_limit(e: &Env, contract_name: String) -> u32 {
        e.storage()
            .instance()
            .get::<BatchDataKey, u32>(&BatchDataKey::ContractBatchLimit(contract_name))
            .unwrap_or_else(|| Self::max_batch_size(e))
    }

    /// Validates and enforces batch size limits, considering optional contract overrides.
    ///
    /// ### Parameters
    /// * `e` - The Soroban environment.
    /// * `batch_size` - The number of operations to process.
    /// * `contract_name` - Optional specific contract for limit override check.
    ///
    /// ### Errors
    /// * Returns `Err(3)` if batch operations are disabled.
    /// * Returns codes from `validate_batch_size` (1 or 2) if size limits are violated.
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

    /// Initializes batch configuration with default values if not already set.
    pub fn initialize_batch_config(e: &Env) {
        if !e.storage().instance().has(&BatchDataKey::Config) {
            let default_config = BatchConfig::default();
            Self::set_config(e, default_config);
        }
    }

    /// Sets the `enabled` flag to `false`, halting all batch processing.
    ///
    /// ### Security
    /// * Emergency circuit breaker to prevent exploitation of batch logic.
    pub fn disable_batch_operations(e: &Env) {
        let mut config = Self::get_config(e);
        config.enabled = false;
        Self::set_config(e, config);
    }

    /// Sets the `enabled` flag to `true`.
    pub fn enable_batch_operations(e: &Env) {
        let mut config = Self::get_config(e);
        config.enabled = true;
        Self::set_config(e, config);
    }

    /// Direct update of the maximum allowed operations per batch.
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
