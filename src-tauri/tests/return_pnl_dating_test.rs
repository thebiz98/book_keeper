// tests/return_pnl_dating_test.rs
//! Test Suite: returns post in the return period (v1.7 H4, FR-56, DD-91, owner D2).
//! Findings R4 (unlinked restocked cost), R5 (closed months stay unchanged), R6 (bill profit and
//! header from return lines), R8 (cumulative refund cap per sale line), L3 (dues paid & credited).
//! Production services on real SQLite.

mod common;

use std::sync::Arc;

use book_keeper::core::dtos::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::accounting_service::AccountingService;
use book_keeper::services::contracts::charge_type_service::ChargeTypeService;
use book_keeper::services::contracts::item_service::ItemService;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::report_service::ReportService;
use book_keeper::services::contracts::return_service::ReturnService;
use book_keeper::services::contracts::sale_service::SaleService;
use common::invariants::*;
use common::ledger::*;
use common::*;

type Inv = InventoryServiceImpl<FifoStrategy>;
type Acct = AccountingServiceImpl<PaymentServiceImpl>;

struct E {
    db: TestDb,
    items: ItemServiceImpl<Inv>,
    sales: SaleServiceImpl<Inv, PaymentServiceImpl>,
    purchases: PurchaseServiceImpl<Inv, PaymentServiceImpl>,
    rets: ReturnServiceImpl<Inv, PaymentServiceImpl, Acct>,
    acct: Acct,
    types: ChargeTypeServiceImpl,
    report: ReportServiceImpl,
}

fn env() -> E {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let acct = AccountingServiceImpl::new(PaymentServiceImpl, db.clone());
    E {
        items: ItemServiceImpl::new(db.clone(), Arc::new(db.indexer()), inv.clone()),
        sales: SaleServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        purchases: PurchaseServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        rets: ReturnServiceImpl::new(inv, PaymentServiceImpl, acct.clone(), db.clone()),
        acct,
        types: ChargeTypeServiceImpl::new(db.clone(), Arc::new(db.charge_type_index())),
        report: ReportServiceImpl::with_fixed_today(db.clone(), "2026-11-20"),
        db,
    }
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 0.0101, "{a} != {b}");
}

fn line(item_id: i64, sale_item_id: Option<i64>, quantity: f64, unit_price: f64, condition: StockCondition) -> CustomerReturnItemPayload {
    CustomerReturnItemPayload { item_id, sale_item_id, quantity, unit_price, condition, unit_cost_override: None }
}

