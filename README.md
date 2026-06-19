# CommitLabs Contracts

Stellar Soroban smart contracts for the CommitLabs protocol.

## Overview

This workspace contains the CommitLabs Soroban contracts, interface crates, and shared support libraries:

- **commitment_core**: Creates and manages commitments, custody, fees, lifecycle status, settlement, early exit, and allocation handoff.
- **commitment_nft**: Mints and manages NFTs that mirror commitment metadata, ownership, active state, and settlement state.
- **attestation_engine**: Records verifier attestations, health metrics, compliance checks, and protocol statistics.
- **allocation_logic**: Registers pools and computes allocation or rebalance summaries for commitment capital.
- **commitment_marketplace**: Handles secondary-market listings, purchases, offers, auctions, and marketplace fees for commitment NFTs.
- **commitment_transformation**: Transforms commitments into tranches, collateralized assets, secondary instruments, and protocol guarantees.
- **price_oracle**: Provides whitelisted price feeds with staleness validation for valuation, marketplace, and compliance flows.
- **mock_oracle**: Supplies deterministic oracle behavior for integration and failure-mode testing.
- **commitment_interface**: Mirrors the live commitment ABI and data shapes for downstream bindings and drift checks.
- **shared_utils**: Provides shared validation, access control, safe math, time, storage, event, and rate-limit helpers.
- **time_lock**: Queues sensitive governance actions behind action-specific execution delays.
- **version-system**: Stores contract version metadata, compatibility information, and build metadata.

## Prerequisites

- Rust (latest stable version)
- Stellar Soroban CLI tools
- Cargo
- Rust targets used by CI: `wasm32-unknown-unknown` and `wasm32v1-none`

## Building

```bash
# Install CI targets
rustup target add wasm32-unknown-unknown wasm32v1-none

# Build all contracts
cargo build --workspace --target wasm32-unknown-unknown --release

# Build individual contract
cd contracts/commitment_nft
cargo build --target wasm32-unknown-unknown --release

# Build a contract with Stellar CLI when installed
stellar contract build
```

## Testing

```bash
# Run interface drift checks used by CI
cargo test -p commitment_interface

# Run workspace tests
cargo test --workspace

# Test specific contract
cd contracts/commitment_nft
cargo test

# Run integration tests from their excluded workspace
cd ../../tests/integration
cargo test
```

## Fee Structure

Protocol fee collection (creation, attestation, transformation, early exit) is documented in [docs/FEES.md](docs/FEES.md).

## Documentation

**Generate API documentation for all contracts**

```bash
bash scripts/generate-docs.sh
```

This command runs `cargo doc --workspace --no-deps` and **copies HTML documentation into `docs/`**.
Open the checked-in crate docs currently available under `docs/`:

- `docs/commitment_nft/index.html`
- `docs/commitment_core/index.html`
- `docs/attestation_engine/index.html`
- `docs/allocation_logic/index.html`
- `docs/shared_utils/index.html`

The docs are generated directly from Rust doc comments (`///` and `//!`) so they stay in sync with the code as it evolves.
You can re-run the script after any changes to regenerate function, parameter, and error documentation, as well as any usage examples included in the comments.

## CI/CD

This repository uses GitHub Actions to automatically build, test, and validate Soroban smart contracts on every push to `main` and every pull request targeting `main`.

### What the CI Does

The CI pipeline performs the following steps:

1. **Checkout** the repository
2. **Install Rust** via rustup (stable toolchain)
3. **Add Soroban targets** (`wasm32-unknown-unknown` and `wasm32v1-none`) for contract and CLI compilation paths
4. **Install Stellar CLI** via Homebrew
5. **Build contracts** using both:
   - Cargo (`cargo build --workspace --target wasm32-unknown-unknown --release`)
   - Stellar CLI (`soroban contract build`)
6. **Run tests** (`cargo test -p commitment_interface`, integration tests, and benchmark test targets)

### When It Runs

