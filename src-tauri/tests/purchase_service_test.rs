// tests/purchase_service_test.rs
//! Test Suite: Inward Purchase Service & Lot Cost Normalization
//! Test Cases: TC-PUR-001 through TC-PUR-003
//! Derived from book_keeper_LLD.md § 11.6 and Spec Miner Report

mod common;

use common::fixtures::*;
use common::*;
use book_keeper::core::dtos::*;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::purchase_service::PurchaseService;

fn setup_purchase_service() -> (PurchaseServiceImpl<InventoryServiceImpl<FifoStrategyImpl>, PaymentServiceImpl>, TestDb) {
    let uow = TestDb::new();
    let inventory_service = InventoryServiceImpl::new(FifoStrategyImpl, uow.clone());
    let payment_service = PaymentServiceImpl;
    let purchase_service = PurchaseServiceImpl::new(inventory_service, payment_service, uow.clone());
    (purchase_service, uow)
}

#[test]
fn test_tc_pur_001_loose_base_unit_inward() {
    let (purchase_service, uow) = setup_purchase_service();
    let mut tx = uow.begin_transaction().expect("begin tx");

    let item = create_test_item(1, "Spiral Register 160pg", None, None, 0.0, 70.0);
    tx.items().create(&item).expect("create item");
    tx.commit().expect("commit");

    let payload = CreatePurchasePayload {
        transaction_code: Some("PUR-1001".to_string()),
        transaction_date: "2026-11-20".to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        paid_amount: 500.0,
        notes: None,
        items: vec![CreatePurchaseItemPayload {
            item_id: 1,
            bundle_id: None,
            bundle_quantity: 50.0,
            unit_cost: 10.0,
        }],
    };
    let pur_id = purchase_service.create_purchase(payload).expect("create purchase");
    assert!(pur_id > 0);

    let mut check_tx = uow.begin_transaction().expect("check tx");
    let pur = check_tx.purchases().find_by_id(pur_id).expect("find").expect("exists");
    assert_eq!(pur.transaction_code, "PUR-1001");
    assert_eq!(pur.total_amount, 500.0);
    assert_eq!(pur.payment_status, PaymentStatus::Paid);

    let lots = check_tx.purchases().list_active_lots_for_item(1).expect("lots");
    assert_eq!(lots.len(), 1);
    assert_eq!(lots[0].remaining_quantity, 50.0);
    assert_eq!(lots[0].unit_cost, 10.0);

    let updated_item = check_tx.items().find_by_id(1).expect("find").expect("exists");
    assert_eq!(updated_item.current_stock, 50.0);
    assert_eq!(updated_item.is_low_stock, false);
}

#[test]
fn test_tc_pur_002_packaging_bundle_inward_normalization() {
    let (purchase_service, uow) = setup_purchase_service();
    let mut tx = uow.begin_transaction().expect("begin tx");

    let item = create_test_item(2, "White Chalk Box", None, None, 0.0, 15.0);
    tx.items().create(&item).expect("create item");

    let bundle = create_test_bundle(1, 2, "Carton", 500.0, Some(5000.0), None);
    tx.bundles().create(&bundle).expect("create bundle");
    tx.commit().expect("commit");

    let payload = CreatePurchasePayload {
        transaction_code: Some("PUR-1002".to_string()),
        transaction_date: "2026-11-20".to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        paid_amount: 8000.0,
        notes: None,
        items: vec![CreatePurchaseItemPayload {
            item_id: 2,
            bundle_id: Some(1),
            bundle_quantity: 2.0,
            unit_cost: 4000.0,
        }],
    };
    let pur_id = purchase_service.create_purchase(payload).expect("bundle purchase");
    assert!(pur_id > 0);

    let mut check_tx = uow.begin_transaction().expect("check tx");
    let lots = check_tx.purchases().list_active_lots_for_item(2).expect("lots");
    assert_eq!(lots.len(), 1);
    assert_eq!(lots[0].quantity, 1000.0);
    assert_eq!(lots[0].unit_cost, 8.0);
    assert_eq!(lots[0].remaining_quantity, 1000.0);

    let updated_item = check_tx.items().find_by_id(2).expect("find").expect("exists");
    assert_eq!(updated_item.current_stock, 1000.0);
}

#[test]
fn test_tc_pur_003_partial_payment_supplier_purchase() {
    let (purchase_service, uow) = setup_purchase_service();
    let mut tx = uow.begin_transaction().expect("begin tx");

    let supplier = create_test_party(5, "National Stationery Wholesalers", PartyType::Vendor, Some("9123456789"));
    tx.parties().create(&supplier).expect("create supplier");

    let item = create_test_item(3, "Classmate Long Notebook", None, None, 0.0, 60.0);
    tx.items().create(&item).expect("create item");
    tx.commit().expect("commit");

    let payload = CreatePurchasePayload {
        transaction_code: Some("PUR-1003".to_string()),
        transaction_date: "2026-11-20".to_string(),
        party_id: Some(5),
        payment_mode: Some("BANK_TRANSFER".to_string()),
        paid_amount: 2000.0,
        notes: Some("Inv #NSW-889".to_string()),
        items: vec![CreatePurchaseItemPayload {
            item_id: 3,
            bundle_id: None,
            bundle_quantity: 100.0,
            unit_cost: 50.0,
        }],
    };
    let pur_id = purchase_service.create_purchase(payload).expect("partial purchase");

    let mut check_tx = uow.begin_transaction().expect("check tx");
    let pur = check_tx.purchases().find_by_id(pur_id).expect("find").expect("exists");
    assert_eq!(pur.total_amount, 5000.0);
    assert_eq!(pur.paid_amount, 2000.0);
    assert_eq!(pur.balance_due, 3000.0);
    assert_eq!(pur.payment_status, PaymentStatus::Partial);
}