impl E {
    fn item(&self, name: &str) -> i64 {
        self.items
            .create_item(CreateItemPayload {
                item_type: None, service_cost: None, components: None, low_stock_threshold: None,
                name: name.into(), category: None, barcode: None, base_unit: "PCS".into(), mrp: None,
                default_sale_price: 10.0, initial_stock: None, initial_cost: None, bundles: None,
            })
            .unwrap()
    }
    fn svc(&self, name: &str, cost: f64, comps: &[(i64, f64)]) -> i64 {
        self.items
            .create_item(CreateItemPayload {
                item_type: Some(ItemType::Service),
                service_cost: Some(cost),
                components: Some(comps.iter().map(|&(i, q)| ServiceComponentInput { component_item_id: i, quantity_per_unit: q }).collect()),
                low_stock_threshold: None,
                name: name.into(), category: None, barcode: None, base_unit: "PRINT".into(), mrp: None,
                default_sale_price: 12.0, initial_stock: None, initial_cost: None, bundles: None,
            })
            .unwrap()
    }
    fn lot(&self, item: i64, qty: f64, cost: f64) {
        let mut p = purchase_payload(None, PaymentStatus::Paid, qty * cost, &[(item, None, qty, cost)]);
        p.transaction_date = "2026-01-05".into();
        self.purchases.create_purchase(p).unwrap();
    }
    fn sell(&self, date: &str, lines: &[(i64, f64, f64)], charges: Vec<BillChargeInput>) -> i64 {
        let l: Vec<(i64, Option<i64>, f64, f64)> = lines.iter().map(|&(i, q, p)| (i, None, q, p)).collect();
        let subtotal: f64 = lines.iter().map(|&(_, q, p)| q * p).sum();
        let mut p = sale_payload(None, PaymentStatus::Paid, subtotal + 1000.0, &l);
        p.transaction_date = date.into();
        p.charges = charges;
        self.sales.create_sale(p).unwrap().sale_id
    }
    fn sale_line(&self, sale: i64, item: i64) -> (i64, f64) {
        let d = self.sales.get_sale_details(sale).unwrap();
        let l = d.items.iter().find(|l| l.sale_item.item_id == item).unwrap();
        (l.sale_item.id, l.net_unit_price)
    }
    fn ret(&self, sale: Option<i64>, date: &str, st: SettlementType, items: Vec<CustomerReturnItemPayload>, repl: Vec<ReplacementItemPayload>) -> Result<ReturnResult, AppError> {
        self.rets.create_customer_return(CustomerReturnPayload {
            original_sale_id: sale,
            party_id: None,
            return_date: date.into(),
            settlement_type: st,
            reason: None,
            notes: None,
            items,
            replacement_items: repl,
        })
    }
    /// Linked return of `qty` units of `item` on `sale` at the net sold rate.
    fn ret_linked(&self, sale: i64, item: i64, qty: f64, cond: StockCondition, date: &str) -> ReturnResult {
        let (sid, rate) = self.sale_line(sale, item);
        self.ret(Some(sale), date, SettlementType::Refund, vec![line(item, Some(sid), qty, rate, cond)], vec![]).unwrap()
    }
    fn pnl(&self, from: &str, to: &str) -> PnLReport {
        self.report.get_pnl_report(from, to).unwrap()
    }
    fn item_profit(&self, from: &str, to: &str) -> ItemProfitReport {
        self.report
            .get_item_profit_report(ItemProfitFilter {
                from_date: from.into(), to_date: to.into(), search: None, sort: None, page: None, page_size: Some(200),
            })
            .unwrap()
    }
    fn bill_profit(&self, sale: i64) -> BillProfitParts {
        self.db.begin_read().unwrap().sales().bill_profit_parts(sale).unwrap()
    }
    fn invariants(&self) {
        assert_ledger_invariant(&self.db);
        assert_lot_flow_invariant(&self.db);
    }
}

const MAR: (&str, &str) = ("2026-03-01", "2026-03-31");
const APR: (&str, &str) = ("2026-04-01", "2026-04-30");

// ------------------------------------ R5: closed months never change ------------------------------------

#[test]
fn r5_march_sale_returned_in_april_leaves_march_unchanged() {
    let e = env();
    let a = e.item("Alpha");
    e.lot(a, 10.0, 6.0);
    let sale = e.sell("2026-03-10", &[(a, 10.0, 10.0)], vec![]);
    let march_before = e.pnl(MAR.0, MAR.1);
    let march_items_before = e.item_profit(MAR.0, MAR.1);
    assert_eq!((march_before.cogs, march_before.gross_profit), (60.0, 40.0));

    e.ret_linked(sale, a, 10.0, StockCondition::Restockable, "2026-04-05");

    assert_eq!(e.pnl(MAR.0, MAR.1), march_before, "a closed month never changes");
    assert_eq!(e.item_profit(MAR.0, MAR.1).rows, march_items_before.rows);
    let april = e.pnl(APR.0, APR.1);
    assert_eq!(april.sales_returns, 100.0);
    assert_eq!(april.returned_goods_cost, 60.0);
    assert_eq!(april.cogs, 0.0);
    assert_eq!(april.gross_profit, -40.0);
    assert_eq!(april.inventory_losses, 0.0);
    e.invariants();
}

#[test]
fn r5_linked_scrap_return_credits_cost_and_books_the_loss_on_the_return_date() {
    let e = env();
    let a = e.item("Alpha");
    e.lot(a, 10.0, 6.0);
    let sale = e.sell("2026-03-10", &[(a, 10.0, 10.0)], vec![]);
    let march_before = e.pnl(MAR.0, MAR.1);
    e.ret_linked(sale, a, 10.0, StockCondition::DamagedScrap, "2026-04-05");
    assert_eq!(e.pnl(MAR.0, MAR.1), march_before);
    let april = e.pnl(APR.0, APR.1);
    assert_eq!((april.returned_goods_cost, april.inventory_losses), (60.0, 60.0));
    // Total cost over both months is the 60 that left stock, counted once.
    let both = e.pnl(MAR.0, APR.1);
    close(both.cogs - both.returned_goods_cost + both.inventory_losses, 60.0);
    close(march_before.cogs + april.cogs - april.returned_goods_cost + april.inventory_losses, 60.0);
    e.invariants();
}

