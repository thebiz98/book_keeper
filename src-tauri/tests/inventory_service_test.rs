// tests/inventory_service_test.rs
//! Test Suite: Centralized Inventory Stock Cache & Adjustments
//! Test Cases: TC-INV-001 through TC-INV-006
//! Derived from book_keeper_LLD.md § 11.4 and Spec Miner Report

mod common;

use common::fixtures::*;
use common::{FifoStrategy, InventoryServiceImpl, TestDb};
use book_keeper::core::dtos::*;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::inventory_service::InventoryService;

#[test]
fn test_tc_inv_001_deplete_stock_updates_cache_atomically() {
    let uow = TestDb::new();
    let inventory_service = InventoryServiceImpl::new(FifoStrategy, uow.clone());

    let mut tx = uow.begin_transaction().expect("begin tx");
    // Setup: Item with current_stock = 25.0
    let item = create_test_item(1, "Camlin Geometry Box", None, None, 25.0, 150.0);
    tx.items().create(&item).expect("create item");

    let purchase_tx = create_test_purchase_tx(1, "PUR-1001", None, 2500.0, 2500.0, "2026-11-20");
    tx.purchases().create_transaction(&purchase_tx).expect("create pur");

    let lot = create_test_purchase_lot(1, 1, 1, None, 25.0, 25.0, 100.0, 25.0);
    tx.purchases().create_purchase_item(&lot).expect("create lot");

    // Action: Deplete 10.0 units
    seed_sale_line(&mut *tx, 10, 1);
    let res = inventory_service.deplete_stock_for_sale(&mut *tx, 10, 1, 10.0).expect("deplete stock");
    assert_eq!(res.total_cogs, 1000.0);

    // Assert: Item current_stock becomes 15.0, is_low_stock is false
    let updated = tx.items().find_by_id(1).expect("find").expect("exists");
    assert_eq!(updated.current_stock, 15.0);
    assert_eq!(updated.is_low_stock, false);
}

#[test]
fn test_tc_inv_002_add_stock_increments_cache() {
    let uow = TestDb::new();
    let inventory_service = InventoryServiceImpl::new(FifoStrategy, uow.clone());

    let mut tx = uow.begin_transaction().expect("begin tx");
    // Setup: Item with stock = 0.0, is_low_stock = true
    let item = create_test_item(2, "Fevicol 100g", None, None, 0.0, 50.0);
    tx.items().create(&item).expect("create item");

    let purchase_tx = create_test_purchase_tx(2, "PUR-1002", None, 3000.0, 3000.0, "2026-11-20");
    tx.purchases().create_transaction(&purchase_tx).expect("create pur");

    let lot = create_test_purchase_lot(2, 2, 2, None, 100.0, 100.0, 30.0, 100.0);

    // Action: Add stock for purchase
    inventory_service.add_stock_for_purchase(&mut *tx, &lot).expect("add stock");

    // Assert: Lot created with remaining 100.0, current_stock = 100.0, is_low_stock = false
    let updated = tx.items().find_by_id(2).expect("find").expect("exists");
    assert_eq!(updated.current_stock, 100.0);
    assert_eq!(updated.is_low_stock, false);
}

#[test]
fn test_tc_inv_003_restore_stock_for_return_increments_cache() {
    let uow = TestDb::new();
    let inventory_service = InventoryServiceImpl::new(FifoStrategy, uow.clone());

    let mut tx = uow.begin_transaction().expect("begin tx");
    // Setup: Item with current_stock = 20.0
    let item = create_test_item(3, "Stapler No. 10", None, None, 20.0, 80.0);
    tx.items().create(&item).expect("create item");

    let purchase_tx = create_test_purchase_tx(3, "PUR-1003", None, 1500.0, 1500.0, "2026-11-20");
    tx.purchases().create_transaction(&purchase_tx).expect("create pur");

    let lot = create_test_purchase_lot(55, 3, 3, None, 25.0, 25.0, 50.0, 20.0);
    tx.purchases().create_purchase_item(&lot).expect("create lot");

    seed_sale_line(&mut *tx, 10, 3);
    let alloc = create_test_allocation(1, 10, 55, 5.0, 50.0);
    tx.allocations().create_allocation(&alloc).expect("create alloc");

    // Action: Restore 5.0 units for return
    inventory_service.restore_stock_for_return(&mut *tx, 10, 3, 5.0).expect("restore stock");

    // Assert: Both Item stock cache AND Lot remaining quantity increment by 5.0
    let updated = tx.items().find_by_id(3).expect("find").expect("exists");
    assert_eq!(updated.current_stock, 25.0, "Item current_stock must increment from 20.0 to 25.0");

    let lots = tx.purchases().list_items_by_transaction_id(3).expect("list lots");
    let updated_lot = lots.iter().find(|l| l.id == 55).expect("lot exists");
    assert_eq!(updated_lot.remaining_quantity, 25.0, "Lot #55 remaining_quantity must increment from 20.0 to 25.0");
}

