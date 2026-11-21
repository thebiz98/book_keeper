// tests/data_access_test.rs
//! Data-access foundation (fix-phase F2/T2): the single code generator, read vs write
//! transactions, and the repository helpers added for the later phases.

mod common;

use common::fixtures::*;
use common::*;
use book_keeper::core::dtos::*;
use book_keeper::core::entities::*;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::return_service::ReturnService;

fn exec(db: &TestDb, sql: &str) {
    db.pool.get().expect("conn").execute_batch(sql).expect("sql");
}

fn next_sale_code(db: &TestDb) -> String {
    let mut tx = db.begin_read().expect("read");
    tx.sales().get_next_transaction_code().expect("code")
}

fn insert_sale_code(db: &TestDb, code: &str) {
    exec(
        db,
        &format!(
            "INSERT INTO sale_transactions (transaction_code, transaction_date, payment_status, total_amount) \
             VALUES ('{code}','2026-11-21','PAID',1)"
        ),
    );
}

fn insert_header(db: &TestDb, code: &str, source: &str) {
    exec(
        db,
        &format!(
            "INSERT INTO purchase_transactions (transaction_code, source, transaction_date, payment_status, total_amount) \
             VALUES ('{code}','{source}','2026-11-21','PAID',0)"
        ),
    );
}

// ---------------------------------------------------------------------------
// next_code
// ---------------------------------------------------------------------------

#[test]
fn next_code_empty_table_starts_at_1001() {
    let db = TestDb::new();
    assert_eq!(next_sale_code(&db), "SAL-1001");
}

#[test]
fn next_code_ignores_legacy_non_numeric_codes() {
    let db = TestDb::new();
    for code in ["SAL-C1", "SAL-HIST-1", "SAL-12x", "SAL-", "OTHER-9999", "sal-5000"] {
        insert_sale_code(&db, code);
    }
    assert_eq!(next_sale_code(&db), "SAL-1001", "only SAL-<digits> codes count");
    insert_sale_code(&db, "SAL-1004");
    assert_eq!(next_sale_code(&db), "SAL-1005");
}

#[test]
fn next_code_uses_max_not_count_so_gaps_are_not_refilled() {
    let db = TestDb::new();
    insert_sale_code(&db, "SAL-1001");
    insert_sale_code(&db, "SAL-1005");
    assert_eq!(next_sale_code(&db), "SAL-1006");
}

#[test]
fn next_code_grows_past_the_padding_width() {
    let db = TestDb::new();
    insert_sale_code(&db, "SAL-9999");
    assert_eq!(next_sale_code(&db), "SAL-10000");
    insert_sale_code(&db, "SAL-10000");
    assert_eq!(next_sale_code(&db), "SAL-10001");
}

#[test]
fn lot_header_sequences_are_independent_per_source() {
    let db = TestDb::new();
    insert_header(&db, "ADJ-0003", "ADJUSTMENT");
    insert_header(&db, "PUR-1001", "PURCHASE");
    let mut tx = db.begin_read().unwrap();
    assert_eq!(tx.purchases().get_next_transaction_code(LotSource::Purchase).unwrap(), "PUR-1002");
    assert_eq!(tx.purchases().get_next_transaction_code(LotSource::Adjustment).unwrap(), "ADJ-0004");
    assert_eq!(tx.purchases().get_next_transaction_code(LotSource::Opening).unwrap(), "OPN-0001");
    assert_eq!(tx.purchases().get_next_transaction_code(LotSource::Return).unwrap(), "RTL-0001");
}

