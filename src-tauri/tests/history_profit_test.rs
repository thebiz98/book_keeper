//! Test Suite: bill and line profit in the sales history and the bill details (FR-34, DD-63).
//! Real services on real SQLite; the profit must agree with the P&L sources.

mod common;

use std::sync::Arc;

use book_keeper::core::dtos::*;
use book_keeper::core::entities::*;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::data::contracts::FtsIndex;
use book_keeper::services::contracts::charge_type_service::ChargeTypeService;
use book_keeper::services::contracts::party_service::PartyService;
use book_keeper::services::contracts::report_service::ReportService;
use book_keeper::services::contracts::return_service::ReturnService;
use book_keeper::services::contracts::sale_service::SaleService;
use common::ledger::*;
use common::stock::seed_purchase_lot;
use common::*;

type Inv = InventoryServiceImpl<FifoStrategy>;
type Acct = AccountingServiceImpl<PaymentServiceImpl>;

struct Env {
    db: TestDb,
    inv: Inv,
    sales: SaleServiceImpl<Inv, PaymentServiceImpl>,
    rets: ReturnServiceImpl<Inv, PaymentServiceImpl, Acct>,
    types: ChargeTypeServiceImpl,
}

fn env() -> Env {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let acct = AccountingServiceImpl::new(PaymentServiceImpl, db.clone());
    let index: Arc<dyn FtsIndex> = Arc::new(db.charge_type_index());
    Env {
        sales: SaleServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        rets: ReturnServiceImpl::new(inv.clone(), PaymentServiceImpl, acct, db.clone()),
        types: ChargeTypeServiceImpl::new(db.clone(), index),
        inv,
        db,
    }
}

impl Env {
    /// Item 1: one lot of `qty` @ `cost`, default price 100.
    fn stocked(&self, qty: f64, cost: f64) {
        seed_stocked_item(&self.db, &self.inv, 1, qty, cost, 100.0);
    }
    fn sell(&self, qty: f64, price: f64, charges: Vec<BillChargeInput>) -> i64 {
        let mut p = sale_payload(None, PaymentStatus::Paid, qty * price + 1000.0, &[(1, None, qty, price)]);
        p.charges = charges;
        self.sales.create_sale(p).expect("sale").sale_id
    }
    fn line_id(&self, sale: i64) -> i64 {
        self.db.begin_read().unwrap().sales().list_items_by_transaction_id(sale).unwrap()[0].id
    }
    fn ty(&self, name: &str, effect: ChargeEffect) -> i64 {
        self.types
            .create_charge_type(CreateChargeTypePayload { name: name.into(), effect, applies_to: ChargeAppliesTo::Both, default_amount: None })
            .unwrap()
            .id
    }
    fn history(&self, include: Option<bool>) -> Vec<SaleHistoryRow> {
        let f = SalesHistoryFilter { include_profit: include, ..Default::default() };
        let mut rows = self.sales.get_sales_history(f).unwrap().items;
        rows.sort_by_key(|r| r.id);
        rows
    }
    fn profit_of(&self, sale: i64) -> f64 {
        self.history(Some(true)).iter().find(|r| r.id == sale).unwrap().profit.unwrap()
    }
    fn details(&self, sale: i64) -> SaleDetailView {
        self.sales.get_sale_details(sale).unwrap()
    }
    fn ret(&self, sale: i64, st: SettlementType, qty: f64, price: f64, cond: StockCondition, issue: Option<f64>) {
        let p = CustomerReturnPayload {
            original_sale_id: Some(sale),
            party_id: None,
            return_date: DAY.to_string(),
            settlement_type: st,
            reason: None,
            notes: None,
            items: vec![CustomerReturnItemPayload {
                item_id: 1,
                sale_item_id: Some(self.line_id(sale)),
                quantity: qty,
                unit_price: price,
                condition: cond,
                unit_cost_override: None,
            }],
            replacement_items: issue.map(|q| vec![ReplacementItemPayload { item_id: 1, quantity: q }]).unwrap_or_default(),
        };
        self.rets.create_customer_return(p).expect("return");
    }
    fn row_counts(&self) -> Vec<i64> {
        let c = self.db.pool.get().unwrap();
        ["sale_transactions", "sale_items", "sale_stock_allocations", "return_transactions", "return_items", "payment_settlements", "purchase_items"]
            .iter()
            .map(|t| c.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0)).unwrap())
            .collect()
    }
}

const R: StockCondition = StockCondition::Restockable;
const D: StockCondition = StockCondition::DamagedScrap;