// ------------------------------------ R4: unlinked restocked returns ------------------------------------

#[test]
fn r4_unlinked_restocked_return_credits_its_lot_cost_and_nets_zero_cost_when_resold() {
    let e = env();
    let a = e.item("Alpha");
    let mut l = line(a, None, 2.0, 9.0, StockCondition::Restockable);
    l.unit_cost_override = Some(6.0);
    e.ret(None, "2026-04-05", SettlementType::Refund, vec![l], vec![]).unwrap();
    let cost: f64 = e
        .db
        .pool
        .get()
        .unwrap()
        .query_row("SELECT cost_amount FROM return_items", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cost, 12.0, "cost = qty x RETURN lot cost");
    let april = e.pnl(APR.0, APR.1);
    assert_eq!((april.sales_returns, april.returned_goods_cost), (18.0, 12.0));
    let row = e.item_profit(APR.0, APR.1).rows.into_iter().find(|r| r.name == "Alpha").expect("row from the return");
    close(row.returned_cost, 12.0);
    close(row.returns_value, 18.0);
    close(row.profit, -6.0);

    // The restocked units sell again: gross COGS takes the RETURN lot cost, the credit offsets it.
    e.sell("2026-04-10", &[(a, 2.0, 10.0)], vec![]);
    let april = e.pnl(APR.0, APR.1);
    assert_eq!(april.cogs, 12.0);
    close(april.cogs - april.returned_goods_cost, 0.0);
    close(april.gross_profit, 20.0 - 18.0);
    e.invariants();
}

// ------------------------------------ P&L = Σ item profit ------------------------------------

#[test]
fn pnl_gross_profit_equals_item_profit_over_ranges_with_returns() {
    let e = env();
    let a = e.item("Alpha");
    let b = e.item("Beta");
    let g = e.item("Gamma");
    let paper = e.item("Paper");
    for (i, c) in [(a, 3.0), (b, 4.0), (g, 5.0), (paper, 2.0)] {
        e.lot(i, 100.0, c);
    }
    let print = e.svc("Print", 3.0, &[(paper, 1.0)]);
    let disc = e
        .types
        .create_charge_type(CreateChargeTypePayload { name: "Disc".into(), effect: ChargeEffect::Discount, applies_to: ChargeAppliesTo::Sale, default_amount: None })
        .unwrap()
        .id;
    let fee = e
        .types
        .create_charge_type(CreateChargeTypePayload { name: "Fee".into(), effect: ChargeEffect::Charge, applies_to: ChargeAppliesTo::Sale, default_amount: None })
        .unwrap()
        .id;
    let s1 = e.sell("2026-03-02", &[(a, 7.0, 9.0), (b, 3.0, 7.5), (print, 2.0, 12.0)], vec![BillChargeInput { charge_type_id: disc, amount: 5.0 }]);
    let s2 = e.sell("2026-03-20", &[(g, 5.0, 8.0), (a, 1.0, 0.0)], vec![BillChargeInput { charge_type_id: fee, amount: 12.0 }]);
    let s3 = e.sell("2026-04-03", &[(b, 6.0, 6.5)], vec![]);
    // April: linked restock and scrap on a March bill, a replacement, an unlinked restock.
    e.ret_linked(s1, a, 2.0, StockCondition::Restockable, "2026-04-04");
    e.ret_linked(s1, b, 1.0, StockCondition::DamagedScrap, "2026-04-06");
    let (gs, gr) = e.sale_line(s2, g);
    e.ret(Some(s2), "2026-04-07", SettlementType::Replacement, vec![line(g, Some(gs), 1.0, gr, StockCondition::Restockable)], vec![ReplacementItemPayload { item_id: b, quantity: 1.0 }]).unwrap();
    let mut un = line(g, None, 1.0, 7.0, StockCondition::Restockable);
    un.unit_cost_override = Some(5.0);
    e.ret(None, "2026-04-08", SettlementType::Refund, vec![un], vec![]).unwrap();
    // May: a linked return on the April bill.
    e.ret_linked(s3, b, 2.0, StockCondition::Restockable, "2026-05-02");

    for (from, to) in [MAR, APR, ("2026-05-01", "2026-05-31"), ("2026-03-01", "2026-05-31"), ("2026-03-15", "2026-04-06"), ("2026-04-04", "2026-04-04")] {
        let p = e.pnl(from, to);
        let ip = e.item_profit(from, to);
        close(ip.totals.profit, p.gross_profit);
        close(ip.reconciliation.items_profit, p.gross_profit);
        close(ip.reconciliation.pnl_gross_profit, p.gross_profit);
        let rows: f64 = ip.rows.iter().map(|r| r.profit).sum();
        close(rows, p.gross_profit);
    }
    e.invariants();
}

