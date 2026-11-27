// tests/report_service_test.rs
//! Test Suite: Financial Reporting & Telemetry Dashboard (F5/T7)
//! Test Cases: TC-REP-001..006, TC-DUES-003, TC-PAY-005 plus trend, low-stock and recent
//! transactions. Derived from book_keeper_LLD.md §6.10 Metric Definitions and §11.
//! All tests pin "today" through `ReportServiceImpl::with_fixed_today` (no real-clock dependence).

mod common;

use std::time::Instant;
use common::fixtures::*;
use common::*;
use book_keeper::core::dtos::*;
use book_keeper::core::entities::*;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::{TransactionContext, UnitOfWork};
use book_keeper::services::contracts::accounting_service::AccountingService;
use book_keeper::services::contracts::inventory_service::InventoryService;
use book_keeper::services::contracts::report_service::ReportService;
use book_keeper::services::contracts::sale_service::SaleService;

const TODAY: &str = "2026-11-20";

fn svc(uow: &TestDb) -> ReportServiceImpl {
    ReportServiceImpl::with_fixed_today(uow.clone(), TODAY)
}

fn ledger(tx: &mut dyn TransactionContext, id: i64, tt: TransactionType, kind: PaymentKind, amount: f64, date: &str) {
    let mut s = create_test_settlement(id, None, "SALE", 1, amount, date);
    s.kind = kind;
    s.transaction_type = tt;
    tx.settlements().create_settlement(&s).expect("ledger row");
}

fn return_tx(id: i64, rt: ReturnType, st: SettlementType, total: f64, date: &str) -> ReturnTransaction {
    ReturnTransaction {
        id,
        return_code: format!("RET-{id}"),
        return_date: date.to_string(),
        return_type: rt,
        party_id: None,
        party_name: None,
        original_transaction_id: None,
        settlement_type: st,
        total_amount: total,
        reason: None,
        notes: None,
        created_at: format!("{date}T11:00:00Z"),
    }
}

fn return_line(id: i64, ret: i64, item: i64, cost: f64, cond: StockCondition) -> ReturnItem {
    ReturnItem {
        id,
        return_transaction_id: ret,
        line_type: ReturnLineType::Returned,
        item_id: item,
        item_name: format!("Item {item}"),
        sale_item_id: Some(1),
        purchase_item_id: None,
        quantity: 1.0,
        unit_price: cost,
        total_amount: cost,
        cost_amount: cost,
        condition: Some(cond),
    }
}

#[test]
fn test_tc_rep_001_consolidated_dashboard_metrics_speed_and_integrity() {
    let uow = TestDb::new();
    let report_service = svc(&uow);

    let mut tx = uow.begin_transaction().expect("begin tx");
    tx.sales().create_transaction(&create_test_sale_tx(1, "SAL-1001", None, 1500.0, 1000.0, TODAY)).unwrap();
    tx.purchases().create_transaction(&create_test_purchase_tx(1, "PUR-1001", None, 800.0, 500.0, TODAY)).unwrap();
    ensure_item(&mut *tx, 1);
    tx.purchases().create_purchase_item(&create_test_purchase_lot(1, 1, 1, None, 100.0, 100.0, 5.0, 60.0)).unwrap();
    tx.sales().create_sale_item(&create_test_sale_item(1, 1, 1, None, 40.0, 40.0, 37.5)).unwrap();
    tx.allocations().create_allocation(&create_test_allocation(1, 1, 1, 40.0, 5.0)).unwrap();
    ledger(&mut *tx, 1, TransactionType::Sale, PaymentKind::Upfront, 1000.0, TODAY);
    ledger(&mut *tx, 2, TransactionType::Purchase, PaymentKind::Upfront, 500.0, TODAY);
    tx.commit().unwrap();

    let start = Instant::now();
    let m = report_service.get_dashboard_metrics().expect("metrics");
    let elapsed = start.elapsed();

    assert_eq!(m.today_sales_amount, 1500.0);
    assert_eq!(m.today_sales_count, 1);
    assert_eq!(m.today_purchase_amount, 800.0);
    assert_eq!(m.today_purchase_count, 1);
    assert_eq!(m.customer_receivables, 500.0);
    assert_eq!(m.vendor_payables, 300.0);
    assert_eq!(m.total_stock_valuation, 300.0);
    assert_eq!(m.cash_in_hand, 500.0); // 1000 in - 500 out
    assert!(elapsed.as_millis() < 50, "dashboard too slow: {elapsed:?}");
}

