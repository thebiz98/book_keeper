use std::sync::Arc;
use crate::core::constants::EPS;
use crate::core::dtos::{
    ItemBillRef, ItemLot, LotDraw, LotDrawComponent, OpeningQuantityResult, UpdateOpeningQuantityPayload, AdjustmentUndoGuard, AdjustmentUndoReason, LotDrawLot, PageRequest, Paginated, PreviewServiceCostPayload, PriorLine,
    ServiceCostPreview, SetItemStockPayload, SetItemStockResult, SetStockMode, SetStockOption, StockAdjustmentPayload,
    StockAdjustmentPreview, StockAdjustmentResult, UndoableWriteoff,
};
use crate::services::r#impl::item_guard::ensure_active_item;
use crate::core::money::{check_entry_date_now, check_finite_amount, net_unit_price, round2};
use crate::core::entities::{
    Item, PurchaseItem, PurchaseTransaction, ReturnTransaction, StockAdjustment, StockOutflowAllocation,
};
use crate::core::error::AppError;
use crate::core::value_objects::{
    AdjustmentReason, ItemType, LotSource, OutflowSourceType, PaymentStatus, ReturnLineType, ReturnType,
    SettlementType, StockCondition,
};
use crate::data::contracts::unit_of_work::{TransactionContext, UnitOfWork};
use crate::services::contracts::inventory_allocation_strategy::{
    DepletionResult, InventoryAllocationStrategy,
};
use crate::services::contracts::inventory_service::{
    InventoryService, LotEdit, ReturnUndoPlan, ReturnUndoStep, VendorReturnCost,
};

/// Sole writer of the stock cache and lot/allocation state. The strategy is injected (DD-08);
/// this type never names a concrete strategy. It reads items through `uow.items()` rather than
/// depending on `ItemService`, so `ItemService -> InventoryService` (initial stock) cannot cycle.
#[derive(Clone)]
pub struct InventoryServiceImpl<S: InventoryAllocationStrategy, U: UnitOfWork = Arc<dyn UnitOfWork>> {
    pub strategy: S,
    pub uow: U,
}

impl<S: InventoryAllocationStrategy, U: UnitOfWork> InventoryServiceImpl<S, U> {
    pub fn new(strategy: S, uow: U) -> Self {
        Self { strategy, uow }
    }
}

fn validation(field: &str, message: impl Into<String>) -> AppError {
    AppError::ValidationError { field: field.to_string(), message: message.into() }
}

fn require_positive(field: &str, qty: f64) -> Result<(), AppError> {
    if !qty.is_finite() || qty <= 0.0 {
        return Err(validation(field, format!("Quantity must be a finite number greater than zero (got {qty})")));
    }
    check_finite_amount(field, qty)?;
    Ok(())
}

fn require_date(field: &str, date: &str) -> Result<(), AppError> {
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| validation(field, format!("'{date}' is not a valid YYYY-MM-DD date")))
}

fn find_item(uow: &mut dyn TransactionContext, item_id: i64) -> Result<Item, AppError> {
    uow.items()
        .find_by_id(item_id)?
        .ok_or_else(|| AppError::not_found("Item", item_id.to_string()))
}

/// Item-history paging (FR-23): default limit 10, max 50.
fn item_history_page(page: Option<usize>, limit: Option<usize>) -> Result<PageRequest, AppError> {
    let limit = limit.unwrap_or(10);
    if limit > 50 {
        return Err(validation("limit", "limit must be between 1 and 50"));
    }
    PageRequest::new(page, Some(limit))
}

/// Active and PHYSICAL (FR-21): the only items whose stock may be adjusted.
fn require_stockable(item: &Item) -> Result<(), AppError> {
    if !item.is_active {
        return Err(validation("item_id", format!("'{}' is deleted. Restore it first.", item.name)));
    }
    if item.item_type != ItemType::Physical {
        return Err(validation("item_id", format!("'{}' is a service item and has no stock", item.name)));
    }
    Ok(())
}

/// FR-36 / X5: a service has no lots, so no stock-moving path may name one.
fn reject_service(item: &Item) -> Result<(), AppError> {
    if item.item_type == ItemType::Service {
        return Err(validation("item_id", format!("'{}' is a service item and has no stock", item.name)));
    }
    Ok(())
}

fn locked(code: &str, message: impl Into<String>) -> AppError {
    AppError::BillLocked { code: code.to_string(), message: message.into() }
}

/// DD-95: the least quantity a lot can be edited to: everything drawn from it, Σ sale allocations
/// `quantity_depleted` (restocked customer-return units included: they are back in `remaining` but
/// still tied to their allocation, and the return delete takes them out again) + Σ outflows.
pub(crate) fn lot_floor(uow: &mut dyn TransactionContext, lot: &PurchaseItem) -> Result<f64, AppError> {
    let drawn = uow.allocations().sum_depleted_for_lot(lot.id)? + uow.outflow_allocations().sum_for_lot(lot.id)?;
    Ok(drawn.max(lot.quantity - lot.remaining_quantity).max(0.0))
}

/// S2 / DD-95: puts `back` units on a lot (a sale or outflow reversal). Never clamped: more than
/// the lot's own quantity (or a negative result) means the stock records disagree, so the whole
/// command fails with `STOCK_INVARIANT` and its transaction rolls back.
fn restore_to_lot(uow: &mut dyn TransactionContext, lot: &PurchaseItem, back: f64) -> Result<(), AppError> {
    let restored = lot.remaining_quantity + back;
    if !restored.is_finite() || restored > lot.quantity + EPS || restored < -EPS {
        return Err(stock_invariant(uow, lot)?);
    }
    uow.purchases().update_remaining_quantity(lot.id, restored)
}

/// The `STOCK_INVARIANT` error for `lot`, named by its bill code.
fn stock_invariant(uow: &mut dyn TransactionContext, lot: &PurchaseItem) -> Result<AppError, AppError> {
    let code = uow
        .purchases()
        .find_transaction_by_id(lot.purchase_transaction_id)?
        .map(|h| h.transaction_code)
        .unwrap_or_else(|| format!("#{}", lot.id));
    Ok(locked(
        "STOCK_INVARIANT",
        format!("Stock records don't match for lot {code} ('{}'). Nothing was changed.", lot.item_name),
    ))
}

impl<S: InventoryAllocationStrategy, U: UnitOfWork> InventoryServiceImpl<S, U> {

    /// FR-31 draw over one item's open lots in the strategy's order, skipping `skip` base units that
    /// earlier cart lines will take first. Returns `(lots, cost, available, shortfall)`; units beyond
    /// the stock cost nothing and show up as shortfall.
    fn draw_lots(
        &self,
        tx: &mut dyn TransactionContext,
        item_id: i64,
        quantity: f64,
        skip: f64,
    ) -> Result<(Vec<LotDrawLot>, f64, f64, f64), AppError> {
        let open = self.strategy.open_lots_in_draw_order(tx, item_id)?;
        let info = tx.purchases().list_lots_for_item(item_id)?;
        let mut skip_left = skip;
        let mut left = quantity;
        let (mut lots, mut cost, mut available) = (Vec::with_capacity(open.len()), 0.0, 0.0);
        for lot in open {
            let rem = lot.remaining_quantity;
            let after_skip = (rem - skip_left).max(0.0);
            skip_left = (skip_left - rem).max(0.0);
            let draw = after_skip.min(left);
            left -= draw;
            cost += draw * lot.unit_cost;
            available += rem;
            let meta = info.iter().find(|l| l.purchase_item_id == lot.id);
            lots.push(LotDrawLot {
                purchase_item_id: lot.id,
                transaction_code: meta.map(|m| m.transaction_code.clone()).unwrap_or_default(),
                source: meta.map(|m| m.source).unwrap_or(LotSource::Purchase),
                unit_cost: lot.unit_cost,
                remaining_quantity: rem,
                draw_quantity: draw,
            });
        }
        let short = skip + quantity - available;
        Ok((lots, cost, available, if short > EPS { short } else { 0.0 }))
    }
    /// FR-20: pushes a new lot cost into everything costed from the lot: sale allocations,
    /// adjustment / replacement outflow rows, and the stored `RETURN_ITEM.cost_amount` of linked
    /// customer lines (replayed) and replacement lines (outflow rows walked in id order).
    fn recost_lot(&self, uow: &mut dyn TransactionContext, lot_id: i64, cost: f64) -> Result<(), AppError> {
        uow.allocations().update_unit_cost_for_lot(lot_id, cost)?;
        uow.outflow_allocations().update_unit_cost_for_lot(lot_id, cost)?;
        for sid in uow.allocations().sale_item_ids_for_lot(lot_id)? {
            let ris = uow.returns().list_customer_returned_items_for_sale_item(sid)?;
            if ris.is_empty() {
                continue;
            }
            let allocs = uow.allocations().find_by_sale_item_id(sid)?;
            let qtys: Vec<f64> = ris.iter().map(|r| r.quantity).collect();
            let takes = self.strategy.replay_reversals(&allocs, &qtys);
            for (ri, t) in ris.iter().zip(takes) {
                let c: f64 = t
                    .iter()
                    .map(|(aid, q)| q * allocs.iter().find(|a| a.id == *aid).map(|a| a.unit_cost).unwrap_or(0.0))
                    .sum();
                uow.returns().update_return_item_cost(ri.id, round2(c))?;
            }
        }
        for rid in uow.outflow_allocations().source_ids_for_lot(lot_id, OutflowSourceType::Replacement)? {
            let outs = uow.outflow_allocations().find_by_source(OutflowSourceType::Replacement, rid)?;
            let lines: Vec<_> = uow
                .returns()
                .find_items_by_return_id(rid)?
                .into_iter()
                .filter(|i| i.line_type == ReturnLineType::ReplacementIssued)
                .collect();
            let mut left: Vec<(f64, f64)> = outs.iter().map(|o| (o.quantity_depleted, o.unit_cost)).collect();
            for line in lines {
                let mut need = line.quantity;
                let mut c = 0.0;
                for (qty, unit) in left.iter_mut() {
                    if need <= EPS {
                        break;
                    }
                    let take = qty.min(need);
                    if take > EPS {
                        *qty -= take;
                        need -= take;
                        c += take * *unit;
                    }
                }
                uow.returns().update_return_item_cost(line.id, round2(c))?;
            }
        }
        Ok(())
    }

