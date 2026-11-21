// tests/list_test.rs
//! Test Suite: Lists, pagination & history/detail views (fix-phase F5 / T8)
//! Test Cases: TC-LIST-01 through TC-LIST-09
//! Covers the one `Paginated` shape, SQL filtering/sorting/paging, history rows, catalog, party
//! rows, dues summary, returns and adjustments lists. Production services on real SQLite.

mod common;

use std::sync::Arc;

use common::fixtures::*;
use common::ledger::*;
use common::*;
use book_keeper::core::dtos::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::accounting_service::AccountingService;
use book_keeper::services::contracts::inventory_service::InventoryService;
use book_keeper::services::contracts::item_service::ItemService;
use book_keeper::services::contracts::party_service::PartyService;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::return_service::ReturnService;
use book_keeper::services::contracts::sale_service::SaleService;

type Inv = InventoryServiceImpl<FifoStrategy>;
type Acct = AccountingServiceImpl<PaymentServiceImpl>;

struct Env {
    db: TestDb,
    inv: Inv,
    sales: SaleServiceImpl<Inv, PaymentServiceImpl>,
    purchases: PurchaseServiceImpl<Inv, PaymentServiceImpl>,
    rets: ReturnServiceImpl<Inv, PaymentServiceImpl, Acct>,
    acct: Acct,
    items: ItemServiceImpl<Inv>,
    parties: PartyServiceImpl,
}

fn env() -> Env {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let acct = AccountingServiceImpl::new(PaymentServiceImpl, db.clone());
    Env {
        sales: SaleServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        purchases: PurchaseServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        rets: ReturnServiceImpl::new(inv.clone(), PaymentServiceImpl, acct.clone(), db.clone()),
        items: ItemServiceImpl::new(db.clone(), Arc::new(db.indexer()), inv.clone()),
        parties: PartyServiceImpl::new(db.clone()),
        acct,
        inv,
        db,
    }
}

