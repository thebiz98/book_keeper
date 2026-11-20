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
        transaction_date: date.to_string(),
        party_id,
        payment_mode: Some("CASH".to_string()),
        payment_status,
        total_amount: total,
        paid_amount: paid,
        balance_due: balance.max(0.0),
        notes: None,
        created_at: format!("{}T10:00:00Z", date),
    }
}

/// Factory helper for creating test PurchaseItem lots (FIFO Lots)
pub fn create_test_purchase_lot(
    id: i64,
    purchase_tx_id: i64,
    item_id: i64,
    bundle_id: Option<i64>,
    bundle_quantity: f64,
    normalized_qty: f64,
    unit_cost: f64,
    remaining_qty: f64,
) -> PurchaseItem {
    PurchaseItem {
        id,
        purchase_transaction_id: purchase_tx_id,
        item_id,
        bundle_id,
        bundle_quantity,
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
        payment_mode: Some("CASH".to_string()),
        payment_status,
        total_amount: total,
        paid_amount: paid,
        balance_due: balance.max(0.0),
        notes: None,
        created_at: format!("{}T10:00:00Z", date),
    }
}

/// Factory helper for creating test SaleItem lines
pub fn create_test_sale_item(
    id: i64,
    sale_tx_id: i64,
    item_id: i64,
    bundle_id: Option<i64>,
    bundle_quantity: f64,
    quantity: f64,
    unit_price: f64,
) -> SaleItem {
    SaleItem {
        id,
        sale_transaction_id: sale_tx_id,
        item_id,
        bundle_id,
        bundle_quantity,
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
        unit_cost,
    }
}

/// Factory helper for creating test PaymentSettlement rows
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
        transaction_type: transaction_type.to_string(),
        transaction_id,
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
        reason,
        notes: None,
        created_at: format!("{}T14:00:00Z", date),
    }
}