    /// The `RETURN` lot an unlinked restockable return line created: the recorded link, else the
    /// legacy `RTL-` header-notes lookup (first unclaimed lot of the item with the line's quantity).
    fn resolve_return_lot(
        uow: &mut dyn TransactionContext,
        r: &ReturnTransaction,
        ri: &crate::core::entities::ReturnItem,
        claimed: &[i64],
    ) -> Result<i64, AppError> {
        if let Some(l) = ri.purchase_item_id {
            return Ok(l);
        }
        let note = format!("Return {}", r.return_code);
        uow.purchases()
            .find_return_lots_by_note(&note, ri.item_id)?
            .into_iter()
            .find(|l| !claimed.contains(&l.id) && (l.quantity - ri.quantity).abs() <= EPS)
            .map(|l| l.id)
            .ok_or_else(|| locked("RETURN_LOT_USED", format!("The stock lot created by {} was not found", r.return_code)))
    }

    /// Writes one adjustment row and its stock effect inside the caller's transaction.
    /// Returns `(adjustment_id, loss_value, unit_cost_used)`.
    #[allow(clippy::too_many_arguments)]
    fn post_adjustment(
        &self,
        tx: &mut dyn TransactionContext,
        item: &Item,
        date: &str,
        quantity: f64,
        reason: AdjustmentReason,
        unit_cost: Option<f64>,
        notes: Option<String>,
    ) -> Result<(i64, f64, Option<f64>), AppError> {
        let mut adj = StockAdjustment {
            id: 0,
            adjustment_date: date.to_string(),
            item_id: item.id,
            quantity,
            unit_cost: None,
            reason,
            notes,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        if quantity < 0.0 {
            let qty = -quantity;
            self.validate_stock_availability(tx, &[(item.id, qty)])?;
            let adj_id = tx.adjustments().create_adjustment(&adj)?;
            let loss = self.issue_stock_outflow(tx, OutflowSourceType::Adjustment, adj_id, item.id, qty)?;
            Ok((adj_id, loss, None))
        } else {
            let cost = match unit_cost {
                Some(c) => c,
                None => tx.purchases().get_latest_lot_cost(item.id)?.ok_or_else(|| {
                    validation("unit_cost", "unit cost required: item has no prior lot")
                })?,
            };
            adj.unit_cost = Some(cost);
            let adj_id = tx.adjustments().create_adjustment(&adj)?;
            self.create_source_lot(
                tx,
                LotSource::Adjustment,
                date,
                item.id,
                quantity,
                Some(cost),
                Some(format!("Stock adjustment #{adj_id}: {reason:?}")),
            )?;
            Ok((adj_id, 0.0, Some(cost)))
        }
    }

    /// Body of `create_adjustment` inside the caller's transaction; shared with the preview (DD-56).
    fn adjustment_in_tx(
        &self,
        tx: &mut dyn TransactionContext,
        payload: &StockAdjustmentPayload,
    ) -> Result<(StockAdjustmentResult, StockAdjustmentPreview), AppError> {
        if !payload.quantity.is_finite() || payload.quantity == 0.0 {
            return Err(validation("quantity", "Adjustment quantity must be finite and non-zero"));
        }
        check_entry_date_now("adjustment_date", &payload.adjustment_date)?;
        if let Some(c) = payload.unit_cost {
            if !c.is_finite() || c < 0.0 {
                return Err(validation("unit_cost", format!("Unit cost must be a finite, non-negative number (got {c})")));
            }
        }

        if payload.reason == AdjustmentReason::UnrecordedStock && payload.quantity < 0.0 {
            return Err(validation("quantity", "Unrecorded stock can only add stock; use Count correction to reduce it"));
        }
        if payload.quantity > 0.0 && !payload.reason.allows_increase() {
            // DD-92 (S3): found stock is Unrecorded stock or a Count correction, never a gain.
            return Err(validation("reason", "Damaged, Expired and Theft can only reduce stock."));
        }

        let item = find_item(tx, payload.item_id)?;
        require_stockable(&item)?;
        // Probed before the write, so the adjustment being made is never listed.
        let undoable_writeoffs = if payload.quantity > 0.0 { self.undoable_writeoffs(tx, item.id)? } else { Vec::new() };
        let (adjustment_id, loss_value, cost_used) = self.post_adjustment(
            tx,
            &item,
            &payload.adjustment_date,
            payload.quantity,
            payload.reason,
            payload.unit_cost,
            payload.notes.clone(),
        )?;
        let new_stock = tx.items().find_by_id(item.id)?.map(|i| i.current_stock).unwrap_or(item.current_stock + payload.quantity);
        let value_change = if payload.quantity > 0.0 {
            round2(payload.quantity * cost_used.unwrap_or(0.0))
        } else {
            -round2(loss_value)
        };
        Ok((
            StockAdjustmentResult { adjustment_id, loss_value },
            StockAdjustmentPreview {
                previous_stock: item.current_stock,
                new_stock,
                unit_cost: cost_used,
                loss_value,
                value_change,
                undoable_writeoffs,
            },
        ))
    }

    /// G7 dry run of undoing one adjustment, inside the caller's transaction (savepoint, rolled back).
    /// `None` when the undo guard refuses it.
    fn undo_probe(&self, tx: &mut dyn TransactionContext, adj: &StockAdjustment) -> Result<Option<UndoableWriteoff>, AppError> {
        tx.savepoint("undo_probe")?;
        let probe = self.undo_adjustment_in_tx(tx, adj.id);
        tx.rollback_to_savepoint("undo_probe")?;
        match probe {
            Ok(g) => Ok(Some(UndoableWriteoff {
                adjustment_id: adj.id,
                adjustment_date: adj.adjustment_date.clone(),
                quantity: -adj.quantity,
                reason: adj.reason,
                loss: g.loss_removed,
            })),
            Err(AppError::BillLocked { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// DD-92: the item's reductions, newest first (max 5), that G7 says can be undone.
    fn undoable_writeoffs(&self, tx: &mut dyn TransactionContext, item_id: i64) -> Result<Vec<UndoableWriteoff>, AppError> {
        let (rows, _) = tx.adjustments().list_adjustments(Some(item_id), 0, 50)?;
        let mut out = Vec::new();
        for a in rows.iter().filter(|a| a.quantity < 0.0) {
            if out.len() >= 5 {
                break;
            }
            if let Some(w) = self.undo_probe(tx, a)? {
                out.push(w);
            }
        }
        Ok(out)
    }

    /// The reductions (write-offs) that drew from any of `lot_ids`, oldest first.
    fn writeoffs_on_lots(tx: &mut dyn TransactionContext, lot_ids: &[i64]) -> Result<Vec<StockAdjustment>, AppError> {
        let mut ids: Vec<i64> = Vec::new();
        for &lot in lot_ids {
            for id in tx.outflow_allocations().source_ids_for_lot(lot, OutflowSourceType::Adjustment)? {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
        ids.sort_unstable();
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(a) = tx.adjustments().find_by_id(id)? {
                out.push(a);
            }
        }
        Ok(out)
    }

    /// "write-off #3 (2026-11-20, 2 PCS), ..." or `None` when no write-off drew from the lots.
    fn name_writeoffs(writeoffs: &[StockAdjustment], unit: &str) -> Option<String> {
        if writeoffs.is_empty() {
            return None;
        }
        let names: Vec<String> = writeoffs
            .iter()
            .map(|a| format!("#{} ({}, {} {unit})", a.id, a.adjustment_date, -a.quantity))
            .collect();
        let noun = if names.len() == 1 { "write-off" } else { "write-offs" };
        Some(format!("{noun} {}", names.join(", ")))
    }

    /// FR-57 scope B: the "Opening stock was entered wrong" path for a Set stock change of `delta`.
    /// `openings` are the item's OPENING lots in id order. Returns the option and, when it is
    /// disabled by write-offs, the undoable ones among them.
    fn opening_option(
        &self,
        tx: &mut dyn TransactionContext,
        item: &Item,
        openings: &[ItemLot],
        delta: f64,
    ) -> Result<(SetStockOption, Vec<UndoableWriteoff>), AppError> {
        let unit = &item.base_unit;
        let label = "Opening stock was entered wrong (no loss)".to_string();
        if delta > 0.0 {
            let newest = openings.last().expect("caller passes at least one opening lot");
            let opt = SetStockOption {
                mode: SetStockMode::OpeningCorrection,
                label,
                detail: format!(
                    "Raises the opening stock by {delta} {unit} at its cost (₹{}). No adjustment is recorded.",
                    newest.unit_cost
                ),
                enabled: true,
                disabled_reason: None,
                takes_unit_cost: false,
            };
            return Ok((opt, Vec::new()));
        }
        let need = -delta;
        // DD-95: a lot can come down to its floor only, so restocked return units are not unused.
        let mut unused = 0.0;
        let mut restocked = 0.0;
        for l in openings {
            let (u, r) = Self::opening_unused(tx, l.purchase_item_id)?;
            unused += u;
            restocked += r;
        }
        let mut opt = SetStockOption {
            mode: SetStockMode::OpeningCorrection,
            label,
            detail: format!("Takes {need} {unit} off the unused opening stock. No loss is booked."),
            enabled: true,
            disabled_reason: None,
            takes_unit_cost: false,
        };
        let mut undoable = Vec::new();
        if unused + EPS < need {
            let lot_ids: Vec<i64> = openings.iter().map(|l| l.purchase_item_id).collect();
            let writeoffs = Self::writeoffs_on_lots(tx, &lot_ids)?;
            let unused_r = (unused * 1e6).round() / 1e6;
            let mut reason = format!(
                "Only {unused_r} {unit} of the opening stock is unused, so it cannot take off {need} {unit}."
            );
            if let Some(named) = Self::name_writeoffs(&writeoffs, unit) {
                reason.push_str(&format!(" The rest is used, including {named}: undo those first, then set the stock again."));
                for a in &writeoffs {
                    if let Some(w) = self.undo_probe(tx, a)? {
                        undoable.push(w);
                    }
                }
            } else {
                reason.push_str(" The rest has been sold.");
            }
            if restocked > EPS {
                reason.push_str(" Units back from a customer return count as sold until that return is deleted.");
            }
            opt.enabled = false;
            opt.disabled_reason = Some(reason);
        }
        Ok((opt, undoable))
    }

    /// S2 dry run of a `RestoreOutflow` step: every lot must have room for the units it gets back,
    /// so the delete guard shows `STOCK_INVARIANT` before the delete is tried.
    fn check_outflow_room(tx: &mut dyn TransactionContext, source_type: OutflowSourceType, source_id: i64) -> Result<(), AppError> {
        let mut per_lot: Vec<(i64, f64)> = Vec::new();
        for o in tx.outflow_allocations().find_by_source(source_type, source_id)? {
            match per_lot.iter_mut().find(|(l, _)| *l == o.purchase_item_id) {
                Some(e) => e.1 += o.quantity_depleted,
                None => per_lot.push((o.purchase_item_id, o.quantity_depleted)),
            }
        }
        for (lot_id, back) in per_lot {
            let lot = tx
                .purchases()
                .find_purchase_item_by_id(lot_id)?
                .ok_or_else(|| AppError::not_found("PurchaseItem", lot_id.to_string()))?;
            if lot.remaining_quantity + back > lot.quantity + EPS {
                return Err(stock_invariant(tx, &lot)?);
            }
        }
        Ok(())
    }

    /// DD-95: `(unused, restocked)` of one opening lot: `unused = quantity - floor` is what an
    /// opening correction may take off; `restocked` = units back from returns still in `remaining`.
    fn opening_unused(tx: &mut dyn TransactionContext, lot_id: i64) -> Result<(f64, f64), AppError> {
        let lot = tx
            .purchases()
            .find_purchase_item_by_id(lot_id)?
            .ok_or_else(|| AppError::not_found("PurchaseItem", lot_id.to_string()))?;
        let unused = (lot.quantity - lot_floor(tx, &lot)?).max(0.0);
        Ok((unused, (lot.remaining_quantity - unused).max(0.0)))
    }

    /// Applies an opening correction of `delta` through G6's `update_opening_quantity_in_tx`:
    /// down takes unused remaining from the newest opening lot first; up raises the newest one.
    /// Returns `(value_change, unit_cost of the raised lot)`.
    fn apply_opening_correction(
        &self,
        tx: &mut dyn TransactionContext,
        openings: &[ItemLot],
        delta: f64,
    ) -> Result<(f64, Option<f64>), AppError> {
        if delta > 0.0 {
            let newest = openings.last().expect("caller passes at least one opening lot");
            let lot = tx
                .purchases()
                .find_purchase_item_by_id(newest.purchase_item_id)?
                .ok_or_else(|| AppError::not_found("PurchaseItem", newest.purchase_item_id.to_string()))?;
            let r = self.update_opening_quantity_in_tx(
                tx,
                &UpdateOpeningQuantityPayload { purchase_item_id: lot.id, quantity: lot.quantity + delta },
            )?;
            return Ok((r.value_change, Some(lot.unit_cost)));
        }
        let mut left = -delta;
        let mut value_change = 0.0;
        for l in openings.iter().rev() {
            if left <= EPS {
                break;
            }
            let lot = tx
                .purchases()
                .find_purchase_item_by_id(l.purchase_item_id)?
                .ok_or_else(|| AppError::not_found("PurchaseItem", l.purchase_item_id.to_string()))?;
            // DD-95: down to the floor only (restocked return units stay).
            let floor = lot_floor(tx, &lot)?;
            let unused = (lot.quantity - floor).max(0.0);
            let take = unused.min(left);
            if take <= EPS {
                continue;
            }
            let quantity = if (unused - take).abs() <= EPS {
                floor // exactly the floor
            } else {
                lot.quantity - take
            };
            let r = self.update_opening_quantity_in_tx(tx, &UpdateOpeningQuantityPayload { purchase_item_id: lot.id, quantity })?;
            value_change += r.value_change;
            left -= take;
        }
        if left > EPS {
            // The option check above makes this unreachable; never write a partial correction.
            return Err(locked("OPENING_STOCK_USED", "The unused opening stock cannot cover this decrease"));
        }
        Ok((round2(value_change), None))
    }

    /// Body of `set_item_stock` inside the caller's transaction; shared with the preview (DD-56).
    fn set_item_stock_in_tx(
        &self,
        tx: &mut dyn TransactionContext,
        p: &SetItemStockPayload,
    ) -> Result<SetItemStockResult, AppError> {
        if !p.counted_quantity.is_finite() || p.counted_quantity < 0.0 {
            return Err(validation("counted_quantity", "Counted stock must be a finite number, 0 or more"));
        }
        check_entry_date_now("adjustment_date", &p.adjustment_date)?;
        if let Some(c) = p.unit_cost {
            if !c.is_finite() || c < 0.0 {
                return Err(validation("unit_cost", format!("Unit cost must be a finite, non-negative number (got {c})")));
            }
        }

        let item = find_item(tx, p.item_id)?;
        require_stockable(&item)?;
        let previous = item.current_stock;
        let delta = ((p.counted_quantity - previous) * 1e6).round() / 1e6;
        let mut out = SetItemStockResult {
            adjustment_id: None,
            delta: 0.0,
            reason: None,
            previous_stock: previous,
            new_stock: previous,
            unit_cost: None,
            value_change: 0.0,
            mode: None,
            options: Vec::new(),
            choice_required: false,
            undoable_writeoffs: Vec::new(),
        };
        if delta.abs() <= EPS {
            // S9: counting 0 over a float residue stores 0, so the item can never read as low at level 0.
            if p.counted_quantity < crate::core::stock_rules::STOCK_RESIDUE_EPS
                && previous != 0.0
                && previous.abs() < crate::core::stock_rules::STOCK_RESIDUE_EPS
            {
                tx.items().update_stock_cache(item.id, 0.0)?;
                out.new_stock = 0.0;
            }
            return Ok(out); // No adjustment written.
        }
        out.delta = delta;

        // FR-57 scope B: the paths for this change, decided here (DD-56). Probes run before any write.
        let openings: Vec<ItemLot> = tx
            .purchases()
            .list_lots_for_item(item.id)?
            .into_iter()
            .filter(|l| l.source == LotSource::Opening)
            .collect();
        if delta > 0.0 {
            out.undoable_writeoffs = self.undoable_writeoffs(tx, item.id)?;
        }
        if !openings.is_empty() {
            let (opt, undoable) = self.opening_option(tx, &item, &openings, delta)?;
            out.options.push(opt);
            if delta < 0.0 {
                out.undoable_writeoffs = undoable;
            }
        }
        out.options.push(if delta > 0.0 {
            SetStockOption {
                mode: SetStockMode::Adjustment,
                label: "Found / unrecorded stock".to_string(),
                detail: "Adds a lot at the unit cost. No gain is booked: profit comes when it sells.".to_string(),
                enabled: true,
                disabled_reason: None,
                takes_unit_cost: true,
            }
        } else {
            SetStockOption {
                mode: SetStockMode::Adjustment,
                label: "Lost, damaged or miscounted (loss)".to_string(),
                detail: "Records a count correction: its FIFO cost is an Inventory Loss.".to_string(),
                enabled: true,
                disabled_reason: None,
                takes_unit_cost: false,
            }
        });
        let mode = match p.mode {
            Some(m) => {
                let opt = out
                    .options
                    .iter()
                    .find(|o| o.mode == m)
                    .ok_or_else(|| validation("mode", format!("'{}' has no opening stock to correct", item.name)))?;
                if !opt.enabled {
                    return Err(validation("mode", opt.disabled_reason.clone().unwrap_or_default()));
                }
                m
            }
            None => {
                let enabled: Vec<SetStockMode> = out.options.iter().filter(|o| o.enabled).map(|o| o.mode).collect();
                if enabled.len() != 1 {
                    // Two paths: the user picks; nothing is applied (the commit refuses this result).
                    out.choice_required = true;
                    return Ok(out);
                }
                enabled[0]
            }
        };
        out.mode = Some(mode);

        match mode {
            SetStockMode::Adjustment => {
                let (reason, cost_in) = if delta > 0.0 {
                    (AdjustmentReason::UnrecordedStock, p.unit_cost)
                } else {
                    (AdjustmentReason::CountCorrection, None)
                };
                let (id, loss, cost_used) =
                    self.post_adjustment(tx, &item, &p.adjustment_date, delta, reason, cost_in, p.notes.clone())?;
                out.adjustment_id = Some(id);
                out.reason = Some(reason);
                out.unit_cost = cost_used;
                out.value_change = if delta > 0.0 { round2(delta * cost_used.unwrap_or(0.0)) } else { -round2(loss) };
            }
            SetStockMode::OpeningCorrection => {
                let (value_change, cost) = self.apply_opening_correction(tx, &openings, delta)?;
                out.unit_cost = cost;
                out.value_change = value_change;
            }
        }
        let mut new_stock = tx.items().find_by_id(item.id)?.map(|i| i.current_stock).unwrap_or(previous + delta);
        if new_stock != 0.0 && new_stock.abs() < crate::core::stock_rules::STOCK_RESIDUE_EPS {
            tx.items().update_stock_cache(item.id, 0.0)?; // S9
            new_stock = 0.0;
        }
        out.new_stock = new_stock;
        Ok(out)
    }

    /// Body of `undo_stock_adjustment` inside the caller's transaction; shared with the guard (DD-56, FR-52).
    /// Every refusal is a `BillLocked` error so the caller can show its message.
    fn undo_adjustment_in_tx(&self, tx: &mut dyn TransactionContext, adjustment_id: i64) -> Result<AdjustmentUndoGuard, AppError> {
        let adj = tx
            .adjustments()
            .find_by_id(adjustment_id)?
            .ok_or_else(|| AppError::not_found("StockAdjustment", adjustment_id.to_string()))?;
        let item = find_item(tx, adj.item_id)?;
        if !item.is_active {
            return Err(locked("ITEM_DELETED", format!("'{}' is deleted. Restore the item first.", item.name)));
        }
        let (loss_removed, value_change);
        if adj.quantity < 0.0 {
            let mut back = 0.0;
            let mut loss = 0.0;
            for o in tx.outflow_allocations().delete_by_source(OutflowSourceType::Adjustment, adjustment_id)? {
                let lot = tx
                    .purchases()
                    .find_purchase_item_by_id(o.purchase_item_id)?
                    .ok_or_else(|| AppError::not_found("PurchaseItem", o.purchase_item_id.to_string()))?;
                let restored = lot.remaining_quantity + o.quantity_depleted;
                if restored > lot.quantity + EPS {
                    return Err(locked(
                        "LOT_OVERFLOW",
                        format!("Putting the stock back would exceed lot {} of '{}'", lot.id, lot.item_name),
                    ));
                }
                tx.purchases().update_remaining_quantity(lot.id, restored)?;
                back += o.quantity_depleted;
                loss += o.quantity_depleted * o.unit_cost;
            }
            if back > 0.0 {
                tx.items().apply_stock_delta(item.id, back)?;
            }
            loss_removed = round2(loss);
            value_change = loss_removed;
        } else {
            let headers: Vec<_> = tx
                .purchases()
                .find_adjustment_headers(adjustment_id)?
                .into_iter()
                .filter(|h| super::purchase_service_impl::adjustment_id_from_notes(h.notes.as_deref()) == Some(adjustment_id))
                .collect();
            if headers.is_empty() {
                return Err(locked("ADJUSTMENT_LOT_MISSING", "The stock lot created by this adjustment was not found"));
            }
            for h in headers {
                for lot in tx.purchases().list_items_by_transaction_id(h.id)? {
                    self.remove_purchase_lot(tx, lot.id).map_err(|e| match e {
                        AppError::BillLocked { code, .. } if code == "CONSUMED_LINE_REMOVED" => locked(
                            "ADJUSTMENT_STOCK_USED",
                            "Stock added by this adjustment has already been sold or used. Delete those sales first.",
                        ),
                        other => other,
                    })?;
                }
                tx.purchases().delete_transaction(h.id)?;
            }
            loss_removed = 0.0;
            value_change = -round2(adj.quantity * adj.unit_cost.unwrap_or(0.0));
        }
        tx.adjustments().delete_adjustment(adjustment_id)?;
        Ok(AdjustmentUndoGuard {
            can_undo: true,
            reason: None,
            quantity: adj.quantity,
            item_id: item.id,
            item_name: item.name,
            base_unit: item.base_unit,
            loss_removed,
            value_change,
        })
    }

    /// Body of `update_opening_lot_quantity` inside the caller's transaction; shared with the preview (DD-56).
    fn update_opening_quantity_in_tx(
        &self,
        tx: &mut dyn TransactionContext,
        p: &UpdateOpeningQuantityPayload,
    ) -> Result<OpeningQuantityResult, AppError> {
        if !p.quantity.is_finite() || p.quantity < 0.0 {
            return Err(validation("quantity", "Quantity must be a finite number, 0 or more"));
        }
        let lot = tx
            .purchases()
            .find_purchase_item_by_id(p.purchase_item_id)?
            .ok_or_else(|| AppError::not_found("PurchaseItem", p.purchase_item_id.to_string()))?;
        let header = tx
            .purchases()
            .find_transaction_by_id(lot.purchase_transaction_id)?
            .ok_or_else(|| AppError::not_found("PurchaseTransaction", lot.purchase_transaction_id.to_string()))?;
        if header.source != LotSource::Opening {
            return Err(validation("purchase_item_id", "Only opening stock can be edited here"));
        }
        let item = find_item(tx, lot.item_id)?;
        // D10: a deleted item is only history; editing its opening stock would use it.
        ensure_active_item(&item)?;
        let old_stock = item.current_stock;
        // DD-95: the floor counts restocked customer-return units as used.
        let consumed = lot_floor(tx, &lot)?;
        if p.quantity + EPS < consumed {
            // Checked here so 0 gets the floor message, not edit_purchase_lot's "must be positive".
            // FR-57: name the write-offs inside "already used" so the user can undo them (G7).
            let writeoffs = Self::writeoffs_on_lots(tx, &[lot.id])?;
            let message = match Self::name_writeoffs(&writeoffs, &item.base_unit) {
                Some(named) => format!(
                    "'{}': at least {consumed} already used, including {named}. Quantity cannot go below that: undo those write-offs first (Returns & Adjustments), then edit again.",
                    lot.item_name
                ),
                None => format!("'{}': at least {consumed} already used, quantity cannot go below that", lot.item_name),
            };
            return Err(locked("LOT_BELOW_CONSUMED", message));
        }
        let mut lot_removed = false;
        if p.quantity <= EPS && consumed <= EPS {
            // Unused lot: remove it, and its header when nothing else hangs off it (OPN headers are not bills).
            self.remove_purchase_lot(tx, lot.id)?;
            if tx.purchases().list_items_by_transaction_id(header.id)?.is_empty() {
                tx.purchases().delete_transaction(header.id)?;
            }
            lot_removed = true;
        } else {
            let edit = LotEdit {
                item_id: lot.item_id,
                item_name: lot.item_name.clone(),
                pack_label: lot.pack_label.clone(),
                pack_quantity: p.quantity,
                quantity: p.quantity,
                unit_cost: lot.unit_cost,
                total_amount: round2(p.quantity * lot.unit_cost),
            };
            self.edit_purchase_lot(tx, lot.id, &edit)?;
        }
        let new_stock = find_item(tx, lot.item_id)?.current_stock;
        Ok(OpeningQuantityResult {
            purchase_item_id: lot.id,
            item_id: lot.item_id,
            old_quantity: lot.quantity,
            new_quantity: if lot_removed { 0.0 } else { p.quantity },
            consumed,
            unit_cost: lot.unit_cost,
            old_stock,
            new_stock,
            value_change: round2((p.quantity - lot.quantity) * lot.unit_cost),
            lot_removed,
        })
    }
}

impl<S: InventoryAllocationStrategy, U: UnitOfWork> InventoryService for InventoryServiceImpl<S, U> {
    fn validate_stock_availability(
        &self,
        uow: &mut dyn TransactionContext,
        requirements: &[(i64, f64)],
    ) -> Result<(), AppError> {
        // Defensive merge: callers should pre-sum, but two lines of one item must be checked together.
        let mut merged: Vec<(i64, f64)> = Vec::new();
        for &(item_id, qty) in requirements {
            require_positive("quantity", qty)?;
            match merged.iter_mut().find(|(id, _)| *id == item_id) {
                Some(entry) => entry.1 += qty,
                None => merged.push((item_id, qty)),
            }
        }
        for (item_id, qty) in merged {
            self.strategy.validate_stock_availability(uow, item_id, qty)?;
        }
        Ok(())
    }

    fn deplete_stock_for_sale(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        item_id: i64,
        required_base_qty: f64,
    ) -> Result<DepletionResult, AppError> {
        let depletion = self
            .strategy
            .deplete_lots_for_sale(uow, sale_item_id, item_id, required_base_qty)?;
        let moved: f64 = depletion.allocations.iter().map(|a| a.quantity_depleted).sum();
        uow.items().apply_stock_delta(item_id, -moved)?;
        Ok(depletion)
    }

    fn add_stock_for_purchase(
        &self,
        uow: &mut dyn TransactionContext,
        purchase_item: &PurchaseItem,
    ) -> Result<(), AppError> {
        require_positive("quantity", purchase_item.quantity)?;
        reject_service(&find_item(uow, purchase_item.item_id)?)?;
        uow.purchases().create_purchase_item(purchase_item)?;
        uow.items()
            .apply_stock_delta(purchase_item.item_id, purchase_item.remaining_quantity)?;
        Ok(())
    }

    fn reverse_sale_line(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        item_id: i64,
        quantity: f64,
        restock: bool,
    ) -> Result<f64, AppError> {
        let result = self
            .strategy
            .reverse_sale_allocations(uow, sale_item_id, quantity, restock)?;
        if result.restocked_quantity > 0.0 {
            uow.items().apply_stock_delta(item_id, result.restocked_quantity)?;
        }
        Ok(result.reversed_cost)
    }

    fn release_sale_line(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        item_id: i64,
        quantity: f64,
    ) -> Result<f64, AppError> {
        let result = self.strategy.release_sale_allocations(uow, sale_item_id, item_id, quantity)?;
        if result.restocked_quantity > 0.0 {
            uow.items().apply_stock_delta(item_id, result.restocked_quantity)?;
        }
        Ok(result.reversed_cost)
    }

    fn release_whole_sale_line(&self, uow: &mut dyn TransactionContext, sale_item_id: i64) -> Result<f64, AppError> {
        // Per lot item: a service line's allocations sit on its components' lots (X4).
        let mut per_item: Vec<(i64, f64)> = Vec::new();
        for a in uow.allocations().find_by_sale_item_id(sale_item_id)? {
            let lot = uow
                .purchases()
                .find_purchase_item_by_id(a.purchase_item_id)?
                .ok_or_else(|| AppError::not_found("PurchaseItem", a.purchase_item_id.to_string()))?;
            match per_item.iter_mut().find(|(i, _)| *i == lot.item_id) {
                Some(e) => e.1 += a.quantity_depleted,
                None => per_item.push((lot.item_id, a.quantity_depleted)),
            }
        }
        let mut cost = 0.0;
        for (item_id, qty) in per_item {
            if qty > 0.0 {
                cost += self.release_sale_line(uow, sale_item_id, item_id, qty)?;
            }
        }
        // Rows within EPS of zero are deleted by the walk; nothing may remain.
        if !uow.allocations().find_by_sale_item_id(sale_item_id)?.is_empty() {
            return Err(locked(
                "STOCK_INVARIANT",
                format!("Sale line #{sale_item_id} still holds stock after a full release"),
            ));
        }
        Ok(cost)
    }

    fn create_source_lot(
        &self,
        uow: &mut dyn TransactionContext,
        source: LotSource,
        date: &str,
        item_id: i64,
        quantity: f64,
        unit_cost: Option<f64>,
        notes: Option<String>,
    ) -> Result<i64, AppError> {
        let ids = self.create_source_lots(uow, source, date, notes, &[(item_id, quantity, unit_cost)])?;
        Ok(ids[0])
    }

    fn create_source_lots(
        &self,
        uow: &mut dyn TransactionContext,
        source: LotSource,
        date: &str,
        notes: Option<String>,
        lines: &[(i64, f64, Option<f64>)],
    ) -> Result<Vec<i64>, AppError> {
        if source == LotSource::Purchase {
            return Err(validation("source", "Purchase lots are created by PurchaseService, not as source lots"));
        }
        if lines.is_empty() {
            return Ok(Vec::new());
        }
        require_date("date", date)?;
        // Validate every line before writing the header.
        let mut resolved: Vec<(Item, f64, f64)> = Vec::with_capacity(lines.len());
        for &(item_id, quantity, unit_cost) in lines {
            require_positive("quantity", quantity)?;
            let item = find_item(uow, item_id)?;
            reject_service(&item)?;
            let cost = match unit_cost {
                Some(c) => c,
                None => uow.purchases().get_latest_lot_cost(item_id)?.ok_or_else(|| {
                    validation("unit_cost", "unit cost required: item has no prior lot")
                })?,
            };
            if !cost.is_finite() || cost < 0.0 {
                return Err(validation("unit_cost", format!("Unit cost must be a finite, non-negative number (got {cost})")));
            }
            crate::core::money::check_line_total("unit_cost", quantity, cost)?;
            resolved.push((item, quantity, cost));
        }

        // DD-07: non-PURCHASE lot headers are not supplier bills (total 0, no party, PAID, no ledger).
        let header = PurchaseTransaction {
            id: 0,
            transaction_code: uow.purchases().get_next_transaction_code(source)?,
            source,
            supplier_invoice_ref: None,
            transaction_date: date.to_string(),
            party_id: None,
            party_name: None,
            payment_mode: None,
            payment_status: PaymentStatus::Paid,
            total_amount: 0.0,
            paid_amount: 0.0,
            balance_due: 0.0,
            notes,
            created_at: chrono::Utc::now().to_rfc3339(),
            // Source-lot headers are not bills: no subtotal, no charges (FR-19).
            items_subtotal: 0.0,
            charges_total: 0.0,
            discounts_total: 0.0,
            updated_at: None,
            is_opening_balance: false,
        };
        let header_id = uow.purchases().create_transaction(&header)?;
        let mut ids = Vec::with_capacity(resolved.len());
        for (item, quantity, cost) in resolved {
            let item_id = item.id;
            let lot = PurchaseItem {
                id: 0,
                purchase_transaction_id: header_id,
                item_id,
                item_name: item.name,
                pack_label: None,
                pack_quantity: quantity,
                quantity,
                unit_cost: cost,
                total_amount: round2(quantity * cost),
                remaining_quantity: quantity,
            };
            ids.push(uow.purchases().create_purchase_item(&lot)?);
            uow.items().apply_stock_delta(item_id, quantity)?;
        }
        Ok(ids)
    }

    fn issue_stock_outflow(
        &self,
        uow: &mut dyn TransactionContext,
        source_type: OutflowSourceType,
        source_id: i64,
        item_id: i64,
        quantity: f64,
    ) -> Result<f64, AppError> {
        reject_service(&find_item(uow, item_id)?)?;
        let out = self
            .strategy
            .deplete_lots_for_outflow(uow, source_type, source_id, item_id, quantity)?;
        let moved: f64 = out.allocations.iter().map(|a| a.quantity_depleted).sum();
        uow.items().apply_stock_delta(item_id, -moved)?;
        Ok(out.total_cost)
    }

    fn remove_stock_for_vendor_return(
        &self,
        uow: &mut dyn TransactionContext,
        return_id: i64,
        item_id: i64,
        lot_id: i64,
        quantity: f64,
    ) -> Result<VendorReturnCost, AppError> {
        require_positive("quantity", quantity)?;
        reject_service(&find_item(uow, item_id)?)?;
        // DD-86: always the named lot; the FIFO fall-back is gone (LLD 6.2 boundary note).
        let lot = uow
            .purchases()
            .find_purchase_item_by_id(lot_id)?
            .ok_or_else(|| AppError::not_found("PurchaseItem", lot_id.to_string()))?;
        if lot.item_id != item_id {
            return Err(validation("purchase_item_id", format!("Lot #{lot_id} does not belong to item #{item_id}")));
        }
        let header = uow
            .purchases()
            .find_transaction_by_id(lot.purchase_transaction_id)?
            .ok_or_else(|| AppError::not_found("PurchaseTransaction", lot.purchase_transaction_id.to_string()))?;
        if header.source != LotSource::Purchase || header.is_opening_balance {
            return Err(validation("purchase_item_id", format!("Lot #{lot_id} is not a purchase lot and cannot be returned to a vendor")));
        }
        if quantity > lot.remaining_quantity + EPS {
            return Err(validation(
                "quantity",
                format!("Return quantity {quantity} exceeds the {} remaining in lot {}", lot.remaining_quantity, header.transaction_code),
            ));
        }
        let take = quantity.min(lot.remaining_quantity);
        uow.purchases()
            .update_remaining_quantity(lot_id, lot.remaining_quantity - take)?;
        uow.outflow_allocations().create_allocation(&StockOutflowAllocation {
            id: 0,
            source_type: OutflowSourceType::VendorReturn,
            source_id: return_id,
            purchase_item_id: lot_id,
            quantity_depleted: take,
            unit_cost: lot.unit_cost,
        })?;
        uow.items().apply_stock_delta(item_id, -take)?;
        let gross_cost = take * lot.unit_cost;
        Ok(VendorReturnCost {
            gross_cost,
            net_value: gross_cost * net_unit_price(1.0, header.items_subtotal, header.discounts_total),
        })
    }

    fn create_adjustment(
        &self,
        payload: StockAdjustmentPayload,
    ) -> Result<StockAdjustmentResult, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let (result, _) = self.adjustment_in_tx(&mut *tx, &payload)?;
        tx.commit()?;
        Ok(result)
    }

    fn create_adjustment_in_tx(
        &self,
        uow: &mut dyn TransactionContext,
        payload: &StockAdjustmentPayload,
    ) -> Result<StockAdjustmentResult, AppError> {
        self.adjustment_in_tx(uow, payload).map(|(result, _)| result)
    }

    fn preview_adjustment(&self, payload: StockAdjustmentPayload) -> Result<StockAdjustmentPreview, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let result = self.adjustment_in_tx(&mut *tx, &payload).map(|(_, preview)| preview);
        tx.rollback()?; // never committed
        result
    }

    fn set_item_stock(&self, p: SetItemStockPayload) -> Result<SetItemStockResult, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let result = self.set_item_stock_in_tx(&mut *tx, &p)?;
        if result.choice_required {
            // The body applied nothing; never default silently to the loss (FR-57 scope B).
            return Err(validation("mode", "Choose why the stock changed: opening stock entered wrong, or a stock adjustment"));
        }
        tx.commit()?;
        Ok(result)
    }

    fn preview_set_item_stock(&self, p: SetItemStockPayload) -> Result<SetItemStockResult, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let result = self.set_item_stock_in_tx(&mut *tx, &p);
        tx.rollback()?; // never committed
        result
    }

    fn update_opening_lot_quantity(&self, p: UpdateOpeningQuantityPayload) -> Result<OpeningQuantityResult, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let result = self.update_opening_quantity_in_tx(&mut *tx, &p)?;
        tx.commit()?;
        Ok(result)
    }

    fn preview_opening_lot_quantity(&self, p: UpdateOpeningQuantityPayload) -> Result<OpeningQuantityResult, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let result = self.update_opening_quantity_in_tx(&mut *tx, &p);
        tx.rollback()?; // never committed
        result
    }