fn field_of<T: std::fmt::Debug>(r: Result<T, AppError>) -> String {
    match r {
        Err(AppError::ValidationError { field, .. }) => field,
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

/// Seeds a sale header (+ `lines` sale lines) with a name snapshot.
fn put_sale(db: &TestDb, id: i64, party: Option<(i64, &str)>, total: f64, paid: f64, date: &str, lines: i64) {
    let mut tx = db.begin_transaction().unwrap();
    let mut s = create_test_sale_tx(id, &format!("SAL-{}", 1000 + id), party.map(|p| p.0), total, paid, date);
    s.party_name = party.map(|p| p.1.to_string());
    tx.sales().create_transaction(&s).unwrap();
    for n in 0..lines {
        ensure_item(&mut *tx, 1);
        tx.sales().create_sale_item(&create_test_sale_item(id * 100 + n, id, 1, None, 1.0, 1.0, 1.0)).unwrap();
    }
    tx.commit().unwrap();
}

fn put_purchase(db: &TestDb, id: i64, source: LotSource, party: Option<(i64, &str)>, inv_ref: Option<&str>, total: f64, paid: f64, date: &str) {
    let mut tx = db.begin_transaction().unwrap();
    let code = match source {
        LotSource::Purchase => format!("PUR-{}", 1000 + id),
        LotSource::Adjustment => format!("ADJ-{id:04}"),
        LotSource::Opening => format!("OPN-{id:04}"),
        LotSource::Return => format!("RTL-{id:04}"),
    };
    let mut p = create_test_purchase_tx(id, &code, party.map(|p| p.0), total, paid, date);
    p.source = source;
    p.party_name = party.map(|p| p.1.to_string());
    p.supplier_invoice_ref = inv_ref.map(str::to_string);
    tx.purchases().create_transaction(&p).unwrap();
    tx.commit().unwrap();
}

fn sale_filter() -> SalesHistoryFilter {
    SalesHistoryFilter::default()
}

fn ids<T>(rows: &[T], f: impl Fn(&T) -> i64) -> Vec<i64> {
    rows.iter().map(f).collect()
}

// ---- TC-LIST-01: pagination contract and input validation -------------------------------------

#[test]
fn test_tc_list_01_pagination_shape_and_validation() {
    let e = env();
    seed_party(&e.db, 1, "Asha", PartyType::Customer);
    for i in 1..=5 {
        put_sale(&e.db, i, Some((1, "Asha")), 10.0, 10.0, "2026-11-10", 1);
    }
    let page = e.sales.get_sales_history(SalesHistoryFilter { page: Some(2), limit: Some(2), ..sale_filter() }).unwrap();
    assert_eq!((page.items.len(), page.total, page.page, page.limit), (2, 5, 2, 2));
    // Past the end: empty items, total still reported.
    let past = e.sales.get_sales_history(SalesHistoryFilter { page: Some(9), limit: Some(2), ..sale_filter() }).unwrap();
    assert_eq!((past.items.len(), past.total), (0, 5));
    // Defaults.
    let d = e.sales.get_sales_history(sale_filter()).unwrap();
    assert_eq!((d.page, d.limit), (1, 50));
    // JSON shape is exactly {items,total,page,limit}.
    let v = serde_json::to_value(&d).unwrap();
    let mut keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, vec!["items", "limit", "page", "total"]);

    // Rejected inputs (never clamped), on every list.
    assert_eq!(field_of(e.sales.get_sales_history(SalesHistoryFilter { page: Some(0), ..sale_filter() })), "page");
    assert_eq!(field_of(e.sales.get_sales_history(SalesHistoryFilter { limit: Some(0), ..sale_filter() })), "limit");
    assert_eq!(field_of(e.sales.get_sales_history(SalesHistoryFilter { limit: Some(201), ..sale_filter() })), "limit");
    assert_eq!(field_of(e.sales.get_sales_history(SalesHistoryFilter { page: Some(usize::MAX), limit: Some(200), ..sale_filter() })), "page");
    assert!(e.sales.get_sales_history(SalesHistoryFilter { limit: Some(200), ..sale_filter() }).is_ok());
    assert_eq!(field_of(e.purchases.get_purchases_history(PurchaseHistoryFilter { limit: Some(500), ..Default::default() })), "limit");
    assert_eq!(field_of(e.items.get_catalog(CatalogFilter { page: Some(0), ..Default::default() })), "page");
    assert_eq!(field_of(e.rets.list_returns(ReturnsFilter { limit: Some(0), ..Default::default() })), "limit");
    assert_eq!(field_of(e.inv.list_adjustments(None, Some(1), Some(1000))), "limit");
    // Date format validation.
    assert_eq!(field_of(e.sales.get_sales_history(SalesHistoryFilter { start_date: Some("21/11/2026".into()), ..sale_filter() })), "start_date");
    assert_eq!(field_of(e.sales.get_sales_history(SalesHistoryFilter { end_date: Some("2026-2-3".into()), ..sale_filter() })), "end_date");
    assert_eq!(
        field_of(e.sales.get_sales_history(SalesHistoryFilter { start_date: Some("2026-11-12".into()), end_date: Some("2026-11-11".into()), ..sale_filter() })),
        "start_date"
    );
    // "ALL" is accepted as a status; JSON accepts "ALL".
    let f: SalesHistoryFilter = serde_json::from_value(serde_json::json!({"payment_status": "ALL", "search_query": "x"})).unwrap();
    assert_eq!(f.payment_status, Some(PaymentStatusFilter::All));
}

// ---- TC-LIST-02: sales filters ----------------------------------------------------------------

#[test]
fn test_tc_list_02_sales_filters() {
    let e = env();
    seed_party(&e.db, 1, "Ramesh Traders", PartyType::Customer);
    seed_party(&e.db, 2, "Sita 100%_Store", PartyType::Customer);
    put_sale(&e.db, 1, Some((1, "Ramesh Traders")), 100.0, 0.0, "2026-11-01", 1); // UNPAID
    put_sale(&e.db, 2, Some((1, "Ramesh Traders")), 100.0, 40.0, "2026-11-05", 1); // PARTIAL
    put_sale(&e.db, 3, Some((2, "Sita 100%_Store")), 100.0, 100.0, "2026-11-10", 1); // PAID
    put_sale(&e.db, 4, None, 50.0, 50.0, "2026-11-10", 1); // walk-in PAID

    let run = |f: SalesHistoryFilter| e.sales.get_sales_history(f).unwrap();
    // Date range is inclusive on both ends.
    let r = run(SalesHistoryFilter { start_date: Some("2026-11-05".into()), end_date: Some("2026-11-10".into()), ..sale_filter() });
    assert_eq!((r.total, ids(&r.items, |x| x.id)), (3, vec![4, 3, 2]));
    let r = run(SalesHistoryFilter { start_date: Some("2026-11-10".into()), end_date: Some("2026-11-10".into()), ..sale_filter() });
    assert_eq!(r.total, 2);
    // Status.
    for (st, want) in [
        (PaymentStatusFilter::All, 4),
        (PaymentStatusFilter::Paid, 2),
        (PaymentStatusFilter::Partial, 1),
        (PaymentStatusFilter::Unpaid, 1),
    ] {
        assert_eq!(run(SalesHistoryFilter { payment_status: Some(st), ..sale_filter() }).total, want, "{st:?}");
    }
    // Party.
    let r = run(SalesHistoryFilter { party_id: Some(1), ..sale_filter() });
    assert_eq!(ids(&r.items, |x| x.id), vec![2, 1]);
    // Search: code, party name (case-insensitive), LIKE metacharacters are literal.
    assert_eq!(ids(&run(SalesHistoryFilter { search_query: Some("sal-1003".into()), ..sale_filter() }).items, |x| x.id), vec![3]);
    assert_eq!(run(SalesHistoryFilter { search_query: Some("ramesh".into()), ..sale_filter() }).total, 2);
    assert_eq!(ids(&run(SalesHistoryFilter { search_query: Some("100%_".into()), ..sale_filter() }).items, |x| x.id), vec![3]);
    assert_eq!(run(SalesHistoryFilter { search_query: Some("%".into()), ..sale_filter() }).total, 1, "% is literal");
    assert_eq!(run(SalesHistoryFilter { search_query: Some("   ".into()), ..sale_filter() }).total, 4, "blank search = no filter");
    // Combined.
    let r = run(SalesHistoryFilter { search_query: Some("ramesh".into()), payment_status: Some(PaymentStatusFilter::Partial), start_date: Some("2026-11-02".into()), ..sale_filter() });
    assert_eq!(ids(&r.items, |x| x.id), vec![2]);
}

// ---- TC-LIST-03: ordering and more than 50 rows ------------------------------------------------

#[test]
fn test_tc_list_03_newest_first_and_page_two() {
    let e = env();
    // Insert in id order, but the last-inserted bill is backdated.
    put_sale(&e.db, 1, None, 10.0, 10.0, "2026-11-10", 1);
    put_sale(&e.db, 2, None, 10.0, 10.0, "2026-11-12", 1);
    put_sale(&e.db, 3, None, 10.0, 10.0, "2026-11-12", 1);
    put_sale(&e.db, 4, None, 10.0, 10.0, "2026-11-01", 1);
    let r = e.sales.get_sales_history(sale_filter()).unwrap();
    assert_eq!(ids(&r.items, |x| x.id), vec![3, 2, 1, 4], "transaction_date DESC, id DESC");

    for i in 5..=64 {
        put_sale(&e.db, i, None, 10.0, 10.0, "2026-10-01", 0);
    }
    let p1 = e.sales.get_sales_history(sale_filter()).unwrap();
    assert_eq!((p1.items.len(), p1.total), (50, 64));
    let p2 = e.sales.get_sales_history(SalesHistoryFilter { page: Some(2), ..sale_filter() }).unwrap();
    assert_eq!((p2.items.len(), p2.total, p2.page), (14, 64, 2));
    let mut all: Vec<i64> = ids(&p1.items, |x| x.id);
    all.extend(ids(&p2.items, |x| x.id));
    all.sort();
    all.dedup();
    assert_eq!(all.len(), 64, "pages are disjoint and complete");
}

// ---- TC-LIST-04: purchases list ----------------------------------------------------------------

#[test]
fn test_tc_list_04_purchases_only_purchase_source_and_invoice_search() {
    let e = env();
    seed_party(&e.db, 1, "Agro Supplies", PartyType::Vendor);
    put_purchase(&e.db, 1, LotSource::Purchase, Some((1, "Agro Supplies")), Some("INV-77/A"), 500.0, 0.0, "2026-11-03");
    put_purchase(&e.db, 2, LotSource::Purchase, None, None, 80.0, 80.0, "2026-11-04");
    put_purchase(&e.db, 3, LotSource::Adjustment, Some((1, "Agro Supplies")), None, 10.0, 0.0, "2026-11-05");
    put_purchase(&e.db, 4, LotSource::Opening, None, None, 10.0, 10.0, "2026-11-05");
    put_purchase(&e.db, 5, LotSource::Return, None, None, 10.0, 10.0, "2026-11-06");

    let run = |f: PurchaseHistoryFilter| e.purchases.get_purchases_history(f).unwrap();
    let all = run(Default::default());
    assert_eq!((all.total, ids(&all.items, |x| x.id)), (2, vec![2, 1]), "ADJ/OPN/RTL excluded");
    assert_eq!(all.items[1].supplier_invoice_ref.as_deref(), Some("INV-77/A"));
    // Search by supplier invoice ref, code and party name.
    assert_eq!(ids(&run(PurchaseHistoryFilter { search_query: Some("inv-77".into()), ..Default::default() }).items, |x| x.id), vec![1]);
    assert_eq!(ids(&run(PurchaseHistoryFilter { search_query: Some("PUR-1002".into()), ..Default::default() }).items, |x| x.id), vec![2]);
    assert_eq!(ids(&run(PurchaseHistoryFilter { search_query: Some("agro".into()), ..Default::default() }).items, |x| x.id), vec![1]);
    // A non-purchase code never matches.
    assert_eq!(run(PurchaseHistoryFilter { search_query: Some("ADJ-".into()), ..Default::default() }).total, 0);
    // Status, party and dates.
    assert_eq!(run(PurchaseHistoryFilter { payment_status: Some(PaymentStatusFilter::Unpaid), ..Default::default() }).total, 1);
    assert_eq!(run(PurchaseHistoryFilter { party_id: Some(1), ..Default::default() }).total, 1);
    let r = run(PurchaseHistoryFilter { start_date: Some("2026-11-04".into()), end_date: Some("2026-11-04".into()), ..Default::default() });
    assert_eq!(ids(&r.items, |x| x.id), vec![2]);
}

// ---- TC-LIST-05: history rows and detail views -------------------------------------------------

#[test]
fn test_tc_list_05_history_row_fields_and_detail_views() {
    let e = env();
    seed_party(&e.db, 1, "Old Name", PartyType::Customer);
    seed_party(&e.db, 2, "Vend", PartyType::Vendor);
    seed_stocked_item(&e.db, &e.inv, 1, 50.0, 5.0, 10.0);
    seed_stocked_item(&e.db, &e.inv, 2, 50.0, 5.0, 10.0);
    let sale = e.sales.create_sale(sale_payload(Some(1), PaymentStatus::Partial, 10.0, &[(1, None, 5.0, 10.0), (2, None, 3.0, 10.0)])).unwrap();
    let mut pp = purchase_payload(Some(2), PaymentStatus::Unpaid, 0.0, &[(1, None, 4.0, 6.0), (2, None, 2.0, 7.0)]);
    pp.supplier_invoice_ref = Some("SUP-9".into());
    let pur = e.purchases.create_purchase(pp).unwrap();

    // Rename the party afterwards: history keeps the snapshot (DD-30).
    {
        let mut tx = e.db.begin_transaction().unwrap();
        let mut p = tx.parties().find_by_id(1).unwrap().unwrap();
        p.name = "New Name".into();
        tx.parties().update(&p).unwrap();
        tx.commit().unwrap();
    }
    let row = &e.sales.get_sales_history(sale_filter()).unwrap().items[0];
    assert_eq!(row.id, sale.sale_id);
    assert_eq!((row.party_name.as_deref(), row.party_id, row.line_count), (Some("Old Name"), Some(1), 2));
    assert_eq!((row.total_amount, row.paid_amount, row.balance_due, row.payment_status), (80.0, 10.0, 70.0 + 0.0, PaymentStatus::Partial).clone());
    assert!(!row.created_at.is_empty());
    assert_eq!(e.sales.get_sales_history(SalesHistoryFilter { search_query: Some("old name".into()), ..sale_filter() }).unwrap().total, 1);
    assert_eq!(e.sales.get_sales_history(SalesHistoryFilter { search_query: Some("new name".into()), ..sale_filter() }).unwrap().total, 0);

    let prow = &e.purchases.get_purchases_history(Default::default()).unwrap().items[0];
    assert_eq!((prow.line_count, prow.supplier_invoice_ref.as_deref(), prow.party_name.as_deref()), (2, Some("SUP-9"), Some("Vend")));

    // Sale detail: snapshot lines, returned qty per line, FIFO breakdown, ledger rows.
    let d0 = e.sales.get_sale_details(sale.sale_id).unwrap();
    assert!(d0.items.iter().all(|l| l.returned_quantity == 0.0));
    let line1 = d0.items[0].sale_item.id;
    let r = CustomerReturnPayload {
        original_sale_id: Some(sale.sale_id), party_id: Some(1), return_date: DAY.to_string(),
        settlement_type: SettlementType::Refund, reason: None, notes: None, replacement_items: vec![],
        items: vec![CustomerReturnItemPayload { item_id: 1, sale_item_id: Some(line1), quantity: 2.0, unit_price: 10.0, condition: StockCondition::Restockable, unit_cost_override: None }],
    };
    e.rets.create_customer_return(r).unwrap();
    let d = e.sales.get_sale_details(sale.sale_id).unwrap();
    assert_eq!(d.items.len(), 2);
    assert_eq!(d.items[0].returned_quantity, 2.0);
    assert_eq!(d.items[1].returned_quantity, 0.0);
    assert_eq!((d.items[0].item_name.as_str(), d.items[0].sale_item.unit_price, d.items[0].sale_item.quantity), ("Item 1", 10.0, 5.0));
    assert!(!d.items[0].allocations.is_empty(), "FIFO cost breakdown present");
    assert!(!d.settlements.is_empty(), "ledger rows present");
    assert_eq!(d.sale.party_name.as_deref(), Some("Old Name"));

    // Purchase detail: invoice ref, stored unit cost, remaining qty per lot.
    let pd = e.purchases.get_purchase_details(pur.purchase_id).unwrap();
    assert_eq!(pd.purchase.supplier_invoice_ref.as_deref(), Some("SUP-9"));
    assert_eq!((pd.items[0].purchase_item.unit_cost, pd.items[0].purchase_item.remaining_quantity, pd.items[0].item_name.as_str()), (6.0, 4.0, "Item 1"));
}

// ---- TC-LIST-06: catalog -----------------------------------------------------------------------

fn new_item(e: &Env, name: &str, cat: Option<&str>, stock: f64, bundles: &[(&str, f64)]) -> i64 {
    let id = e
        .items
        .create_item(CreateItemPayload {
            name: name.into(), category: cat.map(str::to_string), barcode: None, base_unit: "PCS".into(), mrp: None,
            default_sale_price: 10.0, initial_stock: if stock > 0.0 { Some(stock) } else { None },
            initial_cost: if stock > 0.0 { Some(5.0) } else { None },
            bundles: Some(bundles.iter().map(|(n, u)| CreateBundlePayload {
                item_id: None, bundle_name: n.to_string(), units_per_bundle: *u, mrp: None, default_bundle_price: None, barcode: None,
            }).collect()),
        })
        .unwrap();
    id
}

#[test]
fn test_tc_list_06_catalog_filters_search_and_bundles() {
    let e = env();
    let pen = new_item(&e, "Blue Pen", Some("Stationery"), 100.0, &[("Box", 10.0), ("Carton", 100.0)]);
    let pencil = new_item(&e, "Pencil", Some("Stationery"), 2.0, &[]); // low stock
    let _soap = new_item(&e, "Pen Holder Soap", Some("Grocery"), 1.0, &[]); // low stock, other category
    let _rice = new_item(&e, "Rice", None, 50.0, &[]);

    let cat = |f: CatalogFilter| e.items.get_catalog(f).unwrap();
    let all = cat(Default::default());
    assert_eq!((all.total, all.page, all.limit), (4, 1, 50));
    let names: Vec<_> = all.items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, vec!["Blue Pen", "Pen Holder Soap", "Pencil", "Rice"]);
    let bp = all.items.iter().find(|i| i.item_id == pen).unwrap();
    assert_eq!(bp.bundles.len(), 2, "bundles come with the page");
    assert_eq!(bp.bundles[0].units_per_bundle, 10.0);

    let s = cat(CatalogFilter { category: Some("stationery".into()), ..Default::default() });
    assert_eq!(s.total, 2);
    assert_eq!(cat(CatalogFilter { low_stock_only: Some(true), ..Default::default() }).total, 2);
    assert_eq!(cat(CatalogFilter { category: Some("Stationery".into()), low_stock_only: Some(true), ..Default::default() }).items[0].item_id, pencil);
    // Paging in SQL.
    let p = cat(CatalogFilter { limit: Some(3), page: Some(2), ..Default::default() });
    assert_eq!((p.items.len(), p.total), (1, 4));

    // Search goes through the indexer and the other filters still apply.
    let q = cat(CatalogFilter { search: Some("pen".into()), ..Default::default() });
    assert_eq!(q.total, 3, "Blue Pen, Pencil, Pen Holder Soap");
    let q = cat(CatalogFilter { search: Some("pen".into()), category: Some("Stationery".into()), ..Default::default() });
    assert_eq!(q.total, 2);
    let q = cat(CatalogFilter { search: Some("pen".into()), low_stock_only: Some(true), ..Default::default() });
    assert_eq!(q.total, 2, "Pencil + Pen Holder Soap");
    let q = cat(CatalogFilter { search: Some("pen".into()), category: Some("Stationery".into()), low_stock_only: Some(true), ..Default::default() });
    assert_eq!((q.total, q.items[0].item_id), (1, pencil));
    let q = cat(CatalogFilter { search: Some("pen".into()), limit: Some(2), page: Some(2), ..Default::default() });
    assert_eq!((q.items.len(), q.total), (1, 3));
    let bp = cat(CatalogFilter { search: Some("blue".into()), ..Default::default() });
    assert_eq!(bp.items[0].bundles.len(), 2);
    assert_eq!(cat(CatalogFilter { search: Some("zzz".into()), ..Default::default() }).total, 0);

    // Service-level typeahead (default limit) and categories.
    assert_eq!(e.items.search_items_fts("pen", None).unwrap().len(), 3);
    assert_eq!(e.items.search_items_fts("pen", Some(1)).unwrap().len(), 1);
    assert_eq!(e.items.list_categories().unwrap(), vec!["Grocery".to_string(), "Stationery".to_string()]);
}

