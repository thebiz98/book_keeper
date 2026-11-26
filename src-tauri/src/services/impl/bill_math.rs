//! Bill math shared by sale/purchase creation and (FR-20) bill edit/delete: charge line
//! validation and resolution, totals, the discounted net rate and the payment cross-checks.
//! Shared by create and (v8) edit paths; never begins a transaction.
use crate::core::dtos::{BillChargeInput, BillPaymentPreview, BillPreview, PreviewError};
use crate::core::entities::BillCharge;
use crate::core::error::AppError;
use crate::core::money::{round2, MONEY_EPS};
use crate::core::value_objects::{ChargeEffect, LotSource, PaymentStatus, TransactionType};
use crate::data::contracts::unit_of_work::TransactionContext;

/// At most this many charge/discount lines per bill.
pub const MAX_CHARGE_LINES: usize = 50;

/// `system_key` of the charge type that takes the positive line adjustments (FR-47).
pub const LINE_EXTRA_KEY: &str = "LINE_EXTRA";
/// `system_key` of the charge type that takes the negative line adjustments (FR-47).
pub const LINE_DISCOUNT_KEY: &str = "LINE_DISCOUNT";

/// A sale or purchase line total: `round2(qty × rate)`.
pub fn line_total(quantity: f64, unit_price: f64) -> f64 {
    round2(quantity * unit_price)
}

/// FR-47 / DD-81: signed difference between the typed line total and the line total
/// (`> 0` extra charge, `< 0` discount, `0` none). `entered_total` must be finite and `>= 0`.
pub fn line_adjustment(line_total: f64, entered_total: Option<f64>) -> Result<f64, AppError> {
    let Some(entered) = entered_total else { return Ok(0.0) };
    if !entered.is_finite() || entered < 0.0 {
        return Err(validation("entered_total", "The item total must be 0 or more"));
    }
    let diff = round2(round2(entered) - line_total);
    Ok(if diff.abs() < MONEY_EPS { 0.0 } else { diff })
}

/// FR-47: the line adjustments as at most one "Item extra charge" and one "Item discount" line.
pub fn line_adjustment_charges(
    tx: &mut dyn TransactionContext,
    adjustments: &[f64],
) -> Result<Vec<ResolvedCharge>, AppError> {
    let extra = round2(adjustments.iter().filter(|a| **a > 0.0).sum::<f64>());
    let discount = round2(-adjustments.iter().filter(|a| **a < 0.0).sum::<f64>());
    let mut out = Vec::new();
    for (key, amount) in [(LINE_EXTRA_KEY, extra), (LINE_DISCOUNT_KEY, discount)] {
        if amount < MONEY_EPS {
            continue;
        }
        let ct = tx
            .charge_types()
            .find_by_system_key(key)?
            .ok_or_else(|| AppError::DatabaseError(format!("The system charge type {key} is missing")))?;
        out.push(ResolvedCharge { charge_type_id: ct.id, charge_name: ct.name, effect: ct.effect, amount });
    }
    Ok(out)
}

/// A charge line after type resolution: amount rounded, name and effect snapshotted.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedCharge {
    pub charge_type_id: i64,
    pub charge_name: String,
    pub effect: ChargeEffect,
    pub amount: f64,
}

/// Header money fields of a bill: `total_amount = items_subtotal + charges_total - discounts_total`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BillTotals {
    pub items_subtotal: f64,
    pub charges_total: f64,
    pub discounts_total: f64,
    pub total_amount: f64,
}

fn validation(field: &str, message: impl Into<String>) -> AppError {
    AppError::ValidationError { field: field.to_string(), message: message.into() }
}

/// Figures a bill command computes on its way, recorded as it goes so a preview can return the
/// totals even when a later check (stock, payment) fails (DD-56). The real command ignores it.
#[derive(Debug, Clone, Default)]
pub struct BillFigures {
    pub line_totals: Vec<f64>,
    /// FR-47: signed adjustment per line (sale only; empty on purchases).
    pub line_adjustments: Vec<f64>,
    pub totals: Option<BillTotals>,
    pub applied: f64,
    pub open: f64,
}

