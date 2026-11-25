// tests/item_analytics_test.rs
//! Test Suite: item profit and inventory speed reports (FR-38, FR-39, DD-68, v1.4 D3).
//! Production services on real SQLite; the reports are read-only (X6).

mod common;

use std::sync::Arc;

use book_keeper::core::dtos::*;
use book_keeper::core::entities::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::data::sqlite::repositories::item_analytics_sql::{profit_page_sql, speed_page_sql};
use book_keeper::services::contracts::charge_type_service::ChargeTypeService;
use book_keeper::services::contracts::item_service::ItemService;
use book_keeper::services::contracts::party_service::PartyService;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::report_service::ReportService;
use book_keeper::services::contracts::return_service::ReturnService;
use book_keeper::services::contracts::sale_service::SaleService;
use common::ledger::*;
use common::*;

type Inv = InventoryServiceImpl<FifoStrategy>;
type Acct = AccountingServiceImpl<PaymentServiceImpl>;

const FROM: &str = "2026-11-01";
const TO: &str = "2026-11-30";

struct E {
    db: TestDb,
    items: ItemServiceImpl<Inv>,
    sales: SaleServiceImpl<Inv, PaymentServiceImpl>,
    purchases: PurchaseServiceImpl<Inv, PaymentServiceImpl>,
    rets: ReturnServiceImpl<Inv, PaymentServiceImpl, Acct>,
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
        rets: ReturnServiceImpl::new(inv, PaymentServiceImpl, acct, db.clone()),
        types: ChargeTypeServiceImpl::new(db.clone(), Arc::new(db.charge_type_index())),
        report: ReportServiceImpl::with_fixed_today(db.clone(), TO),
        db,
    }
}

fn physical(name: &str) -> CreateItemPayload {
    CreateItemPayload {
        item_type: None, service_cost: None, components: None, low_stock_threshold: None,
        name: name.into(), category: None, barcode: None, base_unit: "PCS".into(), mrp: None,
        default_sale_price: 10.0, initial_stock: None, initial_cost: None, bundles: None,
    }
}

fn pf(from: &str, to: &str) -> ItemProfitFilter {
    ItemProfitFilter { from_date: from.into(), to_date: to.into(), search: None, sort: None, page: None, page_size: None }
}

fn sf(from: &str, to: &str) -> InventorySpeedFilter {
    InventorySpeedFilter { from_date: from.into(), to_date: to.into(), class: None, search: None, sort: None, page: None, page_size: None }
}

fn sort(field: &str, dir: &str) -> Option<ReportSort> {
    Some(ReportSort { field: field.into(), dir: dir.into() })
}

fn field_of<T: std::fmt::Debug>(r: Result<T, AppError>) -> String {
    match r {
        Err(AppError::ValidationError { field, .. }) => field,
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 0.0051, "{a} != {b}");
}

