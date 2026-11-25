// tests/common/fixtures.rs
//! Reusable test fixtures and fluent builders for domain models and DTOs.
//! Built according to Book Keeper LLD Section 3 and Section 11.

#![allow(dead_code, unused_imports)]

use book_keeper::core::entities::*;
use book_keeper::core::value_objects::*;

/// Factory helper for creating test Item entities
pub fn create_test_item(
    id: i64,
    name: &str,
    category: Option<&str>,
    barcode: Option<&str>,
    stock: f64,
    sale_price: f64,
) -> Item {
    Item {
        id,
        name: name.to_string(),
        item_type: ItemType::Physical,
        category: category.map(|s| s.to_string()),
        barcode: barcode.map(|s| s.to_string()),
        base_unit: "PCS".to_string(),
        mrp: Some(sale_price * 1.2),
        default_sale_price: sale_price,
        current_stock: stock,
        is_low_stock: stock < 5.0,
        is_active: true,
        created_at: "2026-11-20T10:00:00Z".to_string(),
        updated_at: "2026-11-20T10:00:00Z".to_string(),
        low_stock_threshold: 5.0,
    }
}

/// Factory helper for creating test ItemBundle entities
pub fn create_test_bundle(
    id: i64,
    item_id: i64,
    name: &str,
    units_per_bundle: f64,
    price: Option<f64>,
    barcode: Option<&str>,
) -> ItemBundle {
    ItemBundle {
        id,
        item_id,
        bundle_name: name.to_string(),
        units_per_bundle,
        mrp: price.map(|p| p * 1.1),
        default_bundle_price: price,
        barcode: barcode.map(|s| s.to_string()),
    }
}

/// Factory helper for creating test Party entities (Customer or Vendor)
pub fn create_test_party(
    id: i64,
    name: &str,
    party_type: PartyType,
    phone: Option<&str>,
) -> Party {
    Party {
        id,
        name: name.to_string(),
        phone: phone.map(|s| s.to_string()),
        party_type,
        notes: Some("Test account".to_string()),
        is_active: true,
        created_at: "2026-11-20T10:00:00Z".to_string(),
        updated_at: "2026-11-20T10:00:00Z".to_string(),
        opening_balance: None,
    }
}

/// Factory helper for creating test PurchaseTransaction entities
pub fn create_test_purchase_tx(
    id: i64,
    code: &str,
    party_id: Option<i64>,
    total: f64,
    paid: f64,
    date: &str,
) -> PurchaseTransaction {
    let balance = total - paid;
    let payment_status = if balance <= 0.0 {
        PaymentStatus::Paid
    } else if paid > 0.0 {
        PaymentStatus::Partial
    } else {
        PaymentStatus::Unpaid
    };

    PurchaseTransaction {
        id,
        transaction_code: code.to_string(),
        source: LotSource::Purchase,
        supplier_invoice_ref: None,
        transaction_date: date.to_string(),
        party_id,
        party_name: None,
        payment_mode: Some("CASH".to_string()),
        payment_status,
        total_amount: total,
        paid_amount: paid,
        balance_due: balance.max(0.0),
        notes: None,
        created_at: format!("{}T10:00:00Z", date),
        items_subtotal: total,
        charges_total: 0.0,
        discounts_total: 0.0,
        updated_at: None,
        is_opening_balance: false,
    }
}

/// Factory helper for creating test PurchaseItem lots (FIFO Lots).
/// `_bundle_id` is accepted for call-site compatibility but ignored: bill lines no longer
/// reference bundles (DD-28); `pack_quantity` is the entered pack quantity, `pack_label` None.
pub fn create_test_purchase_lot(
    id: i64,
    purchase_tx_id: i64,
    item_id: i64,
    _bundle_id: Option<i64>,
    bundle_quantity: f64,
    normalized_qty: f64,
    unit_cost: f64,
    remaining_qty: f64,
) -> PurchaseItem {
    PurchaseItem {
        id,
        purchase_transaction_id: purchase_tx_id,
        item_id,
        item_name: format!("Item {item_id}"),
        pack_label: None,
        pack_quantity: bundle_quantity,
        quantity: normalized_qty,
        unit_cost,
        total_amount: normalized_qty * unit_cost,
        remaining_quantity: remaining_qty,
    }
}

