// tests/e2e_tier3_combinations_test.rs
//! Test Suite: Tier 3 Pairwise & Multi-Module Combinatorial Integration (TC-T3-CMB-001..010)
//! 10 multi-feature workflow integration tests covering cross-subsystem interactions.
//! Derived from TEST_INFRA.md § Feature Inventory and book_keeper_LLD.md.

mod common;

use std::time::Instant;

use common::fixtures::*;
use common::*;
use book_keeper::core::dtos::*;
use book_keeper::core::entities::*;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::search_indexer::ItemSearchIndexer;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::accounting_service::AccountingService;
use book_keeper::services::contracts::inventory_service::InventoryService;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::report_service::ReportService;
use book_keeper::services::contracts::return_service::ReturnService;
use book_keeper::services::contracts::sale_service::SaleService;

struct ComboTestHarness {
    uow: TestDb,
    inventory_service: InventoryServiceImpl<FifoStrategy>,
    purchase_service: PurchaseServiceImpl<InventoryServiceImpl<FifoStrategy>, PaymentServiceImpl>,
    sale_service: SaleServiceImpl<InventoryServiceImpl<FifoStrategy>, PaymentServiceImpl>,
    return_service: ReturnServiceImpl<InventoryServiceImpl<FifoStrategy>, PaymentServiceImpl, AccountingServiceImpl<PaymentServiceImpl>>,
    accounting_service: AccountingServiceImpl<PaymentServiceImpl>,
    report_service: ReportServiceImpl,
    indexer: book_keeper::data::sqlite::fts_indexer::SqliteFts5SearchIndexer,
}

fn create_combo_harness() -> ComboTestHarness {
    let uow = TestDb::new();
    let fifo = FifoStrategy;
    let inventory_service = InventoryServiceImpl::new(fifo, uow.clone());
    let payment_service = PaymentServiceImpl;
    let purchase_service = PurchaseServiceImpl::new(inventory_service.clone(), payment_service, uow.clone());
    let sale_service = SaleServiceImpl::new(inventory_service.clone(), payment_service, uow.clone());
    let accounting_service = AccountingServiceImpl::new(payment_service, uow.clone());
    let return_service = ReturnServiceImpl::new(inventory_service.clone(), payment_service, accounting_service.clone(), uow.clone());
    let report_service = ReportServiceImpl::with_fixed_today(uow.clone(), "2026-11-20");
    let indexer = uow.indexer();

    ComboTestHarness {
        uow,
        inventory_service,
        purchase_service,
        sale_service,
        return_service,
        accounting_service,
        report_service,
        indexer,
    }
}

// ============================================================================
// COMBINATION TESTS (TC-T3-CMB-001..010)
// ============================================================================