- On every push to the `master` branch
- On every pull request targeting the `master` branch

### Fixing CI Failures Locally

If the CI fails, you can reproduce the same environment locally:

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable

# Add Soroban targets
rustup target add wasm32-unknown-unknown wasm32v1-none

# Install Stellar CLI (macOS)
brew tap stellar/stellar-cli
brew install stellar

# Verify installation
stellar --version
soroban --version

# Build contracts
cargo build --workspace --target wasm32-unknown-unknown --release

# Run CI-equivalent local tests
cargo test -p commitment_interface
cd tests/integration && cargo test
```

The CI will fail fast on any build errors or test failures, ensuring that only valid code is merged into the `master` branch.

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

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the current architecture diagram and deployment topology, and [docs/CONTRACT_FUNCTIONS.md](docs/CONTRACT_FUNCTIONS.md) for public entrypoints and access-control notes.

### Core Lifecycle Contracts

- **commitment_core**: Implements initialization, commitment creation, value updates, violation checks, settlement, early exit, allocation handoff, fee collection, pause/emergency controls, and rate-limit administration.
- **commitment_nft**: Implements NFT initialization, core-contract authorization, minting, metadata reads, owner indexes, transfers, active-state reads, settlement, and inactive marking.
- **attestation_engine**: Implements verifier management, attestation recording, paginated attestation reads, health metrics, compliance scoring, fee/drawdown attestations, statistics, and rate limits.
- **allocation_logic**: Implements pool registration, pool status/capacity updates, strategy-based allocation, rebalancing, and allocation/pool reads.

### Market, Oracle, and Transformation Contracts

- **commitment_marketplace**: Implements admin configuration, payment-token allowlists, fixed-price listings, purchases, offers, auctions, listing reads, and marketplace fee handling.
- **commitment_transformation**: Implements transformation fee configuration, transformer authorization, tranche/collateral/instrument/guarantee creation and reads, and fee collection.
- **price_oracle**: Implements oracle allowlisting, price publication, raw and freshness-checked price reads, admin rotation, upgrade hooks, and marketplace/commitment price helpers.
- **mock_oracle**: Implements deterministic price, staleness, delay, volatility, and test-mode controls for integration testing.

### Interface and Shared Support

- **commitment_interface**: Provides ABI-only commitment data shapes and drift tests against live `commitment_core`, `commitment_nft`, and `attestation_engine` sources.
- **shared_utils**: Provides reusable access-control, error, event, safe-math, rate-limit, storage, time, and validation helpers.
- **time_lock**: Implements delayed governance action queues with action-specific delay windows.
- **version-system**: Implements on-chain version metadata, minimum-version controls, history, build metadata, and compatibility records.

## Development Status

The workspace is beyond the original skeleton stage. Storage layouts, access-control helpers, commitment rule validation, Stellar asset transfers, allocation logic, fee collection, oracle reads, marketplace flows, transformation records, and interface drift checks are implemented in the current crates.

Areas still tracked as active hardening work include security review, upgrade and deployment runbooks, broader integration coverage, oracle/marketplace operational assumptions, and keeping generated API docs aligned with source.

## Next Steps

- [x] Implement storage for commitment, NFT, attestation, allocation, marketplace, transformation, oracle, governance, and versioning contracts.
- [x] Add access-control paths for admin, owner, verifier, allocator, transformer, oracle, and marketplace operations.
- [x] Implement commitment rule validation, fee collection, rate limiting, emergency controls, and Stellar asset transfer paths.
- [x] Implement allocation logic and pool registry operations.
- [x] Add interface drift checks for downstream commitment ABI compatibility.
- [ ] Expand end-to-end integration coverage for cross-contract market, oracle, allocation, and transformation flows.
- [ ] Complete security audit preparation and review items in [docs/SECURITY_AUDIT_PREP.md](docs/SECURITY_AUDIT_PREP.md).
- [ ] Keep generated API docs and deployment artifacts synchronized with the current workspace.

## License

MIT
