// tests/item_history_test.rs
//! Test Suite: FR-23 item sales / purchases history (schema v9) on real SQLite: ordering, one row
//! per bill, paging, source filter, date edits, deletes, the line-date invariant and the query plans.

mod common;

use book_keeper::core::dtos::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::data::sqlite::repositories::purchase_repo::{ITEM_PURCHASES_COUNT_SQL, ITEM_PURCHASES_PAGE_SQL};
use book_keeper::data::sqlite::repositories::sale_repo::{ITEM_SALES_COUNT_SQL, ITEM_SALES_PAGE_SQL};
use book_keeper::services::contracts::inventory_service::InventoryService;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::sale_service::SaleService;
use common::invariants::*;
use common::ledger::*;
use common::*;

type Inv = InventoryServiceImpl<FifoStrategy>;

struct Env {
    db: TestDb,
    inv: Inv,
    sales: SaleServiceImpl<Inv, PaymentServiceImpl>,
    purchases: PurchaseServiceImpl<Inv, PaymentServiceImpl>,
}

fn env() -> Env {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    Env {
        sales: SaleServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        purchases: PurchaseServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        inv,
        db,
    }
}

impl Env {
    fn item(&self, id: i64) {
        seed_stocked_item(&self.db, &self.inv, id, 0.0, 0.0, 100.0);
    }
    fn buy(&self, date: &str, lines: &[(i64, f64, f64)]) -> i64 {
        let l: Vec<_> = lines.iter().map(|&(i, q, c)| (i, None, q, c)).collect();
        let mut p = purchase_payload(None, PaymentStatus::Paid, lines.iter().map(|x| x.1 * x.2).sum(), &l);
        p.transaction_date = date.to_string();
        self.purchases.create_purchase(p).expect("purchase").purchase_id
    }
    fn sell(&self, date: &str, lines: &[(i64, f64, f64)]) -> i64 {
        let l: Vec<_> = lines.iter().map(|&(i, q, c)| (i, None, q, c)).collect();
        let mut p = sale_payload(None, PaymentStatus::Paid, lines.iter().map(|x| x.1 * x.2).sum(), &l);
        p.transaction_date = date.to_string();
        self.sales.create_sale(p).expect("sale").sale_id
    }
    fn sales_ids(&self, item: i64, page: usize, limit: usize) -> Vec<i64> {
        self.inv.get_item_sales(item, Some(page), Some(limit)).unwrap().items.iter().map(|b| b.bill_id).collect()
    }
    fn purchase_ids(&self, item: i64) -> Vec<i64> {
        self.inv.get_item_purchases(item, None, None).unwrap().items.iter().map(|b| b.bill_id).collect()
    }
}

#[test]
fn sales_are_ordered_by_date_then_id_and_a_backdated_bill_lands_in_date_order() {
    let e = env();
    e.item(1);
    e.buy("2026-01-01", &[(1, 100.0, 5.0)]);
    let a = e.sell("2026-03-10", &[(1, 1.0, 10.0)]);
    let b = e.sell("2026-03-10", &[(1, 1.0, 10.0)]);
    let c = e.sell("2026-02-01", &[(1, 1.0, 10.0)]); // back-dated, created last
    let d = e.sell("2026-04-01", &[(1, 1.0, 10.0)]);
    assert_eq!(e.sales_ids(1, 1, 10), vec![d, b, a, c]);
    let page = e.inv.get_item_sales(1, None, None).unwrap();
    assert_eq!((page.total, page.page, page.limit), (4, 1, 10));
    assert_eq!(page.items[0].transaction_date, "2026-04-01");
    assert_eq!(page.items[0].party_name, None);
    assert_line_date_invariant(&e.db);
}