#[test]
fn consecutive_returns_get_distinct_ret_codes_for_both_sides() {
    let db = TestDb::new();
    let inventory = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let returns = ReturnServiceImpl::new(inventory, PaymentServiceImpl, db.clone());

    let mut tx = db.begin_transaction().unwrap();
    tx.items().create(&create_test_item(7, "Calculator", None, None, 5.0, 500.0)).unwrap();
    tx.purchases().create_transaction(&create_test_purchase_tx(7, "PUR-1001", None, 1500.0, 1500.0, "2026-11-20")).unwrap();
    tx.purchases().create_purchase_item(&create_test_purchase_lot(70, 7, 7, None, 5.0, 5.0, 300.0, 5.0)).unwrap();
    seed_sale_line(&mut *tx, 15, 7);
    tx.commit().unwrap();

    let customer = |qty: f64| CustomerReturnPayload {
        original_sale_id: Some(15),
        party_id: None,
        return_date: "2026-11-21".to_string(),
        settlement_type: SettlementType::Refund,
        reason: None,
        notes: None,
        items: vec![CustomerReturnItemPayload {
            item_id: 7,
            sale_item_id: Some(15),
            quantity: qty,
            unit_price: 500.0,
            condition: StockCondition::DamagedScrap,
        }],
    };
    assert_eq!(returns.create_customer_return(customer(1.0)).unwrap(), "RET-1001");
    assert_eq!(returns.create_customer_return(customer(1.0)).unwrap(), "RET-1002");
    let vendor = VendorReturnPayload {
        original_purchase_id: None,
        party_id: None,
        return_date: "2026-11-21".to_string(),
        settlement_type: SettlementType::Refund,
        reason: None,
        notes: None,
        items: vec![VendorReturnItemPayload { item_id: 7, purchase_item_id: None, quantity: 1.0, unit_cost: 300.0 }],
    };
    assert_eq!(returns.create_vendor_return(vendor).unwrap(), "RET-1003", "one shared RET- sequence");
    assert_eq!(returns.get_next_return_code().unwrap(), "RET-1004");
}

// ---------------------------------------------------------------------------
// begin_read
// ---------------------------------------------------------------------------

#[test]
fn begin_read_does_not_block_a_concurrent_writer() {
    let db = TestDb::new();
    let mut seed = db.begin_transaction().unwrap();
    seed.items().create(&create_test_item(1, "Pen", None, None, 0.0, 10.0)).unwrap();
    seed.commit().unwrap();

    // Reader holds its snapshot open on one pooled connection ...
    let mut reader = db.begin_read().unwrap();
    assert!(reader.items().find_by_id(1).unwrap().is_some());
    assert!(reader.items().find_by_id(2).unwrap().is_none());

    // ... while a writer on another connection takes the write lock and commits.
    let mut writer = db.begin_transaction().unwrap();
    writer.items().create(&create_test_item(2, "Pencil", None, None, 0.0, 5.0)).unwrap();
    writer.commit().unwrap();

    // The open reader still sees its snapshot; a new reader sees the commit.
    assert!(reader.items().find_by_id(2).unwrap().is_none(), "snapshot isolation");
    drop(reader);
    let mut fresh = db.begin_read().unwrap();
    assert!(fresh.items().find_by_id(2).unwrap().is_some());
}

#[test]
fn read_context_rejects_writes_and_connection_is_writable_again_after_drop() {
    let db = TestDb::new();
    {
        let mut reader = db.begin_read().unwrap();
        let err = reader.items().create(&create_test_item(1, "Pen", None, None, 0.0, 10.0));
        assert!(err.is_err(), "writes on a read context must fail");
        assert!(reader.commit().is_err(), "commit on a read context is a programmer error");
    }
    // Pooled connections are reused: query_only must have been lifted.
    for i in 1..=6 {
        let mut r = db.begin_read().unwrap();
        r.items().find_by_id(1).unwrap();
        drop(r);
        let mut w = db.begin_transaction().unwrap();
        w.items().create(&create_test_item(i, &format!("Item {i}"), None, None, 0.0, 1.0)).unwrap();
        w.commit().unwrap();
    }
}

// ---------------------------------------------------------------------------
// repository helpers
// ---------------------------------------------------------------------------

#[test]
fn find_purchase_item_by_id_finds_lots_across_headers() {
    let db = TestDb::new();
    let mut tx = db.begin_transaction().unwrap();
    tx.items().create(&create_test_item(1, "Pen", None, None, 0.0, 10.0)).unwrap();
    for h in 1..=3 {
        tx.purchases().create_transaction(&create_test_purchase_tx(h, &format!("PUR-{}", 1000 + h), None, 10.0, 10.0, "2026-11-20")).unwrap();
        tx.purchases().create_purchase_item(&create_test_purchase_lot(h * 10, h, 1, None, 5.0, 5.0, 2.0 + h as f64, 5.0)).unwrap();
    }
    let lot = tx.purchases().find_purchase_item_by_id(30).unwrap().expect("lot 30");
    assert_eq!((lot.purchase_transaction_id, lot.unit_cost), (3, 5.0));
    assert!(tx.purchases().find_purchase_item_by_id(31).unwrap().is_none());
    assert_eq!(tx.purchases().get_latest_lot_cost(1).unwrap(), Some(5.0));
    assert_eq!(tx.purchases().get_latest_lot_cost(99).unwrap(), None);
    let lots = tx.purchases().list_lots_for_item(1).unwrap();
    assert_eq!(lots.iter().map(|l| l.purchase_item_id).collect::<Vec<_>>(), vec![10, 20, 30]);
    assert_eq!(lots[0].transaction_code, "PUR-1001");
    assert_eq!(lots[0].source, LotSource::Purchase);
}

