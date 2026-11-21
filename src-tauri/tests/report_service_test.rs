// tests/report_service_test.rs
//! Test Suite: Financial Reporting & Telemetry Dashboard
//! Test Cases: TC-REP-001 through TC-REP-003
//! Derived from book_keeper_LLD.md § 11.9 and Spec Miner Report

mod common;

use std::time::Instant;
use common::fixtures::*;
use common::{ReportServiceImpl, TestDb};
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::report_service::ReportService;

#[test]
fn test_tc_rep_001_consolidated_dashboard_metrics_speed_and_integrity() {
    let uow = TestDb::new();
    let report_service = ReportServiceImpl::with_fixed_today(uow.clone(), "2026-11-20");

    let mut tx = uow.begin_transaction().expect("begin tx");
    // Seed some sales and purchases
    let sale = create_test_sale_tx(1, "SAL-1001", None, 1500.0, 1000.0, "2026-11-20");
    tx.sales().create_transaction(&sale).expect("create sale");

    let pur = create_test_purchase_tx(1, "PUR-1001", None, 800.0, 500.0, "2026-11-20");
    tx.purchases().create_transaction(&pur).expect("create pur");

    ensure_item(&mut *tx, 1);
    let lot = create_test_purchase_lot(1, 1, 1, None, 100.0, 100.0, 5.0, 60.0);
    tx.purchases().create_purchase_item(&lot).expect("create lot");

    // Line on today's sale SAL-1001 so its COGS counts toward today's profit.
    tx.sales().create_sale_item(&create_test_sale_item(1, 1, 1, None, 40.0, 40.0, 37.5)).expect("create sale line");
    let alloc = create_test_allocation(1, 1, 1, 40.0, 5.0);
    tx.allocations().create_allocation(&alloc).expect("create alloc");
    tx.commit().expect("commit");

    // Action: Measure latency and fetch dashboard metrics
    let start = Instant::now();
    let metrics = report_service.get_dashboard_metrics().expect("metrics query succeeds");
    let elapsed = start.elapsed();

    // Assert: Single payload with all KPIs
    assert_eq!(metrics.today_sales, 1500.0);
    assert_eq!(metrics.today_purchases, 800.0);
    assert_eq!(metrics.customer_receivables, 500.0); // 1500 - 1000
    assert_eq!(metrics.vendor_payables, 300.0);      // 800 - 500
    assert_eq!(metrics.total_stock_valuation, 300.0); // 60 * 5.0
    assert_eq!(metrics.today_net_profit, 1300.0);    // 1500 - (40 * 5) = 1500 - 200 = 1300
    assert_eq!(metrics.live_cash_in_drawer, 500.0);   // 1000 - 500

    println!("Dashboard query latency: {:?}", elapsed);
    assert!(elapsed.as_millis() < 50, "Dashboard metrics must execute fast (< 2ms in production, < 50ms in harness)");
}

#[test]
fn test_tc_rep_002_periodic_net_profit_calculation() {
    let uow = TestDb::new();
    let report_service = ReportServiceImpl::with_fixed_today(uow.clone(), "2026-11-20");

    let mut tx = uow.begin_transaction().expect("begin tx");
    // Period sales = 10,000 ₹
    let sale = create_test_sale_tx(1, "SAL-1001", None, 10000.0, 10000.0, "2026-11-20");
    tx.sales().create_transaction(&sale).expect("create sale");

    // Depleted allocations COGS = 7,200 ₹
    ensure_lot(&mut *tx, 1, 1);
    seed_sale_line(&mut *tx, 1, 1);
    let alloc = create_test_allocation(1, 1, 1, 720.0, 10.0);
    tx.allocations().create_allocation(&alloc).expect("create alloc");
    tx.commit().expect("commit");

    // Action: get_pnl_report("2026-11-01", "2026-11-30")
    let pnl = report_service.get_pnl_report("2026-11-01", "2026-11-30").expect("pnl report");

    // Assert: gross_sales = 10,000.0, cogs = 7,200.0, net_profit = 2,800.0
    assert_eq!(pnl.gross_sales, 10000.0);
    assert_eq!(pnl.cogs, 7200.0);
    assert_eq!(pnl.net_profit, 2800.0);
}

#[test]
fn test_tc_rep_003_stock_valuation_accuracy() {
    let uow = TestDb::new();
    let report_service = ReportServiceImpl::with_fixed_today(uow.clone(), "2026-11-20");

    let mut tx = uow.begin_transaction().expect("begin tx");
    // Lot A: 10 @ 20 ₹ = 200 ₹
    ensure_purchase(&mut *tx, 1);
    ensure_item(&mut *tx, 1);
    ensure_item(&mut *tx, 2);
    let lot_a = create_test_purchase_lot(1, 1, 1, None, 10.0, 10.0, 20.0, 10.0);
    tx.purchases().create_purchase_item(&lot_a).expect("create lot a");

    // Lot B: 5 @ 10 ₹ = 50 ₹
    let lot_b = create_test_purchase_lot(2, 1, 2, None, 5.0, 5.0, 10.0, 5.0);
    tx.purchases().create_purchase_item(&lot_b).expect("create lot b");
    tx.commit().expect("commit");

    // Action: get_stock_valuation_report()
    let valuation = report_service.get_stock_valuation_report().expect("valuation report");

    // Assert: total_valuation = 250.0 ₹
    assert_eq!(valuation.total_valuation, 250.0, "Total valuation must equal (10 * 20) + (5 * 10) = 250.0 ₹");
}

#[test]
fn test_tc_rep_004_dashboard_today_excludes_prior_days() {
    let uow = TestDb::new();
    let report_service = ReportServiceImpl::with_fixed_today(uow.clone(), "2026-11-20");

    let mut tx = uow.begin_transaction().expect("begin tx");
    let yesterday = create_test_sale_tx(1, "SAL-1001", None, 700.0, 700.0, "2026-11-19");
    tx.sales().create_transaction(&yesterday).expect("yesterday sale");
    let today = create_test_sale_tx(2, "SAL-1002", None, 300.0, 100.0, "2026-11-20");
    tx.sales().create_transaction(&today).expect("today sale");
    tx.commit().expect("commit");

    let metrics = report_service.get_dashboard_metrics().expect("metrics");
    assert_eq!(metrics.today_sales, 300.0, "only today's sale counts toward today_sales");
    assert_eq!(metrics.today_sales_count, 1);
    // Balances stay all-time: yesterday fully paid, today owes 200.
    assert_eq!(metrics.customer_receivables, 200.0);
}
