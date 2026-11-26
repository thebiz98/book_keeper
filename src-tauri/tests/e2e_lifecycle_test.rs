// tests/e2e_lifecycle_test.rs
//! End-to-End (E2E) Integration Lifecycle Scenario
//! Derived strictly from book_keeper_LLD.md § 11.11, § 3.10 of Spec Miner report, and ORIGINAL_REQUEST.md

mod common;

use common::fixtures::*;
use common::*;
use book_keeper::core::dtos::*;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::report_service::ReportService;
use book_keeper::services::contracts::return_service::ReturnService;
use book_keeper::services::contracts::sale_service::SaleService;

#[test]
fn test_e2e_5_step_lifecycle_scenario() {
    println!("=== STARTING 5-STEP BACKEND E2E LIFECYCLE TEST ===");
    let uow = TestDb::new();
    let inventory_service = InventoryServiceImpl::new(FifoStrategy, uow.clone());
    let payment_service = PaymentServiceImpl;
    let purchase_service = PurchaseServiceImpl::new(inventory_service.clone(), payment_service, uow.clone());
    let sale_service = SaleServiceImpl::new(inventory_service.clone(), payment_service, uow.clone());
    let accounting_service = AccountingServiceImpl::new(payment_service, uow.clone());
    let return_service = ReturnServiceImpl::new(inventory_service.clone(), payment_service, accounting_service, uow.clone());
    let report_service = ReportServiceImpl::with_fixed_today(uow.clone(), "2026-11-20");

    // Initial Setup: Register the item "Pencil"
    {
        let mut tx = uow.begin_transaction().expect("begin tx");
        let item = create_test_item(1, "Apsara Platinum Pencil", Some("Stationery"), Some("8901030381001"), 0.0, 10.0);
        tx.items().create(&item).expect("create item");
        tx.commit().expect("commit item");
    }

    // STEP 1: Inward Purchase 1 (Lot #1) - 100 Pencils @ ₹5/pc (Paid = ₹500)
    println!("--- Step 1: Purchase 100 Pencils @ ₹5/pc ---");
    let pur1_payload = CreatePurchasePayload {
        confirm_move_settlements: false,
        charges: vec![],
        transaction_date: "2026-11-20".to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        supplier_invoice_ref: None, payment_status: PaymentStatus::Paid, paid_amount: 500.0,
        notes: Some("Lot #1 Inward".to_string()),
        items: vec![CreatePurchaseItemPayload { purchase_item_id: None,
            item_id: 1,
            bundle_id: None,
            bundle_quantity: 100.0,
            unit_cost: 5.0,
        }],
    };
    let pur1_id = purchase_service.create_purchase(pur1_payload).expect("purchase 1 succeeds").purchase_id;
    assert!(pur1_id > 0);

    {
        let mut tx = uow.begin_transaction().expect("tx");
        let pur1 = tx.purchases().find_transaction_by_id(pur1_id).unwrap().unwrap();
        assert_eq!(pur1.transaction_code, "PUR-1001");
        assert_eq!(pur1.payment_status, PaymentStatus::Paid);
        assert_eq!(pur1.balance_due, 0.0);

        let lots = tx.purchases().get_active_lots_for_item(1).unwrap();
        assert_eq!(lots.len(), 1, "Exactly 1 lot should exist");
        assert_eq!(lots[0].id, 1, "Lot #1 ID");
        assert_eq!(lots[0].remaining_quantity, 100.0);
        assert_eq!(lots[0].unit_cost, 5.0);

        let item = tx.items().find_by_id(1).unwrap().unwrap();
        assert_eq!(item.current_stock, 100.0);
        assert_eq!(item.is_low_stock, false);
    }

    // STEP 2: Inward Purchase 2 (Lot #2) - 50 Pencils @ ₹6/pc (Paid = ₹300)
    println!("--- Step 2: Purchase 50 Pencils @ ₹6/pc ---");
    let pur2_payload = CreatePurchasePayload {
        confirm_move_settlements: false,
        charges: vec![],
        transaction_date: "2026-11-20".to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        supplier_invoice_ref: None, payment_status: PaymentStatus::Paid, paid_amount: 300.0,
        notes: Some("Lot #2 Inward".to_string()),
        items: vec![CreatePurchaseItemPayload { purchase_item_id: None,
            item_id: 1,
            bundle_id: None,
            bundle_quantity: 50.0,
            unit_cost: 6.0,
        }],
    };
    let pur2_id = purchase_service.create_purchase(pur2_payload).expect("purchase 2 succeeds").purchase_id;
    assert!(pur2_id > 0);

    {
        let mut tx = uow.begin_transaction().expect("tx");
        let pur2 = tx.purchases().find_transaction_by_id(pur2_id).unwrap().unwrap();
        assert_eq!(pur2.transaction_code, "PUR-1002");

        let lots = tx.purchases().get_active_lots_for_item(1).unwrap();
        assert_eq!(lots.len(), 2, "Exactly 2 lots must now exist");
        assert_eq!(lots[1].id, 2, "Lot #2 ID");
        assert_eq!(lots[1].remaining_quantity, 50.0);
        assert_eq!(lots[1].unit_cost, 6.0);

        let item = tx.items().find_by_id(1).unwrap().unwrap();
        assert_eq!(item.current_stock, 150.0, "Total stock is 100 + 50 = 150");
        assert_eq!(item.is_low_stock, false);
    }

    // STEP 3: Counter Sale - 120 Pencils @ ₹10/pc (Paid = ₹1,200)
    println!("--- Step 3: Sale 120 Pencils @ ₹10/pc ---");
    let sale_payload = CreateSalePayload {
        confirm_move_settlements: false,
        charges: vec![],
        transaction_date: "2026-11-20".to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        payment_status: PaymentStatus::Paid, paid_amount: 1200.0,
        notes: Some("Retail Counter Sale".to_string()),
        items: vec![CreateSaleItemPayload { sale_item_id: None,
            item_id: 1,
            bundle_id: None,
            bundle_quantity: 120.0,
            unit_price: 10.0,
        }],
    };
    let sale_summary = sale_service.create_sale(sale_payload).expect("sale succeeds");

    assert_eq!(sale_summary.total_amount, 1200.0);
    assert_eq!(sale_summary.paid_amount, 1200.0);
    assert_eq!(sale_summary.balance_due, 0.0);

    {
        let mut tx = uow.begin_transaction().expect("tx");
        let allocs = tx.allocations().find_by_sale_item_id(1).unwrap();
        assert_eq!(allocs.len(), 2, "SALE_STOCK_ALLOCATION must have exactly 2 records");

        assert_eq!(allocs[0].purchase_item_id, 1);
        assert_eq!(allocs[0].quantity_depleted, 100.0);
        assert_eq!(allocs[0].unit_cost, 5.0);

        assert_eq!(allocs[1].purchase_item_id, 2);
        assert_eq!(allocs[1].quantity_depleted, 20.0);
        assert_eq!(allocs[1].unit_cost, 6.0);

        let item = tx.items().find_by_id(1).unwrap().unwrap();
        assert_eq!(item.current_stock, 30.0, "Current stock must be 150 - 120 = 30");

        let lots = tx.purchases().get_active_lots_for_item(1).unwrap();
        assert_eq!(lots.len(), 1, "Only Lot #2 has active stock");
        assert_eq!(lots[0].id, 2);
        assert_eq!(lots[0].remaining_quantity, 30.0, "Lot #2 remaining quantity: 50 - 20 = 30");

        let cogs = (100.0 * 5.0) + (20.0 * 6.0);
        assert_eq!(cogs, 620.0);
        let gross_profit = 1200.0 - cogs;
        assert_eq!(gross_profit, 580.0, "Gross profit: 1200 - 620 = ₹580");
    }

    // STEP 4: Customer Returns 10 Pencils (Restockable + Cash Refund)
    println!("--- Step 4: Customer Return 10 Pencils (Restockable) ---");
    let return_payload = CustomerReturnPayload {
        return_date: "2026-11-20".to_string(),
        party_id: None,
        original_sale_id: Some(1),
        settlement_type: SettlementType::Refund,
        reason: Some("Customer overbought".to_string()),
        notes: None,
        replacement_items: vec![],
        items: vec![CustomerReturnItemPayload {
            item_id: 1,
            sale_item_id: Some(1),
            quantity: 10.0,
            unit_price: 10.0,
            condition: StockCondition::Restockable,
            unit_cost_override: None,
        }],
    };
    let ret = return_service.create_customer_return(return_payload).expect("return succeeds");
    assert_eq!(ret.return_code, "RET-1001");
    assert_eq!((ret.credited_amount, ret.cash_refunded), (0.0, 100.0), "fully paid bill: all cash");

    {
        let mut tx = uow.begin_transaction().expect("tx");

        let lots = tx.purchases().get_active_lots_for_item(1).unwrap();
        let lot2 = lots.iter().find(|l| l.id == 2).unwrap();
        assert_eq!(lot2.remaining_quantity, 40.0, "Lot #2 remaining increments from 30 to 40");

        let all_pur_items = tx.purchases().list_items_by_transaction_id(1).unwrap();
        let lot1 = all_pur_items.iter().find(|l| l.id == 1).unwrap();
        assert_eq!(lot1.remaining_quantity, 0.0, "Lot #1 must remain fully depleted (0.0)");

        let item = tx.items().find_by_id(1).unwrap().unwrap();
        assert_eq!(item.current_stock, 40.0, "Stock increments from 30 to 40");

        let refunds: Vec<_> = tx.settlements().list_by_transaction(TransactionType::Sale, 1).unwrap()
            .into_iter().filter(|s| s.kind == PaymentKind::RefundOut).collect();
        assert_eq!(refunds.len(), 1);
        assert_eq!(refunds[0].kind, PaymentKind::RefundOut);
        assert_eq!(refunds[0].amount_paid, 100.0, "REFUND_OUT row logged with a positive ₹100");
    }

    // STEP 5: Valuation & Telemetry Audit
    println!("--- Step 5: Verify Valuation & Telemetry Audit ---");
    let valuation_report = report_service.get_stock_valuation_report().expect("valuation report");
    assert_eq!(valuation_report.total_valuation, 240.0, "Total stock valuation: 40 @ ₹6 = ₹240");

    let metrics = report_service.get_dashboard_metrics().expect("dashboard metrics");
    assert_eq!(metrics.total_stock_valuation, 240.0);

    assert_eq!(metrics.cash_in_hand, 300.0, "ledger drawer: 1200 - 800 - 100");
    let pnl = report_service.get_pnl_report("2026-11-20", "2026-11-20").expect("pnl");
    assert_eq!(
        (pnl.gross_sales, pnl.sales_returns, pnl.net_sales, pnl.cogs, pnl.gross_profit, pnl.inventory_losses, pnl.net_profit),
        (1200.0, 100.0, 1100.0, 560.0, 540.0, 0.0, 540.0)
    );

    let sales_cash = 1200.0;
    let purchases_cash = 500.0 + 300.0;
    let refund_cash = 100.0;
    let expected_drawer = sales_cash - purchases_cash - refund_cash;
    assert_eq!(expected_drawer, 300.0, "Cash drawer must reconcile to ₹300");

    {
        let mut tx = uow.begin_transaction().expect("tx");
        let adjs = tx.adjustments().list_adjustments(Some(1), 0, 100).unwrap().0;
        assert_eq!(adjs.len(), 0, "Adjustment table must contain 0 records");
    }

    println!("=== 5-STEP BACKEND E2E LIFECYCLE TEST COMPLETED SUCCESSFULLY! ===");
}
