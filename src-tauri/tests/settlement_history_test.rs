// tests/settlement_history_test.rs
//! Test Suite: FR-42 settlement history read (`get_settlement_history`) on real SQLite.

mod common;

use common::fixtures::*;
use common::ledger::*;
use common::*;
use book_keeper::core::dtos::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::data::contracts::FtsIndex;
use book_keeper::services::contracts::accounting_service::AccountingService;

fn setup() -> (AccountingServiceImpl<PaymentServiceImpl>, TestDb) {
    let db = TestDb::new();
    (AccountingServiceImpl::new(PaymentServiceImpl, db.clone()), db)
}

fn filter(from: &str, to: &str) -> SettlementHistoryFilter {
    SettlementHistoryFilter {
        from_date: from.into(),
        to_date: to.into(),
        side: None,
        party_search: None,
        page: 1,
        page_size: 50,
    }
}

fn party(db: &TestDb, id: i64, name: &str, t: PartyType) {
    seed_party(db, id, name, t);
    db.party_index().reindex(id).unwrap();
}

fn pay_kind(db: &TestDb, t: TransactionType, bill: i64, party: i64, kind: PaymentKind, amt: f64, date: &str) -> i64 {
    let mut tx = db.begin_transaction().unwrap();
    let id = common::ledger::pay(&mut *tx, t, bill, Some(party), kind, amt, date);
    tx.commit().unwrap();
    id
}

/// Customer 1 (sale 1, 1000) and vendor 2 (purchase 1, 800). Returns nothing; caller adds payments.
fn seed(db: &TestDb) {
    party(db, 1, "Ramesh Kumar", PartyType::Customer);
    party(db, 2, "Suresh Traders", PartyType::Vendor);
    let mut tx = db.begin_transaction().unwrap();
    seed_sale_bill(&mut *tx, 1, Some(1), 1000.0, 100.0, "2026-11-01");
    seed_purchase_bill(&mut *tx, 1, Some(2), 800.0, 0.0, "2026-11-01");
    tx.commit().unwrap();
}

fn ledger_len(db: &TestDb) -> usize {
    let mut tx = db.begin_read().unwrap();
    tx.settlements().list_all().unwrap().len()
}

#[test]
fn only_settlement_kind_is_listed() {
    let (svc, db) = setup();
    seed(&db);
    pay_kind(&db, TransactionType::Sale, 1, 1, PaymentKind::Settlement, 200.0, "2026-11-10");
    {
        let mut tx = db.begin_transaction().unwrap();
        tx.settlements()
            .create_settlement(&book_keeper::core::entities::PaymentSettlement {
                id: 0,
                settlement_date: "2026-11-10".into(),
                party_id: Some(1),
                kind: PaymentKind::RefundOut,
                transaction_type: TransactionType::Sale,
                transaction_id: Some(1),
                return_id: None,
                amount_paid: 5.0,
                payment_mode: None,
                notes: None,
                created_at: "2026-11-10T10:00:00Z".into(),
            })
            .unwrap();
        tx.commit().unwrap();
    }
    let page = svc.get_settlement_history(filter("2026-11-01", "2026-11-30")).unwrap();
    // The 100.0 UPFRONT row of the bill and the refund are excluded.
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.rows[0].amount, 200.0);
    assert_eq!(page.rows[0].transaction_code, "SAL-1001");
    assert_eq!(page.rows[0].party_name, "Ramesh Kumar");
    assert!(!page.rows[0].is_opening_balance);
    assert_eq!(page.total_count, 1);
}

#[test]
fn side_filter_and_totals() {
    let (svc, db) = setup();
    seed(&db);
    pay_kind(&db, TransactionType::Sale, 1, 1, PaymentKind::Settlement, 200.0, "2026-11-10");
    pay_kind(&db, TransactionType::Purchase, 1, 2, PaymentKind::Settlement, 300.0, "2026-11-11");
    let all = svc.get_settlement_history(filter("2026-11-01", "2026-11-30")).unwrap();
    assert_eq!((all.total_received, all.total_paid, all.total_count), (200.0, 300.0, 2));
    let mut f = filter("2026-11-01", "2026-11-30");
    f.side = Some(TransactionType::Purchase);
    let p = svc.get_settlement_history(f).unwrap();
    assert_eq!(p.rows.len(), 1);
    assert_eq!(p.rows[0].party_name, "Suresh Traders");
    assert_eq!((p.total_received, p.total_paid), (0.0, 300.0));
}

#[test]
fn date_range_is_inclusive() {
    let (svc, db) = setup();
    seed(&db);
    for (d, a) in [("2026-11-09", 10.0), ("2026-11-10", 20.0), ("2026-11-12", 30.0), ("2026-11-13", 40.0)] {
        pay_kind(&db, TransactionType::Sale, 1, 1, PaymentKind::Settlement, a, d);
    }
    let page = svc.get_settlement_history(filter("2026-11-10", "2026-11-12")).unwrap();
    let amounts: Vec<f64> = page.rows.iter().map(|r| r.amount).collect();
    assert_eq!(amounts, vec![30.0, 20.0]);
}

