#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "Generating Rust API documentation for all contracts..."

# Generate HTML documentation for the entire workspace without pulling in all dependencies.
# Prefer an offline + locked run and fall back to online if needed.
if cargo doc --workspace --no-deps --locked --offline; then
  echo "cargo doc succeeded (offline)"
else
  echo "cargo doc offline failed; retrying with network access..."
  cargo doc --workspace --no-deps --locked
fi

DOC_ROOT="$ROOT_DIR/target/doc"
OUT_ROOT="$ROOT_DIR/docs"

if [[ ! -d "$DOC_ROOT" ]]; then
  echo "Expected documentation directory not found at: $DOC_ROOT"
  exit 1
fi

echo "Copying generated documentation to: $OUT_ROOT"
rm -rf "$OUT_ROOT"
mkdir -p "$OUT_ROOT"

# Copy the entire doc site output
cp -R "$DOC_ROOT"/. "$OUT_ROOT"/

echo "Documentation successfully generated under:"
echo "  $OUT_ROOT"
echo
echo "Key entry points:"
echo "  - $OUT_ROOT/commitment_nft/index.html"
echo "  - $OUT_ROOT/commitment_core/index.html"
echo "  - $OUT_ROOT/attestation_engine/index.html"
echo "  - $OUT_ROOT/allocation_logic/index.html"
echo "  - $OUT_ROOT/shared_utils/index.html"

echo
echo "Tip: You can open these files directly in a browser, or host the contents of docs via any static file server."


