# Performance Benchmarks and Optimizations

This document describes the performance benchmarking infrastructure and optimization strategies for CommitLabs contracts.

## Overview

Performance benchmarking is integrated into the development workflow to ensure contracts remain gas-efficient and performant. The benchmarking system measures:

- **Gas Usage**: Cost of executing each function
- **Execution Time**: Time taken for function execution  
- **Storage Costs**: Cost of storage operations
- **Cross-Contract Calls**: Cost of invoking other contracts
- **Batch Operations**: Efficiency of batch operations

## Running Benchmarks

### Local Development

```bash
# Run all benchmarks
bash scripts/benchmark.sh

# Run specific contract benchmarks
cargo test --package commitment_core --features benchmark --release
cargo test --package commitment_nft --features benchmark --release
cargo test --package attestation_engine --features benchmark --release
cargo test --package allocation_logic --features benchmark --release
```

### CI/CD

Benchmarks run automatically in CI/CD on every push and pull request. Results are saved to `benchmarks/results/` with timestamps.

## Benchmark Results

Benchmark results are stored in `benchmarks/results/` with the following naming convention:
- `benchmark_YYYYMMDD_HHMMSS.log` - Full benchmark output
- `summary_YYYYMMDD_HHMMSS.md` - Summary report

## Optimizations Implemented

### Storage Optimizations

1. **Reduced Storage Reads**: Optimized `create_commitment` to read `TotalCommitments` and `TotalValueLocked` once instead of multiple times
2. **Batch Storage Operations**: Combined related storage reads/writes where possible
3. **Efficient Key Generation**: Optimized commitment ID generation to minimize string operations

### Code Optimizations

1. **Hot Path Optimization**: Optimized frequently called functions like `get_commitment`, `check_violations`
2. **Calculation Efficiency**: Reduced redundant calculations in violation checking
3. **Early Returns**: Added early returns in validation functions to avoid unnecessary processing

## Performance Metrics

### Commitment Core

- `initialize`: Initial contract setup
- `create_commitment`: Creating new commitments (includes token transfer and NFT mint)
- `get_commitment`: Reading commitment data
- `check_violations`: Checking rule violations
- `settle`: Settling commitments at maturity
- `early_exit`: Early exit with penalty

### Commitment NFT

- `initialize`: Contract initialization
- `mint`: Minting new NFTs
- `get_metadata`: Reading NFT metadata
- `owner_of`: Getting NFT owner
- `balance_of`: Getting owner's balance
- `transfer`: Transferring NFTs

### Attestation Engine

- `initialize`: Contract initialization
- `attest`: Recording attestations
- `get_attestations`: Retrieving attestations
- `calculate_compliance_score`: Calculating compliance scores
- `verify_compliance`: Verifying commitment compliance

### Allocation Logic

- `initialize`: Contract initialization
- `register_pool`: Registering liquidity pools
- `allocate`: Allocating funds to pools
- `get_allocation`: Retrieving allocation data
- `get_pool`: Getting pool information
- `rebalance`: Rebalancing allocations

## Benchmarking Best Practices

1. **Run benchmarks in release mode**: Always use `--release` flag for accurate gas measurements
2. **Compare before/after**: Always compare benchmark results before and after optimizations
3. **Track trends**: Monitor gas usage over time to catch regressions
4. **Test edge cases**: Benchmark with various input sizes and edge cases
5. **Document changes**: Document any significant performance changes

## Optimization Recommendations

### High Priority

1. **Batch Operations**: Implement batch versions of operations where possible
2. **Storage Layout**: Optimize storage key structures for efficient access
3. **Cross-Contract Calls**: Minimize cross-contract calls in hot paths

### Medium Priority

1. **String Operations**: Reduce string concatenation and manipulation
2. **Vector Operations**: Optimize vector operations for large datasets
3. **Event Emission**: Batch events where possible

### Low Priority

1. **Code Size**: Reduce contract size through code optimization
2. **Compilation**: Optimize compilation flags for smaller WASM size

## Continuous Monitoring

- Benchmarks run automatically in CI/CD
- Results are compared against baseline metrics
- Performance regressions trigger alerts
- Optimization opportunities are tracked in issues

## Future Improvements

1. **Automated Performance Reports**: Generate detailed performance reports
2. **Gas Cost Tracking**: Track gas costs over time
3. **Optimization Suggestions**: Automated suggestions for optimization
4. **Performance Budgets**: Set and enforce performance budgets
5. **Comparison Tools**: Tools to compare benchmark results across versions
