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

/// E9 / DD-89: how a bill edit treats the stored charge rows. `charges` are the bill's charge lines
/// after the edit, in payload order (for the totals).
#[derive(Debug, Clone, Default)]
pub struct ChargeEditPlan {
    pub charges: Vec<ResolvedCharge>,
    /// Stored rows the user removed or changed.
    remove_ids: Vec<i64>,
    /// Payload rows that match no stored row: resolved against the live charge type.
    added: Vec<ResolvedCharge>,
}

/// In-tx, no writes. Each payload row is matched to the first unused stored row of the same
/// charge type and amount: a match keeps the stored row untouched (its name and effect, even if
/// the type was renamed, flipped or deactivated since). A row that matches nothing is new (or
/// changed) and resolves the live type; stored rows left unmatched are removed.
pub fn plan_bill_charges_edit(
    tx: &mut dyn TransactionContext,
    side: TransactionType,
    transaction_id: i64,
    inputs: &[BillChargeInput],
) -> Result<ChargeEditPlan, AppError> {
    let stored = tx.bill_charges().list_by_transaction(side, transaction_id)?;
    let mut used = vec![false; stored.len()];
    let mut plan = ChargeEditPlan::default();
    let mut unmatched: Vec<BillChargeInput> = Vec::new();
    let mut slots: Vec<Option<ResolvedCharge>> = Vec::with_capacity(inputs.len());
    for c in inputs {
        let amount = round2(c.amount);
        let hit = stored
            .iter()
            .enumerate()
            .position(|(i, s)| !used[i] && s.charge_type_id == c.charge_type_id && (s.amount - amount).abs() <= 1e-9);
        match hit {
            Some(i) => {
                used[i] = true;
                let s = &stored[i];
                slots.push(Some(ResolvedCharge {
                    charge_type_id: s.charge_type_id,
                    charge_name: s.charge_name.clone(),
                    effect: s.effect,
                    amount: s.amount,
                }));
            }
            None => {
                unmatched.push(c.clone());
                slots.push(None);
            }
        }
    }
    let mut fresh = resolve_bill_charges(tx, side, &unmatched)?.into_iter();
    for slot in slots {
        let c = match slot {
            Some(c) => c,
            None => {
                let c = fresh.next().expect("one resolved charge per unmatched input");
                plan.added.push(c.clone());
                c
            }
        };
        plan.charges.push(c);
    }
    plan.remove_ids = stored.iter().zip(&used).filter(|(_, u)| !**u).map(|(s, _)| s.id).collect();
    Ok(plan)
}

/// Writes a [`ChargeEditPlan`]: removes the changed / dropped rows and inserts the new ones. Kept
/// rows are not written.
pub fn apply_bill_charges_edit(
    tx: &mut dyn TransactionContext,
    side: TransactionType,
    transaction_id: i64,
    plan: &ChargeEditPlan,
) -> Result<(), AppError> {
    for id in &plan.remove_ids {
        tx.bill_charges().delete_by_id(*id)?;
    }
    insert_bill_charges(tx, side, transaction_id, &plan.added)?;
    Ok(())
}

/// Units per pack of a stored line (`quantity / pack_quantity`; 1 without a usable pack quantity).
pub fn units_per_pack(quantity: f64, pack_quantity: f64) -> f64 {
    if pack_quantity.is_finite() && pack_quantity > 0.0 { quantity / pack_quantity } else { 1.0 }
}

/// E4 / DD-89: the item's bundle a stored line was entered in. `None` for a base-unit line
/// (no pack label) or when no bundle has those units per pack. A bundle with the same label and
/// units wins; else the only bundle with those units (a renamed bundle).
pub fn resolve_line_bundle(
    tx: &mut dyn TransactionContext,
    item_id: i64,
    pack_label: Option<&str>,
    units: f64,
) -> Result<Option<i64>, AppError> {
    let Some(label) = pack_label else { return Ok(None) };
    let same_units: Vec<_> = tx
        .bundles()
        .find_by_item_id(item_id)?
        .into_iter()
        .filter(|b| (b.units_per_bundle - units).abs() <= 1e-9)
        .collect();
    if let Some(b) = same_units.iter().find(|b| b.bundle_name == label) {
        return Ok(Some(b.id));
    }
    Ok(match same_units.as_slice() {
        [only] => Some(only.id),
        _ => None,
    })
}

/// Removes the charge rows of a bill (FR-20 edit/delete).
pub fn delete_bill_charges(
    tx: &mut dyn TransactionContext,
    side: TransactionType,
    transaction_id: i64,
) -> Result<usize, AppError> {
    tx.bill_charges().delete_by_transaction(side, transaction_id)
}

/// Rate net of the bill discount share (lives in `core::money` so repositories share it, DD-87).
pub use crate::core::money::net_unit_price;

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
