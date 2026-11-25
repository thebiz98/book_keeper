//! Test Suite: bill and line profit in the sales history and the bill details (FR-34, DD-63).
//! Real services on real SQLite; the profit must agree with the P&L sources.

mod common;

use std::sync::Arc;

use book_keeper::core::dtos::*;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::data::contracts::FtsIndex;
use book_keeper::services::contracts::charge_type_service::ChargeTypeService;
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
