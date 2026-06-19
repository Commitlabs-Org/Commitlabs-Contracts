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

## Deterministic fuzz properties
- `commitment_core` fee arithmetic is covered by deterministic fuzz-style seed tests in `contracts/commitment_core/src/fuzz_tests.rs`.
- For every covered `amount >= 0` and `bps in 0..=10000`, `checked_fee_value_from_bps` must produce `fee >= 0`, `fee <= amount`, `net >= 0`, and `net + fee == amount`.
- Boundary seeds include `amount = 0`, `bps = 0`, `bps = 10000`, `i128::MAX / 10000` neighbors, `i128::MAX - 1`, and `i128::MAX`.
- The `create_commitment` fee path is checked with an `i128::MAX` amount so the shared `fee_from_bps` implementation cannot regress to multiply-before-divide overflow.

## Open items before audit
- Capture a coverage report and attach to TEST_COVERAGE.md
- Decide on authorization model for mint/allocate/settle flows
- Finalize commitment ID generation strategy and fee parsing approach
