# commitment_nft Settlement Authorization

## Summary

`commitment_nft::settle` and `commitment_nft::mark_inactive` are lifecycle mutations and must only be driven by the configured `commitment_core` contract.

The contract now enforces two checks for both entrypoints:

1. The passed `caller` address must authorize the invocation via `require_auth()`.
2. The authorized `caller` must equal the stored `CoreContract` address.

This prevents an external account from spoofing the core contract address in the function arguments.

## Public API Change

The public NFT lifecycle entrypoints now use the caller-aware ABI that `commitment_core` was already invoking:

- `settle(caller: Address, token_id: u32)`
- `mark_inactive(caller: Address, token_id: u32)`

For direct generated-client usage, callers must now provide the trusted core contract address as the first argument.

## Trust Boundaries

- Admin:
  - Can set or rotate `CoreContract` via `set_core_contract`.
- commitment_core:
  - Can settle matured NFTs.
  - Can mark NFTs inactive during early-exit style lifecycle transitions.
- NFT owners and external accounts:
  - Cannot directly mutate `is_active` through `settle` or `mark_inactive`.

## Storage Keys Mutated

- `DataKey::NFT(token_id)`:
  - `is_active` flips from `true` to `false`.
- `DataKey::ReentrancyGuard`:
  - Set to `true` during execution and cleared on all handled return paths.

No owner balances, owner token registries, token counters, or total supply values are mutated during settlement.

## Migration Notes

- On-chain storage layout does not change.
- `CURRENT_VERSION` is bumped to `2` to version the external interface change.
- After upgrading, operators should verify:
  - `get_core_contract()` returns the intended live `commitment_core` address.
  - Any generated clients or scripts calling `settle` or `mark_inactive` pass the new `caller` argument.
  - Downstream integrations do not rely on permissionless settlement.

If an existing deployment never set `CoreContract`, settlement and manual inactivation will now fail until the admin configures it.

## Security Notes

- Reentrancy:
  - Both entrypoints remain guarded by `DataKey::ReentrancyGuard`.
  - They do not perform outbound calls.
- Arithmetic:
  - These paths do not perform financial math or rounding.
  - Expiry comparison is a direct `u64` timestamp check: `current_time >= expires_at`.
- Cross-contract behavior:
  - `commitment_core` already invoked the caller-aware shape, so the runtime trust boundary now matches the intended integration design.
