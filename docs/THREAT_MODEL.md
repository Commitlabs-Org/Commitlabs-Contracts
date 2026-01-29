# Threat Model

## Assets
- User funds locked in commitment_core.
- NFT ownership and metadata in commitment_nft.
- Attestation integrity and verifier authorization in attestation_engine.
- Allocation records and pool liquidity totals in allocation_logic.

## Actors
- Commitment owners (users).
- Protocol admins.
- Authorized verifiers.
- External token contract (asset transfers).
- Potential attackers (malicious users or compromised keys).

## Trust boundaries
- Cross-contract calls between commitment_core, commitment_nft, and attestation_engine.
- Token contract transfer operations.
- Admin-managed access control and verifier lists.

## Entry points
- commitment_core: create_commitment, settle, early_exit, allocate, update_value.
- commitment_nft: mint, transfer, settle.
- attestation_engine: attest, record_fees, record_drawdown.
- allocation_logic: register_pool, allocate, rebalance.

## Threats and mitigations

### Access control bypass
- **Threat:** Unauthorized caller invokes privileged functionality.
- **Mitigations:** Admin checks in allocation_logic and attestation_engine; transfer auth in commitment_nft.
- **Gaps:** commitment_core and commitment_nft mint/settle lack auth checks (see Known Limitations).

### Reentrancy
- **Threat:** Reentrant calls during external interactions.
- **Mitigations:** Reentrancy guards and checks-effects-interactions patterns.
- **Audit focus:** Guard cleared on every path and external calls only after state updates.

### Arithmetic overflow/underflow
- **Threat:** Overflow leading to incorrect accounting.
- **Mitigations:** overflow-checks enabled; checked arithmetic in SafeMath and allocation_logic.
- **Audit focus:** Remaining unchecked arithmetic in contracts and conversion of percent/amount values.

### Input validation failures
- **Threat:** Invalid params result in inconsistent state.
- **Mitigations:** Validation module, explicit checks in contracts.
- **Audit focus:** Ensure all externally accessible entry points validate parameters.

### Cross-contract call failures
- **Threat:** Inconsistent state if external contract calls fail.
- **Mitigations:** Checks-effects-interactions; transaction rollback on failure.
- **Audit focus:** Ensure stored state is consistent if external calls revert.

### Storage growth/DoS
- **Threat:** Unbounded vector growth may cause storage bloat or high gas costs.
- **Mitigations:** None currently.
- **Audit focus:** Evaluate pagination or caps for vectors like attestations or owner lists.

### Oracle/attestation manipulation
- **Threat:** Malicious verifiers manipulate compliance score.
- **Mitigations:** Verifier whitelist.
- **Audit focus:** Multi-signer or quorum requirements if needed.

## Residual risks
- Any missing auth checks or placeholder implementations can cause integrity issues.
- Known limitations list includes fields that must be resolved before audit sign-off.
