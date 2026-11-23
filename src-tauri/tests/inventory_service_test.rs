// tests/inventory_service_test.rs
//! Test Suite: InventoryService (sole stock writer) on the production strategy + real SQLite.
//! Test Cases: TC-INV-001 through TC-INV-009, plus guards, startup repair and ItemService opening stock.

mod common;

use std::sync::Arc;
use common::stock::*;
use common::{FifoStrategy, InventoryServiceImpl, ItemServiceImpl, TestDb};
use book_keeper::core::dtos::*;
use book_keeper::core::entities::StockAdjustment;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::inventory_service::InventoryService;
use book_keeper::services::contracts::item_service::ItemService;

type Inv = InventoryServiceImpl<FifoStrategy>;

fn setup(lots: &[(i64, i64, f64, f64)]) -> (TestDb, Inv) {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let mut tx = db.begin_transaction().unwrap();
    for &(lot_id, item_id, qty, cost) in lots {
        if tx.items().find_by_id(item_id).unwrap().is_none() {
            seed_empty_item(&mut *tx, item_id);
        }
        seed_purchase_lot(&inv, &mut *tx, lot_id, item_id, qty, cost);
    }
    tx.commit().unwrap();
    (db, inv)
}

fn stock(db: &TestDb, item_id: i64) -> (f64, bool) {
    let mut r = db.begin_read().unwrap();
    let i = r.items().find_by_id(item_id).unwrap().unwrap();
    (i.current_stock, i.is_low_stock)
}

fn adj(item_id: i64, qty: f64, reason: AdjustmentReason, unit_cost: Option<f64>) -> StockAdjustmentPayload {
    StockAdjustmentPayload {
        adjustment_date: "2026-11-21".into(), item_id, quantity: qty, reason, unit_cost, notes: None,
    }
}

#[test]
fn test_tc_inv_001_deplete_stock_updates_cache_atomically() {
    let (db, inv) = setup(&[(1, 1, 25.0, 100.0)]);
    let mut tx = db.begin_transaction().unwrap();
    seed_sale(&mut *tx, 10, 1);
    let res = inv.deplete_stock_for_sale(&mut *tx, 10, 1, 10.0).unwrap();
    assert_eq!(res.total_cogs, 1000.0);
    tx.commit().unwrap();
    assert_eq!(stock(&db, 1), (15.0, false));
}

#[test]
fn test_tc_inv_002_add_stock_increments_cache() {
    let (db, _inv) = setup(&[(2, 2, 100.0, 30.0)]);
    let mut r = db.begin_read().unwrap();
    assert_eq!(lot_remaining(&mut *r, 2), 100.0);
    assert_eq!(stock(&db, 2), (100.0, false));
}

#[test]
fn test_tc_inv_003_reverse_sale_line_restock_increments_cache() {
    let (db, inv) = setup(&[(3, 3, 25.0, 4.0)]);
    let mut tx = db.begin_transaction().unwrap();
    seed_sale(&mut *tx, 10, 3);
    inv.deplete_stock_for_sale(&mut *tx, 10, 3, 5.0).unwrap();
    assert_eq!(tx.items().find_by_id(3).unwrap().unwrap().current_stock, 20.0);
    let cost = inv.reverse_sale_line(&mut *tx, 10, 3, 5.0, true).unwrap();
    assert_eq!(cost, 20.0);
    assert_eq!(tx.items().find_by_id(3).unwrap().unwrap().current_stock, 25.0);
    assert_eq!(lot_remaining(&mut *tx, 3), 25.0);
    assert_stock_invariant(&mut *tx);
}

#[test]
fn reverse_sale_line_without_restock_leaves_cache_and_lots_unchanged() {
    let (db, inv) = setup(&[(3, 3, 25.0, 4.0)]);
    let mut tx = db.begin_transaction().unwrap();
    seed_sale(&mut *tx, 10, 3);
    inv.deplete_stock_for_sale(&mut *tx, 10, 3, 5.0).unwrap();
    let cost = inv.reverse_sale_line(&mut *tx, 10, 3, 2.0, false).unwrap();
    assert_eq!(cost, 8.0);
    assert_eq!(tx.items().find_by_id(3).unwrap().unwrap().current_stock, 20.0);
    assert_eq!(lot_remaining(&mut *tx, 3), 20.0);
    assert_eq!(tx.allocations().find_by_sale_item_id(10).unwrap()[0].returned_quantity, 2.0);
    assert_stock_invariant(&mut *tx);
}

