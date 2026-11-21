// tests/common/stock.rs
//! Stock-test helpers: seed items and lots through the PRODUCTION InventoryService (so the
//! stock cache is consistent by construction) and assert the cache invariant.

#![allow(dead_code)]

use super::fixtures::*;
use book_keeper::core::constants::EPS;
use book_keeper::data::contracts::unit_of_work::TransactionContext;
use book_keeper::services::contracts::inventory_service::InventoryService;

/// Item with a zero stock cache (stock only ever arrives through InventoryService).
pub fn seed_empty_item(tx: &mut dyn TransactionContext, item_id: i64) {
    let item = create_test_item(item_id, &format!("Item {item_id}"), None, None, 0.0, 100.0);
    tx.items().create(&item).expect("seed item");
}

/// A PURCHASE header + one lot (`quantity`, `unit_cost`) added via `add_stock_for_purchase`.
/// `lot_id` doubles as the header id and must be unique per test.
pub fn seed_purchase_lot(
    inv: &dyn InventoryService,
    tx: &mut dyn TransactionContext,
    lot_id: i64,
    item_id: i64,
    quantity: f64,
    unit_cost: f64,
) {
    let header = create_test_purchase_tx(
        lot_id,
        &format!("PUR-T{lot_id:04}"),
        None,
        quantity * unit_cost,
        quantity * unit_cost,
        "2026-11-20",
    );
    tx.purchases().create_transaction(&header).expect("seed purchase header");
    let lot = create_test_purchase_lot(lot_id, lot_id, item_id, None, quantity, quantity, unit_cost, quantity);
    inv.add_stock_for_purchase(tx, &lot).expect("add stock for purchase");
}

/// Sale header + line `sale_item_id` for `item_id` (the allocation FK parent).
pub fn seed_sale(tx: &mut dyn TransactionContext, sale_item_id: i64, item_id: i64) {
    seed_sale_line(tx, sale_item_id, item_id);
}

pub fn lot_remaining(tx: &mut dyn TransactionContext, lot_id: i64) -> f64 {
    tx.purchases()
        .find_purchase_item_by_id(lot_id)
        .expect("find lot")
        .expect("lot exists")
        .remaining_quantity
}

/// TC-INV-008: `current_stock == Σ lot.remaining_quantity` for EVERY item, and `is_low_stock`
/// agrees with the threshold.
pub fn assert_stock_invariant(tx: &mut dyn TransactionContext) {
    for (item_id, cached, lots) in tx.items().stock_cache_audit().expect("audit") {
        assert!(
            (cached - lots).abs() <= EPS,
            "item {item_id}: cached {cached} != lots {lots}"
        );
        let item = tx.items().find_by_id(item_id).unwrap().unwrap();
        assert_eq!(item.is_low_stock, item.current_stock < book_keeper::core::LOW_STOCK_THRESHOLD, "item {item_id} low flag");
    }
}
