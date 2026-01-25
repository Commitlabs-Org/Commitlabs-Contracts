#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if [[ -f "$ROOT_DIR/config/deploy.env" ]]; then
  # shellcheck disable=SC1091
  source "$ROOT_DIR/config/deploy.env"
fi

NETWORK="${1:-${NETWORK:-}}"
if [[ -z "$NETWORK" ]]; then
  echo "NETWORK is required (testnet|mainnet)."
  exit 1
fi

case "$NETWORK" in
  testnet|mainnet) ;;
  *)
    echo "Unsupported NETWORK: $NETWORK"
    exit 1
    ;;
esac

if [[ -z "${STELLAR_ACCOUNT:-}" ]]; then
  echo "STELLAR_ACCOUNT is required (secret key, identity, or public key)."
  exit 1
fi

if [[ -z "${STELLAR_ADMIN_ADDRESS:-}" ]]; then
  echo "STELLAR_ADMIN_ADDRESS is required (public key for admin role)."
  exit 1
fi

if [[ "$STELLAR_ACCOUNT" == G* && "$STELLAR_ACCOUNT" != "$STELLAR_ADMIN_ADDRESS" ]]; then
  echo "Warning: STELLAR_ACCOUNT does not match STELLAR_ADMIN_ADDRESS."
  echo "Authorization may fail if the signer cannot authorize admin actions."
fi

if [[ -z "${STELLAR_RPC_URL:-}" ]]; then
  if [[ "$NETWORK" == "testnet" ]]; then
    STELLAR_RPC_URL="https://soroban-testnet.stellar.org"
  else
    STELLAR_RPC_URL="https://soroban-mainnet.stellar.org"
  fi
fi

if [[ -z "${STELLAR_NETWORK_PASSPHRASE:-}" ]]; then
  if [[ "$NETWORK" == "testnet" ]]; then
    STELLAR_NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
  else
    STELLAR_NETWORK_PASSPHRASE="Public Global Stellar Network ; September 2015"
  fi
fi

export STELLAR_RPC_URL STELLAR_NETWORK_PASSPHRASE

RUST_TARGET="${RUST_TARGET:-wasm32v1-none}"

echo "Using network: $NETWORK"
echo "RPC URL: $STELLAR_RPC_URL"

"$ROOT_DIR/scripts/build-contracts.sh"

deploy_contract() {
  local name="$1"
  local wasm_path="$2"

  echo "Deploying $name..."
  local contract_id
  contract_id=$(soroban contract deploy -q \
    --wasm "$wasm_path" \
    --source-account "$STELLAR_ACCOUNT" \
    --rpc-url "$STELLAR_RPC_URL" \
    --network-passphrase "$STELLAR_NETWORK_PASSPHRASE")

  if [[ -z "$contract_id" ]]; then
    echo "Failed to deploy $name."
    exit 1
  fi

  echo "$contract_id"
}

invoke_contract() {
  local contract_id="$1"
  shift
  soroban contract invoke -q \
    --id "$contract_id" \
    --source-account "$STELLAR_ACCOUNT" \
    --rpc-url "$STELLAR_RPC_URL" \
    --network-passphrase "$STELLAR_NETWORK_PASSPHRASE" \
    -- "$@"
}

invoke_view() {
  local contract_id="$1"
  shift
  soroban contract invoke -q \
    --send no \
    --id "$contract_id" \
    --source-account "$STELLAR_ACCOUNT" \
    --rpc-url "$STELLAR_RPC_URL" \
    --network-passphrase "$STELLAR_NETWORK_PASSPHRASE" \
    -- "$@"
}

NFT_WASM="$ROOT_DIR/target/${RUST_TARGET}/release/commitment_nft.wasm"
CORE_WASM="$ROOT_DIR/target/${RUST_TARGET}/release/commitment_core.wasm"
ATTEST_WASM="$ROOT_DIR/target/${RUST_TARGET}/release/attestation_engine.wasm"

NFT_CONTRACT_ID="$(deploy_contract "commitment_nft" "$NFT_WASM")"
CORE_CONTRACT_ID="$(deploy_contract "commitment_core" "$CORE_WASM")"
ATTEST_CONTRACT_ID="$(deploy_contract "attestation_engine" "$ATTEST_WASM")"

echo "Initializing contracts..."
invoke_contract "$NFT_CONTRACT_ID" initialize --admin "$STELLAR_ADMIN_ADDRESS"
invoke_contract "$CORE_CONTRACT_ID" initialize --_admin "$STELLAR_ADMIN_ADDRESS" --_nft_contract "$NFT_CONTRACT_ID"
invoke_contract "$NFT_CONTRACT_ID" set_core_contract --core_contract "$CORE_CONTRACT_ID"
invoke_contract "$NFT_CONTRACT_ID" add_authorized_minter --caller "$STELLAR_ADMIN_ADDRESS" --minter "$CORE_CONTRACT_ID"
invoke_contract "$ATTEST_CONTRACT_ID" initialize --admin "$STELLAR_ADMIN_ADDRESS" --commitment_core "$CORE_CONTRACT_ID"

echo "Verifying deployments..."
invoke_view "$NFT_CONTRACT_ID" get_admin
invoke_view "$NFT_CONTRACT_ID" get_core_contract
invoke_view "$ATTEST_CONTRACT_ID" get_attestations --commitment_id "deployment_check"

DEPLOYMENTS_DIR="$ROOT_DIR/deployments"
mkdir -p "$DEPLOYMENTS_DIR"

deployer_account="$STELLAR_ACCOUNT"
if [[ "$deployer_account" == S* ]]; then
  deployer_account="secret_key_redacted"
fi

timestamp="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
deployment_file="$DEPLOYMENTS_DIR/${NETWORK}.json"

cat > "$deployment_file" <<EOF
{
  "network": "$NETWORK",
  "rpc_url": "$STELLAR_RPC_URL",
  "network_passphrase": "$STELLAR_NETWORK_PASSPHRASE",
  "deployer_account": "$deployer_account",
  "admin_address": "$STELLAR_ADMIN_ADDRESS",
  "deployed_at": "$timestamp",
  "contracts": {
    "commitment_nft": "$NFT_CONTRACT_ID",
    "commitment_core": "$CORE_CONTRACT_ID",
    "attestation_engine": "$ATTEST_CONTRACT_ID"
  }
}
EOF

echo "Deployment complete. Contract addresses saved to $deployment_file"
