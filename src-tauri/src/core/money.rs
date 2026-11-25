//! Money helpers shared by every service that touches the payment ledger (DD-01, DD-15).

use crate::core::value_objects::{PaymentKind, TransactionType};

/// Tolerance for comparing two already-rounded money amounts (half a paisa).
pub const MONEY_EPS: f64 = 0.005;

/// Rounds a currency amount to 2 decimal places (DD-15).
pub fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Which way a ledger row moves physical cash (DD-01 cash direction table).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashDirection {
    In,
    Out,
    None,
}

/// Cash direction derived from `(transaction_type, kind)`:
/// in = SALE·UPFRONT/SETTLEMENT and PURCHASE·REFUND_IN; out = PURCHASE·UPFRONT/SETTLEMENT and
/// SALE·REFUND_OUT; everything else (including every RETURN_CREDIT) is non-cash.
pub fn cash_direction(tx_type: TransactionType, kind: PaymentKind) -> CashDirection {
    use PaymentKind::*;
    use TransactionType::*;
    match (tx_type, kind) {
        (Sale, Upfront | Settlement) | (Purchase, RefundIn) => CashDirection::In,
        (Purchase, Upfront | Settlement) | (Sale, RefundOut) => CashDirection::Out,
        _ => CashDirection::None,
    }
}

/// Validates a `YYYY-MM-DD` date string for the named payload field.
pub fn check_date(field: &str, date: &str) -> Result<(), crate::core::error::AppError> {
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").map(|_| ()).map_err(|_| {
        crate::core::error::AppError::ValidationError {
            field: field.to_string(),
            message: format!("{field} must be a valid YYYY-MM-DD date, got '{date}'"),
        }
    })
}

/// Validates an entry date (FR-28, DD-58): a valid `YYYY-MM-DD` that is not after `today`
/// (ISO dates compare lexically). Report filters keep using `check_date`.
pub fn check_entry_date(field: &str, date: &str, today: &str) -> Result<(), crate::core::error::AppError> {
    check_date(field, date)?;
    if date > today {
        return Err(crate::core::error::AppError::ValidationError {
            field: field.to_string(),
            message: "Date cannot be in the future".to_string(),
        });
    }
    Ok(())
}

/// `check_entry_date` against the service clock (`core::clock::today_local`).
pub fn check_entry_date_now(field: &str, date: &str) -> Result<(), crate::core::error::AppError> {
    check_entry_date(field, date, &crate::core::clock::today_local())
}
