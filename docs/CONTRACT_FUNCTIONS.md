# Contract Functions

This document summarizes public entry points for each contract and their access control expectations.

## commitment_core

| Function | Summary | Access control | Notes |
| --- | --- | --- | --- |
| initialize(admin, nft_contract) | Set admin, NFT contract, and counters. | None (single-use). | Panics if already initialized. |
| create_commitment(owner, amount, asset_address, rules) -> String | Creates commitment, transfers assets, mints NFT. | No require_auth; caller supplies owner. | Uses reentrancy guard and rate limiting per owner. |
| get_commitment(commitment_id) -> Commitment | Fetch commitment details. | View. | Panics if not found. |
| get_owner_commitments(owner) -> Vec<String> | List commitment IDs for owner. | View. | Returns empty Vec if none. |
| get_total_commitments() -> u64 | Total commitments count. | View. | Reads instance storage counter. |
| get_total_value_locked() -> i128 | Total value locked across commitments. | View. | Aggregate stored in instance storage. |
| get_admin() -> Address | Fetch admin address. | View. | Panics if not initialized. |
| get_nft_contract() -> Address | Fetch NFT contract address. | View. | Panics if not initialized. |
| update_value(commitment_id, new_value) | Emit value update event. | No require_auth. | Does not update stored commitment value. |
| check_violations(commitment_id) -> bool | Evaluate loss or duration violations. | View. | Emits violation event when violated. |
| get_violation_details(commitment_id) -> (bool, bool, bool, i128, u64) | Detailed violation info. | View. | Calculates loss percent and time remaining. |
| settle(commitment_id) | Settle expired commitment and NFT. | No require_auth. | Transfers assets and calls NFT settle. |
| early_exit(commitment_id, caller) | Exit early with penalty. | Checks caller == owner (no require_auth). | Uses SafeMath to compute penalty. |
| allocate(commitment_id, target_pool, amount) | Allocate assets to pool. | No require_auth. | Transfers assets to target pool. |
| set_rate_limit(caller, function, window, max_calls) | Configure rate limits. | Admin only. | Uses shared RateLimiter. |
| set_rate_limit_exempt(caller, address, exempt) | Configure rate limit exemption. | Admin only. | Uses shared RateLimiter. |

## commitment_nft

| Function | Summary | Access control | Notes |
| --- | --- | --- | --- |
| initialize(admin) -> Result | Set admin and token counters. | None (single-use). | Returns AlreadyInitialized on repeat. |
| set_core_contract(core_contract) -> Result | Set authorized core contract. | Admin require_auth. | Emits CoreContractSet event. |
| get_core_contract() -> Result<Address> | Fetch core contract address. | View. | Fails if not initialized. |
| get_admin() -> Result<Address> | Fetch admin address. | View. | Fails if not initialized. |
| mint(owner, commitment_id, duration_days, max_loss_percent, commitment_type, initial_amount, asset_address, early_exit_penalty) -> Result<u32> | Mint NFT for a commitment. | No require_auth. | Validates inputs and uses reentrancy guard. |
| get_metadata(token_id) -> Result<CommitmentNFT> | Fetch NFT metadata. | View. | Fails if token missing. |
| owner_of(token_id) -> Result<Address> | Fetch NFT owner. | View. | Fails if token missing. |
| transfer(from, to, token_id) -> Result | Transfer NFT ownership. | from.require_auth. | Updates owner balances and token lists. |
| is_active(token_id) -> Result<bool> | Check active status. | View. | Returns error if token missing. |
| total_supply() -> u32 | Total minted NFTs. | View. | Reads token counter. |
| balance_of(owner) -> u32 | NFT balance for owner. | View. | Returns 0 if no NFTs. |
| get_all_metadata() -> Vec<CommitmentNFT> | List all NFTs. | View. | Iterates token IDs. |
| get_nfts_by_owner(owner) -> Vec<CommitmentNFT> | List NFTs for owner. | View. | Returns empty Vec if none. |
| settle(token_id) -> Result | Mark NFT settled after expiry. | No require_auth. | Uses reentrancy guard. |
| is_expired(token_id) -> Result<bool> | Check expiry based on ledger time. | View. | Requires token exists. |
| token_exists(token_id) -> bool | Check if token exists. | View. | Uses persistent storage. |

## attestation_engine

