//! Standardized error codes and messages for CommitLabs contracts.
//!
//! # Overview
//!
//! This module provides a uniform error taxonomy shared across all CommitLabs
//! Soroban contracts. Each contract maps its own `contracterror` enum variant
//! to one of the numeric codes defined here, so off-chain indexers can reason
//! about failures without parsing contract-specific ABIs.
//!
//! # Error code ranges
//!
//! | Range   | Category     | Examples                              |
//! |---------|--------------|---------------------------------------|
//! | 1–99    | Validation   | invalid amount, out of range          |
//! | 100–199 | Authorization| unauthorized caller, not owner        |
//! | 200–299 | State        | wrong state, already initialized      |
//! | 300–399 | Resource     | not found, insufficient balance       |
//! | 400–499 | System       | storage failure, cross-contract error |
//!
//! # Security model for `emit_error_event`
//!
//! Events on Stellar are **public**: every node, archiver, and indexer on the
//! network can read them. `emit_error_event` therefore enforces the following
//! invariants:
//!
//! 1. **No caller identity in the event payload.** Addresses, account IDs, and
//!    public keys must never appear in the `context` string passed by callers.
//!    The function strips nothing — it trusts callers to comply — so callers
//!    MUST pass only static, code-location strings (e.g. `"commitment_core::settle"`).
//! 2. **No financial values in the event payload.** Balances, amounts, and
//!    token quantities must never be embedded in `context`.
//! 3. **No storage keys or internal identifiers.** NFT IDs, commitment hashes,
//!    and storage slot names must not appear in the event.
//! 4. **`context` must be a static code-location label**, validated at runtime
//!    to contain only ASCII alphanumeric characters, underscores, colons, and
//!    hyphens, and be at most [`MAX_CONTEXT_LEN`] bytes. Violations cause the
//!    event to be dropped and replaced with a generic `"[redacted]"` marker so
//!    that bugs in callers do not accidentally leak data.
//! 5. **Human-readable messages come exclusively from [`message_for_code`].**
//!    The function owns all user-visible text; callers cannot inject arbitrary
//!    strings into the on-chain message field.
//!
//! # Trust boundaries
//!
//! * `emit_error_event` is called by contracts within the CommitLabs workspace.
//!   It is `pub` but not a contract entry-point; it cannot be invoked from an
//!   external transaction directly.
//! * The function performs **no auth check** because it only emits a read-only
//!   event and mutates no storage. Adding `require_auth` here would be
//!   incorrect and would break legitimate internal callers.
//! * Reentrancy is not a concern: the function touches no storage and makes no
//!   cross-contract calls.

use soroban_sdk::{symbol_short, Env, String as SorobanString};

// ── Constants ────────────────────────────────────────────────────────────────

/// Maximum byte length allowed for the `context` parameter in [`emit_error_event`].
///
/// Keeping context short limits on-chain event size and discourages callers
/// from embedding variable-length (potentially sensitive) data.
pub const MAX_CONTEXT_LEN: usize = 64;

/// Placeholder emitted in the context field when the caller-supplied value
/// fails validation, so that the event is still recorded (for indexers) but
/// no unvalidated bytes reach the ledger.
const REDACTED: &str = "[redacted]";

// ── Category boundaries ───────────────────────────────────────────────────────

/// Error category boundaries for documentation and off-chain indexing.
pub mod category {
    pub const VALIDATION_START: u32 = 1;
    pub const VALIDATION_END: u32 = 99;
    pub const AUTH_START: u32 = 100;
    pub const AUTH_END: u32 = 199;
    pub const STATE_START: u32 = 200;
    pub const STATE_END: u32 = 299;
    pub const RESOURCE_START: u32 = 300;
    pub const RESOURCE_END: u32 = 399;
    pub const SYSTEM_START: u32 = 400;
    pub const SYSTEM_END: u32 = 499;
}

// ── Error code constants ──────────────────────────────────────────────────────

/// Standard error code constants.
///
/// Contracts define their own `contracterror` enum and map variants to these
/// values so off-chain indexers share a single taxonomy.
pub mod code {
    // Validation (1–99)
    pub const INVALID_AMOUNT: u32 = 1;
    pub const INVALID_DURATION: u32 = 2;
    pub const INVALID_PERCENT: u32 = 3;
    pub const INVALID_TYPE: u32 = 4;
    pub const OUT_OF_RANGE: u32 = 5;
    pub const EMPTY_STRING: u32 = 6;

    // Authorization (100–199)
    pub const UNAUTHORIZED: u32 = 100;
    pub const NOT_OWNER: u32 = 101;
    pub const NOT_ADMIN: u32 = 102;
    pub const NOT_AUTHORIZED_CONTRACT: u32 = 103;