/// Combo 1: Inward -> Sale -> Return -> Adjustment -> Valuation Audit
#[test]
fn test_tc_t3_cmb_001_inward_sale_return_adjustment_valuation() {
    let harness = create_combo_harness();
    {
        let mut tx = harness.uow.begin_transaction().unwrap();
        let item = create_test_item(1, "Geometry Set", None, None, 0.0, 100.0);
        tx.items().create(&item).unwrap();
        tx.commit().unwrap();
    }

    // Step 1: Inward 100 pcs @ ₹60/pc = ₹6,000
    let pur_payload = CreatePurchasePayload {
        charges: vec![],
        transaction_date: "2026-11-20".to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        supplier_invoice_ref: None, payment_status: PaymentStatus::Paid, paid_amount: 6000.0,
        notes: None,
        items: vec![CreatePurchaseItemPayload {
            item_id: 1,
            bundle_id: None,
            bundle_quantity: 100.0,
            unit_cost: 60.0,
        }],
    };
    harness.purchase_service.create_purchase(pur_payload).expect("purchase");

    // Step 2: Sale of 40 pcs @ ₹100/pc (leaves 60 pcs)
    let sale_payload = CreateSalePayload {
        charges: vec![],
        transaction_date: "2026-11-20".to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        payment_status: PaymentStatus::Paid, paid_amount: 4000.0,
        notes: None,
        items: vec![CreateSaleItemPayload {
            item_id: 1,
            bundle_id: None,
            bundle_quantity: 40.0,
            unit_price: 100.0,
        }],
    };
    harness.sale_service.create_sale(sale_payload).expect("sale");

    // Step 3: Customer return of 10 restockable pcs (restores stock to 70 pcs)
    let ret_payload = CustomerReturnPayload {
        return_date: "2026-11-20".to_string(),
        party_id: None,
        original_sale_id: Some(1),
        settlement_type: SettlementType::Refund,
        reason: None,
        notes: None,
        replacement_items: vec![],
        items: vec![CustomerReturnItemPayload {
            item_id: 1,
            sale_item_id: Some(1),
            quantity: 10.0,
            unit_price: 100.0,
            condition: StockCondition::Restockable,
            unit_cost_override: None,
        }],
    };
    harness.return_service.create_customer_return(ret_payload).expect("return");

    // Step 4: Write-off 5 damaged pcs via adjustment (leaves 65 pcs)
    let adj_payload = StockAdjustmentPayload {
        adjustment_date: "2026-11-20".to_string(),
        item_id: 1,
        quantity: -5.0,
        reason: AdjustmentReason::Damaged,
        unit_cost: None,
        notes: None,
    };
    harness.inventory_service.create_adjustment(adj_payload).expect("adjustment");

    // Step 5: Stock Valuation Audit: 65 pcs @ ₹60 = ₹3,900
    let valuation = harness.report_service.get_stock_valuation_report().expect("valuation");
    assert_eq!(valuation.total_valuation, 3900.0, "Valuation must reflect 65 * 60 = ₹3,900");

    let mut check_tx = harness.uow.begin_transaction().unwrap();
    let item = check_tx.items().find_by_id(1).unwrap().unwrap();
    assert_eq!(item.current_stock, 65.0);
}

/// Combo 2: Credit Sale -> Aging Dues -> Cash Drawer Settlement -> Ledger
#[test]
fn test_tc_t3_cmb_002_credit_sale_dues_drawer_settlement_ledger() {
    let harness = create_combo_harness();
    {
        let mut tx = harness.uow.begin_transaction().unwrap();
        let customer = create_test_party(1, "Sharma Coaching", PartyType::Customer, Some("9811122233"));
        tx.parties().create(&customer).unwrap();
        let item = create_test_item(1, "Exam Clipboard", None, None, 50.0, 50.0);
        tx.items().create(&item).unwrap();

        let pur = create_test_purchase_tx(1, "PUR-C2", None, 1500.0, 1500.0, "2026-11-01");
        tx.purchases().create_transaction(&pur).unwrap();
        let lot = create_test_purchase_lot(1, 1, 1, None, 50.0, 50.0, 30.0, 50.0);
        tx.purchases().create_purchase_item(&lot).unwrap();
        tx.commit().unwrap();
    }

    // Step 1: Customer Credit Sale: 30 clipboards @ ₹50 = ₹1,500 due
    let sale_payload = CreateSalePayload {
        charges: vec![],
        transaction_date: "2026-11-01".to_string(),
        party_id: Some(1),
        payment_mode: None,
        payment_status: PaymentStatus::Unpaid, paid_amount: 0.0,
        notes: None,
        items: vec![CreateSaleItemPayload {
            item_id: 1,
            bundle_id: None,
            bundle_quantity: 30.0,
            unit_price: 50.0,
        }],
    };
    harness.sale_service.create_sale(sale_payload).expect("credit sale");

    // Step 2: Verify aging dues reflects ₹1,500
    let dues = harness.accounting_service.get_party_dues_summary(PartyType::Customer).unwrap();
    assert_eq!(dues[0].balance_due, 1500.0);

    // Step 3: Cash settlement installment of ₹600 on 2026-11-15
    let settle = SettlePaymentPayload {
        party_id: 1,
        transaction_type: TransactionType::Sale,
        amount_paid: 600.0,
        settlement_date: "2026-11-15".to_string(),
        payment_mode: Some("CASH".to_string()),
        transaction_id: Some(1),
        notes: Some("Installment payment".to_string()),
    };
    harness.accounting_service.settle_bill_payment(settle).expect("settlement");

    // Step 4: Verify remaining balance is ₹900
    let dues_after = harness.accounting_service.get_party_dues_summary(PartyType::Customer).unwrap();
    assert_eq!(dues_after[0].balance_due, 900.0);

    // Step 5: Cash drawer for 2026-11-15 must reflect the ₹600 cash inflow
    let drawer_today = harness.accounting_service.calculate_live_cash_drawer("2026-11-15").unwrap();
    assert_eq!(drawer_today, 600.0);
}