#[test]
fn test_tc_inv_004_negative_adjustment_creates_audit_trail() {
    let (db, inv) = setup(&[(4, 4, 20.0, 5.0)]);
    let res = inv.create_adjustment(adj(4, -3.0, AdjustmentReason::Damaged, None)).unwrap();
    assert_eq!(res.loss_value, 15.0);
    assert_eq!(stock(&db, 4).0, 17.0);
    let mut r = db.begin_read().unwrap();
    let allocs = r.outflow_allocations().find_by_source(OutflowSourceType::Adjustment, res.adjustment_id).unwrap();
    assert_eq!(allocs.len(), 1);
    assert_eq!((allocs[0].quantity_depleted, allocs[0].unit_cost), (3.0, 5.0));
    let page = inv.list_adjustments(Some(4), Some(1), Some(10)).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].unit_cost, None, "write-offs record no unit cost");
    assert_stock_invariant(&mut *r);
}

#[test]
fn negative_adjustment_beyond_stock_is_rejected_atomically() {
    let (db, inv) = setup(&[(4, 4, 2.0, 5.0)]);
    let err = inv.create_adjustment(adj(4, -3.0, AdjustmentReason::Expired, None)).unwrap_err();
    assert!(matches!(err, AppError::InsufficientStockError { .. }), "{err:?}");
    assert_eq!(stock(&db, 4).0, 2.0);
    assert!(inv.list_adjustments(Some(4), Some(1), Some(10)).unwrap().items.is_empty(), "no adjustment row left behind");
}

#[test]
fn test_tc_inv_005_positive_adjustment_creates_adjustment_source_lot() {
    let (db, inv) = setup(&[(5, 5, 10.0, 7.5)]);
    let res = inv.create_adjustment(adj(5, 10.0, AdjustmentReason::CountCorrection, None)).unwrap();
    assert_eq!(res.loss_value, 0.0);
    assert_eq!(stock(&db, 5).0, 20.0);

    let lots = inv.get_item_lots(5).unwrap();
    assert_eq!(lots.len(), 2);
    let l = &lots[1];
    assert_eq!((l.source, l.transaction_code.as_str(), l.unit_cost, l.quantity), (LotSource::Adjustment, "ADJ-0001", 7.5, 10.0));
    let mut r = db.begin_read().unwrap();
    let h = r.purchases().find_transaction_by_id(l.purchase_transaction_id).unwrap().unwrap();
    assert_eq!((h.total_amount, h.paid_amount, h.balance_due, h.payment_status, h.party_id), (0.0, 0.0, 0.0, PaymentStatus::Paid, None));
    assert_eq!(h.notes.as_deref(), Some(format!("Stock adjustment #{}: CountCorrection", res.adjustment_id).as_str()));
    let history = r.purchases().list_purchases(None, None, None, None, 0, 50).unwrap();
    assert_eq!(history.len(), 1, "ADJ lot is not a supplier bill");
    assert!(r.settlements().list_by_transaction(TransactionType::Purchase, h.id).unwrap().is_empty(), "no ledger row");
    assert_eq!(r.purchases().get_next_transaction_code(LotSource::Purchase).unwrap().split('-').next(), Some("PUR"));
    assert_stock_invariant(&mut *r);
}

#[test]
fn test_tc_inv_006_low_stock_flag_transition() {
    let (db, inv) = setup(&[(6, 6, 6.0, 1.0)]);
    assert_eq!(stock(&db, 6), (6.0, false));
    let mut tx = db.begin_transaction().unwrap();
    seed_sale(&mut *tx, 11, 6);
    inv.deplete_stock_for_sale(&mut *tx, 11, 6, 2.0).unwrap();
    tx.commit().unwrap();
    assert_eq!(stock(&db, 6), (4.0, true));
}

