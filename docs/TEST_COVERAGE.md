# Test Coverage

## Current tests
- commitment_nft: unit tests for initialization, minting, metadata, transfer, settle, and edge cases.
- commitment_core: validation tests, violation checks, and event emission tests (create_commitment integration test is skipped).
- attestation_engine: extensive tests for attestations, health metrics, and access control.
- allocation_logic: security-focused tests for pool registration, allocation, and error paths.
- shared_utils: integration tests for validation, math, storage, and access control helpers.

## Coverage status
- No coverage report is currently checked in.
- Some integration paths (token transfers, NFT minting via commitment_core) are not fully tested due to missing mocks.

## Latest execution (local)
- `cargo test --workspace`: passed (exit code 0).
- `cargo llvm-cov --workspace --lcov --output-path lcov.info`: succeeded (lcov.info generated).
- `cargo llvm-cov --workspace --summary-only` totals:
  - Regions: 77.34% (940 missed / 4148 total)
  - Functions: 75.58% (63 missed / 258 total)
  - Lines: 76.99% (556 missed / 2416 total)

### Notable low-coverage modules
- commitment_core: 39.43% region coverage, 43.03% line coverage.
- shared_utils/access_control: 55.08% region coverage, 47.69% line coverage.

## How to run tests
```bash
cargo test --workspace
```

## How to collect coverage (recommended)
1. Install coverage tooling (example using cargo-llvm-cov):
   ```bash
   cargo install cargo-llvm-cov
   ```
2. Run coverage:
   ```bash
   cargo llvm-cov --workspace --lcov --output-path lcov.info
   ```
3. Attach summary numbers here or upload artifacts.

## Security-focused testing gaps
- Missing tests that assert authorization failures for commitment_core and commitment_nft mint/settle.
- Missing fuzz/property-based tests for arithmetic and validation edge cases.
- Formal verification artifacts not present; invariants are documented in comments only.

## Suggested additions
- Add mock token contracts for create_commitment/settle flows.
- Add fuzz tests for attestation payload parsing.
- Add property tests for allocation distribution invariants.