#[test]
fn an_item_on_two_lines_of_one_bill_gives_one_row_and_paging_totals_are_distinct_bills() {
    let e = env();
    e.item(1);
    e.item(2);
    e.buy("2026-01-01", &[(1, 100.0, 5.0), (2, 100.0, 5.0)]);
    let both = e.sell("2026-02-01", &[(1, 1.0, 10.0), (1, 2.0, 10.0), (2, 1.0, 10.0)]);
    let mut ids = vec![both];
    for day in 2..=6 {
        ids.push(e.sell(&format!("2026-02-{day:02}"), &[(1, 1.0, 10.0)]));
    }
    let p1 = e.inv.get_item_sales(1, Some(1), Some(4)).unwrap();
    assert_eq!((p1.total, p1.items.len()), (6, 4));
    let p2 = e.inv.get_item_sales(1, Some(2), Some(4)).unwrap();
    assert_eq!((p2.total, p2.items.len()), (6, 2));
    let all: Vec<i64> = p1.items.iter().chain(p2.items.iter()).map(|b| b.bill_id).collect();
    assert_eq!(all.iter().filter(|&&i| i == both).count(), 1);
    assert_eq!(e.inv.get_item_sales(1, Some(3), Some(4)).unwrap().items.len(), 0);
    assert_eq!(e.inv.get_item_sales(2, None, None).unwrap().total, 1);
}

#[test]
fn purchases_tab_excludes_opening_adjustment_and_return_lots() {
    let e = env();
    e.item(1);
    let p = e.buy("2026-02-01", &[(1, 10.0, 5.0)]);
    let mut tx = e.db.begin_transaction().unwrap();
    e.inv
        .create_source_lot(&mut *tx, LotSource::Opening, "2026-03-01", 1, 5.0, Some(4.0), None)
        .unwrap();
    tx.commit().unwrap();
    e.inv
        .create_adjustment(StockAdjustmentPayload {
            adjustment_date: "2026-03-02".into(),
            item_id: 1,
            quantity: 3.0,
            reason: AdjustmentReason::UnrecordedStock,
            unit_cost: Some(4.0),
            notes: None,
        })
        .unwrap();
    let page = e.inv.get_item_purchases(1, None, None).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(e.purchase_ids(1), vec![p]);
    assert_eq!(e.inv.get_item_lots(1).unwrap().len(), 3, "all sources still show as lots");
}

#[test]
fn editing_a_bill_date_reorders_and_keeps_line_dates_in_sync_and_delete_removes_the_row() {
    let e = env();
    e.item(1);
    e.buy("2026-01-01", &[(1, 100.0, 5.0)]);
    let a = e.sell("2026-02-01", &[(1, 1.0, 10.0)]);
    let b = e.sell("2026-03-01", &[(1, 1.0, 10.0)]);
    assert_eq!(e.sales_ids(1, 1, 10), vec![b, a]);
    let mut p = sale_payload(None, PaymentStatus::Paid, 10.0, &[(1, None, 1.0, 10.0)]);
    p.transaction_date = "2026-04-01".into();
    e.sales.update_sale(a, p).unwrap();
    assert_eq!(e.sales_ids(1, 1, 10), vec![a, b]);
    assert_line_date_invariant(&e.db);
    e.sales.delete_sale(a).unwrap();
    assert_eq!(e.sales_ids(1, 1, 10), vec![b]);
    assert_eq!(e.inv.get_item_sales(1, None, None).unwrap().total, 1);

    // purchase date edit with a lot-id-preserving line
    let pa = e.buy("2026-05-01", &[(1, 5.0, 5.0)]);
    let pb = e.buy("2026-06-01", &[(1, 5.0, 5.0)]);
    assert_eq!(e.purchase_ids(1)[..2], [pb, pa]);
    let lot = e.db.begin_read().unwrap().purchases().list_items_by_transaction_id(pa).unwrap()[0].id;
    let mut up = purchase_payload(None, PaymentStatus::Paid, 25.0, &[(1, None, 5.0, 5.0)]);
    up.items[0].purchase_item_id = Some(lot);
    up.transaction_date = "2026-07-01".into();
    e.purchases.update_purchase(pa, up).unwrap();
    assert_eq!(e.purchase_ids(1)[..2], [pa, pb]);
    assert_line_date_invariant(&e.db);
    e.purchases.delete_purchase(pb).unwrap();
    assert!(!e.purchase_ids(1).contains(&pb));
}