#[test]
fn test_tc_inv_007_positive_adjustment_without_prior_lot_needs_cost() {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let mut tx = db.begin_transaction().unwrap();
    seed_empty_item(&mut *tx, 7);
    tx.commit().unwrap();

    match inv.create_adjustment(adj(7, 5.0, AdjustmentReason::CountCorrection, None)) {
        Err(AppError::ValidationError { field, .. }) => assert_eq!(field, "unit_cost"),
        other => panic!("expected unit_cost ValidationError, got {other:?}"),
    }
    assert_eq!(stock(&db, 7).0, 0.0);
    assert!(inv.list_adjustments(Some(7), Some(1), Some(10)).unwrap().items.is_empty());

    inv.create_adjustment(adj(7, 5.0, AdjustmentReason::CountCorrection, Some(4.0))).unwrap();
    let lots = inv.get_item_lots(7).unwrap();
    assert_eq!((lots.len(), lots[0].unit_cost, lots[0].source), (1, 4.0, LotSource::Adjustment));
    assert_eq!(inv.list_adjustments(Some(7), Some(1), Some(10)).unwrap().items[0].unit_cost, Some(4.0));
}

#[test]
fn test_tc_inv_009_vendor_return_named_lot() {
    let (db, inv) = setup(&[(9, 9, 10.0, 3.0)]);
    let mut tx = db.begin_transaction().unwrap();
    let value = inv.remove_stock_for_vendor_return(&mut *tx, 55, 9, Some(9), 4.0).unwrap();
    assert_eq!(value, 12.0);
    assert_eq!(lot_remaining(&mut *tx, 9), 6.0);
    let rows = tx.outflow_allocations().find_by_source(OutflowSourceType::VendorReturn, 55).unwrap();
    assert_eq!((rows.len(), rows[0].purchase_item_id, rows[0].quantity_depleted), (1, 9, 4.0));
    assert!(matches!(
        inv.remove_stock_for_vendor_return(&mut *tx, 56, 9, Some(9), 7.0),
        Err(AppError::ValidationError { .. })
    ));
    assert_eq!(lot_remaining(&mut *tx, 9), 6.0);
    assert_stock_invariant(&mut *tx);
}

fn adjustment_rows(inv: &Inv, item: i64) -> Vec<StockAdjustment> {
    inv.list_adjustments(Some(item), Some(1), Some(50)).unwrap().items
}

#[test]
fn test_tc_inv_010_unrecorded_stock_at_entered_cost() {
    let (db, inv) = setup(&[(1, 1, 10.0, 7.5)]);
    let res = inv.create_adjustment(adj(1, 4.0, AdjustmentReason::UnrecordedStock, Some(9.0))).unwrap();
    assert_eq!(res.loss_value, 0.0);
    assert_eq!(stock(&db, 1).0, 14.0);
    let lots = inv.get_item_lots(1).unwrap();
    assert_eq!(lots.len(), 2);
    assert_eq!((lots[1].source, lots[1].unit_cost, lots[1].quantity), (LotSource::Adjustment, 9.0, 4.0));
    let rows = adjustment_rows(&inv, 1);
    assert_eq!((rows[0].reason, rows[0].unit_cost), (AdjustmentReason::UnrecordedStock, Some(9.0)));
    let mut r = db.begin_read().unwrap();
    assert_eq!(r.purchases().list_purchases(None, None, None, None, 0, 50).unwrap().len(), 1, "not a purchase");
    let h = r.purchases().find_transaction_by_id(lots[1].purchase_transaction_id).unwrap().unwrap();
    assert!(r.settlements().list_by_transaction(TransactionType::Purchase, h.id).unwrap().is_empty());
    assert_eq!(h.notes.as_deref(), Some(format!("Stock adjustment #{}: UnrecordedStock", res.adjustment_id).as_str()));
    assert_stock_invariant(&mut *r);
}

#[test]
fn test_tc_inv_011_unrecorded_stock_defaults_to_latest_lot_cost() {
    let (_db, inv) = setup(&[(1, 1, 10.0, 7.5)]);
    inv.create_adjustment(adj(1, 4.0, AdjustmentReason::UnrecordedStock, None)).unwrap();
    assert_eq!(inv.get_item_lots(1).unwrap()[1].unit_cost, 7.5);
    assert_eq!(adjustment_rows(&inv, 1)[0].unit_cost, Some(7.5));
}

