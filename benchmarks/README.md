# Performance Benchmarks

This directory contains performance benchmarking tools and results for the CommitLabs contracts.

## Running Benchmarks

```bash
# Run all benchmarks
bash scripts/benchmark.sh

# Run specific contract benchmarks
cargo test --package commitment_core --features benchmark
cargo test --package commitment_nft --features benchmark
cargo test --package attestation_engine --features benchmark
cargo test --package allocation_logic --features benchmark
```

## Benchmark Metrics

The benchmarks measure:

1. **Gas Usage**: Cost of executing each function
2. **Execution Time**: Time taken for function execution
3. **Storage Costs**: Cost of storage operations
4. **Cross-Contract Calls**: Cost of invoking other contracts
5. **Batch Operations**: Efficiency of batch operations

## Results

Benchmark results are stored in `benchmarks/results/` with timestamps.

## Comparison

Use the comparison tool to compare before/after optimizations:

```bash
cargo run --bin benchmark-compare -- benchmarks/results/before.json benchmarks/results/after.json
```