fn assert_consistent(e: &Env, sale: i64) {
    let d = e.details(sale);
    let s = &d.profit_summary;
    assert_eq!(s.total, e.profit_of(sale), "details total == history profit");
    assert!((s.lines_profit + s.charges + s.return_adjustment - s.total).abs() < 0.005, "footer reconciles: {s:?}");
    let lines: f64 = d.items.iter().map(|i| i.profit).sum();
    assert!((lines - s.lines_profit).abs() < 0.015, "sum of line profits {lines} ~ {}", s.lines_profit);
}

#[test]
fn simple_bill_profit() {
    let e = env();
    e.stocked(10.0, 6.0);
    let sale = e.sell(4.0, 10.0, vec![]);
    assert_eq!(e.profit_of(sale), 16.0);
    let d = e.details(sale);
    assert_eq!((d.items[0].cost_amount, d.items[0].profit), (24.0, 16.0));
    assert_eq!(d.profit_summary, ProfitSummary { lines_profit: 16.0, charges: 0.0, return_adjustment: 0.0, total: 16.0 });
    assert_consistent(&e, sale);
}

#[test]
fn multi_lot_fifo_cost() {
    let e = env();
    e.stocked(2.0, 5.0);
    {
        let mut tx = e.db.begin_transaction().unwrap();
        seed_purchase_lot(&e.inv, &mut *tx, 1001, 1, 5.0, 8.0);
        tx.commit().unwrap();
    }
    let sale = e.sell(4.0, 20.0, vec![]); // 2 @ 5 + 2 @ 8 = 26
    assert_eq!(e.profit_of(sale), 54.0);
    assert_eq!(e.details(sale).items[0].cost_amount, 26.0);
    assert_consistent(&e, sale);
}

#[test]
fn bill_discount_and_charge() {
    let e = env();
    e.stocked(10.0, 60.0);
    let (fee, off) = (e.ty("Transport", ChargeEffect::Charge), e.ty("Round off", ChargeEffect::Discount));
    let sale = e.sell(2.0, 100.0, vec![BillChargeInput { charge_type_id: fee, amount: 20.0 }, BillChargeInput { charge_type_id: off, amount: 10.0 }]);
    // total 210, cost 120
    assert_eq!(e.profit_of(sale), 90.0);
    let d = e.details(sale);
    assert_eq!(d.items[0].profit, 70.0, "200 less its 10 discount share less 120 cost");
    assert_eq!(d.profit_summary, ProfitSummary { lines_profit: 70.0, charges: 20.0, return_adjustment: 0.0, total: 90.0 });
    assert_consistent(&e, sale);
}

#[test]
fn partial_linked_restockable_refund() {
    let e = env();
    e.stocked(10.0, 6.0);
    let sale = e.sell(4.0, 10.0, vec![]);
    e.ret(sale, SettlementType::Refund, 1.0, 10.0, R, None);
    // revenue 40 - refund 10, cost of the 3 units kept = 18
    assert_eq!(e.profit_of(sale), 12.0);
    let d = e.details(sale);
    assert_eq!((d.items[0].cost_amount, d.items[0].profit), (18.0, 22.0));
    assert_eq!(d.profit_summary.return_adjustment, -10.0);
    assert_consistent(&e, sale);
}

#[test]
fn scrap_refund_keeps_the_cost() {
    let e = env();
    e.stocked(10.0, 6.0);
    let sale = e.sell(4.0, 10.0, vec![]);
    e.ret(sale, SettlementType::Refund, 1.0, 10.0, D, None);
    // 40 - 18 (kept units) - 10 refund - 6 scrapped unit
    assert_eq!(e.profit_of(sale), 6.0);
    assert_eq!(e.details(sale).profit_summary.return_adjustment, -16.0);
    assert_consistent(&e, sale);
}

#[test]
fn replacement_adds_issue_cost_and_keeps_revenue() {
    let e = env();
    e.stocked(10.0, 8.0);
    let sale = e.sell(2.0, 20.0, vec![]);
    e.ret(sale, SettlementType::Replacement, 1.0, 20.0, R, Some(1.0));
    // revenue 40 stays; cost: 1 unit kept (8) + 1 replacement issued (8)
    assert_eq!(e.profit_of(sale), 24.0);
    assert_eq!(e.details(sale).profit_summary.return_adjustment, -8.0);
    assert_consistent(&e, sale);
}

#[test]
fn bill_profits_sum_to_the_pnl_net_profit() {
    let e = env();
    e.stocked(30.0, 6.0);
    let fee = e.ty("Transport", ChargeEffect::Charge);
    let off = e.ty("Round off", ChargeEffect::Discount);
    let a = e.sell(4.0, 10.0, vec![BillChargeInput { charge_type_id: fee, amount: 5.0 }, BillChargeInput { charge_type_id: off, amount: 2.0 }]);
    let b = e.sell(4.0, 10.0, vec![]);
    let c = e.sell(2.0, 20.0, vec![]);
    e.ret(a, SettlementType::Refund, 1.0, 8.0, R, None);
    e.ret(b, SettlementType::Refund, 2.0, 10.0, D, None);
    e.ret(c, SettlementType::Replacement, 1.0, 20.0, D, Some(1.0));
    let sum: f64 = e.history(Some(true)).iter().map(|r| r.profit.unwrap()).sum();
    let pnl = ReportServiceImpl::with_fixed_today(e.db.clone(), DAY).get_pnl_report(DAY, DAY).unwrap();
    assert!((pnl.net_profit - sum).abs() < 0.015, "P&L net {} vs bills {}", pnl.net_profit, sum);
    for id in [a, b, c] {
        assert_consistent(&e, id);
    }
}