impl E {
    fn item(&self, name: &str) -> i64 {
        self.items.create_item(physical(name)).unwrap()
    }
    fn svc(&self, name: &str, cost: Option<f64>, comps: &[(i64, f64)]) -> i64 {
        self.items
            .create_item(CreateItemPayload {
                item_type: Some(ItemType::Service),
                service_cost: cost,
                components: Some(comps.iter().map(|&(i, q)| ServiceComponentInput { component_item_id: i, quantity_per_unit: q }).collect()),
                base_unit: "PRINT".into(),
                default_sale_price: 12.0,
                ..physical(name)
            })
            .unwrap()
    }
    fn lot(&self, item: i64, qty: f64, cost: f64) {
        self.purchases
            .create_purchase(purchase_payload(None, PaymentStatus::Paid, qty * cost, &[(item, None, qty, cost)]))
            .unwrap();
    }
    fn sell_on(&self, date: &str, lines: &[(i64, f64, f64)], charges: Vec<BillChargeInput>) -> i64 {
        let subtotal: f64 = lines.iter().map(|&(_, q, p)| q * p).sum();
        let l: Vec<(i64, Option<i64>, f64, f64)> = lines.iter().map(|&(i, q, p)| (i, None, q, p)).collect();
        let mut p = sale_payload(None, PaymentStatus::Paid, if subtotal > 0.0 { subtotal + 1000.0 } else { 0.0 }, &l);
        p.transaction_date = date.into();
        p.charges = charges;
        self.sales.create_sale(p).unwrap().sale_id
    }
    fn sell(&self, lines: &[(i64, f64, f64)]) -> i64 {
        self.sell_on("2026-11-20", lines, vec![])
    }
    fn charge_type(&self, name: &str, effect: ChargeEffect) -> i64 {
        self.types
            .create_charge_type(CreateChargeTypePayload { name: name.into(), effect, applies_to: ChargeAppliesTo::Sale, default_amount: None })
            .unwrap()
            .id
    }
    fn ret(&self, sale: i64, item: i64, qty: f64, cond: StockCondition, st: SettlementType, repl: Vec<ReplacementItemPayload>) {
        let d = self.sales.get_sale_details(sale).unwrap();
        let line = d.items.iter().find(|l| l.sale_item.item_id == item).unwrap();
        self.rets
            .create_customer_return(CustomerReturnPayload {
                original_sale_id: Some(sale),
                party_id: None,
                return_date: "2026-11-21".into(),
                settlement_type: st,
                reason: None,
                notes: None,
                items: vec![CustomerReturnItemPayload {
                    item_id: item,
                    sale_item_id: Some(line.sale_item.id),
                    quantity: qty,
                    unit_price: line.net_unit_price,
                    condition: cond,
                    unit_cost_override: None,
                }],
                replacement_items: repl,
            })
            .unwrap();
    }
    fn profit(&self, f: ItemProfitFilter) -> ItemProfitReport {
        self.report.get_item_profit_report(f).unwrap()
    }
    fn speed(&self, f: InventorySpeedFilter) -> InventorySpeedReport {
        self.report.get_inventory_speed_report(f).unwrap()
    }
    fn row(&self, name: &str) -> ItemProfitRow {
        self.profit(pf(FROM, TO)).rows.into_iter().find(|r| r.name == name).unwrap_or_else(|| panic!("no row {name}"))
    }
    fn table_counts(&self) -> Vec<i64> {
        let c = self.db.pool.get().unwrap();
        ["sale_transactions", "sale_items", "sale_stock_allocations", "purchase_items", "items", "return_items", "payment_settlements"]
            .iter()
            .map(|t| c.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0)).unwrap())
            .collect()
    }
    /// Reconciliation identity of FR-38 against the bill profits of the period (FR-34).
    fn assert_reconciles(&self, bills: usize) {
        let r = self.profit(pf(FROM, TO)).reconciliation;
        close(r.items_profit + r.charges - r.replacement_cost, r.bills_profit);
        let _ = bills;
        let mut tx = self.db.begin_read().unwrap();
        let (sum_bills,): (f64,) = {
            let c = self.db.pool.get().unwrap();
            let mut s = 0.0;
            let ids: Vec<i64> = c
                .prepare("SELECT id FROM sale_transactions WHERE is_opening_balance = 0 AND transaction_date BETWEEN ?1 AND ?2")
                .unwrap()
                .query_map([FROM, TO], |r| r.get(0))
                .unwrap()
                .map(|x| x.unwrap())
                .collect();
            for id in ids {
                s += tx.sales().bill_profit_parts(id).unwrap().total;
            }
            (s,)
        };
        close(sum_bills, r.bills_profit);
    }
}

// ------------------------------------ item profit ------------------------------------

