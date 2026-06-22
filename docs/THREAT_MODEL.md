# Threat Model

## Assets
- User funds locked in commitment_core.
- NFT ownership and metadata in commitment_nft.
- Attestation integrity and verifier authorization in attestation_engine.
- Allocation records and pool liquidity totals in allocation_logic.
- Price integrity and freshness in price_oracle.

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
- Admin-managed oracle whitelist in price_oracle.

## Entry points
- commitment_core: create_commitment, settle, early_exit, allocate, update_value.
- commitment_nft: mint, transfer, settle.
- attestation_engine: attest, record_fees, record_drawdown.
- allocation_logic: register_pool, allocate, rebalance.
- price_oracle: add_oracle, remove_oracle, set_price, set_max_staleness, get_price_valid.

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

### Core / NFT / attestation call-graph drift
- **Threat:** `commitment_core`, `commitment_nft`, and `attestation_engine` diverge on caller expectations, lifecycle semantics, or ABI shape, causing broken state mirroring or misleading compliance outputs.
- **Mitigations:** Reentrancy guards, rollback-based atomicity assumptions, and canonical reads through `commitment_core::get_commitment`.
- **Audit focus:** Verify outbound core-to-NFT calls are atomic on failure, verify NFT lifecycle writes are restricted to intended callers, and verify attestation reads fail closed when core data is unavailable or malformed.
- **Reference:** `docs/CORE_NFT_ATTESTATION_THREAT_REVIEW.md`

### Storage growth/DoS
- **Threat:** Unbounded vector growth may cause storage bloat or high gas costs.
- **Mitigations:** None currently.
- **Audit focus:** Evaluate pagination or caps for vectors like attestations or owner lists.

### Oracle/attestation manipulation
- **Threat:** Malicious verifiers manipulate compliance score.
- **Mitigations:** Verifier whitelist.
- **Audit focus:** Multi-signer or quorum requirements if needed.

### Price oracle manipulation resistance assumptions
- **Threat:** A compromised or malicious whitelisted oracle publishes a manipulated price, or a delayed price remains usable long enough to distort downstream settlement or accounting.
- **Mitigations:** Admin-managed oracle whitelist, `require_auth` on oracle/admin paths, non-negative price validation, `get_price_valid` freshness checks that reject stale and future-dated prices, and a bounded recent-sample median path for high-value reads.
- **Assumptions:** `price_oracle` remains a trusted-publisher registry for `get_price` and `get_price_valid`; those low-level APIs intentionally return the latest accepted write. High-value consumers should use `get_price_high_value`, which requires at least three fresh positive samples and returns the median to reduce single-update manipulation risk. This is not a quorum over unique oracle identities and does not replace off-chain source governance or circuit breakers.
- **Integrator responsibility:** Consumers must call `get_price_valid` with an appropriate staleness bound for the asset and use case; `get_price` is a raw read helper and does not enforce freshness.
- **Freshness boundary matrix:** `get_price_valid` treats `current_time - updated_at < max_staleness_seconds` as fresh, accepts the exact boundary (`age == max_staleness_seconds`), rejects one second past the boundary (`age == max_staleness_seconds + 1`) with `StalePrice`, and rejects future-dated prices where `updated_at > current_time` with `StalePrice`.
- **Legacy config compatibility:** When the structured `OracleConfig` key is absent, `read_config` falls back to the legacy `MaxStalenessSeconds` key and applies the same freshness-boundary matrix. Migration tests should continue to verify this fallback until all deployed legacy instances have been upgraded.
- **Batch behavior:** `get_batch_prices` fails closed: a single stale asset in a batch returns `StalePrice` for the full batch rather than returning partial results.
- **Audit focus:** Whether single-source trust is acceptable for each asset, whether admin key management around whitelist updates is strong enough, and whether downstream contracts choose staleness windows that match liquidation/settlement risk.

## Residual risks
- Any missing auth checks or placeholder implementations can cause integrity issues.
- Known limitations list includes fields that must be resolved before audit sign-off.
