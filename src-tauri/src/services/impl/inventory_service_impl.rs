use std::sync::Arc;
use crate::core::dtos::{PaginatedList, StockAdjustmentPayload};
use crate::core::entities::{PurchaseItem, PurchaseTransaction, StockAdjustment};
use crate::core::error::AppError;
use crate::core::value_objects::{LotSource, PaymentStatus};
use crate::data::contracts::unit_of_work::{TransactionContext, UnitOfWork};
use crate::services::contracts::inventory_allocation_strategy::{
    DepletionResult, InventoryAllocationStrategy,
};
use crate::services::contracts::inventory_service::InventoryService;
use super::fifo_strategy::FifoStrategy;

#[derive(Clone)]
pub struct InventoryServiceImpl<
    S: InventoryAllocationStrategy = FifoStrategy,
    U: UnitOfWork = Arc<dyn UnitOfWork>,
> {
    pub strategy: S,
    pub uow: U,
}

impl<S: InventoryAllocationStrategy, U: UnitOfWork> InventoryServiceImpl<S, U> {
    pub fn new(strategy: S, uow: U) -> Self {
        Self { strategy, uow }
    }
}

impl<S: InventoryAllocationStrategy, U: UnitOfWork> InventoryService for InventoryServiceImpl<S, U> {
    fn validate_stock_availability(
        &self,
        uow: &mut dyn TransactionContext,
        item_id: i64,
        required_base_qty: f64,
    ) -> Result<(), AppError> {
        self.strategy
            .validate_stock_availability(uow, item_id, required_base_qty)
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
        let item = uow
            .items()
            .find_by_id(item_id)?
            .ok_or_else(|| AppError::NotFound {
                entity: "Item".to_string(),
                id: item_id.to_string(),
            })?;
        let new_stock = (item.current_stock - required_base_qty).max(0.0);
        let is_low_stock = new_stock < 5.0;
        uow.items()
            .update_stock_cache(item_id, new_stock, is_low_stock)?;
        Ok(depletion)
    }

    fn add_stock_for_purchase(
        &self,
        uow: &mut dyn TransactionContext,
        purchase_item: &PurchaseItem,
    ) -> Result<(), AppError> {
        uow.purchases().create_purchase_item(purchase_item)?;
        let item = uow
            .items()
            .find_by_id(purchase_item.item_id)?
            .ok_or_else(|| AppError::NotFound {
                entity: "Item".to_string(),
                id: purchase_item.item_id.to_string(),
            })?;
        let new_stock = item.current_stock + purchase_item.quantity;
        let is_low_stock = new_stock < 5.0;
        uow.items()
            .update_stock_cache(item.id, new_stock, is_low_stock)?;
        Ok(())
    }

    fn restore_stock_for_return(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        item_id: i64,
        quantity_to_restore: f64,
    ) -> Result<(), AppError> {
        self.strategy
            .restore_lots_for_return(uow, sale_item_id, quantity_to_restore)?;
        let item = uow
            .items()
            .find_by_id(item_id)?
            .ok_or_else(|| AppError::NotFound {
                entity: "Item".to_string(),
                id: item_id.to_string(),
            })?;
        let new_stock = item.current_stock + quantity_to_restore;
        let is_low_stock = new_stock < 5.0;
        uow.items()
            .update_stock_cache(item.id, new_stock, is_low_stock)?;
        Ok(())
    }

    fn remove_stock_for_vendor_return(
        &self,
        uow: &mut dyn TransactionContext,
        item_id: i64,
        purchase_item_id: Option<i64>,
        quantity: f64,
    ) -> Result<(), AppError> {
        let item = uow
            .items()
            .find_by_id(item_id)?
            .ok_or_else(|| AppError::NotFound {
                entity: "Item".to_string(),
                id: item_id.to_string(),
            })?;
        if item.current_stock < quantity {
            return Err(AppError::InsufficientStockError {
                item_id,
                item_name: item.name.clone(),
                required: quantity,
                available: item.current_stock,
            });
        }
        if let Some(lot_id) = purchase_item_id {
            let lots = uow.purchases().list_active_lots_for_item(item_id)?;
            if let Some(lot) = lots.iter().find(|l| l.id == lot_id) {
                let new_rem = (lot.remaining_quantity - quantity).max(0.0);
                uow.purchases().update_remaining_quantity(lot_id, new_rem)?;
            }
        }
        let new_stock = item.current_stock - quantity;
        let is_low_stock = new_stock < 5.0;
        uow.items()
            .update_stock_cache(item_id, new_stock, is_low_stock)?;
        Ok(())
    }