#[test]
fn party_search_matches_name_prefix() {
    let (svc, db) = setup();
    seed(&db);
    pay_kind(&db, TransactionType::Sale, 1, 1, PaymentKind::Settlement, 200.0, "2026-11-10");
    pay_kind(&db, TransactionType::Purchase, 1, 2, PaymentKind::Settlement, 300.0, "2026-11-11");
    let mut f = filter("2026-11-01", "2026-11-30");
    f.party_search = Some("sur".into());
    let p = svc.get_settlement_history(f.clone()).unwrap();
    assert_eq!(p.rows.len(), 1);
    assert_eq!(p.rows[0].party_id, Some(2));
    assert_eq!(p.total_count, 1);
    f.party_search = Some("zzz".into());
    assert_eq!(svc.get_settlement_history(f.clone()).unwrap().total_count, 0);
    f.party_search = Some("  ".into());
    assert_eq!(svc.get_settlement_history(f).unwrap().total_count, 2);
}

#[test]
fn opening_bill_settlement_is_flagged() {
    let (svc, db) = setup();
    party(&db, 1, "Ramesh Kumar", PartyType::Customer);
    {
        let mut tx = db.begin_transaction().unwrap();
        let mut ob = create_test_sale_tx(1, "OB-0001", Some(1), 500.0, 0.0, "2026-10-01");
        ob.is_opening_balance = true;
        tx.sales().create_transaction(&ob).unwrap();
        tx.commit().unwrap();
    }
    pay_kind(&db, TransactionType::Sale, 1, 1, PaymentKind::Settlement, 150.0, "2026-11-10");
    let page = svc.get_settlement_history(filter("2026-11-01", "2026-11-30")).unwrap();
    assert_eq!(page.rows.len(), 1);
    assert!(page.rows[0].is_opening_balance);
    assert_eq!(page.rows[0].transaction_code, "OB-0001");
}

#[test]
fn totals_ignore_paging_and_order_is_newest_first() {
    let (svc, db) = setup();
    seed(&db);
    pay_kind(&db, TransactionType::Sale, 1, 1, PaymentKind::Settlement, 10.0, "2026-11-10");
    let b = pay_kind(&db, TransactionType::Sale, 1, 1, PaymentKind::Settlement, 20.0, "2026-11-10");
    pay_kind(&db, TransactionType::Sale, 1, 1, PaymentKind::Settlement, 30.0, "2026-11-12");
    let mut f = filter("2026-11-01", "2026-11-30");
    f.page_size = 2;
    let p1 = svc.get_settlement_history(f.clone()).unwrap();
    assert_eq!(p1.rows.iter().map(|r| r.amount).collect::<Vec<_>>(), vec![30.0, 20.0]);
    assert_eq!(p1.rows[1].settlement_id, b);
    assert_eq!((p1.total_count, p1.total_received), (3, 60.0));
    f.page = 2;
    let p2 = svc.get_settlement_history(f).unwrap();
    assert_eq!(p2.rows.iter().map(|r| r.amount).collect::<Vec<_>>(), vec![10.0]);
    assert_eq!((p2.total_count, p2.total_received), (3, 60.0));
}

#[test]
fn invalid_filters_are_rejected() {
    let (svc, _db) = setup();
    let field = |r: Result<SettlementHistoryPage, AppError>| match r {
        Err(AppError::ValidationError { field, .. }) => field,
        other => panic!("expected ValidationError, got {other:?}"),
    };
    let mut f = filter("2026-11-01", "2026-11-30");
    f.page = 0;
    assert_eq!(field(svc.get_settlement_history(f)), "page");
    let mut f = filter("2026-11-01", "2026-11-30");
    f.page_size = 0;
    assert_eq!(field(svc.get_settlement_history(f)), "page_size");
    let mut f = filter("2026-11-01", "2026-11-30");
    f.page_size = 201;
    assert_eq!(field(svc.get_settlement_history(f)), "page_size");
    assert_eq!(field(svc.get_settlement_history(filter("2026-11-30", "2026-11-01"))), "from_date");
}

#[test]
fn read_is_side_effect_free_and_edit_delete_show_on_next_read() {
    let (svc, db) = setup();
    seed(&db);
    let id = pay_kind(&db, TransactionType::Sale, 1, 1, PaymentKind::Settlement, 200.0, "2026-11-10");
    let before = ledger_len(&db);
    let f = filter("2026-11-01", "2026-11-30");
    svc.get_settlement_history(f.clone()).unwrap();
    assert_eq!(ledger_len(&db), before);

    svc.update_settlement(
        id,
        UpdateSettlementPayload {
            amount_paid: 250.0,
            settlement_date: "2026-11-12".into(),
            payment_mode: None,
            notes: Some("fixed".into()),
        },
    )
    .unwrap();
    let p = svc.get_settlement_history(f.clone()).unwrap();
    assert_eq!((p.rows[0].amount, p.rows[0].settlement_date.as_str(), p.total_received), (250.0, "2026-11-12", 250.0));
    assert_eq!(p.rows[0].notes.as_deref(), Some("fixed"));

    svc.delete_settlement(id).unwrap();
    let p = svc.get_settlement_history(f).unwrap();
    assert_eq!((p.rows.len(), p.total_received, p.total_count), (0, 0.0, 0));
}