// ------------------------------------ R6: bill profit and header from lines ------------------------------------

#[test]
fn r6_bill_profit_uses_linked_lines_with_a_free_line_and_matches_item_profit() {
    let e = env();
    let a = e.item("Alpha");
    let f = e.item("Freebie");
    let o = e.item("Other");
    e.lot(a, 10.0, 4.0);
    e.lot(f, 10.0, 2.0);
    let sale = e.sell("2026-04-02", &[(a, 3.0, 10.0), (f, 1.0, 0.0)], vec![]);
    let (fs, _) = e.sale_line(sale, f);
    let (as_, ar) = e.sale_line(sale, a);
    // One return linked to the bill: the free line, one paid unit, and an unlinked unit of another item.
    let r = e
        .ret(
            Some(sale),
            "2026-04-03",
            SettlementType::Refund,
            vec![
                line(f, Some(fs), 1.0, 0.0, StockCondition::Restockable),
                line(a, Some(as_), 1.0, ar, StockCondition::Restockable),
                line(o, None, 1.0, 5.0, StockCondition::DamagedScrap),
            ],
            vec![],
        )
        .unwrap();
    assert_eq!(r.total_amount, 15.0);
    let p = e.bill_profit(sale);
    assert_eq!(p.refunds, 10.0, "linked lines only, not the 15 header");
    // 30 - cost 14 + returned cost (2 + 4) - refund 10 = 12
    close(p.total, 12.0);
    let d = e.sales.get_sale_details(sale).unwrap();
    close(d.profit_summary.total, 12.0);
    close(d.items.iter().map(|l| l.profit).sum::<f64>() + d.profit_summary.return_adjustment, 12.0);
    // One-bill range without the unlinked line's item: Σ item profit = bill profit.
    let ip = e.item_profit("2026-04-01", "2026-04-30");
    let linked: f64 = ip.rows.iter().filter(|r| r.name != "Other").map(|r| r.profit).sum();
    close(linked, p.total);
    e.invariants();
}

#[test]
fn r6_header_total_is_the_sum_of_rounded_lines() {
    let e = env();
    let a = e.item("A");
    let b = e.item("B");
    let c = e.item("C");
    for i in [a, b, c] {
        e.lot(i, 10.0, 0.05);
    }
    let sale = e.sell("2026-04-02", &[(a, 1.0, 0.125), (b, 1.0, 0.125), (c, 1.0, 0.125)], vec![]);
    let lines: Vec<_> = [a, b, c]
        .iter()
        .map(|&i| {
            let (sid, rate) = e.sale_line(sale, i);
            line(i, Some(sid), 1.0, rate, StockCondition::Restockable)
        })
        .collect();
    let r = e.ret(Some(sale), "2026-04-03", SettlementType::Refund, lines, vec![]).unwrap();
    let rows: Vec<(f64,)> = {
        let c = e.db.pool.get().unwrap();
        let mut s = c.prepare("SELECT total_amount FROM return_items ORDER BY id").unwrap();
        s.query_map([], |r| Ok((r.get(0)?,))).unwrap().map(|x| x.unwrap()).collect()
    };
    let sum: f64 = rows.iter().map(|r| r.0).sum();
    let header: f64 = e.db.pool.get().unwrap().query_row("SELECT total_amount FROM return_transactions", [], |r| r.get(0)).unwrap();
    close(header, sum);
    assert!((header - (sum * 100.0).round() / 100.0).abs() < 1e-9, "header {header} = Σ lines {sum}");
    assert!((r.total_amount - header).abs() < 1e-9);
    assert!((e.bill_profit(sale).refunds - header).abs() < 1e-9);
    e.invariants();
}