| Function | Summary | Access control | Notes |
| --- | --- | --- | --- |
| initialize(admin, commitment_core) -> Result | Set admin and core contract. | None (single-use). | Returns AlreadyInitialized on repeat. |
| add_verifier(caller, verifier) -> Result | Authorize verifier address. | Admin require_auth. | Stores verifier flag. |
| remove_verifier(caller, verifier) -> Result | Remove verifier authorization. | Admin require_auth. | Removes verifier flag. |
| is_verifier(address) -> bool | Check verifier authorization. | View. | Admin is implicitly authorized. |
| get_admin() -> Result<Address> | Fetch admin address. | View. | Fails if not initialized. |
| get_core_contract() -> Result<Address> | Fetch core contract address. | View. | Fails if not initialized. |
| get_stored_health_metrics(commitment_id) -> Option<HealthMetrics> | Fetch cached health metrics. | View. | Returns None if missing. |
| attest(caller, commitment_id, attestation_type, data, is_compliant) -> Result | Record attestation. | Verifier require_auth. | Validates commitment, uses rate limiting and reentrancy guard. |
| get_attestations(commitment_id) -> Vec<Attestation> | List attestations for commitment. | View. | Returns empty Vec if none. |
| get_attestation_count(commitment_id) -> u64 | Count attestations. | View. | Stored in persistent storage. |
| get_health_metrics(commitment_id) -> HealthMetrics | Compute current health metrics. | View. | Reads commitment_core data. |
| verify_compliance(commitment_id) -> bool | Check compliance vs rules. | View. | Uses health metrics and rules. |
| record_fees(caller, commitment_id, fee_amount) -> Result | Convenience fee attestation. | Verifier require_auth. | Calls attest() internally. |
| record_drawdown(caller, commitment_id, drawdown_percent) -> Result | Convenience drawdown attestation. | Verifier require_auth. | Calls attest() internally. |
| calculate_compliance_score(commitment_id) -> u32 | Compute compliance score. | View. | Emits ScoreUpd event. |
| get_protocol_statistics() -> (u64, u64, u64, i128) | Aggregate protocol stats. | View. | Reads commitment_core counters. |
| get_verifier_statistics(verifier) -> u64 | Per-verifier attestation count. | View. | Stored in instance storage. |
| set_rate_limit(caller, function, window, max_calls) -> Result | Configure rate limits. | Admin require_auth. | Uses shared RateLimiter. |
| set_rate_limit_exempt(caller, verifier, exempt) -> Result | Configure rate limit exemption. | Admin require_auth. | Uses shared RateLimiter. |

## allocation_logic

| Function | Summary | Access control | Notes |
| --- | --- | --- | --- |
| initialize(admin, commitment_core) -> Result | Set admin, core contract, and registry. | Admin require_auth. | Returns AlreadyInitialized on repeat. |
| register_pool(admin, pool_id, risk_level, apy, max_capacity) -> Result | Register investment pool. | Admin require_auth. | Validates capacity and APY. |
| update_pool_status(admin, pool_id, active) -> Result | Activate/deactivate pool. | Admin require_auth. | Updates pool timestamps. |
| update_pool_capacity(admin, pool_id, new_capacity) -> Result | Update pool capacity. | Admin require_auth. | Ensures capacity >= liquidity. |
| allocate(caller, commitment_id, amount, strategy) -> Result<AllocationSummary> | Allocate funds across pools. | caller.require_auth. | Uses rate limiting and reentrancy guard. |
| rebalance(caller, commitment_id) -> Result<AllocationSummary> | Reallocate using stored strategy. | caller.require_auth. | Requires caller matches allocation owner. |
| get_allocation(commitment_id) -> AllocationSummary | Fetch allocation summary. | View. | Returns empty summary if missing. |
| get_pool(pool_id) -> Result<Pool> | Fetch pool info. | View. | Returns PoolNotFound if missing. |
| get_all_pools() -> Vec<Pool> | Fetch all pools. | View. | Iterates registry. |
| is_initialized() -> bool | Check initialization flag. | View. | Returns false if uninitialized. |
| set_rate_limit(admin, function, window, max_calls) -> Result | Configure rate limits. | Admin require_auth. | Uses shared RateLimiter. |
| set_rate_limit_exempt(admin, address, exempt) -> Result | Configure rate limit exemption. | Admin require_auth. | Uses shared RateLimiter. |

## shared_utils

| Module | Functions | Notes |
| --- | --- | --- |
| access_control | require_admin, require_owner, require_owner_or_admin | Uses Storage::get_admin and require_auth. |
| errors | log_error, panic_with_log, require | Centralized error logging helpers. |
| events | emit_created, emit_updated, emit_transfer, emit_violation | Standard event wrappers. |
| math | add, sub, mul, div, percent, loss_percent, gain_percent | Safe arithmetic with checked operations. |
| rate_limiting | set_limit, clear_limit, check, set_exempt | Fixed-window rate limiter. |
| storage | set_initialized, get_admin, get_or_default | Instance storage helpers. |
| time | now, calculate_expiration, is_expired | Ledger time utilities. |
| validation | require_positive, require_valid_percent, require_valid_commitment_type | Common validation guards. |