#[test]
fn update_remaining_quantity_rejects_out_of_range_values() {
    let db = TestDb::new();
    let mut tx = db.begin_transaction().unwrap();
    tx.items().create(&create_test_item(1, "Pen", None, None, 0.0, 10.0)).unwrap();
    tx.purchases().create_transaction(&create_test_purchase_tx(1, "PUR-1001", None, 10.0, 10.0, "2026-11-20")).unwrap();
    tx.purchases().create_purchase_item(&create_test_purchase_lot(1, 1, 1, None, 5.0, 5.0, 2.0, 5.0)).unwrap();
    assert!(tx.purchases().update_remaining_quantity(1, -1.0).is_err());
    assert!(tx.purchases().update_remaining_quantity(1, 5.5).is_err());
    assert!(tx.purchases().update_remaining_quantity(404, 1.0).is_err());
    tx.purchases().update_remaining_quantity(1, 5.0 + 1e-12).unwrap();
    assert_eq!(tx.purchases().find_purchase_item_by_id(1).unwrap().unwrap().remaining_quantity, 5.0);
}

#[test]
fn list_open_for_party_orders_by_date_then_id_and_excludes_paid_bills() {
    let db = TestDb::new();
    let mut tx = db.begin_transaction().unwrap();
    tx.parties().create(&create_test_party(1, "Asha", PartyType::Customer, None)).unwrap();
    tx.parties().create(&create_test_party(2, "Other", PartyType::Customer, None)).unwrap();
    // (id, date, paid): id 1 is the newest, 2 and 3 share a date, 4 is fully paid, 5 is another party's.
    for (id, date, paid, party) in [
        (1, "2026-11-03", 0.0, 1),
        (2, "2026-11-01", 10.0, 1),
        (3, "2026-11-01", 0.0, 1),
        (4, "2026-11-02", 100.0, 1),
        (5, "2026-11-01", 0.0, 2),
    ] {
        tx.sales().create_transaction(&create_test_sale_tx(id, &format!("SAL-{}", 1000 + id), Some(party), 100.0, paid, date)).unwrap();
    }
    let open: Vec<i64> = tx.sales().list_open_for_party(1).unwrap().iter().map(|s| s.id).collect();
    assert_eq!(open, vec![2, 3, 1]);
}

#[test]
fn purchase_list_open_for_party_filters_source_balance_and_orders() {
    let db = TestDb::new();
    let mut tx = db.begin_transaction().unwrap();
    tx.parties().create(&create_test_party(1, "Vendor", PartyType::Vendor, None)).unwrap();
    for (id, date, paid) in [(1, "2026-11-05", 0.0), (2, "2026-11-02", 0.0), (3, "2026-11-02", 100.0)] {
        tx.purchases().create_transaction(&create_test_purchase_tx(id, &format!("PUR-{}", 1000 + id), Some(1), 100.0, paid, date)).unwrap();
    }
    // Header 4 is an ADJUSTMENT lot header that (wrongly) carries a party and a balance: never a supplier bill.
    let mut adj = create_test_purchase_tx(4, "ADJ-0001", Some(1), 100.0, 0.0, "2026-11-01");
    adj.source = LotSource::Adjustment;
    tx.purchases().create_transaction(&adj).unwrap();
    tx.commit().unwrap();

    let mut read = db.begin_read().unwrap();
    let open: Vec<i64> = read.purchases().list_open_for_party(1).unwrap().iter().map(|p| p.id).collect();
    assert_eq!(open, vec![2, 1]);
    let summary = read.parties().get_party_due_summary(PartyType::Vendor).unwrap();
    assert_eq!(summary.len(), 1);
    assert_eq!((summary[0].total_due, summary[0].unpaid_bills_count), (200.0, 2));
    assert_eq!(summary[0].oldest_bill_date.as_deref(), Some("2026-11-02"));
    // History shows PURCHASE-source headers only.
    let hist = read.purchases().list_transactions(&PurchaseFilter::default()).unwrap();
    assert_eq!(hist.total_count, 3);
}

