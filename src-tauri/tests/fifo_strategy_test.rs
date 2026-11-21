// tests/fifo_strategy_test.rs
//! Test Suite: FIFO allocation strategy (production `FifoStrategy`, real SQLite).
//! Test Cases: TC-FIFO-001 through TC-FIFO-008 (+ input-validation guards).
//! Lots are seeded through the production `InventoryService`, never by hand-editing the cache.

mod common;

use common::fixtures::*;
use common::stock::*;
use common::{FifoStrategy, InventoryServiceImpl, TestDb};
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::OutflowSourceType;
use book_keeper::data::contracts::unit_of_work::{TransactionContext, UnitOfWork};
use book_keeper::services::contracts::inventory_allocation_strategy::InventoryAllocationStrategy;

fn setup(lots: &[(i64, f64, f64)]) -> (TestDb, Box<dyn TransactionContext>) {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let mut tx = db.begin_transaction().expect("begin tx");
    seed_empty_item(&mut *tx, 1);
    for &(id, qty, cost) in lots {
        seed_purchase_lot(&inv, &mut *tx, id, 1, qty, cost);
    }
    (db, tx)
}

#[test]
fn test_tc_fifo_001_single_lot_full_depletion() {
    let (_db, mut tx) = setup(&[(1, 10.0, 5.0)]);
    seed_sale(&mut *tx, 100, 1);
    let res = FifoStrategy.deplete_lots_for_sale(&mut *tx, 100, 1, 10.0).expect("deplete");
    assert_eq!(lot_remaining(&mut *tx, 1), 0.0);
    assert_eq!(res.allocations.len(), 1);
    assert_eq!((res.allocations[0].quantity_depleted, res.allocations[0].unit_cost), (10.0, 5.0));
    assert_eq!(res.total_cogs, 50.0);
}

#[test]
fn test_tc_fifo_002_multi_lot_split_with_differing_costs() {
    let (_db, mut tx) = setup(&[(1, 5.0, 4.0), (2, 10.0, 6.0)]);
    seed_sale(&mut *tx, 100, 1);
    let res = FifoStrategy.deplete_lots_for_sale(&mut *tx, 100, 1, 8.0).expect("deplete");
    assert_eq!(lot_remaining(&mut *tx, 1), 0.0);
    assert_eq!(lot_remaining(&mut *tx, 2), 7.0);
    assert_eq!(res.total_cogs, 38.0);
    assert_eq!(res.allocations.len(), 2);
    assert_eq!(tx.allocations().find_by_sale_item_id(100).unwrap().len(), 2);
}

#[test]
fn test_tc_fifo_003_insufficient_stock_rejected_without_mutation() {
    let (db, mut tx) = setup(&[(1, 4.0, 5.0)]);
    seed_sale(&mut *tx, 100, 1);
    match FifoStrategy.validate_stock_availability(&mut *tx, 1, 5.0) {
        Err(AppError::InsufficientStockError { required, available, .. }) => {
            assert_eq!((required, available), (5.0, 4.0));
        }
        other => panic!("expected InsufficientStockError, got {other:?}"),
    }
    // Depletion validates before it mutates: nothing moves on failure.
    assert!(matches!(
        FifoStrategy.deplete_lots_for_sale(&mut *tx, 100, 1, 5.0),
        Err(AppError::InsufficientStockError { .. })
    ));
    assert_eq!(lot_remaining(&mut *tx, 1), 4.0);
    assert!(tx.allocations().find_by_sale_item_id(100).unwrap().is_empty());
    tx.rollback().unwrap();
    drop(db);
}

#[test]
fn test_tc_fifo_004_restockable_reversal_latest_first() {
    let (_db, mut tx) = setup(&[(1, 2.0, 4.0), (2, 10.0, 6.0)]);
    seed_sale(&mut *tx, 100, 1);
    let sale = FifoStrategy.deplete_lots_for_sale(&mut *tx, 100, 1, 5.0).expect("deplete");
    assert_eq!(
        (sale.allocations[0].quantity_depleted, sale.allocations[1].quantity_depleted),
        (2.0, 3.0)
    );
    let res = FifoStrategy.reverse_sale_allocations(&mut *tx, 100, 4.0, true).expect("reverse");
    let allocs = tx.allocations().find_by_sale_item_id(100).unwrap();
    assert_eq!(allocs[1].returned_quantity, 3.0, "latest depleted (lot 2) first");
    assert_eq!(allocs[0].returned_quantity, 1.0);
    assert_eq!(res.reversed_cost, 3.0 * 6.0 + 1.0 * 4.0);
    assert_eq!((res.reversed_quantity, res.restocked_quantity), (4.0, 4.0));
    assert_eq!(lot_remaining(&mut *tx, 2), 10.0);
    assert_eq!(lot_remaining(&mut *tx, 1), 1.0);
}

