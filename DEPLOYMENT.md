# Deployment Guide

This guide covers building, deploying, initializing, and verifying CommitLabs contracts on
Stellar Soroban testnet and mainnet.

## Prerequisites

- Rust + Cargo
- Soroban CLI (`soroban`)
- Access to a funded deployer account for the target network

## Configuration

Create a local config file:

```bash
cp config/deploy.env.example config/deploy.env
```

Set the required values in `config/deploy.env`:

- `STELLAR_ACCOUNT`: secret key, identity name, or public key used to sign transactions
- `STELLAR_ADMIN_ADDRESS`: public key used as admin in contract initialization

Optional overrides:

- `NETWORK`: `testnet` or `mainnet` (scripts also accept a positional network argument)
- `STELLAR_RPC_URL`: RPC endpoint
- `STELLAR_NETWORK_PASSPHRASE`: network passphrase
- `RUST_TARGET`: default `wasm32v1-none`

## Build

```bash
bash scripts/build-contracts.sh
```

## Deploy to Testnet

```bash
bash scripts/deploy-testnet.sh
```

## Deploy to Mainnet

```bash
bash scripts/deploy-mainnet.sh
```

## Deployment Order

The deployment script enforces the following order:

1. `commitment_nft`
2. `commitment_core`
3. `attestation_engine`

## Initialization Steps

The deployment script performs:

- Initialize `commitment_nft` with admin address
- Initialize `commitment_core` with admin and NFT contract address
- Set `commitment_core` as the NFT core contract
- Add `commitment_core` as an authorized minter
- Initialize `attestation_engine` with admin and core contract address

## Verification Steps

After initialization, the script verifies:

- `commitment_nft.get_admin`
- `commitment_nft.get_core_contract`
- `attestation_engine.get_attestations` (with a dummy commitment id)

These checks confirm the contracts respond and the primary addresses are set.

## Contract Address Storage

Deployment outputs are stored in:

- `deployments/testnet.json`
- `deployments/mainnet.json`

Each file contains network metadata and deployed contract IDs.

## Configuration Options

### Network Defaults

If not explicitly set, the scripts use:

- Testnet RPC: `https://soroban-testnet.stellar.org`
- Testnet passphrase: `Test SDF Network ; September 2015`
- Mainnet RPC: `https://soroban-mainnet.stellar.org`
- Mainnet passphrase: `Public Global Stellar Network ; September 2015`

### Environment Variables

All CLI options can be set via environment variables (per Soroban CLI defaults):

- `STELLAR_ACCOUNT`
- `STELLAR_RPC_URL`
- `STELLAR_NETWORK_PASSPHRASE`
- `STELLAR_FEE`

## Troubleshooting

- **Missing WASM files**: run `bash scripts/build-contracts.sh` and confirm
  the `target/wasm32v1-none/release/*.wasm` artifacts exist.
- **Authorization errors**: ensure `STELLAR_ACCOUNT` signs for
  `STELLAR_ADMIN_ADDRESS` (they should match).
- **RPC timeouts**: override `STELLAR_RPC_URL` to a reliable RPC provider.
- **Invoke failures**: re-run with `--very-verbose` to inspect Soroban CLI logs.

## Security Notes

- Do not commit real secrets or funded accounts.
- Prefer Soroban identities stored in your local key store instead of plain text
  secret keys in shell history.
- Rotate keys after test deployments.
- Restrict admin keys to minimal access and use separate operational keys where possible.

## Rollback Procedures

Smart contracts are immutable once deployed. Rollback should be performed by:

1. Deploying new contract instances.
2. Updating the stored contract IDs in `deployments/<network>.json`.
3. Re-initializing dependent services to point at the new addresses.
