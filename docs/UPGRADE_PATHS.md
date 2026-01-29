# Upgrade Paths

## Current posture
- Contracts are immutable once deployed; there is no built-in upgradeability or proxy pattern.
- New functionality requires deploying new contract instances and updating downstream references.

## Suggested upgrade process
1. **Deploy new contract versions** using the standard deployment scripts.
2. **Initialize with new admin values** and set cross-contract references (NFT core contract, attestation core contract, etc.).
3. **Update contract IDs** in `deployments/<network>.json` and any off-chain services.
4. **Migrate state** if required (export old state, import into new contract instance).
5. **Revoke old permissions** (remove verifier lists, stop using old contract addresses).
6. **Communicate cutover window** to integrators and indexers.

## Data migration considerations
- commitment_core commitments and owner lists are stored in instance storage; direct migration would require admin tooling or a custom migration contract.
- commitment_nft ownership state and metadata require an export/import tool or a re-mint strategy.
- attestation_engine health metrics and attestations are stored in persistent storage; consider exporting via off-chain indexers.
- allocation_logic pools and allocations are stored in persistent storage; migration should preserve pool liquidity totals.

## Versioning
- Include contract version metadata in off-chain configuration and release notes.
- Consider adding a `get_version` function for future releases.