/// Factory helper for creating test SaleTransaction entities
pub fn create_test_sale_tx(
    id: i64,
    code: &str,
    party_id: Option<i64>,
    total: f64,
    paid: f64,
    date: &str,
) -> SaleTransaction {
    let balance = total - paid;
    let payment_status = if balance <= 0.0 {
        PaymentStatus::Paid
    } else if paid > 0.0 {
        PaymentStatus::Partial
    } else {
        PaymentStatus::Unpaid
    };

    SaleTransaction {
        id,
        transaction_code: code.to_string(),
        transaction_date: date.to_string(),
        party_id,
        party_name: None,
        payment_mode: Some("CASH".to_string()),
        payment_status,
        total_amount: total,
        paid_amount: paid,
        balance_due: balance.max(0.0),
        notes: None,
        created_at: format!("{}T10:00:00Z", date),
        items_subtotal: total,
        charges_total: 0.0,
        discounts_total: 0.0,
        updated_at: None,
        is_opening_balance: false,
    }
}

/// Factory helper for creating test SaleItem lines (`_bundle_id` ignored, see DD-28).
pub fn create_test_sale_item(
    id: i64,
    sale_tx_id: i64,
    item_id: i64,
    _bundle_id: Option<i64>,
    bundle_quantity: f64,
    quantity: f64,
    unit_price: f64,
) -> SaleItem {
    SaleItem {
        id,
        sale_transaction_id: sale_tx_id,
        item_id,
        item_name: format!("Item {item_id}"),
        pack_label: None,
        pack_quantity: bundle_quantity,
        quantity,
        unit_price,
        total_amount: quantity * unit_price,
    }
}

/// Factory helper for creating test SaleStockAllocation records
pub fn create_test_allocation(
    id: i64,
    sale_item_id: i64,
    purchase_item_id: i64,
    quantity_depleted: f64,
    unit_cost: f64,
) -> SaleStockAllocation {
    SaleStockAllocation {
        id,
        sale_item_id,
        purchase_item_id,
        quantity_depleted,
        returned_quantity: 0.0,
        unit_cost,
    }
}

/// Factory helper for creating test PaymentSettlement rows (`transaction_type` is "SALE" or
/// "PURCHASE"; the row is a dues `SETTLEMENT`; `amount_paid` must be > 0).
pub fn create_test_settlement(
    id: i64,
    party_id: Option<i64>,
    transaction_type: &str,
    transaction_id: i64,
    amount_paid: f64,
    date: &str,
) -> PaymentSettlement {
    PaymentSettlement {
        id,
        settlement_date: date.to_string(),
        party_id,
        kind: PaymentKind::Settlement,
        transaction_type: match transaction_type {
            "SALE" => TransactionType::Sale,
            "PURCHASE" => TransactionType::Purchase,
            other => panic!("unknown transaction_type {other}"),
        },
        transaction_id: Some(transaction_id),
        return_id: None,
        amount_paid,
        payment_mode: Some("CASH".to_string()),
        notes: None,
        created_at: format!("{}T12:00:00Z", date),
    }
}

/// Factory helper for creating test StockAdjustment rows
pub fn create_test_adjustment(
    id: i64,
    item_id: i64,
    quantity: f64,
    reason: AdjustmentReason,
    date: &str,
) -> StockAdjustment {
    StockAdjustment {
        id,
        adjustment_date: date.to_string(),
        item_id,
        quantity,
        unit_cost: None,
        reason,
        notes: None,
        created_at: format!("{}T14:00:00Z", date),
    }
}