#[test]
fn test_tc_rep_002_periodic_net_profit_calculation() {
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().unwrap();
    tx.sales().create_transaction(&create_test_sale_tx(1, "SAL-1001", None, 10000.0, 10000.0, TODAY)).unwrap();
    ensure_lot(&mut *tx, 1, 1);
    tx.sales().create_sale_item(&create_test_sale_item(1, 1, 1, None, 720.0, 720.0, 13.0)).unwrap();
    tx.allocations().create_allocation(&create_test_allocation(1, 1, 1, 720.0, 10.0)).unwrap();
    tx.commit().unwrap();

    let pnl = svc(&uow).get_pnl_report("2026-11-01", "2026-11-30").unwrap();
    assert_eq!(pnl.gross_sales, 10000.0);
    assert_eq!(pnl.cogs, 7200.0);
    assert_eq!(pnl.net_profit, 2800.0);
}

#[test]
fn test_tc_rep_003_stock_valuation_accuracy_and_categories() {
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().unwrap();
    ensure_purchase(&mut *tx, 1);
    tx.items().create(&create_test_item(1, "Pen", Some("Stationery"), None, 10.0, 25.0)).unwrap();
    tx.items().create(&create_test_item(2, "Loose", None, None, 5.0, 12.0)).unwrap();
    tx.purchases().create_purchase_item(&create_test_purchase_lot(1, 1, 1, None, 10.0, 10.0, 20.0, 10.0)).unwrap();
    tx.purchases().create_purchase_item(&create_test_purchase_lot(2, 1, 2, None, 5.0, 5.0, 10.0, 5.0)).unwrap();
    // Depleted lot is not "active".
    tx.purchases().create_purchase_item(&create_test_purchase_lot(3, 1, 2, None, 5.0, 5.0, 10.0, 0.0)).unwrap();
    tx.commit().unwrap();

    let v = svc(&uow).get_stock_valuation_report().unwrap();
    assert_eq!(v.total_valuation, 250.0);
    assert_eq!(v.active_lots_count, 2);
    assert_eq!(
        v.categories_breakdown,
        vec![
            CategoryValuation { category: "Stationery".into(), items_count: 1, valuation: 200.0 },
            CategoryValuation { category: "Uncategorized".into(), items_count: 1, valuation: 50.0 },
        ]
    );
}

