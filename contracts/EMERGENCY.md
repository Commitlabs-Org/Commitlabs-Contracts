# Emergency Procedures

This document outlines the emergency procedures for the CommitLabs protocol.

## Emergency Functions

The following emergency functions are implemented in the `CommitmentCore` contract:

1.  **`set_emergency_mode(enabled: bool)`**:
    - **Description**: Toggles the global emergency mode for the contract.
    - **Access**: Admin only.
    - **Effect**:
      - Disables `create_commitment`, `allocate`, `update_value`, `settle`, and `early_exit`.
      - Enables emergency-only functions.

2.  **`emergency_withdraw(asset: Address, to: Address, amount: i128)`**:
    - **Description**: Allows the admin to withdraw funds from the contract to a safe address.
    - **Access**: Admin only + Emergency mode must be ON.
    - **Use case**: Rescuing funds during a hack or if a critical bug is found.

3.  **`emergency_settle(commitment_id: String)`**:
    - **Description**: Force settles a specific commitment, returning funds to the owner.
    - **Access**: Admin only + Emergency mode must be ON.
    - **Use case**: Releasing individual commitments that are stuck or if the protocol needs to be wound down.

4.  **`emergency_update_commitment(...)`**:
    - **Description**: Allows the admin to manually adjust the state of a commitment.
    - **Access**: Admin only + Emergency mode must be ON.
    - **Use case**: Fixing state corruption or adjusting parameters during recovery.

## Recovery Procedures

In the event of an emergency:

1.  **Activate Emergency Mode**: Call `set_emergency_mode(true)` immediately to pause all protocol activity.
2.  **Assess Situation**: Identify the cause of the emergency (hack, bug, etc.).
3.  **Secure Funds**: If necessary, use `emergency_withdraw` to move assets to a multi-sig or cold storage.
4.  **Resolve Issue**: Develop and deploy a fix or a new version of the contract.
5.  **Restore State**: Use `emergency_update_commitment` or `emergency_settle` to restore the state for users.
6.  **Deactivate Emergency Mode**: Call `set_emergency_mode(false)` once the situation is resolved and it is safe to resume operations.

## Contact Information

For critical security issues, please contact:

- Security Team: security@commitlabs.com
- Lead Engineer: engineering-leads@commitlabs.com

## Multi-sig & Timelocks

It is highly recommended that the `Admin` address be a Multi-sig contract with a timelock for critical actions like `set_emergency_mode(false)` and `emergency_withdraw`.
