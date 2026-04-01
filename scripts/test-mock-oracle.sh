#!/usr/bin/env bash
set -euo pipefail

echo "Running mock_oracle host unit tests..."
cargo test -p mock_oracle

echo "Building mock_oracle for Soroban target wasm32v1-none..."
cargo build -p mock_oracle --target wasm32v1-none --release

echo "mock_oracle checks completed."