#[test]
fn unknown_item_is_not_found_and_limit_is_capped_at_50() {
    let e = env();
    assert!(matches!(e.inv.get_item_sales(99, None, None), Err(AppError::NotFound { .. })));
    assert!(matches!(e.inv.get_item_purchases(99, None, None), Err(AppError::NotFound { .. })));
    e.item(1);
    assert!(e.inv.get_item_sales(1, Some(1), Some(50)).is_ok());
    assert!(e.inv.get_item_sales(1, Some(1), Some(51)).is_err());
    assert!(e.inv.get_item_purchases(1, Some(1), Some(0)).is_err());
    assert!(e.inv.get_item_purchases(1, Some(0), None).is_err());
}

// ---------------------------------------------------------------- query plans

fn plan(conn: &rusqlite::Connection, sql: &str) -> String {
    let mut st = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
    let rows: Vec<String> = st
        .query_map(rusqlite::params![1_i64, 10_i64, 0_i64], |r| r.get::<_, String>(3))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    rows.join("\n")
}

fn plan_params(conn: &rusqlite::Connection, sql: &str) -> String {
    // Count placeholders: the COUNT queries take one parameter, the page queries three.
    if sql.contains("?3") {
        plan(conn, sql)
    } else {
        let mut st = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
        let rows: Vec<String> = st.query_map([1_i64], |r| r.get::<_, String>(3)).unwrap().map(|x| x.unwrap()).collect();
        rows.join("\n")
    }
}

#[test]
fn item_history_queries_use_the_v9_indexes_without_a_temp_btree() {
    let e = env();
    let conn = e.db.pool.get().unwrap();
    for (sql, index) in [
        (ITEM_SALES_PAGE_SQL, "idx_sale_items_item_date"),
        (ITEM_SALES_COUNT_SQL, "idx_sale_items_item_date"),
        (ITEM_PURCHASES_PAGE_SQL, "idx_purchase_items_item_date"),
        (ITEM_PURCHASES_COUNT_SQL, "idx_purchase_items_item_date"),
    ] {
        let p = plan_params(&conn, sql);
        assert!(p.contains(index), "plan must use {index}:\n{p}\n-- {sql}");
        assert!(!p.contains("TEMP B-TREE"), "no temp b-tree allowed:\n{p}\n-- {sql}");
    }
}

/// `cargo test -- --ignored item_history_benchmark --nocapture`
#[test]
#[ignore]
fn item_history_benchmark_20k_lines_one_item() {
    let e = env();
    e.item(1);
    {
        let mut c = e.db.pool.get().unwrap();
        let tx = c.transaction().unwrap();
        for i in 1..=20_000_i64 {
            let date = format!("2025-{:02}-{:02}", (i % 12) + 1, (i % 28) + 1);
            tx.execute(
                "INSERT INTO sale_transactions (id, transaction_code, transaction_date, payment_status, total_amount, paid_amount)
                 VALUES (?1, ?2, ?3, 'PAID', 10, 10)",
                rusqlite::params![i, format!("SAL-B{i}"), date],
            )
            .unwrap();
            tx.execute(
                "INSERT INTO sale_items (sale_transaction_id, item_id, item_name, pack_quantity, quantity, unit_price, total_amount, transaction_date)
                 VALUES (?1, 1, 'Item 1', 1, 1, 10, 10, ?2)",
                rusqlite::params![i, date],
            )
            .unwrap();
        }
        tx.commit().unwrap();
    }
    for page in [1_usize, 200] {
        let t = std::time::Instant::now();
        let p = e.inv.get_item_sales(1, Some(page), Some(10)).unwrap();
        println!("page {page}: {} rows, total {}, {:?}", p.items.len(), p.total, t.elapsed());
        assert_eq!(p.items.len(), 10);
    }
}
