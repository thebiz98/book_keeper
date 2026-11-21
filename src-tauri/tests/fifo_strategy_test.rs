// tests/fifo_strategy_test.rs
//! Test Suite: Inventory Allocation Strategy (FIFO)
//! Test Cases: TC-FIFO-001 through TC-FIFO-006
//! Derived from book_keeper_LLD.md § 11.2 and Spec Miner Report

mod common;

use common::fixtures::*;
use common::{FifoStrategyImpl, InventoryServiceImpl, TestDb, PaymentServiceImpl, ReturnServiceImpl};
use book_keeper::core::dtos::{CustomerReturnItemPayload, CustomerReturnPayload};
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::inventory_allocation_strategy::InventoryAllocationStrategy;
use book_keeper::services::contracts::return_service::ReturnService;

#[test]
fn test_tc_fifo_001_single_lot_full_depletion() {
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().expect("begin transaction");

    // 1. Setup item and single purchase lot
    let item = create_test_item(1, "Apsara Pencil", Some("Stationery"), None, 10.0, 10.0);
    tx.items().create(&item).expect("create item");

    let purchase_tx = create_test_purchase_tx(1, "PUR-1001", None, 50.0, 50.0, "2026-11-20");
    tx.purchases().create_transaction(&purchase_tx).expect("create purchase tx");

    let lot1 = create_test_purchase_lot(1, 1, 1, None, 10.0, 10.0, 5.0, 10.0);
    tx.purchases().create_purchase_item(&lot1).expect("create lot 1");

    // 2. Action: deplete 10.0 base units for sale
    seed_sale_line(&mut *tx, 1, 1);
    let strategy = FifoStrategyImpl;
    let result = strategy
        .deplete_lots_for_sale(&mut *tx, 1, 1, 10.0)
        .expect("depletion must succeed");

    // 3. Assertions
    assert_eq!(result.total_cogs, 50.0, "COGS must equal 10 * 5.0 = 50.0");
    assert_eq!(result.allocations.len(), 1, "Exactly 1 allocation record must be created");
    assert_eq!(result.allocations[0].purchase_item_id, 1);
    assert_eq!(result.allocations[0].quantity_depleted, 10.0);
    assert_eq!(result.allocations[0].unit_cost, 5.0);

    // Lot #1 remaining quantity becomes 0.0
    let active_lots = tx.purchases().list_active_lots_for_item(1).expect("list active lots");
    assert!(active_lots.is_empty(), "Lot #1 must be fully depleted (remaining == 0.0)");
}

#[test]
fn test_tc_fifo_002_multi_lot_split_depletion_with_differing_costs() {
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().expect("begin transaction");

    // Setup: Lot #1 (5 @ 4.0), Lot #2 (10 @ 6.0)
    let item = create_test_item(1, "Reynolds Pen", Some("Stationery"), None, 15.0, 10.0);
    tx.items().create(&item).expect("create item");

    let purchase_tx = create_test_purchase_tx(1, "PUR-1001", None, 80.0, 80.0, "2026-11-20");
    tx.purchases().create_transaction(&purchase_tx).expect("create purchase tx");

    let lot1 = create_test_purchase_lot(1, 1, 1, None, 5.0, 5.0, 4.0, 5.0);
    let lot2 = create_test_purchase_lot(2, 1, 1, None, 10.0, 10.0, 6.0, 10.0);
    tx.purchases().create_purchase_item(&lot1).expect("create lot 1");
    tx.purchases().create_purchase_item(&lot2).expect("create lot 2");

    // Action: Deplete 8 units
    seed_sale_line(&mut *tx, 1, 1);
    let strategy = FifoStrategyImpl;
    let result = strategy
        .deplete_lots_for_sale(&mut *tx, 1, 1, 8.0)
        .expect("depletion must succeed");

    // Expected COGS: (5 * 4.0) + (3 * 6.0) = 20.0 + 18.0 = 38.0
    assert_eq!(result.total_cogs, 38.0);
    assert_eq!(result.allocations.len(), 2, "Exactly 2 allocations expected");

    // Allocation 1: 5 from Lot #1 @ 4.0
    assert_eq!(result.allocations[0].purchase_item_id, 1);
    assert_eq!(result.allocations[0].quantity_depleted, 5.0);
    assert_eq!(result.allocations[0].unit_cost, 4.0);

    // Allocation 2: 3 from Lot #2 @ 6.0
    assert_eq!(result.allocations[1].purchase_item_id, 2);
    assert_eq!(result.allocations[1].quantity_depleted, 3.0);
    assert_eq!(result.allocations[1].unit_cost, 6.0);

    // Remaining on Lot #2 must be 7.0
    let active_lots = tx.purchases().list_active_lots_for_item(1).expect("list active lots");
    assert_eq!(active_lots.len(), 1, "Only Lot #2 should remain active");
    assert_eq!(active_lots[0].id, 2);
    assert_eq!(active_lots[0].remaining_quantity, 7.0);
}

