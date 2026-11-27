//! Money helpers shared by every service that touches the payment ledger (DD-01, DD-15).

use crate::core::value_objects::{PaymentKind, TransactionType};

/// Tolerance for comparing two already-rounded money amounts (half a paisa).
pub const MONEY_EPS: f64 = 0.005;

/// Rounds a currency amount to 2 decimal places (DD-15).
pub fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Rate net of the bill discount share: `unit_price * clamp(1 - discounts / items_subtotal, 0, 1)`.
/// A bill without a positive subtotal keeps the unit price. Charges never enter it (FR-19, DD-87).
pub fn net_unit_price(unit_price: f64, items_subtotal: f64, discounts_total: f64) -> f64 {
    if items_subtotal <= MONEY_EPS {
        return unit_price;
    }
    unit_price * (1.0 - discounts_total / items_subtotal).clamp(0.0, 1.0)
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

/// DD-94 error code: an entry dated out of order with the bill it follows.
pub const DATE_ORDER: &str = "DATE_ORDER";

/// `2026-11-12` as `12 Nov 2026` for messages; an unparsable value is shown as is.
pub fn display_date(iso: &str) -> String {
    chrono::NaiveDate::parse_from_str(iso, "%Y-%m-%d")
        .map(|d| d.format("%d %b %Y").to_string())
        .unwrap_or_else(|_| iso.to_string())
}

/// DD-94: an event that follows a bill (`what` = "Return", "Payment") cannot be dated before it.
/// `BillLocked { code: DATE_ORDER }`: "Return date can't be before the bill date (12 Nov 2026)".
pub fn check_not_before_bill(what: &str, date: &str, bill_date: &str) -> Result<(), crate::core::error::AppError> {
    if date < bill_date {
        return Err(crate::core::error::AppError::BillLocked {
            code: DATE_ORDER.to_string(),
            message: format!("{what} date can't be before the bill date ({})", display_date(bill_date)),
        });
    }
    Ok(())
}

/// DD-94: a bill (or opening balance) moved to `new_date` must stay on or before its earliest
/// follow-up `(date, noun)`, e.g. `("2026-11-14", "payment")`:
/// "This bill has a payment on 14 Nov 2026, so its date can't be later".
pub fn check_not_after_follow_up(
    new_date: &str,
    earliest: Option<(String, &str)>,
    subject: &str,
) -> Result<(), crate::core::error::AppError> {
    match earliest {
        Some((d, noun)) if new_date > d.as_str() => Err(crate::core::error::AppError::BillLocked {
            code: DATE_ORDER.to_string(),
            message: format!("This {subject} has a {noun} on {}, so its date can't be later", display_date(&d)),
        }),
        _ => Ok(()),
    }
}

/// `check_entry_date` against the service clock (`core::clock::today_local`).
pub fn check_entry_date_now(field: &str, date: &str) -> Result<(), crate::core::error::AppError> {
    check_entry_date(field, date, &crate::core::clock::today_local())
}
