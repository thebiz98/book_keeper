use book_keeper::app_state::build_sqlite_app_state;
use book_keeper::core::dtos::{
    CreateItemPayload, CreatePurchaseItemPayload, CreatePurchasePayload, CreateSaleItemPayload,
    CatalogFilter, CreateSalePayload,
};
use book_keeper::core::value_objects::PaymentStatus;

#[test]
fn sqlite_end_to_end_smoke() {
    let path = std::env::temp_dir().join(format!("bk_smoke_{}.db", uuid::Uuid::new_v4()));
    let state = build_sqlite_app_state(&path).expect("build state");

    let item_id = state
        .item_service
        .create_item(CreateItemPayload { item_type: None, low_stock_threshold: None,
            name: "Lux Soap".to_string(),
            category: Some("Toiletries".to_string()),
            barcode: Some("8901234567890".to_string()),
            base_unit: "pcs".to_string(),
            mrp: Some(12.0),
            default_sale_price: 10.0,
            initial_stock: None,
            initial_cost: None,
            bundles: None,
        })
        .expect("create item");

    state
        .purchase_service
        .create_purchase(CreatePurchasePayload {
            charges: vec![],
            transaction_date: "2026-01-01".to_string(),
            party_id: None,
            supplier_invoice_ref: None, payment_status: PaymentStatus::Paid, paid_amount: 100.0,
            payment_mode: Some("CASH".to_string()),
            notes: None,
            items: vec![CreatePurchaseItemPayload { item_id, bundle_id: None, bundle_quantity: 10.0, unit_cost: 10.0 }],
        })
        .expect("purchase");

    state
        .sale_service
        .create_sale(CreateSalePayload {
            charges: vec![],
            transaction_date: "2026-01-02".to_string(),
            party_id: None,
            payment_status: PaymentStatus::Paid, paid_amount: 30.0,
            payment_mode: Some("CASH".to_string()),
            notes: None,
            items: vec![CreateSaleItemPayload { item_id, bundle_id: None, bundle_quantity: 3.0, unit_price: 10.0 }],
        })
        .expect("sale");

    let second = state
        .sale_service
        .create_sale(CreateSalePayload {
            charges: vec![],
            transaction_date: "2026-01-02".to_string(),
            party_id: None,
            payment_status: PaymentStatus::Paid, paid_amount: 10.0,
            payment_mode: Some("CASH".to_string()),
            notes: None,
            items: vec![CreateSaleItemPayload { item_id, bundle_id: None, bundle_quantity: 1.0, unit_price: 10.0 }],
        })
        .expect("second auto-coded sale must not collide on transaction_code");
    assert_ne!(second.transaction_code, "SAL-1001");

    let catalog = state.item_service.get_catalog(CatalogFilter { page: Some(1), limit: Some(10), ..Default::default() }).expect("catalog");
    let item = catalog.items.iter().find(|i| i.item_id == item_id).expect("item in catalog");
    assert!((item.current_stock - 6.0).abs() < 1e-9, "stock was {}", item.current_stock);

    let hits = state.search_indexer.search("lux").expect("search");
    assert!(hits.iter().any(|h| h.item_id == item_id));
    let by_barcode = state.search_indexer.search("890123").expect("search barcode");
    assert!(by_barcode.iter().any(|h| h.item_id == item_id));

    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), ext));
    }
}

#[test]
fn sales_history_honours_start_end_date_filter() {
    use book_keeper::core::dtos::SalesHistoryFilter;
    let path = std::env::temp_dir().join(format!("bk_hist_{}.db", uuid::Uuid::new_v4()));
    let state = build_sqlite_app_state(&path).expect("build state");
    let item_id = state.item_service.create_item(CreateItemPayload { item_type: None, low_stock_threshold: None,
        name: "Pencil".into(), category: None, barcode: None, base_unit: "pcs".into(), mrp: None,
        default_sale_price: 5.0, initial_stock: None, initial_cost: None, bundles: None,
    }).expect("item");
    state.purchase_service.create_purchase(CreatePurchasePayload {
        charges: vec![],
        transaction_date: "2026-01-01".into(), party_id: None, supplier_invoice_ref: None, payment_status: PaymentStatus::Paid, paid_amount: 50.0,
        payment_mode: Some("CASH".into()), notes: None,
        items: vec![CreatePurchaseItemPayload { item_id, bundle_id: None, bundle_quantity: 10.0, unit_cost: 5.0 }],
    }).expect("purchase");
    for date in ["2026-01-05", "2026-02-05"] {
        state.sale_service.create_sale(CreateSalePayload {
            charges: vec![],
            transaction_date: date.into(), party_id: None, payment_status: PaymentStatus::Paid, paid_amount: 5.0,
            payment_mode: Some("CASH".into()), notes: None,
            items: vec![CreateSaleItemPayload { item_id, bundle_id: None, bundle_quantity: 1.0, unit_price: 5.0 }],
        }).expect("sale");
    }
    // Frontend shape: start_date / end_date only.
    let filter: SalesHistoryFilter = serde_json::from_value(serde_json::json!({
        "start_date": "2026-02-01", "end_date": "2026-02-28", "page": 1, "limit": 50
    })).expect("filter json");
    let page = state.sale_service.get_sales_history(filter).expect("history");
    assert_eq!(page.items.len(), 1, "only the February sale is in range");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), ext));
    }
}

#[test]
fn tc_exp_015_expense_type_index_is_repaired_at_startup() {
    use book_keeper::core::dtos::{CreateExpenseTypePayload, ExpensePayload};
    let path = std::env::temp_dir().join(format!("bk_smoke_exp_{}.db", uuid::Uuid::new_v4()));
    {
        let state = build_sqlite_app_state(&path).expect("build state");
        for n in ["Printer Ink", "Electricity Bill"] {
            state.expense_service.create_expense_type(CreateExpenseTypePayload { name: n.into() }).unwrap();
        }
        let t = state.expense_service.search_expense_types("elec", None).unwrap()[0].id;
        let e = state
            .expense_service
            .create_expense(ExpensePayload { expense_date: "2026-11-01".into(), expense_type_id: t, amount: 5.0, notes: None })
            .unwrap();
        assert_eq!(e.expense_code, "EXP-1001");
    }
    {
        let c = rusqlite::Connection::open(&path).unwrap();
        c.execute("DELETE FROM expense_type_search_fts", []).unwrap();
    }
    let state = build_sqlite_app_state(&path).expect("rebuild state");
    let hits = state.expense_service.search_expense_types("pri", None).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].name, "Printer Ink");
    assert_eq!(state.expense_service.search_expense_types("elec", None).unwrap().len(), 1);
    drop(state);
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), ext));
    }
}
