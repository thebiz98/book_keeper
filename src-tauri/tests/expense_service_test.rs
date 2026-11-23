//! Test Suite: store expenses (TC-EXP-001..010) on real SQLite.

mod common;

use std::sync::Arc;

use book_keeper::core::dtos::{CreateExpenseTypePayload, ExpenseFilter, ExpensePayload};
use book_keeper::core::error::AppError;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::expense_service::ExpenseService;
use common::{ExpenseServiceImpl, TestDb};

fn setup() -> (TestDb, ExpenseServiceImpl) {
    let db = TestDb::new();
    let svc = ExpenseServiceImpl::new(db.clone(), Arc::new(db.expense_type_index()));
    (db, svc)
}

fn ty(svc: &ExpenseServiceImpl, name: &str) -> i64 {
    svc.create_expense_type(CreateExpenseTypePayload { name: name.into() }).unwrap().id
}

fn pay(date: &str, type_id: i64, amount: f64, notes: Option<&str>) -> ExpensePayload {
    ExpensePayload { expense_date: date.into(), expense_type_id: type_id, amount, notes: notes.map(Into::into) }
}

fn names(v: Vec<book_keeper::core::entities::ExpenseType>) -> Vec<String> {
    v.into_iter().map(|t| t.name).collect()
}

fn expense_count(db: &TestDb) -> usize {
    db.begin_read().unwrap().expenses().list(&ExpenseFilter::default(), 0, 200).unwrap().1
}

