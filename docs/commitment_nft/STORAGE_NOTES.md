# Commitment NFT Storage Notes

## Summary
The commitment NFT contract maintains a global list of token IDs for list and
metadata lookups. To keep hot-path operations predictable at scale, the token
ID index is stored in persistent storage instead of instance storage.

## Rationale
- Instance storage has tighter size limits and higher risk of hitting capacity
  as the token ID list grows.
- List endpoints (`get_all_metadata`) are hot paths that should avoid large
  instance storage reads.

## Current Layout
- `DataKey::TokenIds` (Vec<u32>) is stored in **persistent** storage.
- Per-token and per-owner records already live in persistent storage.

## Security Notes
- No auth changes are introduced by this storage move.
- The change only affects where the token ID index is stored, not its contents.
