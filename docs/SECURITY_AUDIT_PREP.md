# Security Audit Preparation

## Scope
- commitment_nft
- commitment_core
- attestation_engine
- allocation_logic
- shared_utils

## Document set
- [Architecture](ARCHITECTURE.md)
- [Contract functions](CONTRACT_FUNCTIONS.md)
- [Security considerations](SECURITY_CONSIDERATIONS.md)
- [Known limitations](KNOWN_LIMITATIONS.md)
- [Upgrade paths](UPGRADE_PATHS.md)
- [Threat model](THREAT_MODEL.md)
- [Security checklist](SECURITY_CHECKLIST.md)
- [Test coverage](TEST_COVERAGE.md)

## Audit artifacts checklist
- Latest contract WASM builds (target/wasm32v1-none/release/*.wasm)
- Deployment addresses (deployments/*.json)
- Admin/verifier operational notes (see DEPLOYMENT.md)
- Test outputs and coverage report (see TEST_COVERAGE.md)

## Review focus areas
- Access control enforcement for privileged paths
- Reentrancy guard usage and guard cleanup on error paths
- Arithmetic safety and overflow behavior
- Cross-contract calls (token transfer, NFT mint/settle, commitment_core reads)
- Storage growth and data consistency for vectors and registries

## Fee Arithmetic Fuzz Invariants

`commitment_core` includes deterministic fuzz-seed tests for basis-point fee arithmetic. For every non-negative amount in the seed grid and every valid `bps` value in `0..=10000`, the checked helper must satisfy:

- `0 <= fee <= amount`
- `net_amount + fee == amount`
- `bps == 0` yields `fee == 0` and `net_amount == amount`
- `bps == 10000` yields `fee == amount` and `net_amount == 0`

The seed grid includes `i128::MAX`-adjacent amounts so the helper computes mathematically valid fees without first overflowing `amount * bps`. Invalid domains, including negative amounts and `bps > 10000`, return `None` instead of producing fee observations.

## Open items before audit
- Capture a coverage report and attach to TEST_COVERAGE.md
- Decide on authorization model for mint/allocate/settle flows
- Finalize commitment ID generation strategy and fee parsing approach