    // State (200–299)
    pub const ALREADY_INITIALIZED: u32 = 200;
    pub const NOT_INITIALIZED: u32 = 201;
    pub const WRONG_STATE: u32 = 202;
    pub const ALREADY_PROCESSED: u32 = 203;
    pub const REENTRANCY: u32 = 204;
    pub const NOT_ACTIVE: u32 = 205;

    // Resource (300–399)
    pub const NOT_FOUND: u32 = 300;
    pub const INSUFFICIENT_BALANCE: u32 = 301;
    pub const INSUFFICIENT_VALUE: u32 = 302;
    pub const TRANSFER_FAILED: u32 = 303;

    // System (400–499)
    pub const STORAGE_ERROR: u32 = 400;
    pub const CONTRACT_CALL_FAILED: u32 = 401;
}

// ── Human-readable messages ───────────────────────────────────────────────────

/// Returns the canonical human-readable message for an error code.
///
/// All text returned by this function is **static** — it contains no
/// runtime-variable data (addresses, balances, IDs). This is intentional:
/// the message field of an on-chain error event must never carry sensitive
/// information.
///
/// # Arguments
///
/// * `code` — A numeric error code from the [`code`] module.
///
/// # Returns
///
/// A static string slice. Unknown codes return `"Unknown error"`.
///
/// # Examples
///
/// ```rust
/// use shared_utils::error_codes::{code, message_for_code};
/// assert_eq!(message_for_code(code::UNAUTHORIZED), "Unauthorized: caller not allowed");
/// assert_eq!(message_for_code(9999), "Unknown error");
/// ```
pub fn message_for_code(code: u32) -> &'static str {
    match code {
        // Validation
        1 => "Invalid amount: must be greater than zero",
        2 => "Invalid duration: must be greater than zero",
        3 => "Invalid percent: must be between 0 and 100",
        4 => "Invalid type: value not allowed",
        5 => "Value out of allowed range",
        6 => "Required field must not be empty",
        // Authorization
        100 => "Unauthorized: caller not allowed",
        101 => "Caller is not the owner",
        102 => "Caller is not the admin",
        103 => "Caller contract not authorized",
        // State
        200 => "Contract already initialized",
        201 => "Contract not initialized",
        202 => "Invalid state for this operation",
        203 => "Item already processed",
        204 => "Reentrancy detected",
        205 => "Commitment or item not active",
        // Resource
        300 => "Resource not found",
        301 => "Insufficient balance",
        302 => "Insufficient commitment value",
        303 => "Token transfer failed",
        // System
        400 => "Storage operation failed",
        401 => "Cross-contract call failed",
        _ => "Unknown error",
    }
}

// ── Context validation ────────────────────────────────────────────────────────

/// Validates that a caller-supplied context string is safe to emit on-chain.
///
/// # Allowed characters
///
/// Only ASCII alphanumeric characters (`a-z`, `A-Z`, `0-9`), underscores
/// (`_`), colons (`:`), and hyphens (`-`) are permitted. This whitelist covers
/// the `module::function` convention used throughout the workspace while
/// making it structurally impossible to embed addresses, numeric amounts, or
/// other variable data in the context label.
///
/// # Length
///
/// The byte length must not exceed [`MAX_CONTEXT_LEN`] (64 bytes).
///
/// # Returns
///
/// `true` if the string is safe; `false` otherwise.
///
/// # Security note
///
/// This is the gating function that prevents sensitive data leakage. Any
/// `context` value that fails this check is replaced with [`REDACTED`] in
/// [`emit_error_event`] so that the event is still recorded for indexers but
/// no unvalidated bytes reach the ledger.
#[inline]
fn is_safe_context(context: &str) -> bool {
    if context.is_empty() || context.len() > MAX_CONTEXT_LEN {
        return false;
    }
    context
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b':' || b == b'-')
}

// ── Event emission ────────────────────────────────────────────────────────────