// -------------------------------------------------------------------------
// Referential seeding helpers.
// The SQLite schema enforces foreign keys, so low-level strategy/repository
// tests must create the parent rows the in-memory doubles never required.
// Seeded parents use high ids / "SEED-" codes so they never collide with ids
// or "SAL-"/"PUR-" code sequences the tests assert on.
// -------------------------------------------------------------------------

use book_keeper::data::contracts::unit_of_work::TransactionContext;

const SEED_ID_BASE: i64 = 900_000;

/// Creates a placeholder item with `item_id` if it does not exist yet.
pub fn ensure_item(tx: &mut dyn TransactionContext, item_id: i64) {
    if tx.items().find_by_id(item_id).expect("find item").is_none() {
        let item = create_test_item(item_id, &format!("Seed Item {item_id}"), None, None, 0.0, 1.0);
        tx.items().create(&item).expect("seed item");
    }
}

/// Creates a placeholder purchase header with `purchase_id` if missing.
pub fn ensure_purchase(tx: &mut dyn TransactionContext, purchase_id: i64) {
    if tx.purchases().find_transaction_by_id(purchase_id).expect("find purchase").is_none() {
        let pur = create_test_purchase_tx(purchase_id, &format!("SEED-PUR-{purchase_id}"), None, 0.0, 0.0, "2026-01-01");
        tx.purchases().create_transaction(&pur).expect("seed purchase");
    }
}

/// Creates a sale header + sale line with id `sale_item_id` for `item_id`.
pub fn seed_sale_line(tx: &mut dyn TransactionContext, sale_item_id: i64, item_id: i64) {
    ensure_item(tx, item_id);
    let sale_id = SEED_ID_BASE + sale_item_id;
    let sale = create_test_sale_tx(sale_id, &format!("SEED-SAL-{sale_item_id}"), None, 0.0, 0.0, "2026-01-01");
    tx.sales().create_transaction(&sale).expect("seed sale");
    let line = create_test_sale_item(sale_item_id, sale_id, item_id, None, 0.0, 0.0, 0.0);
    tx.sales().create_sale_item(&line).expect("seed sale line");
}

/// Creates a stock adjustment header with `adjustment_id` for `item_id` (quantity must be non-zero).
pub fn seed_adjustment(tx: &mut dyn TransactionContext, adjustment_id: i64, item_id: i64) {
    ensure_item(tx, item_id);
    let adj = create_test_adjustment(adjustment_id, item_id, -1.0, AdjustmentReason::Damaged, "2026-01-01");
    tx.adjustments().create_adjustment(&adj).expect("seed adjustment");
}

/// Creates an empty (zero-quantity) purchase lot with `lot_id` if missing.
pub fn ensure_lot(tx: &mut dyn TransactionContext, lot_id: i64, item_id: i64) {
    ensure_item(tx, item_id);
    let pur_id = SEED_ID_BASE + lot_id;
    ensure_purchase(tx, pur_id);
    let lot = create_test_purchase_lot(lot_id, pur_id, item_id, None, 0.0, 0.0, 0.0, 0.0);
    tx.purchases().create_purchase_item(&lot).expect("seed lot");
}

/// Inserts an expense (and its type `type_id` named `Type {type_id}` when missing); `id` doubles
/// as the code number (`EXP-{id}`).
pub fn create_test_expense(tx: &mut dyn TransactionContext, id: i64, date: &str, type_id: i64, amount: f64) {
    if tx.expenses().find_type_by_id(type_id).expect("find type").is_none() {
        let t = ExpenseType { id: type_id, name: format!("Type {type_id}"), is_active: true, created_at: "2026-01-01T00:00:00Z".into() };
        tx.expenses().create_type(&t).expect("seed expense type");
    }
    let name = tx.expenses().find_type_by_id(type_id).unwrap().unwrap().name;
    let e = Expense {
        id,
        expense_code: format!("EXP-{id}"),
        expense_date: date.to_string(),
        expense_type_id: type_id,
        expense_type_name: name,
        amount,
        notes: None,
        created_at: format!("{date}T09:00:00Z"),
        updated_at: None,
    };
    tx.expenses().create(&e).expect("seed expense");
}