#[test]
fn profit_row_uses_discount_share_and_matches_line_profit() {
    let e = env();
    let a = e.item("Alpha");
    e.lot(a, 10.0, 5.0);
    let disc = e.charge_type("Disc", ChargeEffect::Discount);
    let sale = e.sell_on("2026-11-20", &[(a, 4.0, 10.0)], vec![BillChargeInput { charge_type_id: disc, amount: 4.0 }]);
    let r = e.row("Alpha");
    close(r.qty_sold, 4.0);
    close(r.revenue, 36.0); // 40 less 10% share
    close(r.cost, 20.0);
    close(r.profit, 16.0);
    close(r.margin_pct.unwrap(), 44.44);
    // same figure as the line profit of the bill detail (nothing returned)
    let d = e.sales.get_sale_details(sale).unwrap();
    close(d.items[0].profit, r.profit);
    let rep = e.profit(pf(FROM, TO));
    close(rep.totals.revenue, 36.0);
    close(rep.totals.profit, 16.0);
    assert_eq!(rep.total_count, 1);
    e.assert_reconciles(1);
}

#[test]
fn returns_refund_restock_scrap_and_replacement() {
    let e = env();
    let a = e.item("Alpha");
    let b = e.item("Beta");
    let g = e.item("Gamma");
    for i in [a, b, g] {
        e.lot(i, 10.0, 5.0);
    }
    let s1 = e.sell(&[(a, 4.0, 10.0)]);
    let s2 = e.sell(&[(b, 4.0, 10.0)]);
    let s3 = e.sell(&[(g, 4.0, 10.0)]);
    e.ret(s1, a, 1.0, StockCondition::Restockable, SettlementType::Refund, vec![]);
    e.ret(s2, b, 2.0, StockCondition::DamagedScrap, SettlementType::Refund, vec![]);
    e.ret(s3, g, 1.0, StockCondition::Restockable, SettlementType::Replacement, vec![ReplacementItemPayload { item_id: g, quantity: 1.0 }]);
    let ra = e.row("Alpha");
    close(ra.qty_sold, 3.0);
    close(ra.revenue, 30.0); // 40 - refund 10
    close(ra.cost, 15.0); // restocked unit gives its cost back
    let rb = e.row("Beta");
    close(rb.qty_sold, 2.0);
    close(rb.revenue, 20.0);
    close(rb.cost, 20.0); // 2 kept (10) + 2 scrapped units keep their cost (10)
    let rg = e.row("Gamma");
    close(rg.qty_sold, 3.0); // replacement returns still count as returned units
    close(rg.revenue, 40.0); // a replacement does not reduce revenue
    let rep = e.profit(pf(FROM, TO));
    assert!(rep.reconciliation.replacement_cost > 0.0);
    e.assert_reconciles(3);
}

#[test]
fn service_line_counts_misc_cost_and_components_never_credited() {
    let e = env();
    let paper = e.item("Paper");
    e.lot(paper, 20.0, 2.0);
    let print = e.svc("Print", Some(3.0), &[(paper, 1.0)]);
    e.sell(&[(print, 2.0, 12.0)]);
    let r = e.row("Print");
    close(r.qty_sold, 2.0);
    close(r.revenue, 24.0);
    close(r.cost, 6.0); // components 4 + misc 2
    assert!(e.profit(pf(FROM, TO)).rows.iter().all(|r| r.name != "Paper"));
    e.assert_reconciles(1);
}

#[test]
fn opening_bills_and_other_periods_are_excluded() {
    let e = env();
    let a = e.item("Alpha");
    e.lot(a, 10.0, 5.0);
    let parties = PartyServiceImpl::new(e.db.clone(), Arc::new(e.db.party_index()));
    parties
        .create_party(CreatePartyPayload {
            name: "Old".into(), phone: None, party_type: PartyType::Customer, notes: None,
            opening_balance: Some(OpeningBalanceInput { amount: 500.0, as_of_date: "2026-11-10".into() }),
        })
        .unwrap();
    e.sell_on("2026-10-15", &[(a, 1.0, 10.0)], vec![]);
    e.sell_on("2026-11-20", &[(a, 2.0, 10.0)], vec![]);
    let rep = e.profit(pf(FROM, TO));
    assert_eq!(rep.total_count, 1);
    close(rep.rows[0].qty_sold, 2.0);
    close(rep.reconciliation.bills_profit, rep.reconciliation.items_profit);
    // edges are inclusive
    assert_eq!(e.profit(pf("2026-11-20", "2026-11-20")).total_count, 1);
    assert_eq!(e.profit(pf("2026-11-21", "2026-11-30")).total_count, 0);
}

