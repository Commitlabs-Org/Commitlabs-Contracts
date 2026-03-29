# Commitment Transformation Authorization Matrix

This note documents the production-facing authorization model for `commitment_transformation`.

## Roles

| Role                   | Source of truth                                       | Meaning                                                               |
| ---------------------- | ----------------------------------------------------- | --------------------------------------------------------------------- |
| Commitment owner       | `commitment_core.get_commitment(commitment_id).owner` | Beneficial owner of the underlying commitment.                        |
| Admin                  | `DataKey::Admin` in `commitment_transformation`       | Configuration authority for fees and role assignment.                 |
| Authorized transformer | `DataKey::AuthorizedTransformer(address)`             | Protocol executor allowed to perform privileged transformation flows. |

## Access Matrix

| Function group                | Owner | Admin | Authorized transformer |
| ----------------------------- | ----- | ----- | ---------------------- |
| `set_transformation_fee`      | No    | Yes   | No                     |
| `set_authorized_transformer`  | No    | Yes   | No                     |
| `set_fee_recipient`           | No    | Yes   | No                     |
| `withdraw_fees`               | No    | Yes   | No                     |
| `create_tranches`             | Yes   | Yes   | Yes                    |
| `collateralize`               | Yes   | Yes   | Yes                    |
| `create_secondary_instrument` | Yes   | Yes   | Yes                    |
| `add_protocol_guarantee`      | No    | Yes   | Yes                    |
| View getters                  | Yes   | Yes   | Yes                    |

## Trust Boundaries

- `commitment_transformation` treats the configured `CoreContract` address as the canonical source of commitment ownership.
- Owner-bound writes read `commitment_core.get_commitment(commitment_id)` before mutating storage.
- Protocol actors may execute owner-bound transformations for operational workflows, but derived records store the canonical commitment owner, not the executor.
- `add_protocol_guarantee` is intentionally protocol-only because it expresses protocol-backed terms rather than owner-authored metadata.

## Storage Mutations By Operation

| Operation                     | Storage keys mutated                                                                                                                  |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| `set_transformation_fee`      | `TransformationFeeBps`                                                                                                                |
| `set_authorized_transformer`  | `AuthorizedTransformer(address)`                                                                                                      |
| `create_tranches`             | `ReentrancyGuard`, `CollectedFees(asset)` when fee > 0, `TrancheSetCounter`, `TrancheSet(id)`, `CommitmentTrancheSets(commitment_id)` |
| `collateralize`               | `ReentrancyGuard`, `TrancheSetCounter`, `CollateralizedAsset(id)`, `CommitmentCollateral(commitment_id)`                              |
| `create_secondary_instrument` | `ReentrancyGuard`, `TrancheSetCounter`, `SecondaryInstrument(id)`, `CommitmentInstruments(commitment_id)`                             |
| `add_protocol_guarantee`      | `ReentrancyGuard`, `TrancheSetCounter`, `ProtocolGuarantee(id)`, `CommitmentGuarantees(commitment_id)`                                |
| `set_fee_recipient`           | `FeeRecipient`                                                                                                                        |
| `withdraw_fees`               | `CollectedFees(asset)`                                                                                                                |

## Cross-Contract Call Notes

- `commitment_core.get_commitment(commitment_id)` is the only cross-contract call used for authorization and existence checks.
- This call is read-only and happens before the transformation contract sets its reentrancy guard or mutates storage.
- `create_tranches` may also call a token contract to collect a configured fee from the executor.

## Arithmetic Assumptions

- Transformation fees use `shared_utils::fees::fee_from_bps(total_value, fee_bps)`.
- Basis-point fee rounding is floor-rounded.
- Net trancheable value is computed as `total_value - fee_amount` with checked subtraction.
- Tranche ratios must sum to exactly `10000` basis points.
- Individual tranche amounts are computed with integer division, so per-tranche rounding may leave dust unassigned to any tranche.

## Security Notes

- Every state-changing entrypoint requires `caller.require_auth()` through the relevant role helper.
- Owner authorization is not inferred from the caller alone; it is checked against the current commitment record in core.
- Protocol guarantees are reserved to protocol roles to avoid user-created metadata that could be mistaken for protocol-backed protection.
