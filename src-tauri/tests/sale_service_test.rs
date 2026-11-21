// tests/sale_service_test.rs
//! Test Suite: Sale Checkout Orchestration & Rapid Billing
//! Test Cases: TC-SALE-001 through TC-SALE-007
//! Derived from book_keeper_LLD.md § 11.5 and Spec Miner Report

mod common;

use common::fixtures::*;
use common::*;
use book_keeper::core::dtos::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::sale_service::SaleService;

fn setup_sale_service() -> (SaleServiceImpl<InventoryServiceImpl<FifoStrategyImpl>, PaymentServiceImpl>, MockUnitOfWork) {
    let uow = MockUnitOfWork::new();
    let inventory_service = InventoryServiceImpl::new(FifoStrategyImpl, uow.clone());
    let payment_service = PaymentServiceImpl;
    let sale_service = SaleServiceImpl::new(inventory_service, payment_service, uow.clone());
    (sale_service, uow)
}

#[test]
fn test_tc_sale_001_full_cash_sale_paid() {
    let (sale_service, uow) = setup_sale_service();
    let mut tx = uow.begin_transaction().expect("begin tx");

    let item = create_test_item(1, "Pilot Pen Blue", None, None, 20.0, 15.0);
    tx.items().create(&item).expect("create item");

    let pur = create_test_purchase_tx(1, "PUR-1001", None, 200.0, 200.0, "2026-11-20");
    tx.purchases().create_transaction(&pur).expect("create pur");
    let lot = create_test_purchase_lot(1, 1, 1, None, 20.0, 20.0, 10.0, 20.0);
    tx.purchases().create_purchase_item(&lot).expect("create lot");
    tx.commit().expect("commit setup");

    // Action: create_sale with 2 base units, paid_amount = 30.0
    let payload = CreateSalePayload {
        transaction_code: Some("SAL-1001".to_string()),
        transaction_date: "2026-11-20".to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        paid_amount: 30.0,
        notes: None,
        items: vec![CreateSaleItemPayload {
            item_id: 1,
            bundle_id: None,
            bundle_quantity: 2.0,
            unit_price: Some(15.0),
        }],
    };
    let summary = sale_service.create_sale(payload).expect("sale must succeed");

    assert_eq!(summary.payment_status, PaymentStatus::Paid);
    assert_eq!(summary.total_amount, 30.0);
    assert_eq!(summary.paid_amount, 30.0);
    assert_eq!(summary.balance_due, 0.0);

    let mut check_tx = uow.begin_transaction().expect("check tx");
    let updated_item = check_tx.items().find_by_id(1).expect("find").expect("exists");
    assert_eq!(updated_item.current_stock, 18.0);
}

#[test]
fn test_tc_sale_002_partial_payment_sale() {
    let (sale_service, uow) = setup_sale_service();
    let mut tx = uow.begin_transaction().expect("begin tx");

    let item = create_test_item(2, "Parker Pen", None, None, 10.0, 100.0);
    tx.items().create(&item).expect("create item");

    let pur = create_test_purchase_tx(2, "PUR-1002", None, 600.0, 600.0, "2026-11-20");
    tx.purchases().create_transaction(&pur).expect("create pur");
    let lot = create_test_purchase_lot(2, 2, 2, None, 10.0, 10.0, 60.0, 10.0);
    tx.purchases().create_purchase_item(&lot).expect("create lot");
    tx.commit().expect("commit setup");

    let payload = CreateSalePayload {
        transaction_code: Some("SAL-1002".to_string()),
        transaction_date: "2026-11-20".to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        paid_amount: 40.0,
        notes: None,
        items: vec![CreateSaleItemPayload {
            item_id: 2,
            bundle_id: None,
            bundle_quantity: 1.0,
            unit_price: Some(100.0),
        }],
    };
    let summary = sale_service.create_sale(payload).expect("sale must succeed");

    assert_eq!(summary.payment_status, PaymentStatus::Partial);
    assert_eq!(summary.total_amount, 100.0);
    assert_eq!(summary.paid_amount, 40.0);
    assert_eq!(summary.balance_due, 60.0);
}

#[test]
fn test_tc_sale_003_full_credit_sale_unpaid() {
    let (sale_service, uow) = setup_sale_service();
    let mut tx = uow.begin_transaction().expect("begin tx");

    let party = create_test_party(10, "Sharma Ji", PartyType::Customer, Some("9876543210"));
    tx.parties().create(&party).expect("create party");

    let item = create_test_item(3, "Drawing Book A4", None, None, 50.0, 20.0);
    tx.items().create(&item).expect("create item");

    let pur = create_test_purchase_tx(3, "PUR-1003", None, 500.0, 500.0, "2026-11-20");
    tx.purchases().create_transaction(&pur).expect("create pur");
    let lot = create_test_purchase_lot(3, 3, 3, None, 50.0, 50.0, 10.0, 50.0);
    tx.purchases().create_purchase_item(&lot).expect("create lot");
    tx.commit().expect("commit setup");

    let payload = CreateSalePayload {
        transaction_code: Some("SAL-1003".to_string()),
        transaction_date: "2026-11-20".to_string(),
        party_id: Some(10),
        payment_mode: None,
        paid_amount: 0.0,
        notes: Some("Credit sale".to_string()),
        items: vec![CreateSaleItemPayload {
            item_id: 3,
            bundle_id: None,
            bundle_quantity: 5.0,
            unit_price: Some(20.0),
        }],
    };
    let summary = sale_service.create_sale(payload).expect("credit sale");

    assert_eq!(summary.payment_status, PaymentStatus::Unpaid);
    assert_eq!(summary.balance_due, 100.0);

    let mut check_tx = uow.begin_transaction().expect("check tx");
    let unpaid = check_tx.sales().list_unpaid_by_party_id(10).expect("unpaid");
    assert_eq!(unpaid.len(), 1);
    assert_eq!(unpaid[0].balance_due, 100.0);
}

