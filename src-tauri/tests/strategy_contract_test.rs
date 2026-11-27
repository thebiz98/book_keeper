// tests/strategy_contract_test.rs
//! NFR-05 contract: the allocation strategy is swappable through DI. A trivial test-only LIFO
//! strategy is injected as `Arc<dyn InventoryAllocationStrategy>` (both directly into
//! `InventoryServiceImpl` and through `build_sqlite_app_state_with_strategy`) and the stock
//! cache invariant `current_stock == Σ remaining` must still hold after a mixed sequence.

mod common;

use std::sync::Arc;
use common::stock::*;
use common::{FifoStrategy, InventoryServiceImpl, TestDb};
use book_keeper::core::constants::EPS;
use book_keeper::core::dtos::*;
use book_keeper::core::entities::{SaleStockAllocation, StockOutflowAllocation};
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::{TransactionContext, UnitOfWork};
use book_keeper::services::contracts::inventory_allocation_strategy::*;
use book_keeper::services::contracts::inventory_service::InventoryService;
use book_keeper::services::contracts::item_service::ItemService;

/// Test-only LIFO: newest lot first for depletion, oldest-depleted allocation first on reversal.
struct LifoStrategy;

impl LifoStrategy {
    fn plan(uow: &mut dyn TransactionContext, item_id: i64, qty: f64) -> Result<Vec<(i64, f64, f64, f64)>, AppError> {
        if !qty.is_finite() || qty <= 0.0 {
            return Err(AppError::ValidationError { field: "quantity".into(), message: "bad qty".into() });
        }
        let mut lots = uow.purchases().get_active_lots_for_item(item_id)?;
        lots.sort_by_key(|l| std::cmp::Reverse(l.id));
        let total: f64 = lots.iter().map(|l| l.remaining_quantity).sum();
        if total + EPS < qty {
            return Err(AppError::InsufficientStockError { item_id, item_name: "lifo".into(), required: qty, available: total });
        }
        let mut need = qty;
        let mut plan = vec![];
        for l in lots {
            if need <= EPS { break; }
            let take = l.remaining_quantity.min(need);
            need -= take;
            plan.push((l.id, l.remaining_quantity, take, l.unit_cost));
        }
        Ok(plan)
    }
}

impl InventoryAllocationStrategy for LifoStrategy {
    fn open_lots_in_draw_order(&self, uow: &mut dyn TransactionContext, item_id: i64) -> Result<Vec<book_keeper::core::entities::PurchaseItem>, AppError> {
        let mut lots = uow.purchases().get_active_lots_for_item(item_id)?;
        lots.sort_by_key(|l| std::cmp::Reverse(l.id));
        Ok(lots)
    }

    fn validate_stock_availability(&self, uow: &mut dyn TransactionContext, item_id: i64, q: f64) -> Result<(), AppError> {
        Self::plan(uow, item_id, q).map(|_| ())
    }

    fn deplete_lots_for_sale(&self, uow: &mut dyn TransactionContext, sale_item_id: i64, item_id: i64, q: f64) -> Result<DepletionResult, AppError> {
        let mut total_cogs = 0.0;
        let mut allocations = vec![];
        for (lot, rem, take, cost) in Self::plan(uow, item_id, q)? {
            uow.purchases().update_remaining_quantity(lot, rem - take)?;
            let mut a = SaleStockAllocation { id: 0, sale_item_id, purchase_item_id: lot, quantity_depleted: take, returned_quantity: 0.0, unit_cost: cost };
            a.id = uow.allocations().create_allocation(&a)?;
            total_cogs += take * cost;
            allocations.push(a);
        }
        Ok(DepletionResult { total_cogs, allocations })
    }

    fn reverse_sale_allocations(&self, uow: &mut dyn TransactionContext, sale_item_id: i64, q: f64, restock: bool) -> Result<ReversalResult, AppError> {
        let mut allocs = uow.allocations().find_reversible_by_sale_item_id(sale_item_id)?;
        allocs.sort_by_key(|a| a.id); // oldest depletion first (opposite of FIFO)
        let cap: f64 = allocs.iter().map(|a| a.quantity_depleted - a.returned_quantity).sum();
        if !q.is_finite() || q <= 0.0 || cap + EPS < q {
            return Err(AppError::ValidationError { field: "quantity".into(), message: "bad reversal".into() });
        }
        let (mut left, mut cost, mut restocked) = (q, 0.0, 0.0);
        for a in allocs {
            if left <= EPS { break; }
            let take = (a.quantity_depleted - a.returned_quantity).min(left);
            uow.allocations().update_returned_quantity(a.id, a.returned_quantity + take)?;
            if restock {
                let lot = uow.purchases().find_purchase_item_by_id(a.purchase_item_id)?.unwrap();
                uow.purchases().update_remaining_quantity(lot.id, lot.remaining_quantity + take)?;
                restocked += take;
            }
            cost += take * a.unit_cost;
            left -= take;
        }
        Ok(ReversalResult { reversed_cost: cost, reversed_quantity: q, restocked_quantity: restocked })
    }

