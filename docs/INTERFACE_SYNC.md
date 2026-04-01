# Commitment Interface Synchronization

This document explains how the `commitment_interface` stays synchronized with the production contracts (`commitment_core`, `commitment_nft`, and `attestation_engine`).

## Overview

The `commitment_interface` crate provides a stable ABI and shared types for integrators. To prevent the interface from becoming stale when core contracts change, we use compile-time source matching tests.

## Synchronization Mechanism

The `commitment_interface/src/lib.rs` contains a test module that uses `include_str!` to load the source code of core contracts at compile time. It then extracts specific struct definitions and compares them against the definitions in `commitment_interface/src/types.rs`.

### Tracked Structs

- **CommitmentRules**: Synchronized with `commitment_core` and `attestation_engine`.
- **Commitment**: Synchronized with `commitment_core` and `attestation_engine`.
- **CommitmentMetadata**: Synchronized with `commitment_nft`.
- **CommitmentNFT**: Synchronized with `commitment_nft`.

### Tracked Signatures

The tests also verify that the function signatures in `CommitmentInterface` match the public implementations in `commitment_core`.

## How to Update

When a core contract struct is modified:

1.  Update the corresponding struct in `contracts/commitment_interface/src/types.rs`.
2.  Run tests to verify the match:
    ```bash
    cargo test -p commitment_interface --target wasm32v1-none --release
    ```
3.  If function signatures change, update the signature list in `contracts/commitment_interface/src/lib.rs` tests.

## Security Rationale

By enforcing source-level equality, we ensure that any breaking change in the data model or ABI is immediately caught before a new version of the interface is published. This protects integrators from type mismatches and serialization errors.