/// Combo 3: Bundle Inward -> Loose Sale -> Bundle Suggestions
#[test]
fn test_tc_t3_cmb_003_bundle_inward_loose_sale_bundle_suggestions() {
    let harness = create_combo_harness();
    {
        let mut tx = harness.uow.begin_transaction().unwrap();
        let item = create_test_item(1, "Reynolds Gel Pen", None, None, 0.0, 15.0);
        tx.items().create(&item).unwrap();
        let bundle = create_test_bundle(1, 1, "Box of 10", 10.0, Some(130.0), None);
        tx.bundles().create(&bundle).unwrap();
        tx.commit().unwrap();
    }

    // Inward: 5 boxes of 10 = 50 base units @ ₹80/box (cost ₹8/pc)
    let pur_payload = CreatePurchasePayload {
        charges: vec![],
        transaction_date: "2026-11-20".to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        supplier_invoice_ref: None, payment_status: PaymentStatus::Paid, paid_amount: 400.0,
        notes: None,
        items: vec![CreatePurchaseItemPayload {
            item_id: 1,
            bundle_id: Some(1),
            bundle_quantity: 5.0,
            unit_cost: 80.0,
        }],
    };
    harness.purchase_service.create_purchase(pur_payload).expect("bundle inward");

    // Loose sale of 7 base units @ ₹15 = ₹105
    let sale_payload = CreateSalePayload {
        charges: vec![],
        transaction_date: "2026-11-20".to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        payment_status: PaymentStatus::Paid, paid_amount: 105.0,
        notes: None,
        items: vec![CreateSaleItemPayload {
            item_id: 1,
            bundle_id: None,
            bundle_quantity: 7.0,
            unit_price: 15.0,
        }],
    };
    harness.sale_service.create_sale(sale_payload).expect("loose sale");

    let mut check_tx = harness.uow.begin_transaction().unwrap();
    let item = check_tx.items().find_by_id(1).unwrap().unwrap();
    assert_eq!(item.current_stock, 43.0, "50 - 7 = 43");

    let suggestions = check_tx.bundles().get_bundle_suggestions().unwrap();
    assert!(!suggestions.is_empty());
    assert_eq!(suggestions[0].bundle_name, "Box of 10");
}

/// Combo 5: Vendor Credit Purchase -> Vendor Return -> Dues Offset -> Net Profit Audit
#[test]
fn test_tc_t3_cmb_005_vendor_credit_return_dues_offset_profit() {
    let harness = create_combo_harness();
    {
        let mut tx = harness.uow.begin_transaction().unwrap();
        let vendor = create_test_party(1, "Wholesale Corp", PartyType::Vendor, None);
        tx.parties().create(&vendor).unwrap();
        let item = create_test_item(1, "Legal Pad", None, None, 0.0, 50.0);
        tx.items().create(&item).unwrap();
        tx.commit().unwrap();
    }

    // Purchase 50 units @ ₹30 = ₹1,500 on credit
    let pur_payload = CreatePurchasePayload {
        charges: vec![],
        transaction_date: "2026-11-20".to_string(),
        party_id: Some(1),
        payment_mode: None,
        supplier_invoice_ref: None, payment_status: PaymentStatus::Unpaid, paid_amount: 0.0,
        notes: None,
        items: vec![CreatePurchaseItemPayload {
            item_id: 1,
            bundle_id: None,
            bundle_quantity: 50.0,
            unit_cost: 30.0,
        }],
    };
    let pur_id = harness.purchase_service.create_purchase(pur_payload).expect("credit purchase").purchase_id;

    // Return 10 units to vendor (debit note ₹300)
    let ret_payload = VendorReturnPayload {
        return_date: "2026-11-20".to_string(),
        party_id: Some(1),
        original_purchase_id: Some(pur_id),
        settlement_type: SettlementType::Refund,
        reason: Some("Surplus stock returned".to_string()),
        notes: None,
        items: vec![VendorReturnItemPayload {
            item_id: 1,
            purchase_item_id: Some(1),
            quantity: 10.0,
        }],
    };
    harness.return_service.create_vendor_return(ret_payload).expect("vendor return");

    // Sell 20 units @ ₹50 = ₹1,000 revenue. COGS = 20 * 30 = ₹600. Net profit = ₹400.
    let sale_payload = CreateSalePayload {
        charges: vec![],
        transaction_date: "2026-11-20".to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        payment_status: PaymentStatus::Paid, paid_amount: 1000.0,
        notes: None,
        items: vec![CreateSaleItemPayload {
            item_id: 1,
            bundle_id: None,
            bundle_quantity: 20.0,
            unit_price: 50.0,
        }],
    };
    harness.sale_service.create_sale(sale_payload).expect("sale created");

    let pnl = harness.report_service.get_pnl_report("2026-11-01", "2026-11-30").expect("pnl");
    assert_eq!(pnl.gross_sales, 1000.0);
    assert_eq!(pnl.cogs, 600.0);
    assert_eq!(pnl.net_profit, 400.0);
}