#[test]
fn test_tc_inv_012_unrecorded_stock_without_lot_or_cost_is_rejected() {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let mut tx = db.begin_transaction().unwrap();
    seed_empty_item(&mut *tx, 1);
    tx.commit().unwrap();
    match inv.create_adjustment(adj(1, 3.0, AdjustmentReason::UnrecordedStock, None)) {
        Err(AppError::ValidationError { field, .. }) => assert_eq!(field, "unit_cost"),
        other => panic!("expected unit_cost ValidationError, got {other:?}"),
    }
    assert!(adjustment_rows(&inv, 1).is_empty());
    assert!(inv.get_item_lots(1).unwrap().is_empty());
    assert_eq!(stock(&db, 1).0, 0.0);
}

#[test]
fn test_tc_inv_013_unrecorded_stock_must_be_positive() {
    let (db, inv) = setup(&[(1, 1, 10.0, 5.0)]);
    match inv.create_adjustment(adj(1, -2.0, AdjustmentReason::UnrecordedStock, Some(5.0))) {
        Err(AppError::ValidationError { field, .. }) => assert_eq!(field, "quantity"),
        other => panic!("expected quantity ValidationError, got {other:?}"),
    }
    assert!(adjustment_rows(&inv, 1).is_empty());
    assert_eq!(stock(&db, 1).0, 10.0);
}

#[test]
fn adjustments_on_service_items_are_rejected() {
    let (db, inv) = setup(&[(1, 1, 10.0, 5.0)]);
    db.pool.get().unwrap().execute("UPDATE items SET item_type = 'SERVICE' WHERE id = 1", []).unwrap();
    match inv.create_adjustment(adj(1, 1.0, AdjustmentReason::UnrecordedStock, Some(5.0))) {
        Err(AppError::ValidationError { field, .. }) => assert_eq!(field, "item_id"),
        other => panic!("expected item_id ValidationError, got {other:?}"),
    }
}

#[test]
fn vendor_return_rejects_wrong_item_and_non_purchase_lots_and_falls_back_to_fifo() {
    let (db, inv) = setup(&[(1, 1, 10.0, 3.0), (2, 2, 10.0, 8.0)]);
    inv.create_adjustment(adj(1, 5.0, AdjustmentReason::CountCorrection, None)).unwrap();
    let adj_lot = inv.get_item_lots(1).unwrap()[1].purchase_item_id;
    let mut tx = db.begin_transaction().unwrap();
    // lot 2 belongs to item 2
    assert!(matches!(inv.remove_stock_for_vendor_return(&mut *tx, 1, 1, Some(2), 1.0), Err(AppError::ValidationError { .. })));
    // an ADJUSTMENT-source lot is not returnable to a vendor
    assert!(matches!(inv.remove_stock_for_vendor_return(&mut *tx, 1, 1, Some(adj_lot), 1.0), Err(AppError::ValidationError { .. })));
    assert!(matches!(inv.remove_stock_for_vendor_return(&mut *tx, 1, 1, Some(999), 1.0), Err(AppError::NotFound { .. })));
    // None -> FIFO outflow tagged VENDOR_RETURN across lots (10 @3 then 5 @3)
    let value = inv.remove_stock_for_vendor_return(&mut *tx, 2, 1, None, 12.0).unwrap();
    assert_eq!(value, 36.0);
    let rows = tx.outflow_allocations().find_by_source(OutflowSourceType::VendorReturn, 2).unwrap();
    assert_eq!(rows.len(), 2);
    assert_stock_invariant(&mut *tx);
}