    fn create_adjustment(&self, payload: StockAdjustmentPayload) -> Result<i64, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let item = tx
            .items()
            .find_by_id(payload.item_id)?
            .ok_or_else(|| AppError::NotFound {
                entity: "Item".to_string(),
                id: payload.item_id.to_string(),
            })?;
        // Positive adjustments create a lot at the latest lot cost (DD-07/DD-12).
        // TODO(F3, DD-12): accept a user-supplied unit_cost and reject when no cost is derivable.
        let lot_cost = if payload.quantity > 0.0 {
            let latest_lot = tx
                .purchases()
                .get_latest_purchase_item_for_item(payload.item_id)?;
            Some(
                latest_lot
                    .map(|l| l.unit_cost)
                    .unwrap_or(item.default_sale_price * 0.7),
            )
        } else {
            None
        };
        let adj = StockAdjustment {
            id: 0,
            adjustment_date: payload.adjustment_date.clone(),
            item_id: payload.item_id,
            quantity: payload.quantity,
            unit_cost: lot_cost,
            reason: payload.reason,
            notes: payload.notes,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let adj_id = tx.adjustments().create(&adj)?;

        if payload.quantity < 0.0 {
            let write_off_qty = payload.quantity.abs();
            self.strategy
                .deplete_lots_for_adjustment(&mut *tx, adj_id, payload.item_id, write_off_qty)?;
            let new_stock = (item.current_stock - write_off_qty).max(0.0);
            let is_low = new_stock < 5.0;
            tx.items().update_stock_cache(item.id, new_stock, is_low)?;
        } else {
            let unit_cost = lot_cost.unwrap_or(0.0);
            // DD-07: non-PURCHASE lot headers are not supplier bills (total 0, no party, PAID).
            // TODO(F3, DD-07): per-source code sequence (ADJ-0001) instead of the adjustment id.
            let synthetic_tx = PurchaseTransaction {
                id: 0,
                transaction_code: format!("ADJ-{}", adj_id),
                source: LotSource::Adjustment,
                supplier_invoice_ref: None,
                transaction_date: payload.adjustment_date,
                party_id: None,
                party_name: None,
                payment_mode: None,
                payment_status: PaymentStatus::Paid,
                total_amount: 0.0,
                paid_amount: 0.0,
                balance_due: 0.0,
                notes: Some("Synthetic Adjustment Lot".to_string()),
                created_at: chrono::Utc::now().to_rfc3339(),
            };
            let synth_tx_id = tx.purchases().create_transaction(&synthetic_tx)?;
            let synth_lot = PurchaseItem {
                id: 0,
                purchase_transaction_id: synth_tx_id,
                item_id: payload.item_id,
                item_name: item.name.clone(),
                pack_label: None,
                pack_quantity: payload.quantity,
                quantity: payload.quantity,
                unit_cost,
                total_amount: payload.quantity * unit_cost,
                remaining_quantity: payload.quantity,
            };
            tx.purchases().create_purchase_item(&synth_lot)?;
            let new_stock = item.current_stock + payload.quantity;
            let is_low = new_stock < 5.0;
            tx.items().update_stock_cache(item.id, new_stock, is_low)?;
        }
        tx.commit()?;
        Ok(adj_id)
    }

    fn list_adjustments(
        &self,
        item_id: Option<i64>,
        page: usize,
        limit: usize,
    ) -> Result<PaginatedList<StockAdjustment>, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let all = if let Some(id) = item_id {
            tx.adjustments().list_by_item_id(id)?
        } else {
            tx.adjustments().list_adjustments(None, 0, usize::MAX)?
        };
        let total = all.len();
        let items = all
            .into_iter()
            .skip((page.saturating_sub(1)) * limit)
            .take(limit)
            .collect();
        Ok(PaginatedList::new(items, total, page, limit))
    }
}