// ---- TC-LIST-07: party rows --------------------------------------------------------------------

#[test]
fn test_tc_list_07_party_rows_balances_and_counts() {
    let e = env();
    seed_party(&e.db, 1, "zed Cust", PartyType::Customer);
    seed_party(&e.db, 2, "Amar Cust", PartyType::Customer);
    seed_party(&e.db, 3, "Vendor One", PartyType::Vendor);
    seed_party(&e.db, 4, "Vendor Two", PartyType::Vendor);
    {
        let mut tx = e.db.begin_transaction().unwrap();
        seed_sale_bill(&mut *tx, 1, Some(1), 100.0, 30.0, "2026-11-01");
        seed_sale_bill(&mut *tx, 2, Some(1), 50.0, 0.0, "2026-11-02");
        seed_sale_bill(&mut *tx, 3, Some(1), 20.0, 20.0, "2026-11-03");
        seed_purchase_bill(&mut *tx, 1, Some(3), 400.0, 150.0, "2026-11-01");
        tx.commit().unwrap();
    }
    put_purchase(&e.db, 9, LotSource::Adjustment, Some((3, "Vendor One")), None, 999.0, 0.0, "2026-11-04");

    let rows = e.parties.list_parties(None, None).unwrap();
    let names: Vec<_> = rows.iter().map(|r| r.party.name.as_str()).collect();
    assert_eq!(names, vec!["Amar Cust", "Vendor One", "Vendor Two", "zed Cust"], "by name, case-insensitive");
    let by = |n: &str| rows.iter().find(|r| r.party.name == n).unwrap();
    assert_eq!((by("zed Cust").outstanding_balance, by("zed Cust").transaction_count), (120.0, 3));
    assert_eq!((by("Vendor One").outstanding_balance, by("Vendor One").transaction_count), (250.0, 1), "PURCHASE side only");
    assert_eq!((by("Amar Cust").outstanding_balance, by("Amar Cust").transaction_count), (0.0, 0));
    let cust = e.parties.list_parties(Some(PartyType::Customer), Some(true)).unwrap();
    assert_eq!(cust.len(), 2);
    // Flat JSON: Party fields plus the two extras.
    let v = serde_json::to_value(by("zed Cust")).unwrap();
    assert!(v.get("name").is_some() && v.get("outstanding_balance").is_some() && v.get("transaction_count").is_some());

    // Search by name / phone, case-insensitive; type filter.
    assert_eq!(e.parties.search_parties("VENDOR", None).unwrap().len(), 2);
    assert_eq!(e.parties.search_parties("vendor", Some(PartyType::Customer)).unwrap().len(), 0);
    assert_eq!(e.parties.search_parties("", None).unwrap().len(), 4);
    assert_eq!(e.parties.search_parties("%", None).unwrap().len(), 0);
}

