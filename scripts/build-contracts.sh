#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

RUST_TARGET="${RUST_TARGET:-wasm32v1-none}"

echo "Building contracts (target: $RUST_TARGET)..."
cargo build --workspace --target "$RUST_TARGET" --release

echo "Built artifacts:"
for contract in commitment_nft commitment_core attestation_engine; do
  wasm_path="$ROOT_DIR/target/${RUST_TARGET}/release/${contract}.wasm"
  if [[ ! -f "$wasm_path" ]]; then
    echo "Missing build artifact: $wasm_path"
    exit 1
  fi
  echo " - $wasm_path"
done