#[test]
fn test_tc_fifo_003_insufficient_stock_rejection() {
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().expect("begin transaction");

    // Setup: Lot #1 has remaining_quantity = 4.0
    let item = create_test_item(1, "Notebook 200pg", Some("Stationery"), None, 4.0, 50.0);
    tx.items().create(&item).expect("create item");

    let purchase_tx = create_test_purchase_tx(1, "PUR-1001", None, 120.0, 120.0, "2026-11-20");
    tx.purchases().create_transaction(&purchase_tx).expect("create purchase tx");

    let lot1 = create_test_purchase_lot(1, 1, 1, None, 4.0, 4.0, 30.0, 4.0);
    tx.purchases().create_purchase_item(&lot1).expect("create lot 1");

    // Action: Validate stock for 5.0 units
    let strategy = FifoStrategyImpl;
    let err = strategy
        .validate_stock_availability(&mut *tx, 1, 5.0)
        .unwrap_err();

    // Assert: Returns InsufficientStockError { item_id: 1, required: 5.0, available: 4.0 }
    match err {
        AppError::InsufficientStockError {
            item_id,
            required,
            available,
            ..
        } => {
            assert_eq!(item_id, 1);
            assert_eq!(required, 5.0);
            assert_eq!(available, 4.0);
        }
        other => panic!("Expected InsufficientStockError, got {:?}", other),
    }

    // Zero mutations: Lot #1 remains at 4.0
    let lots = tx.purchases().list_active_lots_for_item(1).expect("list active lots");
    assert_eq!(lots[0].remaining_quantity, 4.0);
}

#[test]
fn test_tc_fifo_004_restockable_return_lot_restoration_reverse_order() {
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().expect("begin transaction");

    // Item ID 42 (non-1 ID verifies dynamic resolution)
    let item = create_test_item(42, "Highlighter Yellow", None, None, 15.0, 25.0);
    tx.items().create(&item).expect("create item");

    let purchase_tx = create_test_purchase_tx(10, "PUR-1010", None, 150.0, 150.0, "2026-11-20");
    tx.purchases().create_transaction(&purchase_tx).expect("create purchase tx");

    // Lot 10: 2 units depleted (remaining 3 of 5); Lot 20: 3 units depleted (remaining 7 of 10)
    let lot1 = create_test_purchase_lot(10, 10, 42, None, 5.0, 5.0, 10.0, 3.0);
    let lot2 = create_test_purchase_lot(20, 10, 42, None, 10.0, 10.0, 12.0, 7.0);
    tx.purchases().create_purchase_item(&lot1).expect("create lot 10");
    tx.purchases().create_purchase_item(&lot2).expect("create lot 20");

    seed_sale_line(&mut *tx, 100, 42);
    let alloc1 = create_test_allocation(1, 100, 10, 2.0, 10.0);
    let alloc2 = create_test_allocation(2, 100, 20, 3.0, 12.0);
    tx.allocations().create(&alloc1).expect("create alloc 1");
    tx.allocations().create(&alloc2).expect("create alloc 2");

    // Action: Restore 4 units for return
    let strategy = FifoStrategyImpl;
    strategy
        .restore_lots_for_return(&mut *tx, 100, 4.0)
        .expect("restoration must succeed");

    // Assert: Lot #20 restored first (3 units -> 10.0), Lot #10 restored second (1 unit -> 4.0)
    let lots = tx.purchases().list_items_by_transaction_id(10).expect("list lots");
    let lot20_after = lots.iter().find(|l| l.id == 20).expect("lot 20 exists");
    let lot10_after = lots.iter().find(|l| l.id == 10).expect("lot 10 exists");

    assert_eq!(lot20_after.remaining_quantity, 10.0, "Lot #20 must absorb 3 units restored (7 + 3 = 10)");
    assert_eq!(lot10_after.remaining_quantity, 4.0, "Lot #10 must absorb 1 unit restored (3 + 1 = 4)");
}