// ---- TC-LIST-08: dues summary ------------------------------------------------------------------

#[test]
fn test_tc_list_08_dues_summary_fields() {
    let e = env();
    seed_party(&e.db, 1, "Cust", PartyType::Customer);
    seed_party(&e.db, 2, "Vend", PartyType::Vendor);
    seed_party(&e.db, 3, "Settled Cust", PartyType::Customer);
    {
        let mut tx = e.db.begin_transaction().unwrap();
        // Backdated bill inserted last: earliest date must come from MIN(transaction_date).
        seed_sale_bill(&mut *tx, 1, Some(1), 100.0, 30.0, "2026-11-10");
        seed_sale_bill(&mut *tx, 2, Some(1), 50.0, 0.0, "2026-11-12");
        seed_sale_bill(&mut *tx, 3, Some(1), 80.0, 0.0, "2026-10-20");
        seed_sale_bill(&mut *tx, 4, Some(1), 20.0, 20.0, "2026-09-01"); // settled: not counted
        seed_sale_bill(&mut *tx, 5, Some(3), 20.0, 20.0, "2026-09-01");
        seed_purchase_bill(&mut *tx, 1, Some(2), 400.0, 150.0, "2026-11-05");
        tx.commit().unwrap();
    }
    put_purchase(&e.db, 9, LotSource::Opening, Some((2, "Vend")), None, 999.0, 0.0, "2026-01-01");

    let c = e.acct.get_party_dues_summary(PartyType::Customer).unwrap();
    assert_eq!(c.len(), 1, "settled parties are omitted");
    let r = &c[0];
    assert_eq!((r.party_id, r.party_name.as_str(), r.unsettled_bills_count), (1, "Cust", 3));
    assert_eq!(r.earliest_bill_date, "2026-10-20");
    assert_eq!((r.total_invoiced, r.total_paid, r.balance_due), (230.0, 30.0, 200.0));
    let v = e.acct.get_party_dues_summary(PartyType::Vendor).unwrap();
    assert_eq!(v.len(), 1);
    assert_eq!((v[0].unsettled_bills_count, v[0].total_invoiced, v[0].total_paid, v[0].balance_due), (1, 400.0, 150.0, 250.0));
    assert_eq!(v[0].earliest_bill_date, "2026-11-05");
    // JSON field names for the frontend.
    let j = serde_json::to_value(r).unwrap();
    for k in ["unsettled_bills_count", "earliest_bill_date", "total_invoiced", "total_paid", "balance_due"] {
        assert!(j.get(k).is_some(), "{k}");
    }
}