#[test]
fn search_sort_paging_and_totals_over_all_rows() {
    let e = env();
    let names = ["Apple", "Banana", "Cherry", "Date", "Apricot"];
    for (i, n) in names.iter().enumerate() {
        let id = e.item(n);
        e.lot(id, 10.0, 5.0);
        e.sell(&[(id, 1.0, 10.0 + i as f64 * 10.0)]); // profit 5, 15, 25, 35, 45
    }
    let all = e.profit(pf(FROM, TO));
    assert_eq!(all.total_count, 5);
    assert_eq!(all.rows[0].name, "Apricot"); // default: profit DESC
    close(all.totals.profit, 5.0 + 15.0 + 25.0 + 35.0 + 45.0);

    let mut f = pf(FROM, TO);
    f.sort = sort("name", "ASC");
    f.page_size = Some(2);
    f.page = Some(2);
    let p2 = e.profit(f.clone());
    assert_eq!(p2.rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["Banana", "Cherry"]);
    assert_eq!((p2.page, p2.page_size, p2.total_count), (2, 2, 5));
    close(p2.totals.profit, 125.0); // totals ignore paging

    for (field, first) in [("qty", "Apple"), ("revenue", "Apple"), ("cost", "Apple"), ("margin", "Apple"), ("profit", "Apple")] {
        let mut f = pf(FROM, TO);
        f.sort = sort(field, "ASC");
        assert_eq!(e.profit(f).rows[0].name, first, "sort {field}");
    }
    let mut f = pf(FROM, TO);
    f.sort = sort("margin", "DESC");
    assert_eq!(e.profit(f).rows[0].name, "Apricot");

    let mut f = pf(FROM, TO);
    f.search = Some("ap".into());
    let r = e.profit(f);
    assert_eq!(r.total_count, 2);
    close(r.totals.profit, 50.0);
    // reconciliation ignores the search
    close(r.reconciliation.items_profit, 125.0);
}

#[test]
fn margin_is_none_without_revenue() {
    let e = env();
    let a = e.item("Freebie");
    e.lot(a, 5.0, 5.0);
    e.sell(&[(a, 1.0, 0.0)]);
    let r = e.row("Freebie");
    assert_eq!(r.margin_pct, None);
    close(r.profit, -5.0);
    assert_eq!(e.profit(pf(FROM, TO)).totals.margin_pct, None);
}

#[test]
fn reconciliation_with_charges_and_discounts_and_many_bills() {
    let e = env();
    let a = e.item("Alpha");
    let b = e.item("Beta");
    e.lot(a, 50.0, 3.0);
    e.lot(b, 50.0, 4.0);
    let disc = e.charge_type("Disc", ChargeEffect::Discount);
    let fee = e.charge_type("Fee", ChargeEffect::Charge);
    e.sell_on("2026-11-02", &[(a, 3.0, 9.0), (b, 2.0, 7.0)], vec![BillChargeInput { charge_type_id: disc, amount: 5.0 }]);
    let s2 = e.sell_on("2026-11-05", &[(a, 5.0, 8.0)], vec![BillChargeInput { charge_type_id: fee, amount: 12.0 }]);
    e.sell_on("2026-11-09", &[(b, 4.0, 6.5), (a, 1.0, 9.0)], vec![BillChargeInput { charge_type_id: disc, amount: 3.5 }]);
    e.ret(s2, a, 2.0, StockCondition::DamagedScrap, SettlementType::Refund, vec![]);
    let r = e.profit(pf(FROM, TO)).reconciliation;
    close(r.charges, 12.0);
    e.assert_reconciles(3);
    let _ = r;
}

