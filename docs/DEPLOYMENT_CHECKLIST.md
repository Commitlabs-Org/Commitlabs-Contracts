# Deployment Checklist & Operational Assumptions

This document outlines critical operational and security assumptions required when deploying and integrating the `commitment_nft` and associated contracts.

## Single Deployer Assumption

The Commitment NFT enforces the **Single Deployer Assumption** during its initialization sequence. This means:
1. The contract is deployed with no intrinsic initial `admin` address inside the compiled WASM.
2. The `initialize(env, admin)` function is the only way to lock the `admin` slot. 
3. **Critical Vulnerability if Unchecked:** Since `initialize` is an open endpoint, if it is invoked without authentication *in a separate transaction*, an attacker monitoring the network can front-run the deployment and assume the `admin` role by calling `initialize` before the legitimate deployer does.

To remediate this, `initialize()` enforces `admin.require_auth()`. This means the provided `admin` account **must sign** the envelope that invokes the initialization call. 

## Operational Deployment Checklist

When deploying this system into a production or live-testnet environment, operations must adhere to this checklist:

### Stage 1: Contract Deployment
- [ ] Determine the account address that will serve as `admin`.
- [ ] Build the WASM and deploy it onto the network.
- [ ] Record the resulting Contract ID.

### Stage 2: Synchronous Initialization
- [ ] Ensure `initialize(env, admin)` is called in the exact same transaction bundle (Host invocation) as the deployment operation OR immediately thereafter in a transaction signed by the `admin`.
- [ ] Confirm no malicious actor has front-run the `initialize` endpoint via checking the `admin` state. (Any unauthorized initialization attempt will fail because `admin.require_auth()` will reject signatures not matching the specified `admin`).

### Stage 3: Feature Flags & Permissions
The `admin` must properly authorize other contracts before the system will allow minting / core protocol functionality.
- [ ] Unpause the contract using `unpause()` (if deployed paused).
- [ ] Explicitly map the `CoreContract` interface via `set_core_contract(core_address)`.
- [ ] Add any authorized minters/factory wrappers via `add_authorized_contract(minter_address)`.

**Warning**: Do not ignore `admin.require_auth()` failures. If your initialization transaction fails because the admin signature is missing, do not bypass it. Provide the required Soroban auth envelope.
