# Known Limitations

- commitment_core::generate_commitment_id returns a constant prefix ("commitment_") and does not guarantee uniqueness.
- commitment_core::update_value emits an event but does not persist the new value.
- commitment_core state-changing functions (create_commitment, settle, early_exit, allocate, update_value) do not enforce `require_auth`.
- commitment_nft::mint does not enforce an authorized minter list (DataKey::AuthorizedMinter is unused).
- commitment_nft::settle is not restricted to the core contract.
- commitment_nft::initialize has no auth check and can be called by any deployer.
- commitment_core calls commitment_nft::mint without the `early_exit_penalty` argument expected by the NFT contract.
- attestation_engine fee parsing and volatility calculations are placeholders; `fees_generated` remains zero.
- allocation_logic does not validate commitment IDs against commitment_core or transfer assets; it only records allocations.
- create_commitment integration tests are skipped because token contract calls are not mocked.
- Formal verification artifacts are not present; formal verification sections are comments only.
- Fuzz/property-based tests are not implemented.
- No upgradeability mechanism is implemented (contracts are immutable once deployed).