/// Combo 6: Split Payment Sale (Cash + UPI + Credit) -> Partial Settlement -> Cash Flow
#[test]
fn test_tc_t3_cmb_006_split_payment_partial_settlement_cash_flow() {
    let harness = create_combo_harness();
    {
        let mut tx = harness.uow.begin_transaction().unwrap();
        let customer = create_test_party(1, "Split Customer", PartyType::Customer, None);
        tx.parties().create(&customer).unwrap();
        let item = create_test_item(1, "Executive Diary", None, None, 20.0, 300.0);
        tx.items().create(&item).unwrap();
        let pur = create_test_purchase_tx(1, "PUR-C6", None, 3000.0, 3000.0, "2026-11-20");
        tx.purchases().create_transaction(&pur).unwrap();
        let lot = create_test_purchase_lot(1, 1, 1, None, 20.0, 20.0, 150.0, 20.0);
        tx.purchases().create_purchase_item(&lot).unwrap();
        tx.commit().unwrap();
    }

    // Total = 2 * 300 = ₹600. Cash tendered upfront = ₹200. Balance due = ₹400.
    let sale_payload = CreateSalePayload {
        charges: vec![],
        transaction_date: "2026-11-20".to_string(),
        party_id: Some(1),
        payment_mode: Some("CASH".to_string()),
        payment_status: PaymentStatus::Partial, paid_amount: 200.0,
        notes: None,
        items: vec![CreateSaleItemPayload {
            item_id: 1,
            bundle_id: None,
            bundle_quantity: 2.0,
            unit_price: 300.0,
        }],
    };
    harness.sale_service.create_sale(sale_payload).expect("split sale");

    // Subsequent UPI installment of ₹300
    let settle = SettlePaymentPayload {
        party_id: 1,
        transaction_type: TransactionType::Sale,
        amount_paid: 300.0,
        settlement_date: "2026-11-20".to_string(),
        payment_mode: Some("UPI".to_string()),
        transaction_id: Some(1),
        notes: None,
    };
    harness.accounting_service.settle_bill_payment(settle).expect("settle");

    // Remaining balance must be ₹100
    let dues = harness.accounting_service.get_party_dues_summary(PartyType::Customer).unwrap();
    assert_eq!(dues[0].balance_due, 100.0);

    // Cash flow report reflects ₹200 (cash) + ₹300 (UPI) = ₹500 inflow
    let cash_flow = harness.report_service.get_cash_flow_report("2026-11-01", "2026-11-30").unwrap();
    assert_eq!(cash_flow.sales_cash_inflow, 200.0, "upfront counted once (TC-PAY-005)");
    assert_eq!(cash_flow.customer_settlements_inflow, 300.0);
    assert_eq!(cash_flow.net_cash_balance, 500.0);
}