#[test]
fn test_tc_inv_008_stock_cache_invariant_after_mixed_operations() {
    let (db, inv) = setup(&[(1, 1, 20.0, 5.0), (2, 1, 10.0, 6.0), (3, 2, 8.0, 2.0)]);
    let mut tx = db.begin_transaction().unwrap();
    seed_sale(&mut *tx, 21, 1);
    seed_sale(&mut *tx, 22, 2);
    // sales across lots
    inv.deplete_stock_for_sale(&mut *tx, 21, 1, 25.0).unwrap();
    inv.deplete_stock_for_sale(&mut *tx, 22, 2, 3.0).unwrap();
    assert_stock_invariant(&mut *tx);
    // customer returns: restockable partial, then scrap
    inv.reverse_sale_line(&mut *tx, 21, 1, 7.0, true).unwrap();
    inv.reverse_sale_line(&mut *tx, 21, 1, 2.0, false).unwrap();
    assert_stock_invariant(&mut *tx);
    // replacement issue, opening lot, vendor return (named + FIFO)
    inv.issue_stock_outflow(&mut *tx, OutflowSourceType::Replacement, 900, 2, 2.0).unwrap();
    inv.create_source_lot(&mut *tx, LotSource::Opening, "2026-11-21", 2, 6.0, Some(2.5), None).unwrap();
    inv.create_source_lot(&mut *tx, LotSource::Return, "2026-11-21", 1, 1.5, None, None).unwrap();
    inv.remove_stock_for_vendor_return(&mut *tx, 901, 1, Some(2), 2.0).unwrap();
    inv.remove_stock_for_vendor_return(&mut *tx, 902, 1, None, 3.0).unwrap();
    assert_stock_invariant(&mut *tx);
    tx.commit().unwrap();
    // adjustments in their own transactions
    inv.create_adjustment(adj(1, -4.0, AdjustmentReason::TheftLost, None)).unwrap();
    inv.create_adjustment(adj(2, 3.0, AdjustmentReason::CountCorrection, None)).unwrap();
    let mut r = db.begin_read().unwrap();
    assert_stock_invariant(&mut *r);
    assert!(inv.verify_stock_cache(&mut *r).unwrap().is_empty());
}

#[test]
fn adjustment_validation_rejects_bad_input() {
    let (db, inv) = setup(&[(1, 1, 10.0, 5.0)]);
    for q in [0.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(inv.create_adjustment(adj(1, q, AdjustmentReason::Damaged, None)), Err(AppError::ValidationError { .. })), "{q}");
    }
    let mut bad_date = adj(1, 1.0, AdjustmentReason::Damaged, None);
    bad_date.adjustment_date = "21/11/2026".into();
    assert!(matches!(inv.create_adjustment(bad_date), Err(AppError::ValidationError { .. })));
    let mut empty_date = adj(1, 1.0, AdjustmentReason::Damaged, None);
    empty_date.adjustment_date = String::new();
    assert!(matches!(inv.create_adjustment(empty_date), Err(AppError::ValidationError { .. })));
    assert!(matches!(inv.create_adjustment(adj(404, 1.0, AdjustmentReason::Damaged, Some(1.0))), Err(AppError::NotFound { .. })));
    assert!(matches!(inv.create_adjustment(adj(1, 1.0, AdjustmentReason::Damaged, Some(-2.0))), Err(AppError::ValidationError { .. })));
    // inactive item
    let mut tx = db.begin_transaction().unwrap();
    let mut item = tx.items().find_by_id(1).unwrap().unwrap();
    item.is_active = false;
    tx.items().update(&item).unwrap();
    tx.commit().unwrap();
    assert!(matches!(inv.create_adjustment(adj(1, 1.0, AdjustmentReason::Damaged, Some(1.0))), Err(AppError::ValidationError { .. })));
    assert_eq!(stock(&db, 1).0, 10.0);
}

#[test]
fn create_source_lot_never_derives_cost_from_sale_price() {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let mut tx = db.begin_transaction().unwrap();
    seed_empty_item(&mut *tx, 1); // default_sale_price = 100
    match inv.create_source_lot(&mut *tx, LotSource::Opening, "2026-11-21", 1, 5.0, None, None) {
        Err(AppError::ValidationError { field, .. }) => assert_eq!(field, "unit_cost"),
        other => panic!("expected unit_cost error, got {other:?}"),
    }
    assert!(matches!(inv.create_source_lot(&mut *tx, LotSource::Purchase, "2026-11-21", 1, 5.0, Some(1.0), None), Err(AppError::ValidationError { .. })));
    assert!(matches!(inv.create_source_lot(&mut *tx, LotSource::Opening, "2026-11-21", 1, 0.0, Some(1.0), None), Err(AppError::ValidationError { .. })));
    let lot = inv.create_source_lot(&mut *tx, LotSource::Opening, "2026-11-21", 1, 5.0, Some(2.0), None).unwrap();
    assert_eq!(lot_remaining(&mut *tx, lot), 5.0);
    // next lot without explicit cost reuses the latest lot cost
    let lot2 = inv.create_source_lot(&mut *tx, LotSource::Return, "2026-11-21", 1, 1.0, None, None).unwrap();
    assert_eq!(tx.purchases().find_purchase_item_by_id(lot2).unwrap().unwrap().unit_cost, 2.0);
    assert_stock_invariant(&mut *tx);
}