#[test]
fn test_tc_inv_004_negative_adjustment_creates_audit_trail() {
    let uow = TestDb::new();
    let inventory_service = InventoryServiceImpl::new(FifoStrategy, uow.clone());

    let mut tx = uow.begin_transaction().expect("begin tx");
    let item = create_test_item(4, "Ball Pen Blue", None, None, 20.0, 10.0);
    tx.items().create(&item).expect("create item");

    let purchase_tx = create_test_purchase_tx(4, "PUR-1004", None, 100.0, 100.0, "2026-11-20");
    tx.purchases().create_transaction(&purchase_tx).expect("create pur");

    let lot = create_test_purchase_lot(4, 4, 4, None, 20.0, 20.0, 5.0, 20.0);
    tx.purchases().create_purchase_item(&lot).expect("create lot");
    tx.commit().expect("commit setup");

    // Action: Negative adjustment of -3 units (Damaged)
    let payload = StockAdjustmentPayload {
        adjustment_date: "2026-11-20".to_string(),
        item_id: 4,
        quantity: -3.0,
        reason: AdjustmentReason::Damaged,
        notes: Some("Broken barrels".to_string()),
    };
    let adj_id = inventory_service.create_adjustment(payload).expect("create adjustment");
    assert!(adj_id > 0);

    // Assert: StockAdjustment created, current_stock decremented to 17.0
    let mut check_tx = uow.begin_transaction().expect("check tx");
    let updated = check_tx.items().find_by_id(4).expect("find").expect("exists");
    assert_eq!(updated.current_stock, 17.0);

    // Audit allocations exist
    let allocs = check_tx.outflow_allocations().find_by_source(OutflowSourceType::Adjustment, adj_id).expect("list allocs");
    assert_eq!(allocs.len(), 1);
    assert_eq!(allocs[0].quantity_depleted, 3.0);
    assert_eq!(allocs[0].unit_cost, 5.0);
}

#[test]
fn test_tc_inv_005_positive_adjustment_creates_synthetic_lot() {
    let uow = TestDb::new();
    let inventory_service = InventoryServiceImpl::new(FifoStrategy, uow.clone());

    let mut tx = uow.begin_transaction().expect("begin tx");
    let item = create_test_item(5, "Eraser Dust-Free", None, None, 10.0, 5.0);
    tx.items().create(&item).expect("create item");

    let purchase_tx = create_test_purchase_tx(5, "PUR-1005", None, 50.0, 50.0, "2026-11-20");
    tx.purchases().create_transaction(&purchase_tx).expect("create pur");

    let lot = create_test_purchase_lot(5, 5, 5, None, 10.0, 10.0, 3.0, 10.0);
    tx.purchases().create_purchase_item(&lot).expect("create lot");
    tx.commit().expect("commit setup");

    // Action: Positive adjustment of +10 units (FoundStock / CountCorrection)
    let payload = StockAdjustmentPayload {
        adjustment_date: "2026-11-20".to_string(),
        item_id: 5,
        quantity: 10.0,
        reason: AdjustmentReason::CountCorrection,
        notes: Some("Stock found in back storage".to_string()),
    };
    let adj_id = inventory_service.create_adjustment(payload).expect("create adjustment");
    assert!(adj_id > 0);

    // Assert: Synthetic purchase lot created with remaining_quantity = 10.0
    let mut check_tx = uow.begin_transaction().expect("check tx");
    let updated = check_tx.items().find_by_id(5).expect("find").expect("exists");
    assert_eq!(updated.current_stock, 20.0, "Stock must increment from 10.0 to 20.0");

    let active_lots = check_tx.purchases().get_active_lots_for_item(5).expect("active lots");
    assert_eq!(active_lots.len(), 2, "Must now have 2 active lots");
    let synth = active_lots.iter().find(|l| l.id != 5).expect("synthetic lot exists");
    assert_eq!(synth.remaining_quantity, 10.0);
    assert_eq!(synth.unit_cost, 3.0, "Synthetic lot adopts latest purchase cost");
}

#[test]
fn test_tc_inv_006_low_stock_flag_transition() {
    let uow = TestDb::new();
    let inventory_service = InventoryServiceImpl::new(FifoStrategy, uow.clone());

    let mut tx = uow.begin_transaction().expect("begin tx");
    // Setup: Item with stock = 6.0, is_low_stock = false
    let item = create_test_item(6, "Correction Pen", None, None, 6.0, 35.0);
    tx.items().create(&item).expect("create item");

    let purchase_tx = create_test_purchase_tx(6, "PUR-1006", None, 120.0, 120.0, "2026-11-20");
    tx.purchases().create_transaction(&purchase_tx).expect("create pur");

    let lot = create_test_purchase_lot(6, 6, 6, None, 6.0, 6.0, 20.0, 6.0);
    tx.purchases().create_purchase_item(&lot).expect("create lot");

    // Action: Deplete 2.0 units
    seed_sale_line(&mut *tx, 1, 6);
    inventory_service.deplete_stock_for_sale(&mut *tx, 1, 6, 2.0).expect("deplete stock");

    // Assert: stock transitions 6.0 -> 4.0; is_low_stock transitions false -> true (< 5.0)
    let updated = tx.items().find_by_id(6).expect("find").expect("exists");
    assert_eq!(updated.current_stock, 4.0);
    assert_eq!(updated.is_low_stock, true, "is_low_stock must transition to true when stock < 5");
}