/// Combo 7: Barcode Scan -> Cart Add -> Bundle Multiplier -> Discount -> Split Tender
#[test]
fn test_tc_t3_cmb_007_barcode_scan_bundle_multiplier_tender() {
    let harness = create_combo_harness();
    let barcode = "8901030991122";
    let item = create_test_item(1, "Cello Quick Ball Pen", Some("Pens"), Some(barcode), 100.0, 10.0);
    let bundle = create_test_bundle(1, 1, "Pack of 5", 5.0, Some(45.0), None);
    {
        let mut tx = harness.uow.begin_transaction().unwrap();
        tx.items().create(&item).unwrap();
        tx.bundles().create(&bundle).unwrap();
        let pur = create_test_purchase_tx(1, "PUR-C7", None, 500.0, 500.0, "2026-11-20");
        tx.purchases().create_transaction(&pur).unwrap();
        let lot = create_test_purchase_lot(1, 1, 1, None, 100.0, 100.0, 5.0, 100.0);
        tx.purchases().create_purchase_item(&lot).unwrap();
        tx.commit().unwrap();
    }
    harness.indexer.index_item_with_bundles(&item, &[bundle]).expect("index");

    // 1. Barcode scan
    let start = Instant::now();
    let found = harness.indexer.search(barcode).expect("scan");
    assert!(!found.is_empty());
    assert!(start.elapsed() < std::time::Duration::from_millis(10));

    // 2. Add 2 packs (10 base units) @ ₹45 = ₹90
    let sale_payload = CreateSalePayload {
        charges: vec![],
        transaction_date: "2026-11-20".to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        payment_status: PaymentStatus::Paid, paid_amount: 90.0,
        notes: None,
        items: vec![CreateSaleItemPayload {
            item_id: 1,
            bundle_id: Some(1),
            bundle_quantity: 2.0,
            unit_price: 45.0,
        }],
    };
    let summary = harness.sale_service.create_sale(sale_payload).expect("sale");
    assert_eq!(summary.total_amount, 90.0);
    assert_eq!(summary.payment_status, PaymentStatus::Paid);

    let mut check_tx = harness.uow.begin_transaction().unwrap();
    let updated = check_tx.items().find_by_id(1).unwrap().unwrap();
    assert_eq!(updated.current_stock, 90.0, "100 - 10 = 90");
}

/// Combo 8: Low Stock Alert -> Reorder via Inward Purchase -> Alert Cleared
#[test]
fn test_tc_t3_cmb_008_low_stock_alert_reorder_cleared() {
    let harness = create_combo_harness();
    {
        let mut tx = harness.uow.begin_transaction().unwrap();
        let mut item = create_test_item(1, "Low Stock Marker", None, None, 3.0, 25.0);
        item.is_low_stock = true;
        tx.items().create(&item).unwrap();
        tx.commit().unwrap();
    }

    // 1. Verify alert appears in low stock list
    let alerts = harness.report_service.get_low_stock_alerts().expect("alerts");
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].name, "Low Stock Marker");

    // 2. Inward purchase of 25 units
    let pur_payload = CreatePurchasePayload {
        charges: vec![],
        transaction_date: "2026-11-20".to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        supplier_invoice_ref: None, payment_status: PaymentStatus::Paid, paid_amount: 375.0,
        notes: Some("Reorder low stock".to_string()),
        items: vec![CreatePurchaseItemPayload {
            item_id: 1,
            bundle_id: None,
            bundle_quantity: 25.0,
            unit_cost: 15.0,
        }],
    };
    harness.purchase_service.create_purchase(pur_payload).expect("reorder inward");

    // 3. Verify alert is cleared
    let alerts_after = harness.report_service.get_low_stock_alerts().expect("alerts after");
    assert!(alerts_after.is_empty(), "Replenished stock must clear low stock alert");

    let mut check_tx = harness.uow.begin_transaction().unwrap();
    let item = check_tx.items().find_by_id(1).unwrap().unwrap();
    assert_eq!(item.current_stock, 28.0, "3 + 25 = 28");
    assert_eq!(item.is_low_stock, false);
}