#[test]
fn stock_delta_helper_updates_cache_and_flag_in_one_statement() {
    let db = TestDb::new();
    let mut tx = db.begin_transaction().unwrap();
    tx.items().create(&create_test_item(1, "Pen", None, None, 10.0, 10.0)).unwrap();
    tx.items().apply_stock_delta(1, -6.0).unwrap();
    let it = tx.items().find_by_id(1).unwrap().unwrap();
    assert_eq!((it.current_stock, it.is_low_stock), (4.0, true));
    tx.items().apply_stock_delta(1, 1.0).unwrap();
    let it = tx.items().find_by_id(1).unwrap().unwrap();
    assert_eq!((it.current_stock, it.is_low_stock), (5.0, false), "threshold is strictly below 5");
    assert!(tx.items().apply_stock_delta(99, 1.0).is_err());
}

#[test]
fn item_update_never_writes_stock_cache_or_created_at() {
    let db = TestDb::new();
    let mut tx = db.begin_transaction().unwrap();
    let item = create_test_item(1, "Pen", None, None, 10.0, 10.0);
    tx.items().create(&item).unwrap();
    let mut edited = item.clone();
    edited.name = "Pen Blue".into();
    edited.current_stock = 999.0;
    edited.is_low_stock = true;
    edited.created_at = "1999-01-01T00:00:00Z".into();
    tx.items().update(&edited).unwrap();
    let got = tx.items().find_by_id(1).unwrap().unwrap();
    assert_eq!(got.name, "Pen Blue");
    assert_eq!((got.current_stock, got.is_low_stock), (10.0, false));
    assert_eq!(got.created_at, item.created_at);
}

