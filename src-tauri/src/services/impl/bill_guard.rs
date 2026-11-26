//! FR-20 edit / delete guards (LLD 14.4), evaluated inside the caller's transaction so a command and
//! the UI pre-check (`get_bill_edit_guard`) can never disagree.
use crate::core::dtos::{BillEditGuard, CorrectionTarget, GuardReason, LotEditLimit};
use crate::core::entities::{PaymentSettlement, ReturnTransaction};
use crate::core::error::AppError;
use crate::core::money::round2;
use crate::core::value_objects::{LotSource, OutflowSourceType, PaymentKind, ReturnType, TransactionType};
use crate::data::contracts::unit_of_work::TransactionContext;
use crate::services::contracts::inventory_service::{InventoryService, ReturnUndoPlan};

pub const LINKED_RETURNS: &str = "LINKED_RETURNS";
pub const RETURN_CREDITS: &str = "RETURN_CREDITS";
pub const NOT_A_PURCHASE: &str = "NOT_A_PURCHASE";
pub const CONSUMED_LOTS: &str = "CONSUMED_LOTS";
pub const TOTAL_BELOW_APPLIED: &str = "TOTAL_BELOW_APPLIED";
pub const PARTY_CHANGE_WITH_CREDITS: &str = "PARTY_CHANGE_WITH_CREDITS";
pub const PARTY_REQUIRED_WITH_PAYMENTS: &str = "PARTY_REQUIRED_WITH_PAYMENTS";
pub const NEWER_RETURN: &str = "NEWER_RETURN";
pub const OPENING_BALANCE: &str = "OPENING_BALANCE";

/// Says which tool changes this stock record instead (G8: always point to the right place).
fn not_a_purchase_message(code: &str, source: LotSource) -> String {
    match source {
        LotSource::Opening => format!(
            "{code} is the item's opening stock, not a supplier purchase. Change its quantity from the item's Lots & Purchase Price."
        ),
        LotSource::Adjustment => format!(
            "{code} came from a stock adjustment, not a supplier purchase. Undo it in Adjustments history."
        ),
        _ => format!("{code} is a stock record, not a supplier purchase, and cannot be edited or deleted here."),
    }
}

/// The `BillLocked` error for one guard reason.
pub fn locked(reason: &GuardReason) -> AppError {
    AppError::BillLocked { code: reason.code.clone(), message: reason.message.clone() }
}

/// A `BillLocked` error from a code and message.
pub fn locked_with(code: &str, message: impl Into<String>) -> AppError {
    AppError::BillLocked { code: code.to_string(), message: message.into() }
}

fn reason(code: &str, message: String, blocks_edit: bool, blocks_delete: bool) -> GuardReason {
    GuardReason { code: code.to_string(), message, blocks_edit, blocks_delete }
}

fn codes(returns: &[ReturnTransaction]) -> String {
    returns.iter().map(|r| r.return_code.clone()).collect::<Vec<_>>().join(", ")
}

/// An opening-balance bill (FR-35) is a party balance, not a trade: it can be neither edited nor
/// deleted here. Change it from the party dialog.
fn opening_balance_reason(code: &str) -> GuardReason {
    reason(
        OPENING_BALANCE,
        format!("{code} is the party's opening balance. Change it from the party record."),
        true,
        true,
    )
}

fn finish(mut g: BillEditGuard) -> BillEditGuard {
    g.can_edit = !g.reasons.iter().any(|r| r.blocks_edit);
    g.can_delete = !g.reasons.iter().any(|r| r.blocks_delete);
    g
}

fn blank_guard(target: CorrectionTarget, id: i64, code: String) -> BillEditGuard {
    BillEditGuard {
        target,
        id,
        code,
        can_edit: true,
        can_delete: true,
        can_change_party: true,
        reasons: Vec::new(),
        upfront_amount: 0.0,
        applied_amount: 0.0,
        removed_cash_amount: 0.0,
        removed_credit_amount: 0.0,
        lot_limits: Vec::new(),
    }
}

/// Ledger-derived part shared by sale and purchase guards.
fn ledger_part(
    tx: &mut dyn TransactionContext,
    tx_type: TransactionType,
    id: i64,
    return_type: ReturnType,
    g: &mut BillEditGuard,
) -> Result<(), AppError> {
    let linked = tx.returns().list_linked(return_type, id)?;
    let rc_returns = tx.settlements().return_credit_return_ids(tx_type, id)?;
    let totals = tx.settlements().sum_by_kind_for_transaction(tx_type, id)?;
    g.upfront_amount = round2(totals.upfront);
    g.applied_amount = round2(totals.settlement + totals.return_credit);
    g.removed_cash_amount = round2(totals.upfront + totals.settlement);
    if !linked.is_empty() {
        g.reasons.push(reason(
            LINKED_RETURNS,
            format!("This bill has returns ({}). Delete them first.", codes(&linked)),
            true,
            true,
        ));
    }
    if !rc_returns.is_empty() {
        let mut names = Vec::new();
        for rid in &rc_returns {
            if let Some(r) = tx.returns().find_return_by_id(*rid)? {
                names.push(r.return_code);
            }
        }
        g.reasons.push(reason(
            RETURN_CREDITS,
            format!(
                "Return credit from {} was applied to this bill. Delete that return first to delete the bill or change its party.",
                names.join(", ")
            ),
            false,
            true,
        ));
        g.can_change_party = false;
    }
    Ok(())
}