/// Combo 9: Shop Settings Profile -> Sale Billing -> Summary Computation
#[test]
fn test_tc_t3_cmb_009_shop_settings_profile_and_sale_summary() {
    let harness = create_combo_harness();
    let setting = ShopSetting {
        id: 1,
        shop_name: "Modern Book Mart".to_string(),
        address: Some("Shop 12, Market Square".to_string()),
        phone: Some("011-99887766".to_string()),
        tax_id: Some("07ABCDE1234F1Z5".to_string()),
    };
    {
        let mut tx = harness.uow.begin_transaction().unwrap();
        tx.settings().upsert_shop_setting(&setting).unwrap();
        let item = create_test_item(1, "Notebook Softcover", None, None, 20.0, 40.0);
        tx.items().create(&item).unwrap();
        let pur = create_test_purchase_tx(1, "PUR-C9", None, 400.0, 400.0, "2026-11-20");
        tx.purchases().create_transaction(&pur).unwrap();
        let lot = create_test_purchase_lot(1, 1, 1, None, 20.0, 20.0, 20.0, 20.0);
        tx.purchases().create_purchase_item(&lot).unwrap();
        tx.commit().unwrap();
    }

    let sale_payload = CreateSalePayload {
        charges: vec![],
        transaction_date: "2026-11-20".to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        payment_status: PaymentStatus::Paid, paid_amount: 80.0,
        notes: None,
        items: vec![CreateSaleItemPayload {
            item_id: 1,
            bundle_id: None,
            bundle_quantity: 2.0,
            unit_price: 40.0,
        }],
    };
    let summary = harness.sale_service.create_sale(sale_payload).expect("sale");
    assert_eq!(summary.total_amount, 80.0);

    let mut check_tx = harness.uow.begin_transaction().unwrap();
    let shop = check_tx.settings().get_shop_setting().unwrap();
    assert_eq!(shop.shop_name, "Modern Book Mart");
    assert_eq!(shop.tax_id.as_deref(), Some("07ABCDE1234F1Z5"));
}

/// Combo 10: Item Master Price Update -> Historical Transaction Price Immutability Check
#[test]
fn test_tc_t3_cmb_010_master_price_update_historical_immutability() {
    let harness = create_combo_harness();
    let item = create_test_item(1, "Classmate Spiral Note", None, None, 50.0, 60.0);
    {
        let mut tx = harness.uow.begin_transaction().unwrap();
        tx.items().create(&item).unwrap();
        let pur = create_test_purchase_tx(1, "PUR-C10", None, 1500.0, 1500.0, "2026-11-20");
        tx.purchases().create_transaction(&pur).unwrap();
        let lot = create_test_purchase_lot(1, 1, 1, None, 50.0, 50.0, 30.0, 50.0);
        tx.purchases().create_purchase_item(&lot).unwrap();
        tx.commit().unwrap();
    }

    // Sale 1 executed at initial price ₹60.0
    let sale1 = CreateSalePayload {
        charges: vec![],
        transaction_date: "2026-11-20".to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        payment_status: PaymentStatus::Paid, paid_amount: 120.0,
        notes: None,
        items: vec![CreateSaleItemPayload {
            item_id: 1,
            bundle_id: None,
            bundle_quantity: 2.0,
            unit_price: 60.0,
        }],
    };
    let s1_summary = harness.sale_service.create_sale(sale1).expect("sale 1");
    assert_eq!(s1_summary.total_amount, 120.0);

    // Update Master Item Price: ₹60.0 -> ₹75.0
    {
        let mut tx = harness.uow.begin_transaction().unwrap();
        let mut updated_item = tx.items().find_by_id(1).unwrap().unwrap();
        updated_item.default_sale_price = 75.0;
        tx.items().update(&updated_item).unwrap();
        tx.commit().unwrap();
    }

    // Historical Audit: Sale 1 total must remain strictly ₹120.0 (historical immutability)
    let s1_details = harness.sale_service.get_sale_details(1).expect("s1 details");
    assert_eq!(s1_details.sale.total_amount, 120.0, "Historical invoice total must NOT change");
    assert_eq!(s1_details.items[0].sale_item.unit_price, 60.0, "Historical line item price must NOT change");

    // Sale 2 executed at new updated price ₹75.0
    let sale2 = CreateSalePayload {
        charges: vec![],
        transaction_date: "2026-11-20".to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        payment_status: PaymentStatus::Paid, paid_amount: 150.0,
        notes: None,
        items: vec![CreateSaleItemPayload {
            item_id: 1,
            bundle_id: None,
            bundle_quantity: 2.0,
            unit_price: 75.0,
        }],
    };
    let s2_summary = harness.sale_service.create_sale(sale2).expect("sale 2");
    assert_eq!(s2_summary.total_amount, 150.0, "New sale must adopt updated price 2 * 75 = 150");
}