#[test]
fn test_tc_rep_004_full_pnl_statement() {
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().unwrap();
    ensure_lot(&mut *tx, 1, 1);
    ensure_lot(&mut *tx, 2, 2);
    // Sales 1,000 with FIFO cost 600: 90 @ 6 (540) + 6 @ 10 (60).
    tx.sales().create_transaction(&create_test_sale_tx(1, "SAL-1001", None, 1000.0, 1000.0, TODAY)).unwrap();
    tx.sales().create_sale_item(&create_test_sale_item(1, 1, 1, None, 96.0, 96.0, 10.0)).unwrap();
    let mut a = create_test_allocation(1, 1, 1, 90.0, 6.0);
    a.returned_quantity = 15.0; // 10 restockable refund (60) + 5 scrap refund (30)
    tx.allocations().create_allocation(&a).unwrap();
    let mut b = create_test_allocation(2, 1, 2, 6.0, 10.0);
    b.returned_quantity = 1.0; // replacement return of 1 restockable unit (cost 10)
    tx.allocations().create_allocation(&b).unwrap();
    // R1 restockable REFUND value 100 (cost 60); R2 scrap REFUND value 50 (cost 30);
    // R3 REPLACEMENT returning 1 restockable unit (cost 10).
    tx.returns().create_return_transaction(&return_tx(1, ReturnType::Customer, SettlementType::Refund, 100.0, TODAY)).unwrap();
    tx.returns().create_return_item(&return_line(1, 1, 1, 60.0, StockCondition::Restockable)).unwrap();
    tx.returns().create_return_transaction(&return_tx(2, ReturnType::Customer, SettlementType::Refund, 50.0, TODAY)).unwrap();
    tx.returns().create_return_item(&return_line(2, 2, 1, 30.0, StockCondition::DamagedScrap)).unwrap();
    tx.returns().create_return_transaction(&return_tx(3, ReturnType::Customer, SettlementType::Replacement, 10.0, TODAY)).unwrap();
    tx.returns().create_return_item(&return_line(3, 3, 2, 10.0, StockCondition::Restockable)).unwrap();
    // Replacement issues 1 unit of another item (cost 12).
    tx.outflow_allocations().create_allocation(&StockOutflowAllocation {
        id: 1, source_type: OutflowSourceType::Replacement, source_id: 3, purchase_item_id: 2,
        quantity_depleted: 1.0, unit_cost: 12.0,
    }).unwrap();
    // Write-off cost 20.
    tx.adjustments().create_adjustment(&create_test_adjustment(1, 1, -2.0, AdjustmentReason::Damaged, TODAY)).unwrap();
    tx.outflow_allocations().create_allocation(&StockOutflowAllocation {
        id: 2, source_type: OutflowSourceType::Adjustment, source_id: 1, purchase_item_id: 1,
        quantity_depleted: 2.0, unit_cost: 10.0,
    }).unwrap();
    // Purchases: 700 PURCHASE (counted), 999 ADJUSTMENT source (not), vendor return 200.
    tx.purchases().create_transaction(&create_test_purchase_tx(1, "PUR-1", None, 700.0, 700.0, TODAY)).unwrap();
    let mut adj_src = create_test_purchase_tx(2, "ADJ-1", None, 999.0, 999.0, TODAY);
    adj_src.source = LotSource::Adjustment;
    tx.purchases().create_transaction(&adj_src).unwrap();
    tx.returns().create_return_transaction(&return_tx(4, ReturnType::Vendor, SettlementType::Refund, 200.0, TODAY)).unwrap();
    // DD-87: the vendor-return line is the gross cost that left a PURCHASE lot (outflow rows).
    tx.outflow_allocations().create_allocation(&StockOutflowAllocation {
        id: 3, source_type: OutflowSourceType::VendorReturn, source_id: 4, purchase_item_id: 1,
        quantity_depleted: 20.0, unit_cost: 10.0,
    }).unwrap();
    // Out-of-range activity must not leak in.
    tx.sales().create_transaction(&create_test_sale_tx(2, "SAL-1002", None, 5000.0, 5000.0, "2026-12-01")).unwrap();
    tx.commit().unwrap();

    let p = svc(&uow).get_pnl_report(TODAY, TODAY).unwrap();
    assert_eq!(p.gross_sales, 1000.0);
    assert_eq!(p.sales_returns, 150.0);
    assert_eq!(p.net_sales, 850.0);
    // DD-91: gross 600 + replacement 12; the 100 returned cost (60 + 30 + 10) is credited apart.
    assert_eq!(p.cogs, 612.0);
    assert_eq!(p.returned_goods_cost, 100.0);
    assert_eq!(p.gross_profit, 338.0);
    assert_eq!(p.inventory_losses, 50.0);
    assert_eq!(p.net_profit, 288.0);
    assert_eq!(p.total_purchases, 500.0);
    assert_eq!((p.operating_expenses, p.sales_discounts, p.other_income), (0.0, 0.0, 0.0));
    assert!(p.expenses_by_type.is_empty());
    // Invalid ranges are rejected.
    assert!(svc(&uow).get_pnl_report("2026-11-30", "2026-11-01").is_err());
}

#[test]
fn test_tc_rep_005_cash_flow_buckets_from_ledger() {
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().unwrap();
    use PaymentKind::*;
    use TransactionType::*;
    ledger(&mut *tx, 1, Sale, Upfront, 500.0, "2026-11-10");
    ledger(&mut *tx, 2, Sale, Settlement, 200.0, "2026-11-12");
    ledger(&mut *tx, 3, Sale, RefundOut, 50.0, "2026-11-13");
    ledger(&mut *tx, 4, Purchase, Upfront, 300.0, "2026-11-10");
    ledger(&mut *tx, 5, Purchase, Settlement, 100.0, "2026-11-14");
    ledger(&mut *tx, 6, Purchase, RefundIn, 40.0, "2026-11-15");
    ledger(&mut *tx, 7, Sale, ReturnCredit, 70.0, "2026-11-13");
    ledger(&mut *tx, 8, Sale, Upfront, 9999.0, "2026-12-01"); // outside the range
    tx.commit().unwrap();

    // "Today" (2026-11-20) has no rows: the range, not the clock, drives the report.
    let c = svc(&uow).get_cash_flow_report("2026-11-01", "2026-11-30").unwrap();
    assert_eq!(c.sales_cash_inflow, 500.0);
    assert_eq!(c.customer_settlements_inflow, 200.0);
    assert_eq!(c.vendor_refunds_inflow, 40.0);
    assert_eq!(c.purchase_cash_outflow, 300.0);
    assert_eq!(c.vendor_settlements_outflow, 100.0);
    assert_eq!(c.refunds_cash_outflow, 50.0);
    assert_eq!(c.net_cash_balance, 290.0); // in 740 - out 450, RETURN_CREDIT excluded
    // DD-56: the totals are the backend's, and agree with the net.
    assert_eq!(c.total_inflow, 740.0);
    assert_eq!(c.total_outflow, 450.0);
    assert_eq!(c.total_inflow - c.total_outflow, c.net_cash_balance);
}