#[test]
fn profit_validation_and_no_writes() {
    let e = env();
    let a = e.item("Alpha");
    e.lot(a, 5.0, 5.0);
    e.sell(&[(a, 1.0, 10.0)]);
    let before = e.table_counts();
    let bad = |f: ItemProfitFilter| field_of(e.report.get_item_profit_report(f));
    assert_eq!(bad(pf("2026-11-30", "2026-11-01")), "from_date");
    assert_eq!(bad(pf("nope", TO)), "from_date");
    let mut f = pf(FROM, TO);
    f.sort = sort("bogus", "ASC");
    assert_eq!(bad(f), "sort");
    let mut f = pf(FROM, TO);
    f.sort = sort("name", "UP");
    assert_eq!(bad(f), "sort");
    let mut f = pf(FROM, TO);
    f.page = Some(0);
    assert_eq!(bad(f), "page");
    let mut f = pf(FROM, TO);
    f.page_size = Some(201);
    assert_eq!(bad(f), "limit");
    e.profit(pf(FROM, TO));
    e.speed(sf(FROM, TO));
    assert_eq!(e.table_counts(), before);
}

// ------------------------------------ inventory speed ------------------------------------

#[test]
fn speed_counts_net_units_service_consumption_and_dead_stock() {
    let e = env();
    let paper = e.item("Paper");
    let pen = e.item("Pen");
    let dust = e.item("Dust");
    let empty = e.item("Empty");
    e.lot(paper, 20.0, 2.0);
    e.lot(pen, 10.0, 3.0);
    e.lot(dust, 5.0, 1.0);
    let print = e.svc("Print", Some(2.0), &[(paper, 1.0)]);
    let s = e.sell_on("2026-11-05", &[(print, 3.0, 12.0), (paper, 2.0, 4.0), (pen, 4.0, 5.0)], vec![]);
    e.ret(s, pen, 1.0, StockCondition::Restockable, SettlementType::Refund, vec![]);
    e.sell_on("2026-10-01", &[(dust, 1.0, 2.0)], vec![]); // out of range
    let _ = empty;
    let r = e.speed(sf("2026-11-01", "2026-11-10")); // 10 days
    assert_eq!(r.days, 10);
    let get = |n: &str| r.rows.iter().find(|x| x.name == n).cloned();
    let p = get("Paper").unwrap();
    close(p.units_out, 5.0); // 2 sold + 3 consumed by prints
    close(p.units_per_day, 0.5);
    close(p.stock, 15.0);
    close(p.days_of_cover.unwrap(), 30.0);
    let pn = get("Pen").unwrap();
    close(pn.units_out, 3.0); // returned unit not counted
    let d = get("Dust").unwrap();
    close(d.units_out, 0.0);
    assert_eq!(d.class, SpeedClass::Dead);
    assert_eq!(d.days_of_cover, None);
    assert!(get("Empty").is_none(), "no movement and no stock: omitted");
    assert!(get("Print").is_none(), "services are excluded");
    assert_eq!((r.counts.fast, r.counts.slow, r.counts.dead, r.counts.normal), (1, 1, 1, 0));
    assert_eq!(get("Paper").unwrap().class, SpeedClass::Fast);
    assert_eq!(get("Pen").unwrap().class, SpeedClass::Slow);
}

#[test]
fn speed_period_edges_and_opening_bills() {
    let e = env();
    let a = e.item("Alpha");
    e.lot(a, 10.0, 2.0);
    e.sell_on("2026-11-05", &[(a, 2.0, 4.0)], vec![]);
    e.sell_on("2026-11-06", &[(a, 3.0, 4.0)], vec![]);
    close(e.speed(sf("2026-11-05", "2026-11-05")).rows[0].units_out, 2.0);
    let one = e.speed(sf("2026-11-05", "2026-11-05"));
    assert_eq!(one.days, 1);
    close(e.speed(sf("2026-11-05", "2026-11-06")).rows[0].units_out, 5.0);
    let parties = PartyServiceImpl::new(e.db.clone(), Arc::new(e.db.party_index()));
    parties
        .create_party(CreatePartyPayload {
            name: "Old".into(), phone: None, party_type: PartyType::Customer, notes: None,
            opening_balance: Some(OpeningBalanceInput { amount: 100.0, as_of_date: "2026-11-05".into() }),
        })
        .unwrap();
    close(e.speed(sf("2026-11-05", "2026-11-06")).rows[0].units_out, 5.0);
}