#[test]
fn profit_is_null_unless_requested_and_reads_write_nothing() {
    let e = env();
    e.stocked(10.0, 6.0);
    let sale = e.sell(4.0, 10.0, vec![]);
    let before = e.row_counts();
    for flag in [None, Some(false)] {
        assert!(e.history(flag).iter().all(|r| r.profit.is_none()), "{flag:?}");
    }
    assert_eq!(e.history(Some(true))[0].profit, Some(16.0));
    e.details(sale);
    assert_eq!(e.row_counts(), before);
}

// ------------------------------------ history totals (J1) ------------------------------------

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 0.005, "{a} vs {b}");
}

impl Env {
    fn party(&self, name: &str, opening: Option<OpeningBalanceInput>) -> i64 {
        let parties = PartyServiceImpl::new(self.db.clone(), Arc::new(self.db.party_index()));
        parties
            .create_party(CreatePartyPayload { name: name.into(), phone: None, party_type: PartyType::Customer, notes: None, opening_balance: opening })
            .expect("party")
    }
    /// One line of item 1; `paid` 0 makes it unpaid.
    fn sell_on(&self, date: &str, party: Option<i64>, qty: f64, price: f64, paid: f64) -> i64 {
        let status = if paid <= 0.0 {
            PaymentStatus::Unpaid
        } else if paid >= qty * price {
            PaymentStatus::Paid
        } else {
            PaymentStatus::Partial
        };
        let mut p = sale_payload(party, status, paid, &[(1, None, qty, price)]);
        p.transaction_date = date.to_string();
        self.sales.create_sale(p).expect("sale").sale_id
    }
    fn page(&self, f: SalesHistoryFilter) -> SalesHistoryPage {
        self.sales.get_sales_history(f).unwrap()
    }
    fn totals_filter() -> SalesHistoryFilter {
        SalesHistoryFilter { include_totals: Some(true), include_profit: Some(true), ..Default::default() }
    }
}

/// Five bills over two parties and two dates; one has a return.
fn totals_env() -> (Env, [i64; 5]) {
    let e = env();
    e.stocked(100.0, 6.0);
    let alice = e.party("Alice", None);
    let bob = e.party("Bob", None);
    let b1 = e.sell_on("2026-11-18", Some(alice), 2.0, 10.0, 20.0);
    let b2 = e.sell_on("2026-11-19", Some(alice), 3.0, 10.0, 10.0);
    let b3 = e.sell_on("2026-11-20", Some(bob), 4.0, 10.0, 0.0);
    let b4 = e.sell_on("2026-11-20", Some(bob), 1.0, 12.0, 12.0);
    let b5 = e.sell_on("2026-11-20", None, 5.0, 10.0, 50.0);
    e.ret(b2, SettlementType::Refund, 1.0, 10.0, R, None);
    (e, [b1, b2, b3, b4, b5])
}

#[test]
fn totals_cover_every_page_and_equal_the_row_sums() {
    let (e, _) = totals_env();
    let all = e.page(SalesHistoryFilter { include_profit: Some(true), limit: Some(200), ..Default::default() });
    assert_eq!(all.items.len(), 5);
    let paged = e.page(SalesHistoryFilter { limit: Some(2), ..Env::totals_filter() });
    assert_eq!((paged.items.len(), paged.total), (2, 5));
    let t = paged.totals.expect("totals");
    assert_eq!(t.bill_count, 5);
    close(t.total_amount, all.items.iter().map(|r| r.total_amount).sum());
    close(t.paid_amount, all.items.iter().map(|r| r.paid_amount).sum());
    close(t.balance_due, all.items.iter().map(|r| r.balance_due).sum());
    close(t.profit.unwrap(), all.items.iter().map(|r| r.profit.unwrap()).sum());
    // Paging does not change the totals.
    let page3 = e.page(SalesHistoryFilter { limit: Some(2), page: Some(3), ..Env::totals_filter() });
    assert_eq!(page3.totals, Some(t));
}