#[test]
fn validate_stock_availability_checks_every_item() {
    let (db, inv) = setup(&[(1, 1, 5.0, 1.0), (2, 2, 3.0, 1.0)]);
    let mut tx = db.begin_transaction().unwrap();
    inv.validate_stock_availability(&mut *tx, &[(1, 5.0), (2, 3.0)]).unwrap();
    match inv.validate_stock_availability(&mut *tx, &[(1, 2.0), (2, 4.0)]) {
        Err(AppError::InsufficientStockError { item_id, required, available, .. }) => assert_eq!((item_id, required, available), (2, 4.0, 3.0)),
        other => panic!("{other:?}"),
    }
    // duplicate lines of one item are checked together
    assert!(matches!(inv.validate_stock_availability(&mut *tx, &[(1, 3.0), (1, 3.0)]), Err(AppError::InsufficientStockError { .. })));
}

#[test]
fn verify_and_repair_stock_cache() {
    let (db, inv) = setup(&[(1, 1, 10.0, 1.0)]);
    let mut tx = db.begin_transaction().unwrap();
    tx.items().update_stock_cache(1, 99.0).unwrap(); // corrupt
    assert_eq!(inv.verify_stock_cache(&mut *tx).unwrap(), vec![(1, 99.0, 10.0)]);
    assert_eq!(inv.repair_stock_cache(&mut *tx).unwrap(), vec![(1, 99.0, 10.0)]);
    assert!(inv.verify_stock_cache(&mut *tx).unwrap().is_empty());
    assert_eq!(tx.items().find_by_id(1).unwrap().unwrap().current_stock, 10.0);
}

#[test]
fn tc_low_01_threshold_10_follows_stock_changes() {
    let (db, _inv) = setup(&[(1, 1, 8.0, 1.0)]);
    let mut tx = db.begin_transaction().unwrap();
    tx.items().update_low_stock_threshold(1, 10.0, "2026-11-20T10:00:00Z").unwrap();
    assert!(tx.items().find_by_id(1).unwrap().unwrap().is_low_stock); // 8 < 10
    tx.items().apply_stock_delta(1, 2.0).unwrap(); // 10 -> not low
    assert!(!tx.items().find_by_id(1).unwrap().unwrap().is_low_stock);
    tx.items().apply_stock_delta(1, -1.0).unwrap(); // 9 -> low
    assert!(tx.items().find_by_id(1).unwrap().unwrap().is_low_stock);
}

#[test]
fn tc_low_02_threshold_zero_is_never_low() {
    let (db, _inv) = setup(&[(1, 1, 1.0, 1.0)]);
    let mut tx = db.begin_transaction().unwrap();
    tx.items().update_low_stock_threshold(1, 0.0, "2026-11-20T10:00:00Z").unwrap();
    tx.items().update_stock_cache(1, 0.0).unwrap();
    assert!(!tx.items().find_by_id(1).unwrap().unwrap().is_low_stock);
}

#[test]
fn tc_low_03_recompute_fixes_a_corrupted_flag() {
    let (db, _inv) = setup(&[(1, 1, 10.0, 1.0)]);
    db.pool.get().unwrap().execute("UPDATE items SET is_low_stock = 1 WHERE id = 1", []).unwrap();
    let mut tx = db.begin_transaction().unwrap();
    assert_eq!(tx.items().recompute_low_stock_flags().unwrap(), 1);
    assert!(!tx.items().find_by_id(1).unwrap().unwrap().is_low_stock);
    assert_eq!(tx.items().recompute_low_stock_flags().unwrap(), 0);
}