/// `n` items sold 1..=n units; returns classes in rank order (most units first).
fn classes_for(n: usize) -> Vec<SpeedClass> {
    let e = env();
    for i in 1..=n {
        let id = e.item(&format!("Item{i:02}"));
        e.lot(id, 100.0, 1.0);
        e.sell(&[(id, i as f64, 2.0)]);
    }
    let mut r = e.speed(sf(FROM, TO));
    r.rows.sort_by(|a, b| b.units_out.partial_cmp(&a.units_out).unwrap());
    r.rows.iter().map(|x| x.class).collect()
}

#[test]
fn classification_edges() {
    use SpeedClass::*;
    assert_eq!(classes_for(1), [Fast]);
    assert_eq!(classes_for(2), [Fast, Slow]);
    assert_eq!(classes_for(3), [Fast, Normal, Slow]);
    assert_eq!(classes_for(4), [Fast, Normal, Normal, Slow]);
    assert_eq!(classes_for(10), [Fast, Fast, Normal, Normal, Normal, Normal, Normal, Normal, Slow, Slow]);
    let eleven = classes_for(11); // ceil(2.2) = 3
    assert_eq!(eleven.iter().filter(|c| **c == Fast).count(), 3);
    assert_eq!(eleven.iter().filter(|c| **c == Slow).count(), 3);
}

#[test]
fn classification_ties_break_by_name_and_ignore_search() {
    use SpeedClass::*;
    let e = env();
    for n in ["Bolt", "Nut", "Washer"] {
        let id = e.item(n);
        e.lot(id, 10.0, 1.0);
        e.sell(&[(id, 2.0, 3.0)]);
    }
    let r = e.speed(sf(FROM, TO));
    let by = |n: &str| r.rows.iter().find(|x| x.name == n).unwrap().class;
    assert_eq!((by("Bolt"), by("Nut"), by("Washer")), (Fast, Normal, Slow));
    // searching one item keeps its class from the full ranking
    let mut f = sf(FROM, TO);
    f.search = Some("wash".into());
    let s = e.speed(f);
    assert_eq!(s.rows.len(), 1);
    assert_eq!(s.rows[0].class, Slow);
    assert_eq!((s.counts.slow, s.counts.fast), (1, 0));
}

#[test]
fn speed_class_filter_sort_and_paging() {
    let e = env();
    for i in 1..=5 {
        let id = e.item(&format!("Item{i}"));
        e.lot(id, 100.0, 1.0);
        e.sell(&[(id, i as f64, 2.0)]);
    }
    let dead = e.item("Zdead");
    e.lot(dead, 3.0, 1.0);
    let all = e.speed(sf(FROM, TO));
    assert_eq!(all.total_count, 6);
    assert_eq!((all.counts.fast, all.counts.normal, all.counts.slow, all.counts.dead), (1, 3, 1, 1));
    let mut f = sf(FROM, TO);
    f.class = Some("FAST".into());
    let fast = e.speed(f);
    assert_eq!(fast.rows.len(), 1);
    assert_eq!(fast.rows[0].name, "Item5");
    assert_eq!(fast.counts.dead, 1, "counts ignore the class tab");
    let mut f = sf(FROM, TO);
    f.class = Some("DEAD".into());
    assert_eq!(e.speed(f).rows[0].name, "Zdead");
    let mut f = sf(FROM, TO);
    f.class = Some("ALL".into());
    assert_eq!(e.speed(f).total_count, 6);

    let mut f = sf(FROM, TO);
    f.sort = sort("name", "ASC");
    f.page_size = Some(4);
    f.page = Some(2);
    let p = e.speed(f);
    assert_eq!(p.rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["Item5", "Zdead"]);
    assert_eq!((p.total_count, p.page), (6, 2));

    let mut f = sf(FROM, TO);
    f.sort = sort("cover", "ASC");
    let c = e.speed(f);
    assert_eq!(c.rows.last().unwrap().name, "Zdead", "no cover sorts last");
    for field in ["stock", "units_out", "per_day", "class"] {
        let mut f = sf(FROM, TO);
        f.sort = sort(field, "DESC");
        assert_eq!(e.speed(f).total_count, 6, "{field}");
    }
}