#[test]
fn money_fields_ledger_and_returned_quantity_helpers() {
    let db = TestDb::new();
    let mut tx = db.begin_transaction().unwrap();
    tx.items().create(&create_test_item(1, "Pen", None, None, 10.0, 10.0)).unwrap();
    tx.sales().create_transaction(&create_test_sale_tx(1, "SAL-1001", None, 100.0, 0.0, "2026-11-20")).unwrap();
    tx.sales().create_sale_item(&create_test_sale_item(1, 1, 1, None, 4.0, 4.0, 25.0)).unwrap();
    tx.sales().update_money_fields(1, 40.0, 60.0, PaymentStatus::Partial).unwrap();
    let s = tx.sales().find_transaction_by_id(1).unwrap().unwrap();
    assert_eq!((s.paid_amount, s.balance_due, s.payment_status), (40.0, 60.0, PaymentStatus::Partial));
    assert!(tx.sales().update_money_fields(9, 0.0, 0.0, PaymentStatus::Paid).is_err());
    assert_eq!(tx.sales().find_sale_item_by_id(1).unwrap().unwrap().quantity, 4.0);

    for (id, kind, amount) in [(1, PaymentKind::Upfront, 30.0), (2, PaymentKind::Settlement, 10.0), (3, PaymentKind::ReturnCredit, 5.0)] {
        let mut s = create_test_settlement(id, None, "SALE", 1, amount, "2026-11-20");
        s.kind = kind;
        tx.settlements().create_settlement(&s).unwrap();
    }
    let totals = tx.settlements().sum_by_kind_for_transaction(TransactionType::Sale, 1).unwrap();
    assert_eq!((totals.upfront, totals.settlement, totals.return_credit, totals.refund_out), (30.0, 10.0, 5.0, 0.0));

    // Allocations: reversal order and validated returned quantity.
    tx.purchases().create_transaction(&create_test_purchase_tx(1, "PUR-1001", None, 10.0, 10.0, "2026-11-19")).unwrap();
    tx.purchases().create_purchase_item(&create_test_purchase_lot(1, 1, 1, None, 10.0, 10.0, 2.0, 6.0)).unwrap();
    tx.allocations().create_allocation(&create_test_allocation(1, 1, 1, 3.0, 2.0)).unwrap();
    tx.allocations().create_allocation(&create_test_allocation(2, 1, 1, 1.0, 2.0)).unwrap();
    let ids: Vec<i64> = tx.allocations().find_reversible_by_sale_item_id(1).unwrap().iter().map(|a| a.id).collect();
    assert_eq!(ids, vec![2, 1]);
    tx.allocations().update_returned_quantity(2, 1.0).unwrap();
    let ids: Vec<i64> = tx.allocations().find_reversible_by_sale_item_id(1).unwrap().iter().map(|a| a.id).collect();
    assert_eq!(ids, vec![1], "fully returned allocation drops out");
    assert!(tx.allocations().update_returned_quantity(1, 3.5).is_err());

    // Returned-quantity sums (customer line and vendor lot).
    let mk = |code: &str, rt: ReturnType| ReturnTransaction {
        id: 0, return_code: code.into(), return_date: "2026-11-21".into(), return_type: rt, party_id: None,
        party_name: None, original_transaction_id: None, settlement_type: SettlementType::Refund,
        total_amount: 0.0, reason: None, notes: None, created_at: "2026-11-21T00:00:00Z".into(),
    };
    let line = |rid: i64, sale_item: Option<i64>, lot: Option<i64>, qty: f64| ReturnItem {
        id: 0, return_transaction_id: rid, line_type: ReturnLineType::Returned, item_id: 1, item_name: "Pen".into(),
        sale_item_id: sale_item, purchase_item_id: lot, quantity: qty, unit_price: 1.0, total_amount: qty,
        cost_amount: 0.0, condition: None,
    };
    let r1 = tx.returns().create_return_transaction(&mk("RET-1001", ReturnType::Customer)).unwrap();
    let r2 = tx.returns().create_return_transaction(&mk("RET-1002", ReturnType::Vendor)).unwrap();
    tx.returns().create_return_item(&line(r1, Some(1), None, 2.0)).unwrap();
    tx.returns().create_return_item(&line(r2, None, Some(1), 3.0)).unwrap();
    assert_eq!(tx.sales().get_returned_quantity_for_line(1).unwrap(), 2.0);
    assert_eq!(tx.purchases().get_returned_quantity_for_lot(1).unwrap(), 3.0);
    let item_id = tx.returns().find_items_by_return_id(r1).unwrap()[0].id;
    tx.returns().update_return_item_cost(item_id, 4.5).unwrap();
    assert_eq!(tx.returns().find_items_by_return_id(r1).unwrap()[0].cost_amount, 4.5);
}

#[test]
fn adjustment_and_opening_creators_use_their_own_sequences() {
    use book_keeper::services::contracts::{inventory_service::InventoryService, item_service::ItemService};
    let db = TestDb::new();
    let items = ItemServiceImpl::new(db.clone(), std::sync::Arc::new(db.indexer()));
    let mk = |name: &str| CreateItemPayload {
        name: name.into(), category: None, barcode: None, base_unit: "PCS".into(), mrp: None,
        default_sale_price: 10.0, initial_stock: Some(8.0), initial_cost: Some(6.0), bundles: None,
    };
    let a = items.create_item(mk("Pen")).unwrap();
    items.create_item(mk("Pencil")).unwrap();
    let inventory = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let adj = |qty: f64| StockAdjustmentPayload {
        adjustment_date: "2026-11-21".into(), item_id: a, quantity: qty,
        reason: AdjustmentReason::CountCorrection, notes: None,
    };
    inventory.create_adjustment(adj(2.0)).unwrap();
    inventory.create_adjustment(adj(-1.0)).unwrap(); // no header
    inventory.create_adjustment(adj(3.0)).unwrap();

    let mut r = db.begin_read().unwrap();
    let codes: Vec<String> = r.purchases().list_lots_for_item(a).unwrap().into_iter().map(|l| l.transaction_code).collect();
    assert_eq!(codes, vec!["OPN-0001", "ADJ-0001", "ADJ-0002"]);
    let b_lots = r.purchases().list_lots_for_item(a + 1).unwrap();
    assert_eq!(b_lots[0].transaction_code, "OPN-0002");
    let page = inventory.list_adjustments(Some(a), 1, 2).unwrap();
    assert_eq!((page.items.len(), page.total_count), (2, 3));
    assert_eq!(page.items[0].quantity, 3.0, "newest first");
}
