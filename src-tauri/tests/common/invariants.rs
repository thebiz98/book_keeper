// tests/common/invariants.rs
//! FR-20 invariants, asserted after every correction (success and rejection alike).

#![allow(dead_code)]

use super::TestDb;
use book_keeper::core::money::MONEY_EPS;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;

/// Every sale and every PURCHASE-source bill: `paid = Σ UPFRONT + SETTLEMENT`,
/// `balance = total - paid - Σ RETURN_CREDIT >= 0`, the status follows the DD-01 rule and
/// `total = subtotal + charges - discounts`.
pub fn assert_ledger_invariant(db: &TestDb) {
    let mut tx = db.begin_read().expect("read");
    #[allow(clippy::type_complexity)]
    let mut bills: Vec<(TransactionType, i64, String, f64, f64, f64, PaymentStatus, f64, f64, f64)> = Vec::new();
    for s in tx.sales().list_sales(None, None, None, None, 0, 100_000).unwrap() {
        bills.push((
            TransactionType::Sale, s.id, s.transaction_code, s.total_amount, s.paid_amount, s.balance_due,
            s.payment_status, s.items_subtotal, s.charges_total, s.discounts_total,
        ));
    }
    for p in tx.purchases().list_purchases(None, None, None, None, 0, 100_000).unwrap() {
        bills.push((
            TransactionType::Purchase, p.id, p.transaction_code, p.total_amount, p.paid_amount, p.balance_due,
            p.payment_status, p.items_subtotal, p.charges_total, p.discounts_total,
        ));
    }
    for (t, id, code, total, paid, balance, status, sub, chg, disc) in bills {
        let l = tx.settlements().sum_by_kind_for_transaction(t, id).unwrap();
        assert!((paid - (l.upfront + l.settlement)).abs() < 1e-6, "{code}: paid {paid} != ledger {}", l.upfront + l.settlement);
        let expect = total - paid - l.return_credit;
        assert!(balance >= -1e-9 && (balance - expect).abs() < 1e-6, "{code}: balance {balance} != {expect}");
        let want = if balance <= MONEY_EPS {
            PaymentStatus::Paid
        } else if paid + l.return_credit > MONEY_EPS {
            PaymentStatus::Partial
        } else {
            PaymentStatus::Unpaid
        };
        assert_eq!(status, want, "{code} status");
        assert!((total - (sub + chg - disc)).abs() < 1e-6, "{code}: total {total} != {sub}+{chg}-{disc}");
    }
}

/// Per item: Σ(lot quantity - remaining) = Σ sale depleted + Σ outflow depleted - Σ linked
/// RESTOCKABLE returned quantity; `0 <= returned <= depleted`; no orphan allocations.
pub fn assert_lot_flow_invariant(db: &TestDb) {
    let c = db.pool.get().unwrap();
    let rows: Vec<(i64, f64, f64)> = {
        let mut st = c
            .prepare(
                "SELECT i.id, \
                   COALESCE((SELECT SUM(quantity - remaining_quantity) FROM purchase_items WHERE item_id = i.id), 0), \
                   COALESCE((SELECT SUM(a.quantity_depleted) FROM sale_stock_allocations a JOIN purchase_items p ON p.id = a.purchase_item_id WHERE p.item_id = i.id), 0) \
                 + COALESCE((SELECT SUM(o.quantity_depleted) FROM stock_outflow_allocations o JOIN purchase_items p ON p.id = o.purchase_item_id WHERE p.item_id = i.id), 0) \
                 - COALESCE((SELECT SUM(ri.quantity) FROM return_items ri WHERE ri.item_id = i.id AND ri.line_type = 'RETURNED' \
                     AND ri.condition = 'RESTOCKABLE' AND ri.sale_item_id IS NOT NULL), 0) \
                 FROM items i",
            )
            .unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().map(|x| x.unwrap()).collect()
    };
    for (item, consumed, flow) in rows {
        assert!((consumed - flow).abs() < 1e-6, "item {item}: lots consumed {consumed} != allocations {flow}");
    }
    let bad: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM sale_stock_allocations WHERE returned_quantity < 0 OR returned_quantity > quantity_depleted",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(bad, 0, "returned out of range");
    let orphans: i64 = c
        .query_row("SELECT COUNT(*) FROM sale_stock_allocations WHERE sale_item_id NOT IN (SELECT id FROM sale_items)", [], |r| r.get(0))
        .unwrap();
    assert_eq!(orphans, 0, "orphan allocations");
}
