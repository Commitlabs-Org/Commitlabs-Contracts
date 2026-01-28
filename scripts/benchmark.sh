#!/bin/bash
# Performance Benchmarking Script for Soroban Contracts
# Measures gas usage, execution time, and storage costs

set -e

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_TARGET="${RUST_TARGET:-wasm32v1-none}"
BENCHMARK_DIR="${ROOT_DIR}/benchmarks"
RESULTS_DIR="${BENCHMARK_DIR}/results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Starting Performance Benchmarks${NC}"
echo "======================================"

# Create directories
mkdir -p "${BENCHMARK_DIR}"
mkdir -p "${RESULTS_DIR}"

# Build contracts in release mode
echo -e "${YELLOW}Building contracts for benchmarking...${NC}"
cd "${ROOT_DIR}"
cargo build --target "${RUST_TARGET}" --release

# Run benchmark tests
echo -e "${YELLOW}Running benchmark tests...${NC}"
cargo test --workspace --release --features benchmark -- --nocapture 2>&1 | tee "${RESULTS_DIR}/benchmark_${TIMESTAMP}.log"

# Extract metrics from test output
echo -e "${YELLOW}Extracting metrics...${NC}"

# Generate summary report
cat > "${RESULTS_DIR}/summary_${TIMESTAMP}.md" << EOF
# Performance Benchmark Summary
Generated: $(date)

## Test Results
\`\`\`
$(tail -50 "${RESULTS_DIR}/benchmark_${TIMESTAMP}.log")
\`\`\`

## Next Steps
- Review individual contract benchmarks
- Compare with previous runs
- Identify optimization opportunities
EOF

echo -e "${GREEN}Benchmarks completed!${NC}"
echo "Results saved to: ${RESULTS_DIR}/benchmark_${TIMESTAMP}.log"
echo "Summary: ${RESULTS_DIR}/summary_${TIMESTAMP}.md"