impl BillFigures {
    pub fn set_totals(&mut self, line_totals: &[f64], totals: BillTotals, applied: f64) {
        self.line_totals = line_totals.to_vec();
        self.totals = Some(totals);
        self.applied = applied;
        self.open = round2((totals.total_amount - applied).max(0.0));
    }
}

/// Turns the outcome of a rolled-back dry run into a `BillPreview`. A business error becomes
/// `errors` (with whatever totals were reached); any other error stays a failure.
pub fn bill_preview(
    fig: BillFigures,
    outcome: Result<Option<BillPaymentPreview>, AppError>,
) -> Result<BillPreview, AppError> {
    let (payment, errors) = match outcome {
        Ok(p) => (p, Vec::new()),
        Err(e) => match PreviewError::from_app_error(&e) {
            Some(pe) => (None, vec![pe]),
            None => return Err(e),
        },
    };
    let t = fig.totals.unwrap_or(BillTotals {
        items_subtotal: 0.0,
        charges_total: 0.0,
        discounts_total: 0.0,
        total_amount: 0.0,
    });
    Ok(BillPreview {
        line_totals: fig.line_totals,
        line_adjustments: fig.line_adjustments,
        items_subtotal: t.items_subtotal,
        charges_total: t.charges_total,
        discounts_total: t.discounts_total,
        total_amount: t.total_amount,
        applied_amount: fig.applied,
        open_amount: fig.open,
        payment,
        errors,
    })
}

/// Pre-transaction checks: at most 50 lines; each amount finite and `round2(amount) > 0`.
pub fn validate_charge_inputs(charges: &[BillChargeInput]) -> Result<(), AppError> {
    if charges.len() > MAX_CHARGE_LINES {
        return Err(validation("charges", format!("A bill can have at most {MAX_CHARGE_LINES} charge lines")));
    }
    for c in charges {
        if !c.amount.is_finite() || round2(c.amount) <= 0.0 {
            return Err(validation("charges.amount", "Charge amount must be greater than 0"));
        }
    }
    Ok(())
}

/// In-tx: each type exists, is active and covers `side`; amount is rounded; name and effect are
/// snapshotted.
pub fn resolve_bill_charges(
    tx: &mut dyn TransactionContext,
    side: TransactionType,
    charges: &[BillChargeInput],
) -> Result<Vec<ResolvedCharge>, AppError> {
    let mut out = Vec::with_capacity(charges.len());
    for c in charges {
        let ct = tx.charge_types().find_by_id(c.charge_type_id)?.ok_or_else(|| AppError::NotFound {
            entity: "ChargeType".to_string(),
            id: c.charge_type_id.to_string(),
        })?;
        if !ct.is_active {
            return Err(validation("charge_type_id", format!("'{}' is inactive", ct.name)));
        }
        if !ct.applies_to.covers(side) {
            let word = if side == TransactionType::Sale { "sale" } else { "purchase" };
            return Err(validation("charge_type_id", format!("'{}' cannot be used on a {word} bill", ct.name)));
        }
        out.push(ResolvedCharge {
            charge_type_id: ct.id,
            charge_name: ct.name,
            effect: ct.effect,
            amount: round2(c.amount),
        });
    }
    Ok(out)
}

/// `items_subtotal = round2(sum line_totals)`; charge/discount sums rounded; the total never goes
/// below zero (discounts exceeding the bill are a `charges` validation error).
pub fn compute_bill_totals(line_totals: &[f64], charges: &[ResolvedCharge]) -> Result<BillTotals, AppError> {
    let items_subtotal = round2(line_totals.iter().sum::<f64>());
    let sum = |e: ChargeEffect| round2(charges.iter().filter(|c| c.effect == e).map(|c| c.amount).sum::<f64>());
    let charges_total = sum(ChargeEffect::Charge);
    let discounts_total = sum(ChargeEffect::Discount);
    let raw = items_subtotal + charges_total - discounts_total;
    if raw < -MONEY_EPS {
        return Err(validation(
            "charges",
            format!(
                "Discounts (₹{discounts_total:.2}) exceed the bill amount (₹{:.2})",
                items_subtotal + charges_total
            ),
        ));
    }
    Ok(BillTotals { items_subtotal, charges_total, discounts_total, total_amount: round2(raw).max(0.0) })
}