#[test]
fn test_tc_rep_006_non_purchase_lots_excluded_from_purchases_and_payables() {
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().unwrap();
    ensure_item(&mut *tx, 1);
    for (id, code, src) in [(1, "PUR-1", LotSource::Purchase), (2, "ADJ-1", LotSource::Adjustment), (3, "OPN-1", LotSource::Opening)] {
        let mut p = create_test_purchase_tx(id, code, None, 100.0, 0.0, TODAY);
        p.source = src;
        tx.purchases().create_transaction(&p).unwrap();
        tx.purchases().create_purchase_item(&create_test_purchase_lot(id, id, 1, None, 10.0, 10.0, 10.0, 10.0)).unwrap();
    }
    tx.commit().unwrap();

    let m = svc(&uow).get_dashboard_metrics().unwrap();
    assert_eq!(m.today_purchase_count, 1);
    assert_eq!(m.today_purchase_amount, 100.0);
    assert_eq!(m.vendor_payables, 100.0);
    assert_eq!(m.total_stock_valuation, 300.0, "valuation includes ADJ and OPN lots");
    assert_eq!(svc(&uow).get_pnl_report(TODAY, TODAY).unwrap().total_purchases, 100.0);
}

fn sale_harness() -> (TestDb, SaleServiceImpl<InventoryServiceImpl<FifoStrategy>, PaymentServiceImpl>, AccountingServiceImpl<PaymentServiceImpl>) {
    let uow = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, uow.clone());
    let sale = SaleServiceImpl::new(inv, PaymentServiceImpl, uow.clone());
    let acc = AccountingServiceImpl::new(PaymentServiceImpl, uow.clone());
    let mut tx = uow.begin_transaction().unwrap();
    tx.parties().create(&create_test_party(1, "Cust", PartyType::Customer, None)).unwrap();
    tx.items().create(&create_test_item(1, "Notebook", None, None, 100.0, 100.0)).unwrap();
    tx.purchases().create_transaction(&create_test_purchase_tx(1, "PUR-1", None, 500.0, 500.0, TODAY)).unwrap();
    tx.purchases().create_purchase_item(&create_test_purchase_lot(1, 1, 1, None, 100.0, 100.0, 5.0, 100.0)).unwrap();
    tx.commit().unwrap();
    (uow, sale, acc)
}

fn sale_payload(qty: f64, status: PaymentStatus, paid: f64, party: Option<i64>) -> CreateSalePayload {
    CreateSalePayload {
        confirm_move_settlements: false,
        charges: vec![],
        transaction_date: TODAY.to_string(),
        party_id: party,
        payment_mode: Some("CASH".to_string()),
        payment_status: status,
        paid_amount: paid,
        notes: None,
        items: vec![CreateSaleItemPayload { sale_item_id: None, item_id: 1, bundle_id: None, bundle_quantity: qty, unit_price: 100.0 }],
    }
}

/// Item 1 with one PURCHASE lot of 10 @ 5, plus a sale service; returns inventory service too.
fn unrec_harness() -> (TestDb, InventoryServiceImpl<FifoStrategy>, SaleServiceImpl<InventoryServiceImpl<FifoStrategy>, PaymentServiceImpl>) {
    let uow = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, uow.clone());
    let sale = SaleServiceImpl::new(inv.clone(), PaymentServiceImpl, uow.clone());
    let mut tx = uow.begin_transaction().unwrap();
    tx.items().create(&create_test_item(1, "Notebook", Some("Paper"), None, 10.0, 100.0)).unwrap();
    tx.purchases().create_transaction(&create_test_purchase_tx(1, "PUR-1001", None, 50.0, 50.0, TODAY)).unwrap();
    tx.purchases().create_purchase_item(&create_test_purchase_lot(1, 1, 1, None, 10.0, 10.0, 5.0, 10.0)).unwrap();
    tx.commit().unwrap();
    (uow, inv, sale)
}

