use std::sync::Arc;
use crate::core::constants::EPS;
use crate::core::dtos::{
    ItemLot, PageRequest, Paginated, SetItemStockPayload, SetItemStockResult, StockAdjustmentPayload,
    StockAdjustmentResult,
};
use crate::core::money::round2;
use crate::core::entities::{Item, PurchaseItem, PurchaseTransaction, StockAdjustment, StockOutflowAllocation};
use crate::core::error::AppError;
use crate::core::value_objects::{AdjustmentReason, ItemType, LotSource, OutflowSourceType, PaymentStatus};
use crate::data::contracts::unit_of_work::{TransactionContext, UnitOfWork};
use crate::services::contracts::inventory_allocation_strategy::{
    DepletionResult, InventoryAllocationStrategy,
};
use crate::services::contracts::inventory_service::InventoryService;

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

/// Active and PHYSICAL (FR-21): the only items whose stock may be adjusted.
fn require_stockable(item: &Item) -> Result<(), AppError> {
    if !item.is_active {
        return Err(validation("item_id", format!("Item '{}' is inactive", item.name)));
    }
    if item.item_type != ItemType::Physical {
        return Err(validation("item_id", format!("'{}' is a service item and has no stock", item.name)));
    }
    Ok(())
}

impl<S: InventoryAllocationStrategy, U: UnitOfWork> InventoryServiceImpl<S, U> {
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
        find_item(uow, purchase_item.item_id)?;
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
            let cost = match unit_cost {
                Some(c) => c,
                None => uow.purchases().get_latest_lot_cost(item_id)?.ok_or_else(|| {
                    validation("unit_cost", "unit cost required: item has no prior lot")
                })?,
            };
            if !cost.is_finite() || cost < 0.0 {
                return Err(validation("unit_cost", format!("Unit cost must be a finite, non-negative number (got {cost})")));
            }
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
                total_amount: quantity * cost,
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
        purchase_item_id: Option<i64>,
        quantity: f64,
    ) -> Result<f64, AppError> {
        require_positive("quantity", quantity)?;
        let Some(lot_id) = purchase_item_id else {
            return self.issue_stock_outflow(uow, OutflowSourceType::VendorReturn, return_id, item_id, quantity);
        };

        // A named lot is the user's choice, not a strategy concern (LLD 6.2 boundary note).
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
        if header.source != LotSource::Purchase {
            return Err(validation("purchase_item_id", format!("Lot #{lot_id} is not a purchase lot and cannot be returned to a vendor")));
        }
        if quantity > lot.remaining_quantity + EPS {
            return Err(validation(
                "quantity",
                format!("Return quantity {quantity} exceeds the {} remaining in lot #{lot_id}", lot.remaining_quantity),
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
        Ok(take * lot.unit_cost)
    }

    fn create_adjustment(
        &self,
        payload: StockAdjustmentPayload,
    ) -> Result<StockAdjustmentResult, AppError> {
        if !payload.quantity.is_finite() || payload.quantity == 0.0 {
            return Err(validation("quantity", "Adjustment quantity must be finite and non-zero"));
        }
        require_date("adjustment_date", &payload.adjustment_date)?;
        if let Some(c) = payload.unit_cost {
            if !c.is_finite() || c < 0.0 {
                return Err(validation("unit_cost", format!("Unit cost must be a finite, non-negative number (got {c})")));
            }
        }

        if payload.reason == AdjustmentReason::UnrecordedStock && payload.quantity < 0.0 {
            return Err(validation("quantity", "Unrecorded stock can only add stock; use Count correction to reduce it"));
        }

        let mut tx = self.uow.begin_transaction()?;
        let item = find_item(&mut *tx, payload.item_id)?;
        require_stockable(&item)?;
        let (adjustment_id, loss_value, _) = self.post_adjustment(
            &mut *tx,
            &item,
            &payload.adjustment_date,
            payload.quantity,
            payload.reason,
            payload.unit_cost,
            payload.notes.clone(),
        )?;
        tx.commit()?;
        Ok(StockAdjustmentResult { adjustment_id, loss_value })
    }

    fn set_item_stock(&self, p: SetItemStockPayload) -> Result<SetItemStockResult, AppError> {
        if !p.counted_quantity.is_finite() || p.counted_quantity < 0.0 {
            return Err(validation("counted_quantity", "Counted stock must be a finite number, 0 or more"));
        }
        require_date("adjustment_date", &p.adjustment_date)?;
        if let Some(c) = p.unit_cost {
            if !c.is_finite() || c < 0.0 {
                return Err(validation("unit_cost", format!("Unit cost must be a finite, non-negative number (got {c})")));
            }
        }

        let mut tx = self.uow.begin_transaction()?;
        let item = find_item(&mut *tx, p.item_id)?;
        require_stockable(&item)?;
        let previous = item.current_stock;
        let delta = ((p.counted_quantity - previous) * 1e6).round() / 1e6;
        if delta.abs() <= EPS {
            // Nothing written; dropping the context rolls back.
            return Ok(SetItemStockResult {
                adjustment_id: None,
                delta: 0.0,
                reason: None,
                previous_stock: previous,
                new_stock: previous,
                unit_cost: None,
                value_change: 0.0,
            });
        }
        let (reason, cost_in) = if delta > 0.0 {
            (AdjustmentReason::UnrecordedStock, p.unit_cost)
        } else {
            (AdjustmentReason::CountCorrection, None)
        };
        let (id, loss, cost_used) =
            self.post_adjustment(&mut *tx, &item, &p.adjustment_date, delta, reason, cost_in, p.notes.clone())?;
        let new_stock = tx.items().find_by_id(item.id)?.map(|i| i.current_stock).unwrap_or(previous + delta);
        tx.commit()?;
        let value_change = if delta > 0.0 { round2(delta * cost_used.unwrap_or(0.0)) } else { -round2(loss) };
        Ok(SetItemStockResult {
            adjustment_id: Some(id),
            delta,
            reason: Some(reason),
            previous_stock: previous,
            new_stock,
            unit_cost: cost_used,
            value_change,
        })
    }

    fn get_item_lots(&self, item_id: i64) -> Result<Vec<ItemLot>, AppError> {
        let mut tx = self.uow.begin_read()?;
        tx.purchases().list_lots_for_item(item_id)
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
}
