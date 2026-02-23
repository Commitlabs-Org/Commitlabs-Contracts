#!/usr/bin/env bash
# Validates monitoring config for CI. Exit 0 if config is present and valid.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CONFIG="$ROOT_DIR/monitoring/config.yaml"

if [[ ! -f "$CONFIG" ]]; then
  echo "Error: monitoring config not found at $CONFIG"
  exit 1
fi

# Require key sections (grep for YAML keys)
for section in "metrics:" "events:" "alerts:" "notification:"; do
  if ! grep -q "^${section}" "$CONFIG"; then
    echo "Error: missing section $section in $CONFIG"
    exit 1
  fi
done

echo "Monitoring config validated: $CONFIG"
exit 0
