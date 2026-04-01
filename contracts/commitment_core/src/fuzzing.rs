//! No-std-friendly fuzz helper surfaces for `commitment_core`.
//!
//! These helpers are intentionally pure and allocation-free so a future host-side
//! fuzz target can feed arbitrary bytes and integers into them without needing a
//! Soroban `Env`. The contract tests in this crate use the same helpers as
//! deterministic seed cases.

use shared_utils::fees::{BPS_MAX, BPS_SCALE};

const GENERATED_ID_PREFIX: &[u8] = b"c_";
const MAX_COMMITMENT_ID_BYTES: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitmentIdShape {
    Empty,
    TooLong,
    InvalidPrefix,
    MissingDigits,
    NonDigitSuffix,
    ValidGenerated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AmountShape {
    NonPositive,
    InvalidFeeBps,
    FeeOverflow,
    NetUnderflow,
    Valid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AmountObservation {
    pub shape: AmountShape,
    pub fee: Option<i128>,
    pub net: Option<i128>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommitmentInputObservation {
    pub id_shape: CommitmentIdShape,
    pub amount: AmountObservation,
}

pub fn classify_generated_commitment_id_bytes(bytes: &[u8]) -> CommitmentIdShape {
    if bytes.is_empty() {
        return CommitmentIdShape::Empty;
    }

    if bytes.len() > MAX_COMMITMENT_ID_BYTES {
        return CommitmentIdShape::TooLong;
    }

    if !bytes.starts_with(GENERATED_ID_PREFIX) {
        return CommitmentIdShape::InvalidPrefix;
    }

    let suffix = &bytes[GENERATED_ID_PREFIX.len()..];
    if suffix.is_empty() {
        return CommitmentIdShape::MissingDigits;
    }

    if suffix.iter().all(u8::is_ascii_digit) {
        CommitmentIdShape::ValidGenerated
    } else {
        CommitmentIdShape::NonDigitSuffix
    }
}

pub fn checked_fee_from_bps(amount: i128, fee_bps: u32) -> Option<i128> {
    if fee_bps > BPS_MAX {
        return None;
    }

    amount
        .checked_mul(fee_bps as i128)?
        .checked_div(BPS_SCALE as i128)
}

pub fn observe_amount(amount: i128, fee_bps: u32) -> AmountObservation {
    if amount <= 0 {
        return AmountObservation {
            shape: AmountShape::NonPositive,
            fee: None,
            net: None,
        };
    }

    if fee_bps > BPS_MAX {
        return AmountObservation {
            shape: AmountShape::InvalidFeeBps,
            fee: None,
            net: None,
        };
    }

    let Some(fee) = checked_fee_from_bps(amount, fee_bps) else {
        return AmountObservation {
            shape: AmountShape::FeeOverflow,
            fee: None,
            net: None,
        };
    };

    let Some(net) = amount.checked_sub(fee) else {
        return AmountObservation {
            shape: AmountShape::NetUnderflow,
            fee: Some(fee),
            net: None,
        };
    };

    AmountObservation {
        shape: AmountShape::Valid,
        fee: Some(fee),
        net: Some(net),
    }
}

pub fn observe_commitment_input(commitment_id: &[u8], amount: i128, fee_bps: u32) -> CommitmentInputObservation {
    CommitmentInputObservation {
        id_shape: classify_generated_commitment_id_bytes(commitment_id),
        amount: observe_amount(amount, fee_bps),
    }
}