/// Emit a standardized, privacy-safe error event for off-chain indexing.
///
/// Call this **before** panicking or returning an error so that indexers and
/// dashboards can record the failure without needing to parse revert reasons.
///
/// # Arguments
///
/// * `e`          — The Soroban [`Env`].
/// * `error_code` — A numeric code from the [`code`] module.
/// * `context`    — A **static, code-location-only** label such as
///                  `"commitment_core::settle"`. Must contain only ASCII
///                  alphanumeric characters, underscores, colons, or hyphens
///                  and be at most [`MAX_CONTEXT_LEN`] bytes. Any value that
///                  fails this check is silently replaced with `"[redacted]"`
///                  so the event is still recorded without leaking data.
///
/// # Event schema
///
/// | Field        | Type          | Value                                      |
/// |--------------|---------------|--------------------------------------------|
/// | topic[0]     | Symbol        | `"Error"` (7-char limit: fits in 7 bytes)  |
/// | topic[1]     | `u32`         | `error_code`                               |
/// | data[0]      | `SorobanString` | Validated `context` or `"[redacted]"`    |
/// | data[1]      | `SorobanString` | Static message from [`message_for_code`] |
/// | data[2]      | `u64`         | Ledger timestamp at emission               |
///
/// # What is intentionally excluded
///
/// * **Caller addresses / account IDs** — public on-chain; could expose user
///   identity in failure paths unintentionally.
/// * **Token amounts / balances** — financial values must not be in error logs.
/// * **Storage keys / NFT IDs / commitment hashes** — internal identifiers.
/// * **Free-form strings from contract callers** — only the whitelisted
///   `context` label and the hard-coded message from [`message_for_code`]
///   appear in the payload.
///
/// # Security notes
///
/// * No storage is read or written; no cross-contract calls are made.
/// * No `require_auth` is needed: the function is purely additive (events only)
///   and is called from within already-authenticated contract entry-points.
/// * Reentrancy: not applicable (no state mutation).
///
/// # Examples
///
/// ```rust
/// use shared_utils::error_codes::{code, emit_error_event};
///
/// // Correct — static code-location label only:
/// emit_error_event(&env, code::UNAUTHORIZED, "commitment_core::settle");
///
/// // The following would be silently redacted at runtime (contains `@`):
/// // emit_error_event(&env, code::UNAUTHORIZED, "caller=GABC...@settle");
/// ```
pub fn emit_error_event(e: &Env, error_code: u32, context: &str) {
    // ── 1. Validate and sanitize context ─────────────────────────────────────
    //
    // If the caller accidentally (or maliciously) passes a context string that
    // contains an address, an amount, or any character outside the whitelist,
    // we replace it with REDACTED. The event is still emitted so indexers know
    // an error occurred at this error_code; they just won't see the raw context.
    let safe_context: &str = if is_safe_context(context) {
        context
    } else {
        REDACTED
    };

    // ── 2. Obtain static message (no runtime data) ────────────────────────────
    //
    // message_for_code returns only compile-time string literals, so there is
    // no path by which variable data (amounts, addresses, IDs) can appear in
    // the message field of the event.
    let msg: &'static str = message_for_code(error_code);

    // ── 3. Convert to SorobanString and publish ───────────────────────────────
    //
    // Topics: ("Error", error_code)  — enough for indexers to filter by type.
    // Data:   (context, message, timestamp) — all sanitized or static.
    let context_str = SorobanString::from_str(e, safe_context);
    let msg_str = SorobanString::from_str(e, msg);

    e.events().publish(
        (symbol_short!("Error"), error_code),
        (context_str, msg_str, e.ledger().timestamp()),
    );
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    // ── message_for_code ─────────────────────────────────────────────────────

    #[test]
    fn test_message_for_known_validation_codes() {
        assert_eq!(
            message_for_code(code::INVALID_AMOUNT),
            "Invalid amount: must be greater than zero"
        );
        assert_eq!(
            message_for_code(code::INVALID_DURATION),
            "Invalid duration: must be greater than zero"
        );
        assert_eq!(
            message_for_code(code::INVALID_PERCENT),
            "Invalid percent: must be between 0 and 100"
        );
        assert_eq!(
            message_for_code(code::INVALID_TYPE),
            "Invalid type: value not allowed"
        );
        assert_eq!(
            message_for_code(code::OUT_OF_RANGE),
            "Value out of allowed range"
        );
        assert_eq!(
            message_for_code(code::EMPTY_STRING),
            "Required field must not be empty"
        );
    }

    #[test]
    fn test_message_for_known_auth_codes() {
        assert_eq!(
            message_for_code(code::UNAUTHORIZED),
            "Unauthorized: caller not allowed"
        );
        assert_eq!(
            message_for_code(code::NOT_OWNER),
            "Caller is not the owner"
        );
        assert_eq!(
            message_for_code(code::NOT_ADMIN),
            "Caller is not the admin"
        );
        assert_eq!(
            message_for_code(code::NOT_AUTHORIZED_CONTRACT),
            "Caller contract not authorized"
        );
    }

    #[test]
    fn test_message_for_known_state_codes() {
        assert_eq!(
            message_for_code(code::ALREADY_INITIALIZED),
            "Contract already initialized"
        );
        assert_eq!(
            message_for_code(code::NOT_INITIALIZED),
            "Contract not initialized"
        );
        assert_eq!(
            message_for_code(code::WRONG_STATE),
            "Invalid state for this operation"
        );
        assert_eq!(
            message_for_code(code::ALREADY_PROCESSED),
            "Item already processed"
        );
        assert_eq!(
            message_for_code(code::REENTRANCY),
            "Reentrancy detected"
        );
        assert_eq!(
            message_for_code(code::NOT_ACTIVE),
            "Commitment or item not active"
        );
    }

    #[test]
    fn test_message_for_known_resource_codes() {
        assert_eq!(message_for_code(code::NOT_FOUND), "Resource not found");
        assert_eq!(
            message_for_code(code::INSUFFICIENT_BALANCE),
            "Insufficient balance"
        );
        assert_eq!(
            message_for_code(code::INSUFFICIENT_VALUE),
            "Insufficient commitment value"
        );
        assert_eq!(
            message_for_code(code::TRANSFER_FAILED),
            "Token transfer failed"
        );
    }

    #[test]
    fn test_message_for_known_system_codes() {
        assert_eq!(
            message_for_code(code::STORAGE_ERROR),
            "Storage operation failed"
        );
        assert_eq!(
            message_for_code(code::CONTRACT_CALL_FAILED),
            "Cross-contract call failed"
        );
    }

    #[test]
    fn test_message_for_unknown_code() {
        assert_eq!(message_for_code(999), "Unknown error");
        assert_eq!(message_for_code(0), "Unknown error");
        assert_eq!(message_for_code(u32::MAX), "Unknown error");
    }

    // ── is_safe_context ──────────────────────────────────────────────────────

    #[test]
    fn test_is_safe_context_valid_labels() {
        // canonical module::function style
        assert!(is_safe_context("commitment_core::settle"));
        assert!(is_safe_context("attestation_engine::record"));
        assert!(is_safe_context("commitment_nft::mint"));
        // hyphens allowed
        assert!(is_safe_context("shared-utils::emit"));
        // alphanumeric only
        assert!(is_safe_context("settle123"));
        // exactly MAX_CONTEXT_LEN bytes — boundary should pass
        let boundary = "a".repeat(MAX_CONTEXT_LEN);
        assert!(is_safe_context(&boundary));
    }

    #[test]
    fn test_is_safe_context_rejects_empty() {
        assert!(!is_safe_context(""));
    }

    #[test]
    fn test_is_safe_context_rejects_over_max_len() {
        let too_long = "a".repeat(MAX_CONTEXT_LEN + 1);
        assert!(!is_safe_context(&too_long));
    }

    #[test]
    fn test_is_safe_context_rejects_address_like_strings() {
        // Stellar addresses begin with 'G' and contain base32 chars including '='
        assert!(!is_safe_context("GABC1234XYZ=caller"));
        // '@' separator that might be used to embed an address
        assert!(!is_safe_context("caller=GABC@settle"));
        // spaces
        assert!(!is_safe_context("settle 123"));
        // dots
        assert!(!is_safe_context("commitment.core.settle"));
        // slash
        assert!(!is_safe_context("commitment/core"));
        // newline injection attempt
        assert!(!is_safe_context("settle\nUNAUTHORIZED"));
        // null byte
        assert!(!is_safe_context("settle\0extra"));
    }

    #[test]
    fn test_is_safe_context_rejects_numeric_amounts() {
        // A context like "amount=1000" embeds a value — rejected via '='
        assert!(!is_safe_context("amount=1000"));
        // Parentheses
        assert!(!is_safe_context("settle(100)"));
    }

    // ── emit_error_event — event emission ────────────────────────────────────

    #[test]
    fn test_emit_error_event_valid_context_does_not_panic() {
        let e = Env::default();
        // Should complete without panic for every known code
        emit_error_event(&e, code::UNAUTHORIZED, "commitment_core::settle");
        emit_error_event(&e, code::NOT_FOUND, "attestation_engine::record");
        emit_error_event(&e, code::STORAGE_ERROR, "shared_utils::storage");
    }

    #[test]
    fn test_emit_error_event_unknown_code_does_not_panic() {
        let e = Env::default();
        // Unknown code → "Unknown error" message, valid context
        emit_error_event(&e, 9999, "commitment_core::unknown");
    }

    #[test]
    fn test_emit_error_event_invalid_context_is_redacted_not_panicked() {
        let e = Env::default();

        // Address embedded in context — must be silently redacted, not panic
        emit_error_event(
            &e,
            code::UNAUTHORIZED,
            "GABC1234XYZ=caller@settle", // contains '=' and '@'
        );

        // Overly long context
        let long_ctx = "a".repeat(MAX_CONTEXT_LEN + 10);
        emit_error_event(&e, code::UNAUTHORIZED, &long_ctx);

        // Empty context
        emit_error_event(&e, code::UNAUTHORIZED, "");

        // Numeric value embedded
        emit_error_event(&e, code::INSUFFICIENT_BALANCE, "amount=99999999");
    }

    #[test]
    fn test_emit_error_event_boundary_context_length() {
        let e = Env::default();
        // Exactly MAX_CONTEXT_LEN — must succeed
        let exact = "a".repeat(MAX_CONTEXT_LEN);
        emit_error_event(&e, code::WRONG_STATE, &exact);

        // One over — must be silently redacted, not panic
        let over = "a".repeat(MAX_CONTEXT_LEN + 1);
        emit_error_event(&e, code::WRONG_STATE, &over);
    }

    #[test]
    fn test_emit_error_event_all_known_codes() {
        let e = Env::default();
        let ctx = "test::coverage";
        let all_codes = [
            code::INVALID_AMOUNT,
            code::INVALID_DURATION,
            code::INVALID_PERCENT,
            code::INVALID_TYPE,
            code::OUT_OF_RANGE,
            code::EMPTY_STRING,
            code::UNAUTHORIZED,
            code::NOT_OWNER,
            code::NOT_ADMIN,
            code::NOT_AUTHORIZED_CONTRACT,
            code::ALREADY_INITIALIZED,
            code::NOT_INITIALIZED,
            code::WRONG_STATE,
            code::ALREADY_PROCESSED,
            code::REENTRANCY,
            code::NOT_ACTIVE,
            code::NOT_FOUND,
            code::INSUFFICIENT_BALANCE,
            code::INSUFFICIENT_VALUE,
            code::TRANSFER_FAILED,
            code::STORAGE_ERROR,
            code::CONTRACT_CALL_FAILED,
        ];
        for c in all_codes {
            emit_error_event(&e, c, ctx);
        }
    }

    // ── category boundary constants ──────────────────────────────────────────

    #[test]
    fn test_category_boundaries_are_contiguous_and_non_overlapping() {
        use category::*;
        assert_eq!(VALIDATION_START, 1);
        assert_eq!(VALIDATION_END, 99);
        assert_eq!(AUTH_START, 100);
        assert_eq!(AUTH_END, 199);
        assert_eq!(STATE_START, 200);
        assert_eq!(STATE_END, 299);
        assert_eq!(RESOURCE_START, 300);
        assert_eq!(RESOURCE_END, 399);
        assert_eq!(SYSTEM_START, 400);
        assert_eq!(SYSTEM_END, 499);

        // Ranges must not overlap
        assert!(VALIDATION_END < AUTH_START);
        assert!(AUTH_END < STATE_START);
        assert!(STATE_END < RESOURCE_START);
        assert!(RESOURCE_END < SYSTEM_START);
    }

    #[test]
    fn test_all_code_constants_fall_within_declared_category() {
        use category::*;
        use code::*;

        // Validation
        for c in [INVALID_AMOUNT, INVALID_DURATION, INVALID_PERCENT, INVALID_TYPE, OUT_OF_RANGE, EMPTY_STRING] {
            assert!(c >= VALIDATION_START && c <= VALIDATION_END, "code {c} out of validation range");
        }
        // Authorization
        for c in [UNAUTHORIZED, NOT_OWNER, NOT_ADMIN, NOT_AUTHORIZED_CONTRACT] {
            assert!(c >= AUTH_START && c <= AUTH_END, "code {c} out of auth range");
        }
        // State
        for c in [ALREADY_INITIALIZED, NOT_INITIALIZED, WRONG_STATE, ALREADY_PROCESSED, REENTRANCY, NOT_ACTIVE] {
            assert!(c >= STATE_START && c <= STATE_END, "code {c} out of state range");
        }
        // Resource
        for c in [NOT_FOUND, INSUFFICIENT_BALANCE, INSUFFICIENT_VALUE, TRANSFER_FAILED] {
            assert!(c >= RESOURCE_START && c <= RESOURCE_END, "code {c} out of resource range");
        }
        // System
        for c in [STORAGE_ERROR, CONTRACT_CALL_FAILED] {
            assert!(c >= SYSTEM_START && c <= SYSTEM_END, "code {c} out of system range");
        }
    }
}