#[test]
fn tc_low_04_rust_and_sql_rules_agree() {
    use book_keeper::core::{is_low_stock, ItemType};
    let (db, _inv) = setup(&[(1, 1, 1.0, 1.0)]);
    let mut tx = db.begin_transaction().unwrap();
    let mut n = 10;
    for stock in [0.0, 4.5, 5.0, 6.0] {
        for thr in [0.0, 5.0, 5.5] {
            n += 1;
            let mut it = tx.items().find_by_id(1).unwrap().unwrap();
            it.id = n;
            it.name = format!("grid{n}");
            it.barcode = None;
            it.current_stock = stock;
            it.low_stock_threshold = thr;
            tx.items().create(&it).unwrap();
            let got = tx.items().find_by_id(n).unwrap().unwrap();
            assert_eq!(got.is_low_stock, is_low_stock(ItemType::Physical, stock, thr), "stock {stock} thr {thr}");
        }
    }
}

#[test]
fn tc_low_08_service_items_are_never_low() {
    use book_keeper::core::ItemType;
    let (db, _inv) = setup(&[(1, 1, 1.0, 1.0)]);
    let mut tx = db.begin_transaction().unwrap();
    let mut it = tx.items().find_by_id(1).unwrap().unwrap();
    it.id = 50;
    it.name = "Svc".into();
    it.barcode = None;
    it.item_type = ItemType::Service;
    it.current_stock = 0.0;
    tx.items().create(&it).unwrap();
    assert!(!tx.items().find_by_id(50).unwrap().unwrap().is_low_stock);
    tx.items().apply_stock_delta(50, -1.0).unwrap();
    assert!(!tx.items().find_by_id(50).unwrap().unwrap().is_low_stock);
}

#[test]
fn startup_repairs_a_drifted_stock_cache() {
    let path = std::env::temp_dir().join(format!("bk_inv_startup_{}.db", uuid::Uuid::new_v4()));
    {
        let app = book_keeper::app_state::build_sqlite_app_state(&path).unwrap();
        let id = app.item_service.create_item(CreateItemPayload { item_type: None, low_stock_threshold: None,
            name: "Pen".into(), category: None, barcode: None, base_unit: "PCS".into(), mrp: None,
            default_sale_price: 10.0, initial_stock: Some(8.0), initial_cost: Some(6.0), bundles: None,
        }).unwrap();
        assert_eq!(app.inventory_service.get_item_lots(id).unwrap()[0].source, LotSource::Opening);
    }
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute("UPDATE items SET current_stock = 123.0", []).unwrap();
    }
    let app = book_keeper::app_state::build_sqlite_app_state(&path).unwrap();
    let mut r = app.unit_of_work.begin_read().unwrap();
    assert_eq!(r.items().find_by_id(1).unwrap().unwrap().current_stock, 8.0);
    drop(r);
    drop(app);
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), ext));
    }
}

#[test]
fn item_create_with_opening_stock_goes_through_inventory_service() {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let items = ItemServiceImpl::new(db.clone(), Arc::new(db.indexer()), inv.clone());
    let mk = |name: &str, cost: Option<f64>| CreateItemPayload { item_type: None, low_stock_threshold: None,
        name: name.into(), category: None, barcode: None, base_unit: "PCS".into(), mrp: None,
        default_sale_price: 10.0, initial_stock: Some(8.0), initial_cost: cost, bundles: None,
    };
    // No cost and no prior lot: rejected, and the item is not left behind (same transaction).
    match items.create_item(mk("Ghost", None)) {
        Err(AppError::ValidationError { field, .. }) => assert_eq!(field, "unit_cost"),
        other => panic!("{other:?}"),
    }
    assert_eq!(items.get_catalog(Default::default()).unwrap().items.len(), 0);

    let id = items.create_item(mk("Pen", Some(6.0))).unwrap();
    assert_eq!(stock(&db, id), (8.0, false));
    let lots = inv.get_item_lots(id).unwrap();
    assert_eq!((lots[0].source, lots[0].transaction_code.as_str(), lots[0].unit_cost), (LotSource::Opening, "OPN-0001", 6.0));
    let mut r = db.begin_read().unwrap();
    assert_stock_invariant(&mut *r);
}
