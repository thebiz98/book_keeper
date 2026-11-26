// tests/common/ledger.rs
//! Money-test helpers: seed parties, stocked items and ledger-consistent bills through the
//! PRODUCTION services, and build checkout / inward payloads.

#![allow(dead_code)]

use super::fixtures::*;
use super::stock::*;
use super::TestDb;
use book_keeper::core::dtos::*;
use book_keeper::core::entities::*;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::{TransactionContext, UnitOfWork};
use book_keeper::services::contracts::inventory_service::InventoryService;
use book_keeper::services::contracts::payment_service::PaymentService;
use book_keeper::services::PaymentServiceImpl;

pub const DAY: &str = "2026-11-20";

/// Commits one party row.
pub fn seed_party(db: &TestDb, id: i64, name: &str, party_type: PartyType) {
    let mut tx = db.begin_transaction().expect("begin");
    tx.parties().create(&create_test_party(id, name, party_type, None)).expect("party");
    tx.commit().expect("commit");
}

/// Commits an item whose stock arrives as one purchase lot of `qty` @ `cost` (cache stays consistent).
pub fn seed_stocked_item(
    db: &TestDb,
    inv: &dyn InventoryService,
    item_id: i64,
    qty: f64,
    cost: f64,
    price: f64,
) {
    let mut tx = db.begin_transaction().expect("begin");
    let item = create_test_item(item_id, &format!("Item {item_id}"), None, None, 0.0, price);
    tx.items().create(&item).expect("item");
    if qty > 0.0 {
        seed_purchase_lot(inv, &mut *tx, item_id, item_id, qty, cost);
    }
    tx.commit().expect("commit");
}

/// Sale header with `paid 0 / balance total` plus an UPFRONT ledger row of `upfront` (if > 0),
/// so the header is ledger-consistent. Runs inside the caller's transaction.
pub fn seed_sale_bill(
    tx: &mut dyn TransactionContext,
    id: i64,
    party_id: Option<i64>,
    total: f64,
    upfront: f64,
    date: &str,
) {
    let sale = create_test_sale_tx(id, &format!("SAL-{}", 1000 + id), party_id, total, 0.0, date);
    tx.sales().create_transaction(&sale).expect("sale header");
    if upfront > 0.0 {
        pay(tx, TransactionType::Sale, id, party_id, PaymentKind::Upfront, upfront, date);
    }
}

/// PURCHASE-source header, ledger-consistent like [`seed_sale_bill`].
pub fn seed_purchase_bill(
    tx: &mut dyn TransactionContext,
    id: i64,
    party_id: Option<i64>,
    total: f64,
    upfront: f64,
    date: &str,
) {
    let pur = create_test_purchase_tx(id, &format!("PUR-{}", 1000 + id), party_id, total, 0.0, date);
    tx.purchases().create_transaction(&pur).expect("purchase header");
    if upfront > 0.0 {
        pay(tx, TransactionType::Purchase, id, party_id, PaymentKind::Upfront, upfront, date);
    }
}

/// One balance-reducing ledger entry through `PaymentService.record_payment`.
pub fn pay(
    tx: &mut dyn TransactionContext,
    tx_type: TransactionType,
    tx_id: i64,
    party_id: Option<i64>,
    kind: PaymentKind,
    amount: f64,
    date: &str,
) -> i64 {
    PaymentServiceImpl
        .record_payment(
            tx,
            RecordPaymentPayload {
                party_id,
                transaction_type: tx_type,
                transaction_id: tx_id,
                kind,
                return_id: None,
                settlement_date: date.to_string(),
                amount_paid: amount,
                payment_mode: Some("CASH".to_string()),
                notes: None,
            },
        )
        .expect("record_payment")
}

/// A REFUND-type return header (FK parent for RETURN_CREDIT / refund rows).
pub fn seed_return(
    tx: &mut dyn TransactionContext,
    id: i64,
    return_type: ReturnType,
    party_id: Option<i64>,
    original: Option<i64>,
    total: f64,
    date: &str,
) {
    let r = ReturnTransaction {
        id,
        return_code: format!("RET-{}", 1000 + id),
        return_date: date.to_string(),
        return_type,
        party_id,
        party_name: None,
        original_transaction_id: original,
        settlement_type: SettlementType::Refund,
        total_amount: total,
        reason: None,
        notes: None,
        created_at: format!("{date}T10:00:00Z"),
    };
    tx.returns().create_return_transaction(&r).expect("return header");
}

/// Checkout payload; `lines` = (item_id, bundle_id, bundle_quantity, unit_price).
pub fn sale_payload(
    party_id: Option<i64>,
    status: PaymentStatus,
    paid: f64,
    lines: &[(i64, Option<i64>, f64, f64)],
) -> CreateSalePayload {
    CreateSalePayload {
        confirm_move_settlements: false,
        charges: vec![],
        transaction_date: DAY.to_string(),
        party_id,
        payment_status: status,
        paid_amount: paid,
        payment_mode: Some("CASH".to_string()),
        notes: None,
        items: lines
            .iter()
            .map(|&(item_id, bundle_id, bundle_quantity, unit_price)| CreateSaleItemPayload { sale_item_id: None,
                item_id,
                bundle_id,
                bundle_quantity,
                unit_price,
            })
            .collect(),
    }
}

/// Inward payload; `lines` = (item_id, bundle_id, bundle_quantity, unit_cost).
pub fn purchase_payload(
    party_id: Option<i64>,
    status: PaymentStatus,
    paid: f64,
    lines: &[(i64, Option<i64>, f64, f64)],
) -> CreatePurchasePayload {
    CreatePurchasePayload {
        confirm_move_settlements: false,
        charges: vec![],
        transaction_date: DAY.to_string(),
        party_id,
        supplier_invoice_ref: None,
        payment_status: status,
        paid_amount: paid,
        payment_mode: Some("CASH".to_string()),
        notes: None,
        items: lines
            .iter()
            .map(|&(item_id, bundle_id, bundle_quantity, unit_cost)| CreatePurchaseItemPayload { purchase_item_id: None,
                item_id,
                bundle_id,
                bundle_quantity,
                unit_cost,
            })
            .collect(),
    }
}

/// Settlement payload for one party.
pub fn settle_payload(
    party_id: i64,
    tx_type: TransactionType,
    bill: Option<i64>,
    amount: f64,
) -> SettlePaymentPayload {
    SettlePaymentPayload {
        party_id,
        transaction_type: tx_type,
        transaction_id: bill,
        settlement_date: DAY.to_string(),
        amount_paid: amount,
        payment_mode: Some("CASH".to_string()),
        notes: None,
    }
}

/// Counts every ledger row in the database (used to prove "no rows written").
pub fn ledger_row_count(db: &TestDb) -> usize {
    let mut tx = db.begin_read().expect("read");
    tx.settlements().list_all().expect("list").len()
}