#[test]
fn test_tc_fifo_005_scrap_reversal_without_restock() {
    let (_db, mut tx) = setup(&[(1, 10.0, 3.0)]);
    seed_sale(&mut *tx, 100, 1);
    FifoStrategy.deplete_lots_for_sale(&mut *tx, 100, 1, 5.0).unwrap();
    let res = FifoStrategy.reverse_sale_allocations(&mut *tx, 100, 5.0, false).expect("reverse");
    assert_eq!(lot_remaining(&mut *tx, 1), 5.0, "lots unchanged");
    let allocs = tx.allocations().find_by_sale_item_id(100).unwrap();
    assert_eq!(allocs.iter().map(|a| a.returned_quantity).sum::<f64>(), 5.0);
    assert_eq!(res.reversed_cost, 15.0);
    assert_eq!(res.restocked_quantity, 0.0);
}

#[test]
fn test_tc_fifo_006_outflow_depletion_logs_source() {
    let (_db, mut tx) = setup(&[(1, 10.0, 12.0)]);
    seed_adjustment(&mut *tx, 7, 1);
    let res = FifoStrategy
        .deplete_lots_for_outflow(&mut *tx, OutflowSourceType::Adjustment, 7, 1, 3.0)
        .expect("outflow");
    assert_eq!(lot_remaining(&mut *tx, 1), 7.0);
    assert_eq!(res.total_cost, 36.0);
    let rows = tx.outflow_allocations().find_by_source(OutflowSourceType::Adjustment, 7).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].purchase_item_id, rows[0].quantity_depleted, rows[0].unit_cost), (1, 3.0, 12.0));
}

#[test]
fn test_tc_fifo_007_repeated_partial_returns_never_over_restore() {
    let (_db, mut tx) = setup(&[(1, 5.0, 2.0)]);
    seed_sale(&mut *tx, 100, 1);
    FifoStrategy.deplete_lots_for_sale(&mut *tx, 100, 1, 5.0).unwrap();
    FifoStrategy.reverse_sale_allocations(&mut *tx, 100, 3.0, true).expect("first");
    FifoStrategy.reverse_sale_allocations(&mut *tx, 100, 2.0, true).expect("second");
    assert_eq!(lot_remaining(&mut *tx, 1), 5.0);
    assert_eq!(tx.allocations().find_by_sale_item_id(100).unwrap()[0].returned_quantity, 5.0);
    assert!(matches!(
        FifoStrategy.reverse_sale_allocations(&mut *tx, 100, 1.0, true),
        Err(AppError::ValidationError { .. })
    ));
    assert_eq!(lot_remaining(&mut *tx, 1), 5.0, "nothing changed by the failed third return");
    assert_eq!(tx.allocations().find_by_sale_item_id(100).unwrap()[0].returned_quantity, 5.0);
}

#[test]
fn test_tc_fifo_008_fractional_quantities_within_tolerance() {
    let (_db, mut tx) = setup(&[(1, 0.1, 10.0), (2, 0.2, 10.0)]);
    seed_sale(&mut *tx, 100, 1);
    FifoStrategy.deplete_lots_for_sale(&mut *tx, 100, 1, 0.3).expect("0.1 + 0.2 covers 0.3");
    assert!(lot_remaining(&mut *tx, 1).abs() <= 1e-9);
    assert!(lot_remaining(&mut *tx, 2).abs() <= 1e-9);
}

#[test]
fn fifo_rejects_non_positive_and_non_finite_quantities_before_mutating() {
    let (_db, mut tx) = setup(&[(1, 10.0, 5.0)]);
    seed_sale(&mut *tx, 100, 1);
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(FifoStrategy.validate_stock_availability(&mut *tx, 1, bad), Err(AppError::ValidationError { .. })), "{bad}");
        assert!(matches!(FifoStrategy.deplete_lots_for_sale(&mut *tx, 100, 1, bad), Err(AppError::ValidationError { .. })), "{bad}");
        assert!(matches!(FifoStrategy.reverse_sale_allocations(&mut *tx, 100, bad, true), Err(AppError::ValidationError { .. })), "{bad}");
        assert!(matches!(
            FifoStrategy.deplete_lots_for_outflow(&mut *tx, OutflowSourceType::Replacement, 1, 1, bad),
            Err(AppError::ValidationError { .. })
        ), "{bad}");
    }
    assert_eq!(lot_remaining(&mut *tx, 1), 10.0);
}

#[test]
fn fifo_order_is_by_lot_id_regardless_of_repository_order() {
    // Lot 2 is created before lot 1 is topped up: ordering is by lot id, owned by the strategy.
    let (_db, mut tx) = setup(&[(1, 3.0, 1.0), (2, 3.0, 2.0), (3, 3.0, 3.0)]);
    seed_sale(&mut *tx, 100, 1);
    let res = FifoStrategy.deplete_lots_for_sale(&mut *tx, 100, 1, 4.0).unwrap();
    let ids: Vec<i64> = res.allocations.iter().map(|a| a.purchase_item_id).collect();
    assert_eq!(ids, vec![1, 2]);
    assert_eq!(res.total_cogs, 3.0 * 1.0 + 1.0 * 2.0);
}
