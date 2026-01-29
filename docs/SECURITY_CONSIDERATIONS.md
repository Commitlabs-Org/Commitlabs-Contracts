# Security Considerations

## Access control review
- Admin-only functions in allocation_logic and attestation_engine require `require_auth` and compare caller to stored admin.
- commitment_nft `set_core_contract` enforces admin auth, but `initialize`, `mint`, and `settle` do not require auth.
- commitment_core state-changing functions (`create_commitment`, `settle`, `early_exit`, `allocate`, `update_value`) do not call `require_auth` and accept caller-provided addresses.
- Attestation recording requires caller authorization (`is_authorized_verifier`) and `require_auth`.

## Reentrancy protection
- commitment_core, commitment_nft, allocation_logic, and attestation_engine use reentrancy guards stored in instance storage.
- commitment_core functions with external calls follow checks-effects-interactions and clear the guard before returning.
- Reentrancy guard state is reverted on transaction failure; audit should confirm all error paths are safe.

## Integer overflow / underflow
- Release profile enables overflow checks (`overflow-checks = true`).
- shared_utils `SafeMath` uses checked arithmetic and is used in commitment_core for loss/penalty calculations.
- allocation_logic uses checked math for pool capacity enforcement.
- Audit should verify remaining arithmetic uses checked operations where appropriate.

## Input validation
- commitment_core validates commitment rules using shared Validation utilities.
- commitment_nft validates duration, max loss, commitment type, and amount.
- allocation_logic validates APY bounds, capacity, and allocation amounts.
- attestation_engine validates attestation type and required data fields.

## Error handling
- commitment_nft, allocation_logic, and attestation_engine use contract error enums.
- commitment_core and shared_utils rely on `panic!` for error handling.
- Audit should evaluate error consistency and whether panics are acceptable for the protocol.

## Cross-contract calls
- commitment_core calls token contracts for transfers and commitment_nft for mint/settle.
- attestation_engine invokes commitment_core to read commitments.
- The commitment_core mint call does not include the `early_exit_penalty` parameter expected by commitment_nft::mint. This must be reconciled before audit.

## Storage growth and data consistency
- Vectors for owner commitments, attestations, pool registry, and token IDs grow without bounds; consider pagination or caps.
- allocation_logic does not validate commitment ownership against commitment_core; allocations are local to allocation_logic.

## Rate limiting
- shared_utils RateLimiter is available and configurable by admin.
- Rate limits are disabled until explicitly configured; audit should confirm desired defaults.

## Event emission
- Key state changes emit events (commitments created, allocations, attestations, mint/transfer/settle).
- Off-chain indexers should verify expected topics and payload consistency.