pub fn evaluate_sale_guard(tx: &mut dyn TransactionContext, sale_id: i64) -> Result<BillEditGuard, AppError> {
    let sale = tx
        .sales()
        .find_transaction_by_id(sale_id)?
        .ok_or_else(|| AppError::not_found("SaleTransaction", sale_id.to_string()))?;
    let is_opening = sale.is_opening_balance;
    let mut g = blank_guard(CorrectionTarget::Sale, sale_id, sale.transaction_code);
    if is_opening {
        g.reasons.push(opening_balance_reason(&g.code.clone()));
        g.can_change_party = false;
        return Ok(finish(g));
    }
    ledger_part(tx, TransactionType::Sale, sale_id, ReturnType::Customer, &mut g)?;
    Ok(finish(g))
}

pub fn evaluate_purchase_guard(tx: &mut dyn TransactionContext, purchase_id: i64) -> Result<BillEditGuard, AppError> {
    let p = tx
        .purchases()
        .find_transaction_by_id(purchase_id)?
        .ok_or_else(|| AppError::not_found("PurchaseTransaction", purchase_id.to_string()))?;
    let mut g = blank_guard(CorrectionTarget::Purchase, purchase_id, p.transaction_code.clone());
    if p.is_opening_balance {
        g.reasons.push(opening_balance_reason(&p.transaction_code));
        g.can_change_party = false;
        return Ok(finish(g));
    }
    if p.source != LotSource::Purchase {
        g.reasons.push(reason(
            NOT_A_PURCHASE,
            not_a_purchase_message(&p.transaction_code, p.source),
            true,
            true,
        ));
        return Ok(finish(g));
    }
    ledger_part(tx, TransactionType::Purchase, purchase_id, ReturnType::Vendor, &mut g)?;
    let mut any_referenced = false;
    for lot in tx.purchases().list_items_by_transaction_id(purchase_id)? {
        let referenced = tx.purchases().is_lot_referenced(lot.id)?;
        let vendor_returned = tx.outflow_allocations().exists_for_lot(lot.id, OutflowSourceType::VendorReturn)?;
        any_referenced |= referenced;
        g.lot_limits.push(LotEditLimit {
            purchase_item_id: lot.id,
            consumed_quantity: (lot.quantity - lot.remaining_quantity).max(0.0),
            referenced,
            vendor_returned,
        });
    }
    if any_referenced {
        g.reasons.push(reason(
            CONSUMED_LOTS,
            "Some of this stock has already been sold, adjusted or returned, so the purchase cannot be deleted.".to_string(),
            false,
            true,
        ));
    }
    Ok(finish(g))
}

/// Delete checks for a return, shared by the guard and `delete_return`: newest first, then the
/// inventory dry run. `BillLocked` on failure.
pub fn check_return_deletable(
    tx: &mut dyn TransactionContext,
    r: &ReturnTransaction,
    inv: &dyn InventoryService,
) -> Result<ReturnUndoPlan, AppError> {
    if let Some(newer) = tx.returns().newer_linked_return(r)? {
        return Err(locked_with(
            NEWER_RETURN,
            format!("{newer} was made after {} on the same bill. Delete {newer} first.", r.return_code),
        ));
    }
    inv.plan_return_undo(tx, r)
}

pub fn evaluate_return_guard(
    tx: &mut dyn TransactionContext,
    return_id: i64,
    inv: &dyn InventoryService,
) -> Result<BillEditGuard, AppError> {
    let r = tx
        .returns()
        .find_return_by_id(return_id)?
        .ok_or_else(|| AppError::not_found("ReturnTransaction", return_id.to_string()))?;
    let mut g = blank_guard(CorrectionTarget::Return, return_id, r.return_code.clone());
    g.can_change_party = false;
    let ledger: Vec<PaymentSettlement> = tx.returns().find_ledger_by_return_id(return_id)?;
    let sum = |k: PaymentKind| round2(ledger.iter().filter(|l| l.kind == k).map(|l| l.amount_paid).sum::<f64>());
    g.removed_cash_amount = round2(sum(PaymentKind::RefundOut) + sum(PaymentKind::RefundIn));
    g.removed_credit_amount = sum(PaymentKind::ReturnCredit);
    if let Err(AppError::BillLocked { code, message }) = check_return_deletable(tx, &r, inv) {
        g.reasons.push(GuardReason { code, message, blocks_edit: false, blocks_delete: true });
    }
    let mut g = finish(g);
    g.can_edit = false; // returns are deleted and re-entered, never edited
    Ok(g)
}