fn field_of(e: AppError) -> String {
    match e {
        AppError::ValidationError { field, .. } => field,
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

#[test]
fn tc_exp_001_create_type_trims_indexes_and_is_searchable() {
    let (_db, svc) = setup();
    let t = svc.create_expense_type(CreateExpenseTypePayload { name: "  Printer \t Ink ".into() }).unwrap();
    assert_eq!(t.name, "Printer Ink");
    assert!(t.is_active);
    assert_eq!(names(svc.search_expense_types("pri", None).unwrap()), vec!["Printer Ink"]);
}

#[test]
fn tc_exp_002_create_type_is_idempotent() {
    let (_db, svc) = setup();
    let a = ty(&svc, "Printer Ink");
    let b = ty(&svc, "printer   ink");
    assert_eq!(a, b);
    assert_eq!(svc.list_expense_types().unwrap().len(), 1);
}

#[test]
fn tc_exp_003_invalid_type_names() {
    let (_db, svc) = setup();
    for bad in ["", "   ", &"x".repeat(81)] {
        let e = svc.create_expense_type(CreateExpenseTypePayload { name: bad.into() }).unwrap_err();
        assert_eq!(field_of(e), "name");
    }
    assert!(svc.create_expense_type(CreateExpenseTypePayload { name: "x".repeat(80) }).is_ok());
}

#[test]
fn tc_exp_004_rename_rules_and_snapshot() {
    let (_db, svc) = setup();
    let ink = ty(&svc, "Printer Ink");
    let tea = ty(&svc, "Tea");
    let exp = svc.create_expense(pay("2026-11-01", ink, 10.0, None)).unwrap();
    // case-only change of itself is fine
    assert_eq!(svc.update_expense_type(ink, CreateExpenseTypePayload { name: "PRINTER INK".into() }).unwrap().name, "PRINTER INK");
    // clash with another type
    let e = svc.update_expense_type(ink, CreateExpenseTypePayload { name: "tea".into() }).unwrap_err();
    assert_eq!(field_of(e), "name");
    let renamed = svc.update_expense_type(ink, CreateExpenseTypePayload { name: "Toner".into() }).unwrap();
    assert_eq!(renamed.name, "Toner");
    assert_eq!(names(svc.search_expense_types("ton", None).unwrap()), vec!["Toner"]);
    assert!(svc.search_expense_types("printer", None).unwrap().is_empty(), "old name gone from index");
    let stored = svc.list_expenses(ExpenseFilter::default()).unwrap().items;
    assert_eq!(stored[0].expense_type_name, "Printer Ink", "old expenses keep the snapshot");
    assert_eq!(stored[0].id, exp.id);
    assert!(matches!(
        svc.update_expense_type(9999, CreateExpenseTypePayload { name: "Z".into() }).unwrap_err(),
        AppError::NotFound { .. }
    ));
    let _ = tea;
}

#[test]
fn tc_exp_005_search_behaviour() {
    let (_db, svc) = setup();
    for n in ["Electricity Bill", "Printer Ink", "Tea & Snacks", "Rent"] {
        ty(&svc, n);
    }
    assert_eq!(names(svc.search_expense_types("ele bi", None).unwrap()), vec!["Electricity Bill"]);
    assert_eq!(svc.search_expense_types("", None).unwrap().len(), 4);
    assert_eq!(svc.search_expense_types("  ", Some(2)).unwrap().len(), 2, "blank query capped at the limit");
    assert!(svc.search_expense_types("!!!", None).unwrap().is_empty(), "punctuation only");
    assert_eq!(svc.search_expense_types("r", Some(1)).unwrap().len(), 1, "limit respected");
}

#[test]
fn tc_exp_006_create_expense_codes_snapshot_and_rounding() {
    let (_db, svc) = setup();
    let t = ty(&svc, "Tea");
    let a = svc.create_expense(pay("2026-11-02", t, 10.005, Some("  milk  "))).unwrap();
    assert_eq!(a.expense_code, "EXP-1001");
    assert_eq!(a.expense_type_name, "Tea");
    assert_eq!(a.amount, 10.01);
    assert_eq!(a.notes.as_deref(), Some("milk"));
    assert!(a.updated_at.is_none());
    let b = svc.create_expense(pay("2026-11-02", t, 5.0, None)).unwrap();
    assert_eq!(b.expense_code, "EXP-1002");
    assert_eq!(svc.get_next_expense_code().unwrap(), "EXP-1003");
    assert_eq!(svc.get_next_expense_code().unwrap(), "EXP-1003", "preview does not consume");
}

#[test]
fn tc_exp_007_validation_writes_nothing() {
    let (db, svc) = setup();
    let t = ty(&svc, "Tea");
    for amt in [0.0, -5.0, f64::NAN, 0.001] {
        assert_eq!(field_of(svc.create_expense(pay("2026-11-02", t, amt, None)).unwrap_err()), "amount");
    }
    assert_eq!(field_of(svc.create_expense(pay("2026-13-40", t, 5.0, None)).unwrap_err()), "expense_date");
    assert!(matches!(
        svc.create_expense(pay("2026-11-02", 9999, 5.0, None)).unwrap_err(),
        AppError::NotFound { ref entity, .. } if entity == "ExpenseType"
    ));
    let long = "n".repeat(501);
    assert_eq!(field_of(svc.create_expense(pay("2026-11-02", t, 5.0, Some(&long))).unwrap_err()), "notes");
    assert_eq!(expense_count(&db), 0);
}

#[test]
fn tc_exp_008_update_keeps_code_and_refreshes_snapshot() {
    let (_db, svc) = setup();
    let tea = ty(&svc, "Tea");
    let rent = ty(&svc, "Rent");
    let e = svc.create_expense(pay("2026-11-02", tea, 10.0, None)).unwrap();
    let u = svc.update_expense(e.id, pay("2026-11-05", rent, 99.5, Some("Nov"))).unwrap();
    assert_eq!(u.expense_code, e.expense_code);
    assert_eq!(u.created_at, e.created_at);
    assert!(u.updated_at.is_some());
    assert_eq!((u.expense_type_id, u.expense_type_name.as_str(), u.amount), (rent, "Rent", 99.5));
    assert_eq!(u.expense_date, "2026-11-05");
    let stored = svc.list_expenses(ExpenseFilter::default()).unwrap().items.remove(0);
    assert_eq!(stored, u);
    assert!(matches!(svc.update_expense(777, pay("2026-11-05", rent, 1.0, None)).unwrap_err(), AppError::NotFound { .. }));
}

#[test]
fn tc_exp_009_delete_never_reuses_the_code() {
    let (db, svc) = setup();
    let t = ty(&svc, "Tea");
    let a = svc.create_expense(pay("2026-11-02", t, 1.0, None)).unwrap();
    let b = svc.create_expense(pay("2026-11-02", t, 2.0, None)).unwrap();
    assert_eq!(b.expense_code, "EXP-1002");
    svc.delete_expense(b.id).unwrap();
    assert!(matches!(svc.delete_expense(b.id).unwrap_err(), AppError::NotFound { .. }));
    assert_eq!(expense_count(&db), 1);
    assert_eq!(svc.get_next_expense_code().unwrap(), "EXP-1003", "deleted newest code is not reused");
    assert_eq!(svc.create_expense(pay("2026-11-02", t, 3.0, None)).unwrap().expense_code, "EXP-1003");
    let _ = a;
}

#[test]
fn tc_exp_010_list_filters_paging_and_period_total() {
    let (_db, svc) = setup();
    let tea = ty(&svc, "Tea");
    let ink = ty(&svc, "Printer Ink");
    svc.create_expense(pay("2026-11-01", tea, 10.0, Some("milk"))).unwrap();
    svc.create_expense(pay("2026-11-02", ink, 20.0, Some("Cartridge 100%"))).unwrap();
    svc.create_expense(pay("2026-11-02", tea, 30.0, None)).unwrap();
    svc.create_expense(pay("2026-11-03", tea, 40.0, None)).unwrap();
    let all = svc.list_expenses(ExpenseFilter::default()).unwrap();
    assert_eq!((all.total, all.period_total), (4, 100.0));
    let dates: Vec<_> = all.items.iter().map(|e| (e.expense_date.as_str(), e.amount)).collect();
    assert_eq!(dates, vec![("2026-11-03", 40.0), ("2026-11-02", 30.0), ("2026-11-02", 20.0), ("2026-11-01", 10.0)]);
    let range = svc
        .list_expenses(ExpenseFilter { start_date: Some("2026-11-02".into()), end_date: Some("2026-11-02".into()), ..Default::default() })
        .unwrap();
    assert_eq!((range.total, range.period_total), (2, 50.0), "inclusive range");
    assert_eq!(svc.list_expenses(ExpenseFilter { expense_type_id: Some(ink), ..Default::default() }).unwrap().total, 1);
    let by = |q: &str| svc.list_expenses(ExpenseFilter { search_query: Some(q.into()), ..Default::default() }).unwrap().total;
    assert_eq!(by("exp-1002"), 1, "code");
    assert_eq!(by("PRINTER"), 1, "type name, case-insensitive");
    assert_eq!(by("milk"), 1, "notes");
    assert_eq!(by("100%"), 1, "LIKE wildcard escaped");
    assert_eq!(by("%"), 1, "bare % is literal");
    let p = svc.list_expenses(ExpenseFilter { page: Some(2), limit: Some(3), ..Default::default() }).unwrap();
    assert_eq!((p.items.len(), p.total, p.period_total), (1, 4, 100.0), "period total spans all pages");
    assert!(svc.list_expenses(ExpenseFilter { start_date: Some("2026-11-05".into()), end_date: Some("2026-11-01".into()), ..Default::default() }).is_err());
    assert!(svc.list_expenses(ExpenseFilter { page: Some(0), ..Default::default() }).is_err());
}