fn set_stock(item: i64, counted: f64, cost: Option<f64>) -> SetItemStockPayload {
    SetItemStockPayload { item_id: item, counted_quantity: counted, unit_cost: cost, adjustment_date: TODAY.into(), notes: None }
}

#[test]
fn tc_rep_unrec_1_unrecorded_stock_adds_valuation_only() {
    let (uow, inv, _sale) = unrec_harness();
    let rs = svc(&uow);
    let val0 = rs.get_stock_valuation_report().unwrap();
    let pnl0 = rs.get_pnl_report(TODAY, TODAY).unwrap();
    let cash0 = rs.get_cash_flow_report(TODAY, TODAY).unwrap();
    let dash0 = rs.get_dashboard_metrics().unwrap();
    let recent0 = rs.get_recent_transactions(20).unwrap();

    let res = inv.set_item_stock(set_stock(1, 30.0, Some(3.0))).unwrap();
    assert_eq!(res.value_change, 60.0);

    let val1 = rs.get_stock_valuation_report().unwrap();
    assert_eq!(val1.total_valuation, val0.total_valuation + 60.0);
    assert_eq!(val1.active_lots_count, val0.active_lots_count + 1);
    let cat = |v: &StockValuationReport| v.categories_breakdown.iter().map(|c| c.valuation).sum::<f64>();
    assert_eq!(cat(&val1), cat(&val0) + 60.0);

    assert_eq!(rs.get_pnl_report(TODAY, TODAY).unwrap(), pnl0, "P&L untouched");
    assert_eq!(rs.get_cash_flow_report(TODAY, TODAY).unwrap(), cash0, "cash flow untouched");
    let mut dash1 = rs.get_dashboard_metrics().unwrap();
    assert_eq!(dash1.total_stock_valuation, dash0.total_stock_valuation + 60.0);
    dash1.total_stock_valuation = dash0.total_stock_valuation;
    assert_eq!(dash1, dash0, "dashboard purchases/payables/profit untouched");

    let recent1 = rs.get_recent_transactions(20).unwrap();
    assert_eq!(recent1, recent0);
    assert!(recent1.iter().all(|t| !t.code.starts_with("ADJ")));
    let mut r = uow.begin_read().unwrap();
    let hist = r.purchases().list_purchases(None, None, None, None, 0, 50).unwrap();
    assert_eq!(hist.len(), 1);
    assert!(hist[0].transaction_code.starts_with("PUR"));
}

#[test]
fn tc_rep_unrec_2_later_sale_and_writeoff_use_the_entered_cost() {
    let (uow, inv, sale) = unrec_harness();
    inv.set_item_stock(set_stock(1, 45.0, Some(3.0))).unwrap(); // 10 @5 + 35 @3
    sale.create_sale(CreateSalePayload {
        confirm_move_settlements: false,
        charges: vec![],
        transaction_date: TODAY.to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        payment_status: PaymentStatus::Paid,
        paid_amount: 3500.0,
        notes: None,
        items: vec![CreateSaleItemPayload { sale_item_id: None, item_id: 1, bundle_id: None, bundle_quantity: 35.0, unit_price: 100.0 }],
    })
    .unwrap();
    let rs = svc(&uow);
    assert_eq!(rs.get_pnl_report(TODAY, TODAY).unwrap().cogs, 125.0, "10 x 5 + 25 x 3");
    // 10 units remain (all from the unrecorded lot @3); count them down to 5.
    let res = inv.set_item_stock(set_stock(1, 5.0, None)).unwrap();
    assert_eq!((res.delta, res.value_change), (-5.0, -15.0));
    assert_eq!(rs.get_pnl_report(TODAY, TODAY).unwrap().inventory_losses, 15.0);
}

#[test]
fn test_tc_pay_005_upfront_counted_once_in_cash_flow() {
    let (uow, sale, _acc) = sale_harness();
    sale.create_sale(sale_payload(1.0, PaymentStatus::Partial, 40.0, Some(1))).unwrap();

    let c = svc(&uow).get_cash_flow_report(TODAY, TODAY).unwrap();
    assert_eq!(c.sales_cash_inflow, 40.0, "upfront counted once, not 80");
    assert_eq!(c.net_cash_balance, 40.0);
}

