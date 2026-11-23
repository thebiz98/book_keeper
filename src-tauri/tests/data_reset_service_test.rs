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
        .create_item(CreateItemPayload { item_type: None,
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
            transaction_date: "2026-11-20".into(), party_id: None, payment_mode: Some("CASH".into()),
            supplier_invoice_ref: None, payment_status: PaymentStatus::Paid, paid_amount: 35.0, notes: None,
            items: vec![CreatePurchaseItemPayload { item_id: item, bundle_id: None, bundle_quantity: 5.0, unit_cost: 7.0 }],
        })
        .unwrap();
    app.sale_service
        .create_sale(CreateSalePayload {
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
    assert_eq!(e.app.sale_service.get_next_sale_code().unwrap(), "SAL-1001");
    assert!(e.app.inventory_service.verify_stock_cache(&mut *e.app.unit_of_work.begin_transaction().unwrap()).unwrap().is_empty());

    // The app keeps working afterwards.
    seed_sale_again(&e.app, item);
    assert_eq!(stock(&e.app, item), 8.0);
}

fn seed_sale_again(app: &AppState, item: i64) {
    app.sale_service
        .create_sale(CreateSalePayload {
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
    // Ids restart, so a fresh item is #1 again.
    let id = e.app.item_service.create_item(CreateItemPayload { item_type: None,
        name: "Ink".into(), category: None, barcode: None, base_unit: "PCS".into(), mrp: None,
        default_sale_price: 5.0, initial_stock: None, initial_cost: None, bundles: None,
    }).unwrap();
    assert_eq!(id, 1);
}