#[test]
fn test_tc_sale_004_bundle_unit_sale_multiplier_conversion() {
    let (sale_service, uow) = setup_sale_service();
    let mut tx = uow.begin_transaction().expect("begin tx");

    let item = create_test_item(4, "Gel Pen Black", None, None, 100.0, 10.0);
    tx.items().create(&item).expect("create item");

    let bundle = create_test_bundle(1, 4, "Box of 10", 10.0, Some(90.0), None);
    tx.bundles().create(&bundle).expect("create bundle");

    let pur = create_test_purchase_tx(4, "PUR-1004", None, 500.0, 500.0, "2026-11-20");
    tx.purchases().create_transaction(&pur).expect("create pur");
    let lot = create_test_purchase_lot(4, 4, 4, None, 100.0, 100.0, 5.0, 100.0);
    tx.purchases().create_purchase_item(&lot).expect("create lot");
    tx.commit().expect("commit setup");

    let payload = CreateSalePayload {
        transaction_code: Some("SAL-1004".to_string()),
        transaction_date: "2026-11-20".to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        paid_amount: 180.0,
        notes: None,
        items: vec![CreateSaleItemPayload {
            item_id: 4,
            bundle_id: Some(1),
            bundle_quantity: 2.0,
            unit_price: Some(90.0), // rate per Box (LLD TC-SALE-004)
        }],
    };
    let summary = sale_service.create_sale(payload).expect("bundle sale");
    assert_eq!(summary.total_amount, 180.0);

    let mut check_tx = uow.begin_transaction().expect("check tx");
    let updated = check_tx.items().find_by_id(4).expect("find").expect("exists");
    assert_eq!(updated.current_stock, 80.0);
}

#[test]
fn test_tc_sale_005_low_stock_flag_transition() {
    let (sale_service, uow) = setup_sale_service();
    let mut tx = uow.begin_transaction().expect("begin tx");

    let item = create_test_item(5, "Sketch Pen Set", None, None, 6.0, 45.0);
    tx.items().create(&item).expect("create item");

    let pur = create_test_purchase_tx(5, "PUR-1005", None, 180.0, 180.0, "2026-11-20");
    tx.purchases().create_transaction(&pur).expect("create pur");
    let lot = create_test_purchase_lot(5, 5, 5, None, 6.0, 6.0, 30.0, 6.0);
    tx.purchases().create_purchase_item(&lot).expect("create lot");
    tx.commit().expect("commit setup");

    let payload = CreateSalePayload {
        transaction_code: Some("SAL-1005".to_string()),
        transaction_date: "2026-11-20".to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        paid_amount: 90.0,
        notes: None,
        items: vec![CreateSaleItemPayload {
            item_id: 5,
            bundle_id: None,
            bundle_quantity: 2.0,
            unit_price: Some(45.0),
        }],
    };
    sale_service.create_sale(payload).expect("sale");

    let mut check_tx = uow.begin_transaction().expect("check tx");
    let updated = check_tx.items().find_by_id(5).expect("find").expect("exists");
    assert_eq!(updated.current_stock, 4.0);
    assert_eq!(updated.is_low_stock, true);

    let low_stock_items = check_tx.items().list_catalog(None, true, 0, 10).expect("catalog");
    assert!(low_stock_items.iter().any(|i| i.id == 5));
}

#[test]
fn test_tc_sale_006_empty_cart_validation() {
    let (sale_service, _uow) = setup_sale_service();

    let payload = CreateSalePayload {
        transaction_code: Some("SAL-1006".to_string()),
        transaction_date: "2026-11-20".to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        paid_amount: 0.0,
        notes: None,
        items: vec![],
    };
    let err = sale_service.create_sale(payload).unwrap_err();

    match err {
        AppError::ValidationError { field, message } => {
            assert_eq!(field, "items");
            assert!(message.contains("items cannot be empty"));
        }
        other => panic!("Expected ValidationError, got {:?}", other),
    }
}

#[test]
fn test_tc_sale_007_sequential_transaction_code_generation() {
    let (sale_service, uow) = setup_sale_service();
    let mut tx = uow.begin_transaction().expect("begin tx");

    let sale = create_test_sale_tx(5, "SAL-1005", None, 100.0, 100.0, "2026-11-20");
    tx.sales().create_transaction(&sale).expect("create sale");
    tx.commit().expect("commit");

    let next_code = sale_service.get_next_sale_code().expect("get next code");
    assert_eq!(next_code, "SAL-1006");
}