    fn deplete_lots_for_outflow(&self, uow: &mut dyn TransactionContext, source_type: OutflowSourceType, source_id: i64, item_id: i64, q: f64) -> Result<OutflowDepletionResult, AppError> {
        let mut total_cost = 0.0;
        let mut allocations = vec![];
        for (lot, rem, take, cost) in Self::plan(uow, item_id, q)? {
            uow.purchases().update_remaining_quantity(lot, rem - take)?;
            let mut a = StockOutflowAllocation { id: 0, source_type, source_id, purchase_item_id: lot, quantity_depleted: take, unit_cost: cost };
            a.id = uow.outflow_allocations().create_allocation(&a)?;
            total_cost += take * cost;
            allocations.push(a);
        }
        Ok(OutflowDepletionResult { total_cost, allocations })
    }
}

fn adj(item_id: i64, qty: f64) -> StockAdjustmentPayload {
    StockAdjustmentPayload {
        adjustment_date: "2026-11-21".into(), item_id, quantity: qty,
        reason: AdjustmentReason::CountCorrection, unit_cost: None, notes: None,
    }
}

/// Runs the same mixed sequence against any injected strategy and checks the invariant.
fn exercise(db: &TestDb, inv: &dyn InventoryService) {
    let mut tx = db.begin_transaction().unwrap();
    seed_empty_item(&mut *tx, 1);
    seed_purchase_lot(inv, &mut *tx, 1, 1, 10.0, 2.0);
    seed_purchase_lot(inv, &mut *tx, 2, 1, 10.0, 3.0);
    seed_sale(&mut *tx, 50, 1);
    inv.deplete_stock_for_sale(&mut *tx, 50, 1, 14.0).unwrap();
    assert_stock_invariant(&mut *tx);
    inv.reverse_sale_line(&mut *tx, 50, 1, 5.0, true).unwrap();
    inv.reverse_sale_line(&mut *tx, 50, 1, 1.0, false).unwrap();
    inv.issue_stock_outflow(&mut *tx, OutflowSourceType::Replacement, 7, 1, 2.0).unwrap();
    seed_purchase_lot(inv, &mut *tx, 3, 1, 5.0, 4.0); // fresh lot: named return works under any order
    inv.remove_stock_for_vendor_return(&mut *tx, 8, 1, 3, 1.0).unwrap();
    assert_stock_invariant(&mut *tx);
    tx.commit().unwrap();
    inv.create_adjustment(adj(1, -2.0)).unwrap();
    inv.create_adjustment(adj(1, 4.0)).unwrap();
    let mut r = db.begin_read().unwrap();
    assert_stock_invariant(&mut *r);
    assert!(inv.verify_stock_cache(&mut *r).unwrap().is_empty());
}

#[test]
fn injected_lifo_strategy_drives_depletion_order_and_keeps_invariant() {
    let db = TestDb::new();
    let strategy: Arc<dyn InventoryAllocationStrategy> = Arc::new(LifoStrategy);
    let inv = InventoryServiceImpl::new(strategy, db.clone());

    let mut tx = db.begin_transaction().unwrap();
    seed_empty_item(&mut *tx, 1);
    seed_purchase_lot(&inv, &mut *tx, 1, 1, 5.0, 2.0);
    seed_purchase_lot(&inv, &mut *tx, 2, 1, 5.0, 9.0);
    seed_sale(&mut *tx, 50, 1);
    let res = inv.deplete_stock_for_sale(&mut *tx, 50, 1, 6.0).unwrap();
    // LIFO: newest lot (2 @ 9) first, then 1 from lot 1 @ 2.
    assert_eq!(res.allocations[0].purchase_item_id, 2);
    assert_eq!(res.total_cogs, 5.0 * 9.0 + 1.0 * 2.0);
    assert_stock_invariant(&mut *tx);
    tx.commit().unwrap();
}

#[test]
fn invariant_holds_for_fifo_and_lifo_on_the_same_sequence() {
    for strategy in [
        Arc::new(FifoStrategy) as Arc<dyn InventoryAllocationStrategy>,
        Arc::new(LifoStrategy) as Arc<dyn InventoryAllocationStrategy>,
    ] {
        let db = TestDb::new();
        let inv = InventoryServiceImpl::new(strategy, db.clone());
        exercise(&db, &inv);
    }
}

#[test]
fn strategy_is_swappable_through_app_state_di() {
    let tmp = common::TempDir::new("strategy");
    let path = tmp.join("shop.db");
    let app = book_keeper::app_state::build_sqlite_app_state_with_strategy(&path, Arc::new(LifoStrategy)).unwrap();
    let id = app.item_service.create_item(CreateItemPayload { item_type: None, service_cost: None, components: None, low_stock_threshold: None,
        name: "Pen".into(), category: None, barcode: None, base_unit: "PCS".into(), mrp: None,
        default_sale_price: 10.0, initial_stock: Some(5.0), initial_cost: Some(2.0), bundles: None,
    }).unwrap();
    app.inventory_service.create_adjustment(adj(id, 5.0)).unwrap(); // 2nd lot, same cost
    let res = app.inventory_service.create_adjustment(adj(id, -6.0)).unwrap();
    assert_eq!(res.loss_value, 12.0);
    // LIFO took the newest (ADJ) lot completely first.
    let lots = app.inventory_service.get_item_lots(id).unwrap();
    assert_eq!((lots[0].remaining_quantity, lots[1].remaining_quantity), (4.0, 0.0));
    let mut r = app.unit_of_work.begin_read().unwrap();
    assert_stock_invariant(&mut *r);
    drop(r);
    drop(app);
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), ext));
    }
}
