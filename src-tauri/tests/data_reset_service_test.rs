//! Test Suite: testing-only DataResetService (gate, pre-reset backup, scopes).

use book_keeper::app_state::{build_sqlite_app_state_with_config, AppState};
use book_keeper::config::AppConfig;
use book_keeper::core::dtos::*;
use book_keeper::core::entities::CreatePartyPayload;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::*;
use book_keeper::services::contracts::data_reset_service::ResetScope;
use book_keeper::services::contracts::*;
use std::path::PathBuf;

struct Env {
    app: AppState,
    db: PathBuf,
    backups: PathBuf,
}

fn env(testing_mode: bool) -> Env {
    let dir = std::env::temp_dir().join(format!("bk_reset_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("shop.db");
    let backups = dir.join("backups");
    let cfg = AppConfig { testing_mode, ..AppConfig::default() };
    let app = build_sqlite_app_state_with_config(&db, cfg, backups.clone()).unwrap();
    Env { app, db, backups }
}

/// Item "Pen" with 10 opening units and a bundle, a customer, a purchase of 5, and a
/// part-paid sale of 12 to the customer (so there are dues).
fn seed(app: &AppState) -> i64 {
    let item = app
        .item_service
        .create_item(CreateItemPayload { item_type: None, low_stock_threshold: None,
            name: "Pen".into(), category: None, barcode: None, base_unit: "PCS".into(), mrp: None,
            default_sale_price: 10.0, initial_stock: Some(10.0), initial_cost: Some(6.0),
            bundles: Some(vec![CreateBundlePayload {
                item_id: None, bundle_name: "Box".into(), units_per_bundle: 10.0, mrp: None,
                default_bundle_price: None, barcode: None,
            }]),
        })
        .unwrap();
    let party = app
        .party_service
        .create_party(CreatePartyPayload { name: "Ram".into(), phone: None, party_type: PartyType::Customer, notes: None })
        .unwrap();
    app.purchase_service
        .create_purchase(CreatePurchasePayload {
            charges: vec![],
            transaction_date: "2026-11-20".into(), party_id: None, payment_mode: Some("CASH".into()),
            supplier_invoice_ref: None, payment_status: PaymentStatus::Paid, paid_amount: 35.0, notes: None,
            items: vec![CreatePurchaseItemPayload { purchase_item_id: None, item_id: item, bundle_id: None, bundle_quantity: 5.0, unit_cost: 7.0 }],
        })
        .unwrap();
    app.sale_service
        .create_sale(CreateSalePayload {
            charges: vec![],
            transaction_date: "2026-11-21".into(), party_id: Some(party), payment_mode: Some("CASH".into()),
            payment_status: PaymentStatus::Partial, paid_amount: 50.0, notes: None,
            items: vec![CreateSaleItemPayload { item_id: item, bundle_id: None, bundle_quantity: 12.0, unit_price: 10.0 }],
        })
        .unwrap();
    item
}

fn count(db: &PathBuf, table: &str) -> i64 {
    let c = rusqlite::Connection::open(db).unwrap();
    c.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap()
}

fn stock(app: &AppState, id: i64) -> f64 {
    app.unit_of_work.begin_read().unwrap().items().find_by_id(id).unwrap().unwrap().current_stock
}

fn dir_entries(p: &PathBuf) -> Vec<String> {
    std::fs::read_dir(p)
        .map(|rd| rd.filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().to_string()).collect())
        .unwrap_or_default()
}

#[test]
fn refused_when_testing_mode_is_off_and_nothing_changes() {
    let e = env(false);
    let item = seed(&e.app);
    let err = e.app.data_reset_service.reset(ResetScope::All, e.backups.to_str().unwrap()).unwrap_err();
    assert!(matches!(err, AppError::OperationNotAllowed(_)), "{err:?}");
    assert_eq!(count(&e.db, "sale_transactions"), 1);
    assert_eq!(count(&e.db, "items"), 1);
    assert_eq!(stock(&e.app, item), 3.0);
    assert!(dir_entries(&e.backups).is_empty(), "no backup written when refused");
}

#[test]
fn transactions_scope_restores_opening_stock_and_keeps_masters() {
    let e = env(true);
    let item = seed(&e.app);
    assert_eq!(stock(&e.app, item), 3.0); // 10 + 5 - 12

    let summary = e.app.data_reset_service.reset(ResetScope::Transactions, e.backups.to_str().unwrap()).unwrap();
    assert!(std::path::Path::new(&summary.backup_path).exists());
    assert!(dir_entries(&e.backups).iter().any(|n| n.starts_with("book_keeper_prereset_")));
    // The backup still holds the sale.
    let b = rusqlite::Connection::open(&summary.backup_path).unwrap();
    let sales: i64 = b.query_row("SELECT COUNT(*) FROM sale_transactions", [], |r| r.get(0)).unwrap();
    assert_eq!(sales, 1);

    for t in ["sale_transactions", "sale_items", "sale_stock_allocations", "payment_settlements",
              "return_transactions", "stock_adjustments", "stock_outflow_allocations"] {
        assert_eq!(count(&e.db, t), 0, "{t}");
    }
    assert_eq!(count(&e.db, "purchase_transactions"), 1, "only the OPENING lot header stays");
    assert_eq!(count(&e.db, "purchase_items"), 1);
    assert_eq!(stock(&e.app, item), 10.0, "back to opening stock");
    assert_eq!(count(&e.db, "items"), 1);
    assert_eq!(count(&e.db, "item_bundles"), 1);
    assert_eq!(count(&e.db, "parties"), 1);
    assert_eq!(e.app.party_index.indexed_count().unwrap(), 1, "parties stay searchable");
    assert_eq!(e.app.sale_service.get_next_sale_code().unwrap(), "SAL-1001");
    assert!(e.app.inventory_service.verify_stock_cache(&mut *e.app.unit_of_work.begin_transaction().unwrap()).unwrap().is_empty());

    // The app keeps working afterwards.
    seed_sale_again(&e.app, item);
    assert_eq!(stock(&e.app, item), 8.0);
}

fn seed_sale_again(app: &AppState, item: i64) {
    app.sale_service
        .create_sale(CreateSalePayload {
            charges: vec![],
            transaction_date: "2026-11-22".into(), party_id: None, payment_mode: Some("CASH".into()),
            payment_status: PaymentStatus::Paid, paid_amount: 20.0, notes: None,
            items: vec![CreateSaleItemPayload { item_id: item, bundle_id: None, bundle_quantity: 2.0, unit_price: 10.0 }],
        })
        .unwrap();
}

#[test]
fn all_scope_empties_everything_but_the_shop_profile() {
    let e = env(true);
    seed(&e.app);
    e.app.data_reset_service.reset(ResetScope::All, e.backups.to_str().unwrap()).unwrap();
    for t in ["items", "item_bundles", "parties", "purchase_transactions", "purchase_items",
              "sale_transactions", "payment_settlements"] {
        assert_eq!(count(&e.db, t), 0, "{t}");
    }
    assert_eq!(count(&e.db, "shop_settings"), 1);
    assert_eq!(e.app.search_indexer.indexed_item_count().unwrap(), 0);
    assert_eq!(count(&e.db, "party_search_fts"), 0, "reset All rebuilds every registered FTS index");
    // Ids restart, so a fresh item is #1 again.
    let id = e.app.item_service.create_item(CreateItemPayload { item_type: None, low_stock_threshold: None,
        name: "Ink".into(), category: None, barcode: None, base_unit: "PCS".into(), mrp: None,
        default_sale_price: 5.0, initial_stock: None, initial_cost: None, bundles: None,
    }).unwrap();
    assert_eq!(id, 1);
}

fn seed_expenses(app: &AppState) -> i64 {
    use book_keeper::core::dtos::{CreateExpenseTypePayload, ExpensePayload};
    let t = app.expense_service.create_expense_type(CreateExpenseTypePayload { name: "Tea".into() }).unwrap();
    for amt in [10.0, 20.0] {
        app.expense_service
            .create_expense(ExpensePayload { expense_date: "2026-11-21".into(), expense_type_id: t.id, amount: amt, notes: None })
            .unwrap();
    }
    t.id
}

#[test]
fn tc_exp_014_transactions_scope_clears_expenses_and_restarts_codes_but_keeps_types() {
    let e = env(true);
    seed(&e.app);
    seed_expenses(&e.app);
    assert_eq!(e.app.expense_service.get_next_expense_code().unwrap(), "EXP-1003");
    let summary = e.app.data_reset_service.reset(ResetScope::Transactions, e.backups.to_str().unwrap()).unwrap();
    assert!(summary.deleted.iter().any(|d| d.table == "expenses" && d.rows == 2));
    assert_eq!(count(&e.db, "expenses"), 0);
    assert_eq!(count(&e.db, "expense_types"), 1);
    assert_eq!(e.app.expense_service.get_next_expense_code().unwrap(), "EXP-1001", "codes restart");
    assert_eq!(e.app.expense_service.search_expense_types("te", None).unwrap().len(), 1, "types stay searchable");
}

#[test]
fn tc_exp_014_all_scope_clears_types_index_and_code_sequences() {
    let e = env(true);
    seed(&e.app);
    seed_expenses(&e.app);
    let summary = e.app.data_reset_service.reset(ResetScope::All, e.backups.to_str().unwrap()).unwrap();
    assert!(summary.deleted.iter().any(|d| d.table == "expense_types" && d.rows == 1));
    assert_eq!(count(&e.db, "expense_types"), 0);
    assert_eq!(count(&e.db, "expense_type_search_fts"), 0);
    assert_eq!(count(&e.db, "code_sequences"), 0);
    assert!(e.app.expense_service.search_expense_types("te", None).unwrap().is_empty());
    assert_eq!(e.app.sale_service.get_next_sale_code().unwrap(), "SAL-1001");
}

#[test]
fn bill_charges_cleared_by_transactions_and_types_by_all() {
    let e = env(true);
    seed(&e.app);
    let t = e
        .app
        .charge_type_service
        .create_charge_type(CreateChargeTypePayload {
            name: "Transport".into(), effect: ChargeEffect::Charge, applies_to: ChargeAppliesTo::Both, default_amount: None,
        })
        .unwrap();
    e.app
        .sale_service
        .create_sale(CreateSalePayload {
            charges: vec![BillChargeInput { charge_type_id: t.id, amount: 5.0 }],
            transaction_date: "2026-11-22".into(), party_id: None, payment_mode: None,
            payment_status: PaymentStatus::Paid, paid_amount: 15.0, notes: None,
            items: vec![CreateSaleItemPayload { item_id: 1, bundle_id: None, bundle_quantity: 1.0, unit_price: 10.0 }],
        })
        .unwrap();
    assert_eq!(count(&e.db, "bill_charges"), 1);
    e.app.data_reset_service.reset(ResetScope::Transactions, e.backups.to_str().unwrap()).unwrap();
    assert_eq!(count(&e.db, "bill_charges"), 0);
    assert_eq!(count(&e.db, "charge_types"), 1, "types kept");
    assert_eq!(e.app.charge_type_service.search_charge_types("tra", TransactionType::Sale, None).unwrap().len(), 1);
    e.app.data_reset_service.reset(ResetScope::All, e.backups.to_str().unwrap()).unwrap();
    assert_eq!(count(&e.db, "charge_types"), 0);
    assert_eq!(count(&e.db, "charge_type_search_fts"), 0);
    assert!(e.app.charge_type_service.search_charge_types("tra", TransactionType::Sale, None).unwrap().is_empty());
}