#[test]
fn speed_validation_and_no_writes() {
    let e = env();
    let a = e.item("Alpha");
    e.lot(a, 5.0, 5.0);
    let bad = |f: InventorySpeedFilter| field_of(e.report.get_inventory_speed_report(f));
    assert_eq!(bad(sf(TO, FROM)), "from_date");
    let mut f = sf(FROM, TO);
    f.class = Some("SPEEDY".into());
    assert_eq!(bad(f), "class");
    let mut f = sf(FROM, TO);
    f.sort = sort("name", "sideways");
    assert_eq!(bad(f), "sort");
    let mut f = sf(FROM, TO);
    f.page_size = Some(0);
    assert_eq!(bad(f), "limit");
}

// ------------------------------------ get_item (exact row click) ------------------------------------

#[test]
fn get_item_found_service_can_make_not_found_and_no_writes() {
    use book_keeper::services::contracts::item_service::ItemService;
    let e = env();
    let paper = e.item("Paper");
    e.lot(paper, 10.0, 2.0);
    let print = e.svc("Print", Some(3.0), &[(paper, 2.0)]);
    let before = e.table_counts();
    let p = e.items.get_item(paper).unwrap();
    assert_eq!((p.item_id, p.name.as_str(), p.item_type), (paper, "Paper", ItemType::Physical));
    close(p.current_stock, 10.0);
    assert_eq!(p.can_make, None);
    let s = e.items.get_item(print).unwrap();
    assert_eq!(s.item_type, ItemType::Service);
    assert_eq!(s.service_cost, Some(3.0));
    assert_eq!(s.can_make, Some(5));
    match e.items.get_item(9999) {
        Err(AppError::NotFound { entity, .. }) => assert_eq!(entity, "Item"),
        other => panic!("expected NotFound, got {other:?}"),
    }
    assert_eq!(e.table_counts(), before);
}

// ------------------------------------ query plans ------------------------------------

fn plan(e: &E, sql: &str, nparams: usize) -> String {
    let c = e.db.pool.get().unwrap();
    let mut stmt = c.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
    let params: Vec<rusqlite::types::Value> = (1..=nparams)
        .map(|i| match i {
            1 => "2026-11-01".to_string().into(),
            2 => "2026-11-30".to_string().into(),
            _ => rusqlite::types::Value::Null,
        })
        .collect();
    let rows: Vec<String> = stmt
        .query_map(rusqlite::params_from_iter(params), |r| r.get::<_, String>(3))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    rows.join("\n")
}

#[test]
fn query_plans_use_the_date_and_line_indexes() {
    let e = env();
    for (name, sql, n) in [
        ("profit", profit_page_sql("profit", true), 5usize),
        ("speed", speed_page_sql("units_out", true), 7usize),
    ] {
        let p = plan(&e, &sql, n);
        assert!(p.contains("idx_sale_tx_date"), "{name}: {p}");
        assert!(p.contains("idx_sale_items_tx"), "{name}: {p}");
        assert!(p.contains("idx_sale_alloc_sale_item"), "{name}: {p}");
        for line in p.lines() {
            let t = line.trim();
            for table in ["si", "sale_items", "a", "sale_stock_allocations", "s", "sale_transactions", "pi", "purchase_items"] {
                let scan = format!("SCAN {table}");
                assert!(t != scan && !t.starts_with(&format!("{scan} ")), "{name} scans {table}: {p}");
            }
        }
    }
}