#[test]
fn test_tc_dues_003_drawer_includes_upfront_and_settled_dues_once() {
    let (uow, sale, acc) = sale_harness();
    sale.create_sale(sale_payload(10.0, PaymentStatus::Paid, 1000.0, None)).unwrap();
    let id = sale.create_sale(sale_payload(4.0, PaymentStatus::Unpaid, 0.0, Some(1))).unwrap().sale_id;
    acc.settle_bill_payment(SettlePaymentPayload {
        party_id: 1,
        transaction_type: TransactionType::Sale,
        amount_paid: 400.0,
        settlement_date: TODAY.to_string(),
        payment_mode: Some("CASH".to_string()),
        transaction_id: Some(id),
        notes: None,
    }).unwrap();

    let m = svc(&uow).get_dashboard_metrics().unwrap();
    assert_eq!(m.cash_in_hand, 1400.0);
    assert_eq!(m.customer_receivables, 0.0);
}

#[test]
fn test_trend_zero_fill_and_buckets() {
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().unwrap();
    tx.sales().create_transaction(&create_test_sale_tx(1, "S1", None, 100.0, 100.0, "2026-11-20")).unwrap();
    tx.sales().create_transaction(&create_test_sale_tx(2, "S2", None, 50.0, 50.0, "2026-11-20")).unwrap();
    tx.sales().create_transaction(&create_test_sale_tx(3, "S3", None, 30.0, 30.0, "2026-11-15")).unwrap();
    tx.sales().create_transaction(&create_test_sale_tx(4, "S4", None, 70.0, 70.0, "2026-05-02")).unwrap();
    tx.purchases().create_transaction(&create_test_purchase_tx(1, "P1", None, 80.0, 80.0, "2026-11-20")).unwrap();
    let mut adj = create_test_purchase_tx(2, "A1", None, 999.0, 999.0, "2026-11-20");
    adj.source = LotSource::Adjustment;
    tx.purchases().create_purchase_item(&create_test_purchase_lot(1, 1, 1, None, 1.0, 1.0, 1.0, 1.0)).ok();
    tx.purchases().create_transaction(&adj).unwrap();
    tx.commit().unwrap();
    let r = svc(&uow);

    let d7 = r.get_sales_purchase_trend("7d").unwrap();
    assert_eq!(d7.len(), 7);
    assert_eq!(d7.first().unwrap().date, "2026-11-14");
    assert_eq!(d7.last().unwrap().date, "2026-11-20");
    assert_eq!((d7[6].sales_amount, d7[6].purchase_amount), (150.0, 80.0));
    assert_eq!(d7[1].sales_amount, 30.0); // 2026-11-15
    assert_eq!(d7[0].sales_amount, 0.0);

    let d30 = r.get_sales_purchase_trend("30d").unwrap();
    assert_eq!(d30.len(), 30);
    assert_eq!(d30[0].date, "2026-10-22");

    let y = r.get_sales_purchase_trend("1y").unwrap();
    assert_eq!(y.len(), 12);
    assert_eq!(y[0].date, "2025-12-01");
    assert_eq!(y[11].date, "2026-11-01");
    assert_eq!((y[11].sales_amount, y[11].purchase_amount), (180.0, 80.0));
    assert_eq!(y[5].date, "2026-05-01");
    assert_eq!(y[5].sales_amount, 70.0);
    assert_eq!(y[10].sales_amount, 0.0);

    assert!(r.get_sales_purchase_trend("90d").is_err());
}

#[test]
fn test_low_stock_ordering_and_no_cap() {
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().unwrap();
    for i in 1..=60i64 {
        // stock 0.0 .. 4.9, descending with id so ordering must come from the query
        let stock = (60 - i) as f64 / 12.5;
        tx.items().create(&create_test_item(i, &format!("Item {i:02}"), None, None, stock, 10.0)).unwrap();
    }
    tx.items().create(&create_test_item(100, "Plenty", None, None, 50.0, 10.0)).unwrap();
    tx.commit().unwrap();

    let alerts = svc(&uow).get_low_stock_alerts().unwrap();
    assert_eq!(alerts.len(), 60, "no 50-item cap; non-low items excluded");
    assert!(alerts.windows(2).all(|w| w[0].current_stock <= w[1].current_stock));
    assert_eq!(alerts[0].name, "Item 60");
}