// ------------------------------------ R8: cumulative refund cap ------------------------------------

#[test]
fn r8_refunds_over_all_returns_of_a_line_stay_within_its_net_total() {
    let e = env();
    let a = e.item("Alpha");
    e.lot(a, 10.0, 0.05);
    let sale = e.sell("2026-04-02", &[(a, 2.0, 0.125)], vec![]); // line total 0.25
    let (sid, rate) = e.sale_line(sale, a);
    let one = |price: f64| e.ret(Some(sale), "2026-04-03", SettlementType::Refund, vec![line(a, Some(sid), 1.0, price, StockCondition::Restockable)], vec![]);
    assert_eq!(one(rate).unwrap().total_amount, 0.13); // half a paisa rounds up
    match one(rate) {
        Err(AppError::ValidationError { field, message }) => {
            assert_eq!(field, "unit_price");
            assert!(message.contains("can be at most") && message.contains("0.12"), "{message}");
        }
        other => panic!("expected the cumulative cap, got {other:?}"),
    }
    assert_eq!(one(0.12).unwrap().total_amount, 0.12);
    let refunded: f64 = e.db.pool.get().unwrap().query_row("SELECT SUM(total_amount) FROM return_items", [], |r| r.get(0)).unwrap();
    assert!(refunded <= 0.25 + 1e-9, "Σ refunds {refunded} ≤ net line total");
    e.invariants();
}

#[test]
fn r8_cap_is_net_of_the_bill_discount_and_counts_lines_of_the_same_payload() {
    let e = env();
    let a = e.item("Alpha");
    e.lot(a, 1000.0, 0.01);
    let disc = e
        .types
        .create_charge_type(CreateChargeTypePayload { name: "Disc".into(), effect: ChargeEffect::Discount, applies_to: ChargeAppliesTo::Sale, default_amount: None })
        .unwrap()
        .id;
    // 1000 x 0.335 = 335.00, 10% off: net line total 301.50, net rate 0.3015.
    let sale = e.sell("2026-04-02", &[(a, 1000.0, 0.335)], vec![BillChargeInput { charge_type_id: disc, amount: 33.5 }]);
    let (sid, rate) = e.sale_line(sale, a);
    close(rate, 0.3015);
    let part = |q: f64| line(a, Some(sid), q, rate, StockCondition::Restockable);
    e.ret(Some(sale), "2026-04-03", SettlementType::Refund, vec![part(499.0), part(1.0)], vec![]).unwrap();
    e.ret(Some(sale), "2026-04-04", SettlementType::Refund, vec![part(500.0)], vec![]).unwrap();
    let refunded: f64 = e.db.pool.get().unwrap().query_row("SELECT SUM(total_amount) FROM return_items", [], |r| r.get(0)).unwrap();
    assert!(refunded <= 301.5 + 0.005, "Σ refunds {refunded} ≤ 301.50");
    e.invariants();
}

// ------------------------------------ L3: dues paid & credited ------------------------------------

#[test]
fn l3_dues_row_counts_return_credits_as_paid_and_credited() {
    let e = env();
    seed_party(&e.db, 1, "Cust", PartyType::Customer);
    let a = e.item("Alpha");
    e.lot(a, 10.0, 4.0);
    let mut p = sale_payload(Some(1), PaymentStatus::Partial, 5.0, &[(a, None, 3.0, 10.0)]);
    p.transaction_date = "2026-04-02".into();
    let sale = e.sales.create_sale(p).unwrap().sale_id;
    e.ret_linked(sale, a, 1.0, StockCondition::Restockable, "2026-04-03");
    let rows = e.acct.get_party_dues_summary(PartyType::Customer).unwrap();
    let r = &rows[0];
    assert_eq!((r.total_invoiced, r.total_paid, r.paid_and_credited, r.balance_due), (30.0, 5.0, 15.0, 15.0));
    close(r.total_invoiced - r.paid_and_credited, r.balance_due);
}