#[test]
fn test_tc_fifo_005_scrap_return_zero_lot_restoration() {
    let uow = TestDb::new();
    let inventory_service = InventoryServiceImpl::new(FifoStrategyImpl, uow.clone());
    let payment_service = PaymentServiceImpl;
    let return_service = ReturnServiceImpl::new(inventory_service, payment_service, uow.clone());

    let mut tx = uow.begin_transaction().expect("begin tx");
    let item = create_test_item(7, "Calculator Scientific", None, None, 5.0, 500.0);
    tx.items().create(&item).expect("create item");

    let purchase_tx = create_test_purchase_tx(7, "PUR-1007", None, 1500.0, 1500.0, "2026-11-20");
    tx.purchases().create_transaction(&purchase_tx).expect("create pur");

    let lot = create_test_purchase_lot(70, 7, 7, None, 5.0, 5.0, 300.0, 0.0);
    tx.purchases().create_purchase_item(&lot).expect("create lot");

    seed_sale_line(&mut *tx, 15, 7);
    let alloc = create_test_allocation(1, 15, 70, 5.0, 300.0);
    tx.allocations().create(&alloc).expect("create alloc");
    tx.commit().expect("commit");

    // Action: Customer returns 1 scrap unit
    let payload = CustomerReturnPayload {
        return_code: Some("RET-1005".to_string()),
        return_date: "2026-11-20".to_string(),
        party_id: None,
        original_sale_id: Some(15),
        settlement_type: SettlementType::Refund,
        reason: Some("Cracked display - unusable".to_string()),
        notes: None,
        items: vec![CustomerReturnItemPayload {
            item_id: 7,
            sale_item_id: Some(15),
            quantity: 1.0,
            unit_price: 500.0,
            condition: StockCondition::DamagedScrap,
        }],
    };
    let ret_code = return_service.create_customer_return(payload).expect("scrap return succeeds");
    assert_eq!(ret_code, "RET-1005");

    // Assert: Exactly 0 units restored to active lots and current stock remains unchanged
    let mut check_tx = uow.begin_transaction().expect("check tx");
    let lots = check_tx.purchases().list_items_by_transaction_id(7).unwrap();
    let lot_after = lots.iter().find(|l| l.id == 70).expect("lot exists");
    assert_eq!(lot_after.remaining_quantity, 0.0, "Scrap return must NOT restore purchase lot remaining quantity");

    let item_after = check_tx.items().find_by_id(7).unwrap().expect("item exists");
    assert_eq!(item_after.current_stock, 5.0, "Scrap return must NOT increment item stock cache");
}

#[test]
fn test_tc_fifo_006_negative_stock_adjustment_fifo_depletion() {
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().expect("begin transaction");

    // Setup: Lot #1 has qty = 10, unit_cost = 12.0
    let item = create_test_item(1, "Marker Pen", None, None, 10.0, 20.0);
    tx.items().create(&item).expect("create item");

    let purchase_tx = create_test_purchase_tx(1, "PUR-1001", None, 120.0, 120.0, "2026-11-20");
    tx.purchases().create_transaction(&purchase_tx).expect("create purchase tx");

    let lot1 = create_test_purchase_lot(1, 1, 1, None, 10.0, 10.0, 12.0, 10.0);
    tx.purchases().create_purchase_item(&lot1).expect("create lot 1");

    // Action: Deplete 3.0 units for negative stock adjustment (damaged write-off)
    let strategy = FifoStrategyImpl;
    seed_adjustment(&mut *tx, 1, 1);
    let adj_result = strategy
        .deplete_lots_for_adjustment(&mut *tx, 1, 1, 3.0)
        .expect("adjustment depletion must succeed");

    // Assert: Loss value = 3.0 * 12.0 = 36.0 ₹
    assert_eq!(adj_result.total_loss_value, 36.0);
    assert_eq!(adj_result.allocations.len(), 1);
    assert_eq!(adj_result.allocations[0].source_id, 1);
    assert_eq!(adj_result.allocations[0].source_type, OutflowSourceType::Adjustment);
    assert_eq!(adj_result.allocations[0].purchase_item_id, 1);
    assert_eq!(adj_result.allocations[0].quantity_depleted, 3.0);
    assert_eq!(adj_result.allocations[0].unit_cost, 12.0);

    // Lot #1 remaining quantity becomes 7.0
    let active_lots = tx.purchases().list_active_lots_for_item(1).expect("list active lots");
    assert_eq!(active_lots[0].remaining_quantity, 7.0);
}