#[test]
fn low_stock_alerts_use_each_items_own_level() {
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().unwrap();
    let mut a = create_test_item(1, "Above5", None, None, 15.0, 10.0);
    a.low_stock_threshold = 20.0;
    a.is_low_stock = true;
    let mut b = create_test_item(2, "Never", None, None, 3.0, 10.0);
    b.low_stock_threshold = 0.0;
    tx.items().create(&a).unwrap();
    tx.items().create(&b).unwrap();
    tx.commit().unwrap();
    let alerts = svc(&uow).get_low_stock_alerts().unwrap();
    assert_eq!(alerts.len(), 1);
    assert_eq!((alerts[0].id, alerts[0].low_stock_threshold), (1, 20.0));
}

#[test]
fn test_recent_transactions_union_newest_first() {
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().unwrap();
    let mut s = create_test_sale_tx(1, "SAL-1", None, 100.0, 40.0, "2026-11-18");
    s.party_name = Some("Asha".into());
    s.created_at = "2026-11-18T09:00:00Z".into();
    tx.sales().create_transaction(&s).unwrap();
    let mut p = create_test_purchase_tx(1, "PUR-1", None, 80.0, 80.0, "2026-11-19");
    p.created_at = "2026-11-19T09:00:00Z".into();
    tx.purchases().create_transaction(&p).unwrap();
    let mut o = create_test_purchase_tx(2, "OPN-1", None, 5.0, 5.0, "2026-11-20");
    o.source = LotSource::Opening;
    tx.purchases().create_transaction(&o).unwrap();
    let mut r = return_tx(1, ReturnType::Customer, SettlementType::Refund, 25.0, "2026-11-20");
    r.created_at = "2026-11-20T09:00:00Z".into();
    tx.returns().create_return_transaction(&r).unwrap();
    tx.commit().unwrap();

    let all = svc(&uow).get_recent_transactions(10).unwrap();
    let kinds: Vec<_> = all.iter().map(|t| t.kind).collect();
    assert_eq!(kinds, vec![RecentTransactionKind::Return, RecentTransactionKind::Purchase, RecentTransactionKind::Sale]);
    assert_eq!(all[0].payment_status, None);
    assert_eq!(all[0].amount, 25.0);
    assert_eq!(all[1].payment_status, Some(PaymentStatus::Paid));
    assert_eq!(all[2].party_name.as_deref(), Some("Asha"));
    assert_eq!(all[2].payment_status, Some(PaymentStatus::Partial));
    assert_eq!(svc(&uow).get_recent_transactions(2).unwrap().len(), 2);
    assert!(svc(&uow).get_recent_transactions(0).is_err());
}

#[test]
fn test_dashboard_today_excludes_prior_days() {
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().unwrap();
    tx.sales().create_transaction(&create_test_sale_tx(1, "SAL-1001", None, 700.0, 700.0, "2026-11-19")).unwrap();
    tx.sales().create_transaction(&create_test_sale_tx(2, "SAL-1002", None, 300.0, 100.0, TODAY)).unwrap();
    tx.commit().unwrap();

    let m = svc(&uow).get_dashboard_metrics().unwrap();
    assert_eq!(m.today_sales_amount, 300.0);
    assert_eq!(m.today_sales_count, 1);
    assert_eq!(m.customer_receivables, 200.0);
}

#[test]
fn tc_exp_011_cash_in_hand_subtracts_expenses_of_every_day() {
    use PaymentKind::*;
    use TransactionType::*;
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().unwrap();
    ledger(&mut *tx, 1, Sale, Upfront, 1000.0, TODAY);
    ledger(&mut *tx, 2, Purchase, Upfront, 300.0, TODAY);
    ledger(&mut *tx, 3, Sale, RefundOut, 50.0, TODAY);
    create_test_expense(&mut *tx, 1001, TODAY, 1, 120.0);
    create_test_expense(&mut *tx, 1002, "2026-11-19", 1, 80.0);
    tx.commit().unwrap();

    let m = svc(&uow).get_dashboard_metrics().unwrap();
    assert_eq!(m.cash_in_hand, 450.0); // 650 - 120 - 80 (yesterday's expense still counts)
    assert_eq!(m.today_expenses_amount, 120.0);
    let acc = AccountingServiceImpl::new(PaymentServiceImpl, uow.clone());
    assert_eq!(acc.calculate_cash_in_hand(TODAY).unwrap(), 450.0);
}