#[test]
fn filters_narrow_the_totals() {
    let (e, _) = totals_env();
    let unpaid = e.page(SalesHistoryFilter { payment_status: Some(PaymentStatusFilter::Unpaid), ..Env::totals_filter() });
    let t = unpaid.totals.unwrap();
    assert_eq!(t.bill_count, 1);
    close(t.total_amount, 40.0);
    close(t.balance_due, 40.0);
    let alice = e.page(SalesHistoryFilter { search_query: Some("alice".into()), ..Env::totals_filter() });
    let t = alice.totals.unwrap();
    assert_eq!(t.bill_count, 2);
    close(t.total_amount, 50.0);
    let dated = e.page(SalesHistoryFilter { start_date: Some("2026-11-19".into()), end_date: Some("2026-11-19".into()), ..Env::totals_filter() });
    let t = dated.totals.unwrap();
    assert_eq!(t.bill_count, 1);
    close(t.total_amount, 30.0);
}

#[test]
fn totals_exclude_opening_bills() {
    let (e, _) = totals_env();
    e.party("Owes", Some(OpeningBalanceInput { amount: 500.0, as_of_date: DAY.to_string() }));
    let t = e.page(Env::totals_filter()).totals.unwrap();
    assert_eq!(t.bill_count, 5);
    let all = e.page(SalesHistoryFilter { include_profit: Some(true), limit: Some(200), ..Default::default() });
    close(t.balance_due, all.items.iter().map(|r| r.balance_due).sum());
    close(t.total_amount, all.items.iter().map(|r| r.total_amount).sum());
    assert!(t.balance_due < 500.0, "the 500 opening bill is not counted");
}

#[test]
fn totals_profit_none_without_include_profit_and_totals_none_without_flag() {
    let (e, _) = totals_env();
    let t = e.page(SalesHistoryFilter { include_totals: Some(true), ..Default::default() }).totals.unwrap();
    assert!(t.profit.is_none());
    for flag in [None, Some(false)] {
        let p = e.page(SalesHistoryFilter { include_totals: flag, include_profit: Some(true), ..Default::default() });
        assert!(p.totals.is_none(), "{flag:?}");
        assert_eq!(p.total, 5);
    }
}

#[test]
fn totals_profit_equals_the_item_profit_bills_profit() {
    let (e, _) = totals_env();
    let t = e.page(Env::totals_filter()).totals.unwrap();
    let rep = ReportServiceImpl::with_fixed_today(e.db.clone(), DAY)
        .get_item_profit_report(ItemProfitFilter { from_date: "2026-11-18".into(), to_date: DAY.into(), search: None, sort: None, page: None, page_size: None })
        .unwrap();
    close(t.profit.unwrap(), rep.reconciliation.bills_profit);
}

/// Timing check for the performance claim: run with `cargo test --test history_profit_test -- --ignored`.
#[test]
#[ignore]
fn totals_with_profit_on_20k_bills_under_150ms() {
    let e = env();
    e.stocked(1.0, 6.0);
    {
        let c = e.db.pool.get().unwrap();
        c.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
        let t = e.sell_on(DAY, None, 1.0, 10.0, 10.0);
        // Clone the seeded bill (header + line) 20 000 times at the SQL level.
        let cols: Vec<String> = c
            .prepare("SELECT name FROM pragma_table_info('sale_items') WHERE name NOT IN ('id', 'sale_transaction_id')")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
        let cols = cols.join(", ");
        let mut copies = String::from("BEGIN;");
        for i in 0..20_000 {
            copies.push_str(&format!(
                "INSERT INTO sale_transactions (transaction_code, transaction_date, party_name, total_amount, paid_amount, balance_due, payment_status, items_subtotal, charges_total, discounts_total, is_opening_balance, created_at, updated_at)                  SELECT 'PERF-{i}', transaction_date, party_name, total_amount, paid_amount, balance_due, payment_status, items_subtotal, charges_total, discounts_total, 0, created_at, updated_at FROM sale_transactions WHERE id = {t};                 INSERT INTO sale_items (sale_transaction_id, {cols}) SELECT last_insert_rowid(), {cols} FROM sale_items WHERE sale_transaction_id = {t};"
            ));
        }
        copies.push_str("COMMIT;");
        c.execute_batch(&copies).expect("seed");
    }
    let base = std::time::Instant::now();
    e.page(SalesHistoryFilter { include_totals: Some(true), ..Default::default() });
    eprintln!("totals without profit: {:?}", base.elapsed());
    let start = std::time::Instant::now();
    let t = e.page(Env::totals_filter()).totals.unwrap();
    let took = start.elapsed();
    eprintln!("totals with profit over 20k bills: {took:?}");
    assert_eq!(t.bill_count, 20_001);
    // SQLite is compiled unoptimised in the dev profile; the 150 ms budget is for release builds.
    let budget = if cfg!(debug_assertions) { 1000 } else { 150 };
    assert!(took < std::time::Duration::from_millis(budget), "took {took:?}");
}
