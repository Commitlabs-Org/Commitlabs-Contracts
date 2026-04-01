# Test Coverage

## Current tests
- commitment_nft: unit tests for initialization, minting, metadata, transfer, settle, and edge cases.
- commitment_core: validation tests, violation checks, and event emission tests (create_commitment integration test is skipped).
- attestation_engine: extensive tests for attestations, health metrics, and access control.
- allocation_logic: security-focused tests for pool registration, allocation, and error paths.
- shared_utils: integration tests for validation, math, storage, and access control helpers.
- commitment_marketplace: comprehensive tests for marketplace operations including buy flow payment token and NFT transfer failure handling.

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

## Marketplace Buy Flow Test Coverage
The commitment_marketplace contract now includes comprehensive unit tests for buy flow payment token and NFT transfer failure handling:

### Payment Token Transfer Failure Tests
- `test_buy_nft_payment_token_transfer_failure_handling`: Tests handling of payment token transfer failures
- `test_buy_nft_zero_fee_scenario`: Verifies zero fee calculation and transfer logic
- `test_buy_nft_maximum_fee_scenario`: Tests maximum fee scenarios and transfer amounts
- `test_buy_nft_different_payment_tokens`: Validates multiple payment token support

### NFT Transfer Failure Tests  
- `test_buy_nft_nft_transfer_failure_handling`: Tests handling of NFT transfer failures
- `test_buy_nft_state_consistency_after_failure`: Verifies state consistency after transfer failures
- `test_buy_nft_event_emission_on_success`: Tests proper event emission on successful transfers

### Reentrancy and Security Tests
- `test_buy_nft_reentrancy_protection`: Validates reentrancy guard functionality
- `test_buy_nft_reentrancy_guard_cleanup_on_success`: Tests guard cleanup on success
- `test_buy_nft_reentrancy_guard_cleanup_on_failure`: Tests guard cleanup on failure
- `test_buy_nft_concurrent_safety`: Tests concurrent operation safety

### Edge Case and Error Handling Tests
- `test_buy_nonexistent_listing_fails`: Tests error handling for non-existent listings
- `test_buy_nft_fee_calculation_accuracy`: Validates precise fee calculations
- `test_buy_nft_edge_case_prices`: Tests edge case price scenarios
- `test_buy_nft_removes_listing_before_external_calls`: Validates checks-effects-interactions pattern

### Security Notes
- Tests follow checks-effects-interactions pattern to prevent reentrancy attacks
- Transfer failures are properly handled with appropriate error propagation
- State consistency is maintained even in partial failure scenarios
- Reentrancy guards are properly managed in both success and failure paths