// ---- TC-LIST-09: returns, adjustments, SQL paging ----------------------------------------------

#[test]
fn test_tc_list_09_returns_and_adjustments_lists() {
    let e = env();
    seed_stocked_item(&e.db, &e.inv, 1, 500.0, 5.0, 10.0);
    for (i, date) in ["2026-11-01", "2026-11-05", "2026-11-03"].iter().enumerate() {
        let p = CustomerReturnPayload {
            original_sale_id: None, party_id: None, return_date: date.to_string(), settlement_type: SettlementType::Refund,
            reason: None, notes: None, replacement_items: vec![],
            items: vec![CustomerReturnItemPayload { item_id: 1, sale_item_id: None, quantity: 1.0 + i as f64, unit_price: 10.0, condition: StockCondition::Restockable, unit_cost_override: None }],
        };
        e.rets.create_customer_return(p).unwrap();
    }
    let r = e.rets.list_returns(ReturnsFilter { page: Some(1), limit: Some(2), ..Default::default() }).unwrap();
    assert_eq!((r.total, r.items.len(), r.page, r.limit), (3, 2, 1, 2));
    assert_eq!(r.items[0].return_date, "2026-11-05");
    let r = e.rets.list_returns(ReturnsFilter { start_date: Some("2026-11-03".into()), end_date: Some("2026-11-05".into()), ..Default::default() }).unwrap();
    assert_eq!(r.total, 2);
    assert_eq!(field_of(e.rets.list_returns(ReturnsFilter { start_date: Some("bad".into()), ..Default::default() })), "start_date");

    // 60 adjustments: newest first, paged in SQL, page 2 reachable.
    for n in 0..60 {
        let date = format!("2026-10-{:02}", 1 + (n % 28));
        e.inv
            .create_adjustment(StockAdjustmentPayload {
                adjustment_date: date, item_id: 1, quantity: -1.0, reason: AdjustmentReason::Damaged, unit_cost: None, notes: None,
            })
            .unwrap();
    }
    let p1 = e.inv.list_adjustments(Some(1), None, None).unwrap();
    assert_eq!((p1.items.len(), p1.total, p1.page, p1.limit), (50, 60, 1, 50));
    let p2 = e.inv.list_adjustments(Some(1), Some(2), None).unwrap();
    assert_eq!(p2.items.len(), 10);
    let dates: Vec<_> = p1.items.iter().map(|a| a.adjustment_date.clone()).collect();
    let mut sorted = dates.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(dates, sorted, "adjustment_date DESC");
    assert_eq!(e.inv.list_adjustments(Some(99), None, None).unwrap().total, 0);
}