/// Inserts the charge rows of a bill. On the purchase side the header must be a PURCHASE-source bill.
pub fn insert_bill_charges(
    tx: &mut dyn TransactionContext,
    side: TransactionType,
    transaction_id: i64,
    charges: &[ResolvedCharge],
) -> Result<Vec<i64>, AppError> {
    if side == TransactionType::Purchase && !charges.is_empty() {
        let header = tx.purchases().find_transaction_by_id(transaction_id)?.ok_or_else(|| AppError::NotFound {
            entity: "PurchaseTransaction".to_string(),
            id: transaction_id.to_string(),
        })?;
        if header.source != LotSource::Purchase {
            return Err(validation("charges", "Only purchase bills can carry charges"));
        }
    }
    let mut ids = Vec::with_capacity(charges.len());
    for c in charges {
        ids.push(tx.bill_charges().create(&BillCharge {
            id: 0,
            transaction_type: side,
            transaction_id,
            charge_type_id: c.charge_type_id,
            charge_name: c.charge_name.clone(),
            effect: c.effect,
            amount: c.amount,
        })?);
    }
    Ok(ids)
}

/// Removes the charge rows of a bill (FR-20 edit/delete).
pub fn delete_bill_charges(
    tx: &mut dyn TransactionContext,
    side: TransactionType,
    transaction_id: i64,
) -> Result<usize, AppError> {
    tx.bill_charges().delete_by_transaction(side, transaction_id)
}

/// Sold rate net of the bill discount share: `unit_price * clamp(1 - discounts / items_subtotal, 0, 1)`.
/// A bill without a positive subtotal keeps the unit price.
pub fn net_unit_price(unit_price: f64, items_subtotal: f64, discounts_total: f64) -> f64 {
    if items_subtotal <= MONEY_EPS {
        return unit_price;
    }
    unit_price * (1.0 - discounts_total / items_subtotal).clamp(0.0, 1.0)
}

/// Sale payment cross-check (DD-02): returns `(upfront, change)`.
pub fn sale_payment_split(status: PaymentStatus, total_amount: f64, paid: f64) -> Result<(f64, f64), AppError> {
    Ok(match status {
        PaymentStatus::Paid => {
            if total_amount <= MONEY_EPS {
                if paid > MONEY_EPS {
                    return Err(validation("paid_amount", "A zero-value bill takes no payment"));
                }
                (0.0, 0.0)
            } else if paid + MONEY_EPS < total_amount {
                return Err(validation("paid_amount", "PAID needs paid_amount >= total"));
            } else {
                (total_amount, round2((paid - total_amount).max(0.0)))
            }
        }
        PaymentStatus::Partial => {
            if !(paid > MONEY_EPS && paid + MONEY_EPS < total_amount) {
                return Err(validation("paid_amount", "PARTIAL needs 0 < paid_amount < total"));
            }
            (paid, 0.0)
        }
        PaymentStatus::Unpaid => {
            if paid > MONEY_EPS {
                return Err(validation("paid_amount", "UNPAID needs paid_amount = 0"));
            }
            if total_amount <= MONEY_EPS {
                return Err(validation("payment_status", "A zero-value bill must be PAID"));
            }
            (0.0, 0.0)
        }
    })
}

