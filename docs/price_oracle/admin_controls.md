# Price Oracle: Admin Controls and Oracle Rotation

This document describes the admin-only controls for managing oracle sources in the `price_oracle` contract, including whitelisting, removal, and rotation of trusted price publishers.

## Overview

The `price_oracle` contract is a trusted-publisher registry. Only addresses whitelisted by the admin can publish prices. The admin is responsible for:
- Adding new oracle addresses (trusted publishers)
- Removing or rotating out compromised or obsolete oracles
- Transferring admin authority when needed

## Functions

| Function | Summary | Access Control | Notes |
|----------|---------|---------------|-------|
| `add_oracle(caller, oracle_address)` | Add a trusted price publisher | Admin require_auth | Whitelisted oracle can overwrite the latest price for any asset it updates |
| `remove_oracle(caller, oracle_address)` | Remove a trusted price publisher | Admin require_auth | Prevents further updates from that address |
| `set_admin(caller, new_admin)` | Transfer oracle admin authority | Admin require_auth | Transfers control over whitelist and configuration |

## Oracle Rotation

Oracle rotation is performed by removing an old oracle address and adding a new one. Only the admin can perform these actions. This ensures that if an oracle key is compromised or needs to be replaced, the admin can promptly update the whitelist.

### Example: Rotating an Oracle
1. `remove_oracle(caller, old_oracle_address)`
2. `add_oracle(caller, new_oracle_address)`

## Security Notes
- Only the admin can modify the whitelist. Attempts by non-admins will fail with `Unauthorized`.
- Whitelisted oracles are trusted to publish honest prices. Compromised oracles can overwrite the latest price for any asset.
- Downstream contracts should always use `get_price_valid` and set appropriate staleness windows.

## See Also
- [CONTRACT_FUNCTIONS.md](../CONTRACT_FUNCTIONS.md#price_oracle)
- [THREAT_MODEL.md](../THREAT_MODEL.md#price-oracle-manipulation-resistance-assumptions)