fn set_opening_cash(uow: &TestDb, amount: f64) {
    let mut tx = uow.begin_transaction().unwrap();
    let mut s = tx.settings().get_shop_setting().unwrap();
    s.opening_cash = amount;
    tx.settings().upsert_shop_setting(&s).unwrap();
    tx.commit().unwrap();
}

#[test]
fn cash_in_hand_carries_across_days_and_includes_opening_cash() {
    use PaymentKind::*;
    use TransactionType::*;
    let uow = TestDb::new();
    set_opening_cash(&uow, 200.0);
    let mut tx = uow.begin_transaction().unwrap();
    ledger(&mut *tx, 1, Sale, Upfront, 1000.0, "2026-11-19"); // yesterday
    ledger(&mut *tx, 2, Purchase, Upfront, 300.0, "2026-11-01"); // weeks ago
    create_test_expense(&mut *tx, 1001, TODAY, 1, 120.0);
    tx.commit().unwrap();
    let m = svc(&uow).get_dashboard_metrics().unwrap();
    assert_eq!(m.cash_in_hand, 780.0); // 200 + 1000 - 300 - 120
    assert_eq!(m.today_expenses_amount, 120.0, "expenses tile stays today-only");
}

#[test]
fn cash_in_hand_ignores_future_dated_rows_and_credit_sales() {
    use PaymentKind::*;
    use TransactionType::*;
    let uow = TestDb::new();
    let before = svc(&uow).get_dashboard_metrics().unwrap().cash_in_hand;
    assert_eq!(before, 0.0);
    let mut tx = uow.begin_transaction().unwrap();
    // A credit sale: bill total 900, nothing paid, so no ledger cash row.
    tx.sales().create_transaction(&create_test_sale_tx(1, "SAL-1001", None, 900.0, 0.0, TODAY)).unwrap();
    ledger(&mut *tx, 1, Sale, Upfront, 500.0, "2099-01-01"); // after today
    create_test_expense(&mut *tx, 1001, "2099-01-01", 1, 77.0);
    tx.commit().unwrap();
    assert_eq!(svc(&uow).get_dashboard_metrics().unwrap().cash_in_hand, 0.0);
}

#[test]
fn tc_exp_012_cash_flow_report_has_expenses_outflow() {
    use PaymentKind::*;
    use TransactionType::*;
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().unwrap();
    ledger(&mut *tx, 1, Sale, Upfront, 500.0, "2026-11-10");
    ledger(&mut *tx, 2, Sale, ReturnCredit, 70.0, "2026-11-10");
    create_test_expense(&mut *tx, 1001, "2026-11-10", 1, 120.0);
    create_test_expense(&mut *tx, 1002, "2026-11-11", 1, 80.0);
    create_test_expense(&mut *tx, 1003, "2026-12-01", 1, 999.0); // outside
    tx.commit().unwrap();
    let c = svc(&uow).get_cash_flow_report("2026-11-10", "2026-11-11").unwrap();
    assert_eq!(c.expenses_cash_outflow, 200.0);
    assert_eq!(c.net_cash_balance, 300.0); // 500 - 200, RETURN_CREDIT excluded
}

#[test]
fn tc_exp_013_pnl_operating_expenses_and_breakdown() {
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().unwrap();
    ensure_lot(&mut *tx, 1, 1);
    tx.sales().create_transaction(&create_test_sale_tx(1, "SAL-1001", None, 1000.0, 1000.0, TODAY)).unwrap();
    tx.sales().create_sale_item(&create_test_sale_item(1, 1, 1, None, 100.0, 100.0, 10.0)).unwrap();
    tx.allocations().create_allocation(&create_test_allocation(1, 1, 1, 100.0, 6.0)).unwrap(); // cogs 600
    create_test_expense(&mut *tx, 1001, TODAY, 1, 40.0); // "Type 1"
    create_test_expense(&mut *tx, 1002, TODAY, 2, 60.0); // "Type 2"
    create_test_expense(&mut *tx, 1003, "2026-12-30", 1, 999.0); // outside
    tx.commit().unwrap();
    let p = svc(&uow).get_pnl_report(TODAY, TODAY).unwrap();
    assert_eq!(p.gross_profit, 400.0);
    assert_eq!(p.operating_expenses, 100.0);
    assert_eq!(p.net_profit, 300.0);
    let by: Vec<_> = p.expenses_by_type.iter().map(|t| (t.expense_type_name.as_str(), t.amount)).collect();
    assert_eq!(by, vec![("Type 2", 60.0), ("Type 1", 40.0)]);
}