    fn undo_stock_adjustment(&self, adjustment_id: i64) -> Result<AdjustmentUndoGuard, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let result = self.undo_adjustment_in_tx(&mut *tx, adjustment_id)?;
        tx.commit()?;
        Ok(result)
    }

    fn get_adjustment_undo_guard(&self, adjustment_id: i64) -> Result<AdjustmentUndoGuard, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let adj = tx
            .adjustments()
            .find_by_id(adjustment_id)?
            .ok_or_else(|| AppError::not_found("StockAdjustment", adjustment_id.to_string()))?;
        let item = find_item(&mut *tx, adj.item_id)?;
        let result = match self.undo_adjustment_in_tx(&mut *tx, adjustment_id) {
            Ok(g) => Ok(g),
            Err(AppError::BillLocked { code, message }) => Ok(AdjustmentUndoGuard {
                can_undo: false,
                reason: Some(AdjustmentUndoReason { code, message }),
                quantity: adj.quantity,
                item_id: item.id,
                item_name: item.name,
                base_unit: item.base_unit,
                loss_removed: 0.0,
                value_change: 0.0,
            }),
            Err(e) => Err(e),
        };
        tx.rollback()?; // never committed
        result
    }

    fn preview_lot_draw(&self, item_id: i64, quantity: f64, prior_lines: &[PriorLine]) -> Result<LotDraw, AppError> {
        if !quantity.is_finite() || quantity < 0.0 {
            return Err(validation("quantity", format!("must be a finite number >= 0 (got {quantity})")));
        }
        for p in prior_lines {
            if !p.quantity.is_finite() || p.quantity < 0.0 {
                return Err(validation("prior_lines", format!("must be finite numbers >= 0 (got {})", p.quantity)));
            }
        }
        let mut tx = self.uow.begin_read()?;
        let item = find_item(&mut *tx, item_id)?;

        // Base units each stocked item has already been promised to earlier cart lines; a service
        // line counts as its components (FR-37, expanded here so the UI never knows the rule).
        let mut prior: std::collections::HashMap<i64, f64> = std::collections::HashMap::new();
        for p in prior_lines {
            let pi = find_item(&mut *tx, p.item_id)?;
            if pi.item_type == ItemType::Service {
                for c in tx.service_components().list_for_service(pi.id)? {
                    *prior.entry(c.component_item_id).or_insert(0.0) += p.quantity * c.quantity_per_unit;
                }
            } else {
                *prior.entry(pi.id).or_insert(0.0) += p.quantity;
            }
        }

        if item.item_type == ItemType::Physical {
            let skip = prior.get(&item_id).copied().unwrap_or(0.0);
            let (lots, cost, available, shortfall) = self.draw_lots(&mut *tx, item_id, quantity, skip)?;
            return Ok(LotDraw {
                lots,
                draw_cost: cost,
                available,
                shortfall,
                components: Vec::new(),
                components_cost: 0.0,
                misc_cost: 0.0,
                line_cost: cost,
            });
        }

        let mut components = Vec::new();
        let (mut components_cost, mut units_available, mut units_short) = (0.0, f64::INFINITY, 0.0_f64);
        for c in tx.service_components().list_for_service(item_id)? {
            let comp = find_item(&mut *tx, c.component_item_id)?;
            let need = quantity * c.quantity_per_unit;
            let skip = prior.get(&comp.id).copied().unwrap_or(0.0);
            let (lots, cost, available, shortfall) = self.draw_lots(&mut *tx, comp.id, need, skip)?;
            components_cost += cost;
            units_available = units_available.min(((available - skip).max(0.0) / c.quantity_per_unit + 1e-9).floor());
            units_short = units_short.max(shortfall / c.quantity_per_unit);
            components.push(LotDrawComponent {
                item_id: comp.id,
                name: comp.name,
                base_unit: comp.base_unit,
                quantity: need,
                lots,
                draw_cost: cost,
                available,
                shortfall,
            });
        }
        let target = item.service_cost.map(|c| round2(c * quantity)).unwrap_or(0.0);
        let misc = (target - components_cost).max(0.0);
        Ok(LotDraw {
            lots: Vec::new(),
            draw_cost: components_cost,
            available: if units_available.is_finite() { units_available } else { quantity },
            shortfall: if units_short > EPS { units_short } else { 0.0 },
            components,
            components_cost,
            misc_cost: misc,
            line_cost: components_cost + misc,
        })
    }

    fn preview_service_cost(&self, payload: PreviewServiceCostPayload) -> Result<ServiceCostPreview, AppError> {
        let service_cost = crate::services::r#impl::item_service_impl::validate_service_cost(payload.service_cost)?;
        let mut tx = self.uow.begin_read()?;
        let rows = crate::services::r#impl::item_service_impl::validate_components(&mut *tx, None, &payload.components)?;
        let mut components_cost = 0.0;
        for (component_id, qty) in rows {
            components_cost += self.draw_lots(&mut *tx, component_id, qty, 0.0)?.1;
        }
        let misc = (service_cost.unwrap_or(0.0) - components_cost).max(0.0);
        Ok(ServiceCostPreview {
            components_cost: round2(components_cost),
            misc_cost: round2(misc),
            unit_cost: round2(components_cost + misc),
        })
    }

    fn get_item_lots(&self, item_id: i64) -> Result<Vec<ItemLot>, AppError> {
        let mut tx = self.uow.begin_read()?;
        let mut lots = tx.purchases().list_lots_for_item(item_id)?;
        // DD-92 (S7): the strategy's own draw order (FIFO = entry order, by id), never the date.
        let order = self.strategy.open_lots_in_draw_order(&mut *tx, item_id)?;
        for l in lots.iter_mut() {
            l.fifo_rank = order.iter().position(|o| o.id == l.purchase_item_id).map(|i| i as i64 + 1);
        }
        Ok(lots)
    }

    fn get_item_sales(&self, item_id: i64, page: Option<usize>, limit: Option<usize>) -> Result<Paginated<ItemBillRef>, AppError> {
        let req = item_history_page(page, limit)?;
        let mut tx = self.uow.begin_read()?;
        find_item(&mut *tx, item_id)?;
        let (rows, total) = tx.sales().list_bills_for_item(item_id, req.offset(), req.limit)?;
        Ok(Paginated::new(rows, total, req.page, req.limit))
    }

    fn get_item_purchases(&self, item_id: i64, page: Option<usize>, limit: Option<usize>) -> Result<Paginated<ItemBillRef>, AppError> {
        let req = item_history_page(page, limit)?;
        let mut tx = self.uow.begin_read()?;
        find_item(&mut *tx, item_id)?;
        let (rows, total) = tx.purchases().list_bills_for_item(item_id, req.offset(), req.limit)?;
        Ok(Paginated::new(rows, total, req.page, req.limit))
    }

    fn list_adjustments(
        &self,
        item_id: Option<i64>,
        page: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Paginated<StockAdjustment>, AppError> {
        let req = PageRequest::new(page, limit)?;
        let mut tx = self.uow.begin_read()?;
        let (items, total) = tx.adjustments().list_adjustments(item_id, req.offset(), req.limit)?;
        Ok(Paginated::new(items, total, req.page, req.limit))
    }

    fn verify_stock_cache(
        &self,
        uow: &mut dyn TransactionContext,
    ) -> Result<Vec<(i64, f64, f64)>, AppError> {
        Ok(uow
            .items()
            .stock_cache_audit()?
            .into_iter()
            .filter(|(_, cached, lots)| (cached - lots).abs() > EPS)
            .collect())
    }

    fn repair_stock_cache(
        &self,
        uow: &mut dyn TransactionContext,
    ) -> Result<Vec<(i64, f64, f64)>, AppError> {
        let bad = self.verify_stock_cache(uow)?;
        for &(item_id, cached, lots) in &bad {
            uow.items().apply_stock_delta(item_id, lots - cached)?;
        }
        uow.items().recompute_low_stock_flags()?;
        Ok(bad)
    }

    fn reverse_sale_bill_stock(&self, uow: &mut dyn TransactionContext, sale_id: i64) -> Result<(), AppError> {
        let mut per_item: Vec<(i64, f64)> = Vec::new();
        for line in uow.sales().list_items_by_transaction_id(sale_id)? {
            for a in uow.allocations().find_by_sale_item_id(line.id)? {
                let back = a.quantity_depleted - a.returned_quantity;
                if back <= EPS {
                    continue;
                }
                let lot = uow
                    .purchases()
                    .find_purchase_item_by_id(a.purchase_item_id)?
                    .ok_or_else(|| AppError::not_found("PurchaseItem", a.purchase_item_id.to_string()))?;
                restore_to_lot(uow, &lot, back)?;
                // X4: credit the lot's own item. A service line's allocations sit on its components' lots.
                match per_item.iter_mut().find(|(i, _)| *i == lot.item_id) {
                    Some(e) => e.1 += back,
                    None => per_item.push((lot.item_id, back)),
                }
            }
            uow.allocations().delete_by_sale_item_id(line.id)?;
        }
        for (item_id, qty) in per_item {
            uow.items().apply_stock_delta(item_id, qty)?;
        }
        Ok(())
    }

    fn edit_purchase_lot(&self, uow: &mut dyn TransactionContext, lot_id: i64, e: &LotEdit) -> Result<(), AppError> {
        let lot = uow
            .purchases()
            .find_purchase_item_by_id(lot_id)?
            .ok_or_else(|| AppError::not_found("PurchaseItem", lot_id.to_string()))?;
        require_positive("quantity", e.quantity)?;
        let consumed = lot.quantity - lot.remaining_quantity;
        // DD-95: never below what was drawn, restocked return units included.
        let floor = lot_floor(uow, &lot)?;
        let referenced = uow.purchases().is_lot_referenced(lot_id)?;
        if e.item_id != lot.item_id {
            if referenced {
                return Err(locked(
                    "CONSUMED_LINE_CHANGED",
                    format!("'{}' has already been sold or moved, so its item cannot change", lot.item_name),
                ));
            }
            find_item(uow, e.item_id)?;
        }
        if e.quantity + EPS < floor {
            return Err(locked(
                "LOT_BELOW_CONSUMED",
                format!("'{}': at least {floor} already used, quantity cannot go below that", lot.item_name),
            ));
        }
        let new_remaining = (e.quantity - consumed).max(0.0);
        let cost_changed = (e.unit_cost - lot.unit_cost).abs() > 1e-9;
        if cost_changed && uow.outflow_allocations().exists_for_lot(lot_id, OutflowSourceType::VendorReturn)? {
            return Err(locked(
                "VENDOR_RETURNED_COST_CHANGE",
                format!("'{}' was returned to the vendor at its old cost; delete that vendor return to change the cost", lot.item_name),
            ));
        }
        uow.purchases().update_purchase_item(&PurchaseItem {
            id: lot_id,
            purchase_transaction_id: lot.purchase_transaction_id,
            item_id: e.item_id,
            item_name: e.item_name.clone(),
            pack_label: e.pack_label.clone(),
            pack_quantity: e.pack_quantity,
            quantity: e.quantity,
            unit_cost: e.unit_cost,
            total_amount: e.total_amount,
            remaining_quantity: new_remaining,
        })?;
        if e.item_id == lot.item_id {
            let delta = new_remaining - lot.remaining_quantity;
            if delta.abs() > 0.0 {
                uow.items().apply_stock_delta(e.item_id, delta)?;
            }
        } else {
            uow.items().apply_stock_delta(lot.item_id, -lot.remaining_quantity)?;
            uow.items().apply_stock_delta(e.item_id, new_remaining)?;
        }
        if cost_changed && referenced {
            self.recost_lot(uow, lot_id, e.unit_cost)?;
        }
        Ok(())
    }

    fn remove_purchase_lot(&self, uow: &mut dyn TransactionContext, lot_id: i64) -> Result<(), AppError> {
        let lot = uow
            .purchases()
            .find_purchase_item_by_id(lot_id)?
            .ok_or_else(|| AppError::not_found("PurchaseItem", lot_id.to_string()))?;
        if uow.purchases().is_lot_referenced(lot_id)? {
            return Err(locked(
                "CONSUMED_LINE_REMOVED",
                format!("'{}' has already been sold or moved, so its line cannot be removed", lot.item_name),
            ));
        }
        uow.items().apply_stock_delta(lot.item_id, -lot.remaining_quantity)?;
        uow.purchases().delete_purchase_item(lot_id)?;
        Ok(())
    }

    fn plan_return_undo(&self, uow: &mut dyn TransactionContext, r: &ReturnTransaction) -> Result<ReturnUndoPlan, AppError> {
        let mut plan = ReturnUndoPlan::default();
        match r.return_type {
            ReturnType::Vendor => {
                Self::check_outflow_room(uow, OutflowSourceType::VendorReturn, r.id)?;
                plan.steps.push(ReturnUndoStep::RestoreOutflow {
                    source_type: OutflowSourceType::VendorReturn,
                    source_id: r.id,
                });
            }
            ReturnType::Customer => {
                let mut restock_need: Vec<(i64, f64)> = Vec::new();
                let mut claimed: Vec<i64> = Vec::new();
                // Replacement units go back first, so they count as available when the returned
                // (restocked) units are taken out again.
                let mut restored: Vec<(i64, f64)> = Vec::new();
                if r.settlement_type == SettlementType::Replacement {
                    Self::check_outflow_room(uow, OutflowSourceType::Replacement, r.id)?;
                    plan.steps.push(ReturnUndoStep::RestoreOutflow {
                        source_type: OutflowSourceType::Replacement,
                        source_id: r.id,
                    });
                    for o in uow.outflow_allocations().find_by_source(OutflowSourceType::Replacement, r.id)? {
                        match restored.iter_mut().find(|(l, _)| *l == o.purchase_item_id) {
                            Some(e) => e.1 += o.quantity_depleted,
                            None => restored.push((o.purchase_item_id, o.quantity_depleted)),
                        }
                    }
                }
                let items = uow.returns().find_items_by_return_id(r.id)?;
                for ri in items.iter().filter(|i| i.line_type == ReturnLineType::Returned) {
                    let restock = ri.condition == Some(StockCondition::Restockable);
                    if let Some(sid) = ri.sale_item_id {
                        let allocs = uow.allocations().find_by_sale_item_id(sid)?;
                        let ris = uow.returns().list_customer_returned_items_for_sale_item(sid)?;
                        let qtys: Vec<f64> = ris.iter().map(|x| x.quantity).collect();
                        let takes = self.strategy.replay_reversals(&allocs, &qtys);
                        let idx = ris
                            .iter()
                            .position(|x| x.id == ri.id)
                            .ok_or_else(|| AppError::not_found("ReturnItem", ri.id.to_string()))?;
                        let mine = takes[idx].clone();
                        if restock {
                            for (aid, t) in &mine {
                                if let Some(a) = allocs.iter().find(|a| a.id == *aid) {
                                    match restock_need.iter_mut().find(|(l, _)| *l == a.purchase_item_id) {
                                        Some(e) => e.1 += t,
                                        None => restock_need.push((a.purchase_item_id, *t)),
                                    }
                                }
                            }
                        }
                        plan.steps.push(ReturnUndoStep::Unreverse {
                            sale_item_id: sid,
                            item_id: ri.item_id,
                            takes: mine,
                            restock,
                        });
                    } else if restock {
                        let lot_id = Self::resolve_return_lot(uow, r, ri, &claimed)?;
                        claimed.push(lot_id);
                        let lot = uow
                            .purchases()
                            .find_purchase_item_by_id(lot_id)?
                            .ok_or_else(|| locked("RETURN_LOT_USED", format!("The stock lot created by {} was not found", r.return_code)))?;
                        if uow.purchases().is_lot_referenced_except_return(lot_id, r.id)?
                            || lot.remaining_quantity + EPS < lot.quantity
                        {
                            return Err(locked(
                                "RETURN_LOT_USED",
                                format!("Stock restocked by {} has already been sold or moved", r.return_code),
                            ));
                        }
                        plan.steps.push(ReturnUndoStep::RemoveReturnLot { lot_id });
                    }
                }
                for (lot_id, need) in restock_need {
                    let lot = uow
                        .purchases()
                        .find_purchase_item_by_id(lot_id)?
                        .ok_or_else(|| AppError::not_found("PurchaseItem", lot_id.to_string()))?;
                    let back = restored.iter().find(|(l, _)| *l == lot_id).map(|(_, q)| *q).unwrap_or(0.0);
                    if lot.remaining_quantity + back + EPS < need {
                        return Err(locked(
                            "RESTOCKED_CONSUMED",
                            format!(
                                "{} units of '{}' restocked by {} have already been sold or moved",
                                need, lot.item_name, r.return_code
                            ),
                        ));
                    }
                }
            }
        }
        Ok(plan)
    }

    fn apply_return_undo(&self, uow: &mut dyn TransactionContext, plan: &ReturnUndoPlan) -> Result<(), AppError> {
        for step in &plan.steps {
            match step {
                ReturnUndoStep::Unreverse { sale_item_id, item_id, takes, restock } => {
                    let allocs = uow.allocations().find_by_sale_item_id(*sale_item_id)?;
                    let mut removed = 0.0;
                    for (aid, take) in takes {
                        let a = allocs
                            .iter()
                            .find(|a| a.id == *aid)
                            .ok_or_else(|| AppError::not_found("SaleStockAllocation", aid.to_string()))?;
                        uow.allocations()
                            .update_returned_quantity(a.id, (a.returned_quantity - take).max(0.0))?;
                        if *restock {
                            let lot = uow
                                .purchases()
                                .find_purchase_item_by_id(a.purchase_item_id)?
                                .ok_or_else(|| AppError::not_found("PurchaseItem", a.purchase_item_id.to_string()))?;
                            if lot.remaining_quantity + EPS < *take {
                                return Err(locked("RESTOCKED_CONSUMED", "Restocked units have already been sold or moved"));
                            }
                            uow.purchases()
                                .update_remaining_quantity(lot.id, (lot.remaining_quantity - take).max(0.0))?;
                            removed += take;
                        }
                    }
                    if removed > 0.0 {
                        uow.items().apply_stock_delta(*item_id, -removed)?;
                    }
                }
                ReturnUndoStep::RemoveReturnLot { lot_id } => {
                    let lot = uow
                        .purchases()
                        .find_purchase_item_by_id(*lot_id)?
                        .ok_or_else(|| AppError::not_found("PurchaseItem", lot_id.to_string()))?;
                    uow.items().apply_stock_delta(lot.item_id, -lot.remaining_quantity)?;
                    uow.purchases().delete_purchase_item(lot.id)?;
                    if uow.purchases().list_items_by_transaction_id(lot.purchase_transaction_id)?.is_empty() {
                        uow.purchases().delete_transaction(lot.purchase_transaction_id)?;
                    }
                }
                ReturnUndoStep::RestoreOutflow { source_type, source_id } => {
                    for o in uow.outflow_allocations().delete_by_source(*source_type, *source_id)? {
                        let lot = uow
                            .purchases()
                            .find_purchase_item_by_id(o.purchase_item_id)?
                            .ok_or_else(|| AppError::not_found("PurchaseItem", o.purchase_item_id.to_string()))?;
                        restore_to_lot(uow, &lot, o.quantity_depleted)?;
                        uow.items().apply_stock_delta(lot.item_id, o.quantity_depleted)?;
                    }
                }
            }
        }
        Ok(())
    }
}
