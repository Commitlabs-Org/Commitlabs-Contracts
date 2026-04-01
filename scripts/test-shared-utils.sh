#!/usr/bin/env bash
set -euo pipefail

echo "Running shared_utils host unit tests..."
cargo test -p shared_utils

echo "Building shared_utils for Soroban target wasm32v1-none..."
cargo build -p shared_utils --target wasm32v1-none --release

echo "shared_utils checks completed."
