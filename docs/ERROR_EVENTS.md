# Error Events — `shared_utils`

> **Integrator-facing reference** for the `emit_error_event` API in
> `contracts/shared_utils/src/error_codes.rs`.

---

## Purpose

Every CommitLabs contract calls `emit_error_event` immediately before
returning an error or panicking. This gives off-chain indexers and dashboards a
structured, queryable signal for every contract failure **without exposing
sensitive runtime data** on the public Stellar ledger.

---

## Event Schema

```
Topics : ("Error" : Symbol, error_code : u32)
Data   : (context : String, message : String, timestamp : u64)
```

| Field       | Type     | Description                                                                 |
|-------------|----------|-----------------------------------------------------------------------------|
| `"Error"`   | `Symbol` | Fixed discriminant; use to filter all CommitLabs error events.              |
| `error_code`| `u32`    | Numeric code from the table below. Stable across deployments.               |
| `context`   | `String` | Static code-location label, e.g. `"commitment_core::settle"`. See §Safety. |
| `message`   | `String` | Human-readable description from `message_for_code`. Always static text.    |
| `timestamp` | `u64`    | Ledger UNIX timestamp at the moment of emission.                            |

---

## Error Code Reference

### Validation (1–99)

| Code | Constant           | Message                                    |
|------|--------------------|--------------------------------------------|
| 1    | `INVALID_AMOUNT`   | Invalid amount: must be greater than zero  |
| 2    | `INVALID_DURATION` | Invalid duration: must be greater than zero|
| 3    | `INVALID_PERCENT`  | Invalid percent: must be between 0 and 100 |
| 4    | `INVALID_TYPE`     | Invalid type: value not allowed            |
| 5    | `OUT_OF_RANGE`     | Value out of allowed range                 |
| 6    | `EMPTY_STRING`     | Required field must not be empty           |

### Authorization (100–199)

| Code | Constant                  | Message                          |
|------|---------------------------|----------------------------------|
| 100  | `UNAUTHORIZED`            | Unauthorized: caller not allowed |
| 101  | `NOT_OWNER`               | Caller is not the owner          |
| 102  | `NOT_ADMIN`               | Caller is not the admin          |
| 103  | `NOT_AUTHORIZED_CONTRACT` | Caller contract not authorized   |

### State (200–299)

| Code | Constant              | Message                             |
|------|-----------------------|-------------------------------------|
| 200  | `ALREADY_INITIALIZED` | Contract already initialized        |
| 201  | `NOT_INITIALIZED`     | Contract not initialized            |
| 202  | `WRONG_STATE`         | Invalid state for this operation    |
| 203  | `ALREADY_PROCESSED`   | Item already processed              |
| 204  | `REENTRANCY`          | Reentrancy detected                 |
| 205  | `NOT_ACTIVE`          | Commitment or item not active       |

### Resource (300–399)

| Code | Constant               | Message                       |
|------|------------------------|-------------------------------|
| 300  | `NOT_FOUND`            | Resource not found            |
| 301  | `INSUFFICIENT_BALANCE` | Insufficient balance          |
| 302  | `INSUFFICIENT_VALUE`   | Insufficient commitment value |
| 303  | `TRANSFER_FAILED`      | Token transfer failed         |

### System (400–499)

| Code | Constant               | Message                      |
|------|------------------------|------------------------------|
| 400  | `STORAGE_ERROR`        | Storage operation failed     |
| 401  | `CONTRACT_CALL_FAILED` | Cross-contract call failed   |

---

## Safety Contract for Callers

`emit_error_event` enforces a strict **context validation** step before
publishing anything to the ledger. Because Stellar events are public and
permanent, the following rules are **mandatory** for all callers:

### ✅ Allowed in `context`

- ASCII letters (`a-z`, `A-Z`)
- ASCII digits (`0-9`)
- Underscores (`_`)
- Colons (`:`)
- Hyphens (`-`)
- Maximum **64 bytes** total

The canonical format is `"module::function"`, e.g.:

```rust
emit_error_event(&env, code::UNAUTHORIZED, "commitment_core::settle");
emit_error_event(&env, code::NOT_FOUND,    "attestation_engine::record");
emit_error_event(&env, code::WRONG_STATE,  "commitment_nft::burn");
```

### ❌ Never allowed in `context`

| Forbidden data              | Reason                                          |
|-----------------------------|-------------------------------------------------|
| Caller addresses / pub keys | Permanent public identity leak                  |
| Token amounts / balances    | Financial data must not appear in error logs    |
| NFT / commitment IDs        | Internal identifiers enable correlation attacks |
| Storage key names           | Reveals internal architecture                   |
| Free-form user strings      | Uncontrolled, may contain any of the above      |

### What happens on violation

If a `context` value fails validation (forbidden character, empty, or over 64
bytes), the function **silently replaces it with `"[redacted]"`** and still
emits the event. This means:

- Indexers still see that an error of `error_code` occurred.
- No unvalidated bytes reach the ledger.
- The contract does **not** panic — a bad context label is a caller bug, not a
  reason to abort the transaction.

---

## Trust Boundaries

| Boundary                  | Detail                                                                   |
|---------------------------|--------------------------------------------------------------------------|
| Who can call              | Any contract in the CommitLabs workspace (not an external entry-point).  |
| Auth required             | None — the function is purely additive (events, no storage mutation).    |
| Cross-contract calls      | None.                                                                    |
| Storage mutation          | None.                                                                    |
| Reentrancy risk           | None.                                                                    |
| Arithmetic risk           | None (no arithmetic performed).                                          |

---

## Indexing Example (off-chain)

```typescript
// Filter all CommitLabs error events from a Horizon/RPC stream
const isErrorEvent = (event) =>
  event.topics[0] === "Error" &&
  typeof event.topics[1] === "number";

// Decode
const errorCode = event.topics[1];          // u32
const context   = event.data[0];            // "commitment_core::settle"
const message   = event.data[1];            // "Unauthorized: caller not allowed"
const timestamp = event.data[2];            // Unix seconds
```

---

## Changelog

| Version | Change                                                                   |
|---------|--------------------------------------------------------------------------|
| 0.2.0   | Added `is_safe_context` whitelist validation; introduced `MAX_CONTEXT_LEN` (64); unsafe context now emits `[redacted]` instead of raw string. |
| 0.1.0   | Initial `emit_error_event` with no context validation.                   |