# CommitLabs Contracts

Stellar Soroban smart contracts for the CommitLabs protocol.

## Overview

This directory contains the core smart contracts for CommitLabs:

- **commitment_nft**: NFT contract representing liquidity commitments
- **commitment_core**: Core contract for creating and managing commitments
- **attestation_engine**: Contract for verifying and recording commitment health

## Prerequisites

- Rust (latest stable version)
- Stellar Soroban CLI tools
- Cargo

## Building

```bash
# Build all contracts
cargo build --target wasm32-unknown-unknown --release

# Build individual contract
cd contracts/commitment_nft
cargo build --target wasm32-unknown-unknown --release
```

## Testing

```bash
# Run all tests
cargo test

# Test specific contract
cd contracts/commitment_nft
cargo test
```

## CI/CD

This repository uses GitHub Actions to automatically build, test, and validate Soroban smart contracts on every push to `main` and every pull request targeting `main`.

### What the CI Does

The CI pipeline performs the following steps:

1. **Checkout** the repository
2. **Install Rust** via rustup (stable toolchain)
3. **Add Soroban target** (`wasm32v1-none`) for contract compilation
4. **Install Stellar CLI** via Homebrew
5. **Build contracts** using both:
   - Cargo (`cargo build --target wasm32v1-none --release`)
   - Stellar CLI (`soroban contract build`)
6. **Run tests** (`cargo test --target wasm32v1-none --release`)

### When It Runs

- On every push to the `main` branch
- On every pull request targeting the `main` branch

### Fixing CI Failures Locally

If the CI fails, you can reproduce the same environment locally:

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable

# Add Soroban target
rustup target add wasm32v1-none

# Install Stellar CLI (macOS)
brew tap stellar/stellar-cli
brew install stellar

# Verify installation
stellar --version
soroban --version

# Build contracts
cargo build --target wasm32v1-none --release

# Run tests
cargo test --workspace
```

The CI will fail fast on any build errors or test failures, ensuring that only valid code is merged into the main branch.

## Deployment
Deployment is managed via scripts and documented in `DEPLOYMENT.md`.

```bash
# Build all contracts
bash scripts/build-contracts.sh

# Deploy to testnet
bash scripts/deploy-testnet.sh

# Deploy to mainnet
bash scripts/deploy-mainnet.sh
```

## Contract Structure

### Commitment NFT (`commitment_nft`)

Represents a liquidity commitment as an NFT. Each NFT contains:
- Commitment metadata (duration, risk tolerance, type)
- Owner information
- Active status

### Commitment Core (`commitment_core`)

Manages the lifecycle of commitments:
- Creation with rules
- Value tracking
- Settlement at maturity
- Early exit handling

### Attestation Engine (`attestation_engine`)

Continuously verifies commitment health:
- Records attestations
- Tracks health metrics
- Verifies compliance
- Calculates compliance scores

## Development Status

⚠️ **Early Development**: These contracts are basic skeletons with placeholder implementations. Core functionality needs to be implemented.

## Next Steps

- [ ] Implement storage for all contracts
- [ ] Add proper access control
- [ ] Implement commitment rule enforcement
- [ ] Add integration with Stellar asset contracts
- [ ] Implement allocation logic
- [ ] Add comprehensive tests
- [ ] Security audit

## License

MIT

