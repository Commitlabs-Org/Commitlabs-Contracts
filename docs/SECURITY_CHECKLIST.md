# Security Checklist

## Access control
- [ ] Verify all privileged functions require `require_auth`.
- [ ] Verify commitment_core enforces caller/owner/admin requirements where intended.
- [ ] Verify commitment_nft mint/settle authorization model.
- [ ] Confirm verifier whitelist management is admin-only.

## Reentrancy protection
- [ ] Validate guard set/clear in every state-changing function.
- [ ] Confirm external calls are performed after state updates.

## Arithmetic safety
- [ ] Check for unchecked arithmetic in all contracts.
- [ ] Confirm overflow-checks enabled in release profile.

## Input validation
- [ ] Ensure all public entry points validate arguments.
- [ ] Validate commitment_id uniqueness and formatting.

## Cross-contract interactions
- [ ] Verify commitment_core <-> commitment_nft interface signature alignment.
- [ ] Review token transfer paths for correct asset and amount handling.
- [ ] Confirm attestation_engine commitment existence checks use expected core contract.

## Storage and events
- [ ] Review storage growth for vectors and registries.
- [ ] Confirm event emissions for state transitions.

## Testing and verification
- [ ] Run full test suite (cargo test --workspace).
- [ ] Produce coverage report and attach to TEST_COVERAGE.md.
- [ ] Add security-focused tests for missing auth checks.
- [ ] Add fuzz/property tests for arithmetic and validation.
- [ ] Assess feasibility of formal verification for core invariants.