/// FR-25 / V4 payment rule for a lot price edit. `applied = SETTLEMENT + RETURN_CREDIT`,
/// `upfront_old = UPFRONT`, `old_balance_due` is the bill's balance before the edit. Returns the new
/// UPFRONT amount, or `BillLocked TOTAL_BELOW_APPLIED` when the new total is below what was applied.
pub fn lot_price_upfront(
    new_total: f64,
    applied: f64,
    upfront_old: f64,
    old_balance_due: f64,
) -> Result<f64, AppError> {
    let open = new_total - applied;
    if open < -MONEY_EPS {
        return Err(AppError::BillLocked {
            code: "TOTAL_BELOW_APPLIED".to_string(),
            message: format!(
                "The new total \u{20b9}{new_total:.2} is below the \u{20b9}{applied:.2} already applied to this bill"
            ),
        });
    }
    let open = round2(open.max(0.0));
    if old_balance_due <= MONEY_EPS && applied <= MONEY_EPS {
        return Ok(round2(new_total));
    }
    Ok(round2(upfront_old.min(open)))
}

/// Purchase payment cross-check (DD-02): returns the upfront amount.
pub fn purchase_upfront(status: PaymentStatus, total_amount: f64, paid: f64) -> Result<f64, AppError> {
    if paid > total_amount + MONEY_EPS {
        return Err(validation("paid_amount", "paid_amount cannot exceed the purchase total"));
    }
    match status {
        PaymentStatus::Paid => {
            if (paid - total_amount).abs() > MONEY_EPS {
                return Err(validation("paid_amount", "PAID needs paid_amount = total"));
            }
        }
        PaymentStatus::Partial => {
            if !(paid > MONEY_EPS && paid + MONEY_EPS < total_amount) {
                return Err(validation("paid_amount", "PARTIAL needs 0 < paid_amount < total"));
            }
        }
        PaymentStatus::Unpaid => {
            if paid > MONEY_EPS {
                return Err(validation("paid_amount", "UNPAID needs paid_amount = 0"));
            }
            if total_amount <= MONEY_EPS {
                return Err(validation("payment_status", "A zero-value bill must be PAID"));
            }
        }
    }
    Ok(if status == PaymentStatus::Paid { total_amount } else { paid })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rc(effect: ChargeEffect, amount: f64) -> ResolvedCharge {
        ResolvedCharge { charge_type_id: 1, charge_name: "x".into(), effect, amount }
    }

    #[test]
    fn subtotal_is_rounded_once() {
        let t = compute_bill_totals(&[33.33, 33.33, 33.34], &[]).unwrap();
        assert_eq!(t.items_subtotal, 100.0);
        assert_eq!(t.total_amount, 100.0);
    }

    #[test]
    fn charges_sum_without_float_drift() {
        let t = compute_bill_totals(&[100.0], &[rc(ChargeEffect::Charge, 0.1), rc(ChargeEffect::Charge, 0.2)]).unwrap();
        assert_eq!(t.charges_total, 0.3);
        assert_eq!(t.total_amount, 100.3);
    }

    #[test]
    fn discounts_equal_to_subtotal_give_zero() {
        let t = compute_bill_totals(&[100.0], &[rc(ChargeEffect::Discount, 100.0)]).unwrap();
        assert_eq!(t.total_amount, 0.0);
    }

    #[test]
    fn discounts_above_the_bill_are_rejected() {
        let e = compute_bill_totals(&[100.0], &[rc(ChargeEffect::Discount, 100.5)]).unwrap_err();
        assert!(matches!(e, AppError::ValidationError { ref field, .. } if field == "charges"));
    }

    #[test]
    fn net_rate_spreads_the_discount() {
        assert_eq!(net_unit_price(100.0, 200.0, 20.0), 90.0);
        assert_eq!(net_unit_price(100.0, 0.0, 0.0), 100.0);
        assert_eq!(net_unit_price(100.0, 200.0, 500.0), 0.0);
    }

    #[test]
    fn input_validation() {
        let ok = BillChargeInput { charge_type_id: 1, amount: 5.0 };
        assert!(validate_charge_inputs(&[ok.clone()]).is_ok());
        for bad in [0.0, -1.0, f64::NAN, 0.004] {
            assert!(validate_charge_inputs(&[BillChargeInput { charge_type_id: 1, amount: bad }]).is_err());
        }
        assert!(validate_charge_inputs(&vec![ok; MAX_CHARGE_LINES + 1]).is_err());
    }
}
