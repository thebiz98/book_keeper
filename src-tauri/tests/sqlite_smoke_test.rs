use book_keeper::app_state::build_sqlite_app_state;
use book_keeper::core::dtos::{
    CreateItemPayload, CreatePurchaseItemPayload, CreatePurchasePayload, CreateSaleItemPayload,
    CreateSalePayload,
};

#[test]
fn sqlite_end_to_end_smoke() {
    let path = std::env::temp_dir().join(format!("bk_smoke_{}.db", uuid::Uuid::new_v4()));
    let state = build_sqlite_app_state(&path).expect("build state");

    let item_id = state
        .item_service
        .create_item(CreateItemPayload {
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
            transaction_code: None,
            transaction_date: "2026-01-01".to_string(),
            party_id: None,
            paid_amount: 100.0,
            payment_mode: Some("CASH".to_string()),
            notes: None,
            items: vec![CreatePurchaseItemPayload { item_id, bundle_id: None, bundle_quantity: 10.0, unit_cost: 10.0 }],
        })
        .expect("purchase");

    state
        .sale_service
        .create_sale(CreateSalePayload {
            transaction_code: None,
            transaction_date: "2026-01-02".to_string(),
            party_id: None,
            paid_amount: 30.0,
            payment_mode: Some("CASH".to_string()),
            notes: None,
            items: vec![CreateSaleItemPayload { item_id, bundle_id: None, bundle_quantity: 3.0, unit_price: Some(10.0) }],
        })
        .expect("sale");

    let second = state
        .sale_service
        .create_sale(CreateSalePayload {
            transaction_code: None,
            transaction_date: "2026-01-02".to_string(),
            party_id: None,
            paid_amount: 10.0,
            payment_mode: Some("CASH".to_string()),
            notes: None,
            items: vec![CreateSaleItemPayload { item_id, bundle_id: None, bundle_quantity: 1.0, unit_price: Some(10.0) }],
        })
        .expect("second auto-coded sale must not collide on transaction_code");
    assert_ne!(second.transaction_code, "SAL-1001");

    let catalog = state.item_service.get_catalog(None, false, 1, 10).expect("catalog");
    let item = catalog.items.iter().find(|i| i.id == item_id).expect("item in catalog");
    assert!((item.current_stock - 6.0).abs() < 1e-9, "stock was {}", item.current_stock);

    let hits = state.search_indexer.search("lux").expect("search");
    assert!(hits.iter().any(|h| h.item_id == item_id));
    let by_barcode = state.search_indexer.search("890123").expect("search barcode");
    assert!(by_barcode.iter().any(|h| h.item_id == item_id));

    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), ext));
    }
}
