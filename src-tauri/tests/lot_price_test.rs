// tests/lot_price_test.rs
//! Test Suite: FR-25 lot Purchase Price edit on real SQLite. Every case ends with the ledger,
//! stock-cache and lot-flow invariants.

mod common;

use book_keeper::core::dtos::*;
use book_keeper::core::entities::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::accounting_service::AccountingService;
use book_keeper::services::contracts::inventory_service::InventoryService;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::return_service::ReturnService;
use book_keeper::services::contracts::sale_service::SaleService;
use book_keeper::services::r#impl::bill_math::lot_price_upfront;
use common::invariants::*;
use common::ledger::*;
use common::*;
use PaymentStatus::*;

type Inv = InventoryServiceImpl<FifoStrategy>;
type Acct = AccountingServiceImpl<PaymentServiceImpl>;

struct Env {
    db: TestDb,
    inv: Inv,
    sales: SaleServiceImpl<Inv, PaymentServiceImpl>,
    purchases: PurchaseServiceImpl<Inv, PaymentServiceImpl>,
    rets: ReturnServiceImpl<Inv, PaymentServiceImpl, Acct>,
    acct: Acct,
}

fn env() -> Env {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let acct = AccountingServiceImpl::new(PaymentServiceImpl, db.clone());
    Env {
        sales: SaleServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        purchases: PurchaseServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        rets: ReturnServiceImpl::new(inv.clone(), PaymentServiceImpl, acct.clone(), db.clone()),
        acct,
        inv,
        db,
    }
}

impl Env {
    fn item(&self, id: i64) {
        seed_stocked_item(&self.db, &self.inv, id, 0.0, 0.0, 100.0);
    }
    fn buy(&self, party: Option<i64>, st: PaymentStatus, paid: f64, lines: &[(i64, f64, f64)]) -> i64 {
        let l: Vec<_> = lines.iter().map(|&(i, q, c)| (i, None, q, c)).collect();
        self.purchases.create_purchase(purchase_payload(party, st, paid, &l)).expect("purchase").purchase_id
    }
    fn pur(&self, id: i64) -> PurchaseTransaction {
        self.db.begin_read().unwrap().purchases().find_transaction_by_id(id).unwrap().unwrap()
    }
    fn lots(&self, purchase: i64) -> Vec<PurchaseItem> {
        self.db.begin_read().unwrap().purchases().list_items_by_transaction_id(purchase).unwrap()
    }
    fn lot(&self, id: i64) -> PurchaseItem {
        self.db.begin_read().unwrap().purchases().find_purchase_item_by_id(id).unwrap().unwrap()
    }
    fn ledger(&self, id: i64) -> Vec<PaymentSettlement> {
        self.db.begin_read().unwrap().settlements().list_by_transaction(TransactionType::Purchase, id).unwrap()
    }
    fn set(&self, lot: i64, price: f64) -> Result<LotPriceUpdateResult, AppError> {
        self.purchases.update_lot_purchase_price(UpdateLotPricePayload { purchase_item_id: lot, unit_price: price })
    }
    fn f(&self, sql: &str) -> f64 {
        self.db.pool.get().unwrap().query_row(sql, [], |r| r.get::<_, Option<f64>>(0)).unwrap().unwrap_or(0.0)
    }
    fn snap(&self) -> String {
        let c = self.db.pool.get().unwrap();
        let q = |sql: &str| -> String { c.query_row(sql, [], |r| r.get::<_, Option<String>>(0)).unwrap().unwrap_or_default() };
        [
            q("SELECT group_concat(id||':'||total_amount||':'||paid_amount||':'||COALESCE(updated_at,'')) FROM purchase_transactions"),
            q("SELECT group_concat(id||':'||quantity||':'||remaining_quantity||':'||unit_cost||':'||total_amount) FROM purchase_items"),
            q("SELECT group_concat(id||':'||kind||':'||amount_paid) FROM payment_settlements"),
            q("SELECT group_concat(id||':'||unit_cost) FROM sale_stock_allocations"),
            q("SELECT group_concat(id||':'||COALESCE(unit_cost,'')) FROM stock_adjustments"),
        ]
        .join("|")
    }
    fn check(&self) {
        assert_ledger_invariant(&self.db);
        assert_lot_flow_invariant(&self.db);
        let mut tx = self.db.begin_read().unwrap();
        let bad = self.inv.verify_stock_cache(&mut *tx).unwrap();
        assert!(bad.is_empty(), "stock cache drift: {bad:?}");
    }
}

fn code<T: std::fmt::Debug>(r: Result<T, AppError>) -> String {
    match r {
        Err(AppError::BillLocked { code, .. }) => code,
        other => panic!("expected BillLocked, got {other:?}"),
    }
}

fn kind_sum(rows: &[PaymentSettlement], k: PaymentKind) -> f64 {
    rows.iter().filter(|r| r.kind == k).map(|r| r.amount_paid).sum()
}

#[test]
fn payment_rule_table() {
    let cases: serde_json::Value = serde_json::from_str(include_str!("fixtures/lot_price_cases.json")).unwrap();
    for c in cases.as_array().unwrap() {
        let n = |k: &str| c[k].as_f64().unwrap();
        let got = lot_price_upfront(n("newTotal"), n("applied"), n("upfrontOld"), n("oldBalanceDue"));
        match c["error"].as_str() {
            Some(code) => match got {
                Err(AppError::BillLocked { code: got_code, .. }) => assert_eq!(got_code, code, "{}", c["name"]),
                other => panic!("{}: expected {code}, got {other:?}", c["name"]),
            },
            None => assert_eq!(got.unwrap(), n("upfrontNew"), "{}", c["name"]),
        }
    }
}

#[test]
fn cash_bill_price_up_and_down_stays_paid_and_cash_follows() {
    let e = env();
    e.item(1);
    let p = e.buy(None, Paid, 100.0, &[(1, 10.0, 10.0)]);
    let lot = e.lots(p)[0].id;
    let b = e.set(lot, 12.0).unwrap().bill.unwrap();
    assert_eq!((b.old_total, b.new_total, b.new_paid, b.balance_due, b.payment_status), (100.0, 120.0, 120.0, 0.0, Paid));
    assert_eq!(e.lot(lot).unit_cost, 12.0);
    assert_eq!(kind_sum(&e.ledger(p), PaymentKind::Upfront), 120.0);
    assert!(e.pur(p).updated_at.is_some());
    e.check();
    let b = e.set(lot, 8.0).unwrap().bill.unwrap();
    assert_eq!(b.new_paid, 80.0);
    assert_eq!(kind_sum(&e.ledger(p), PaymentKind::Upfront), 80.0);
    assert_eq!(e.pur(p).payment_status, Paid);
    e.check();
}

#[test]
fn partial_bill_balance_moves_and_becomes_paid_when_upfront_covers_it() {
    let e = env();
    seed_party(&e.db, 1, "V", PartyType::Vendor);
    e.item(1);
    let p = e.buy(Some(1), Partial, 40.0, &[(1, 10.0, 10.0)]);
    let lot = e.lots(p)[0].id;
    let b = e.set(lot, 15.0).unwrap().bill.unwrap();
    assert_eq!((b.new_total, b.new_paid, b.balance_due, b.payment_status), (150.0, 40.0, 110.0, Partial));
    let b = e.set(lot, 3.0).unwrap().bill.unwrap();
    assert_eq!((b.new_total, b.new_paid, b.balance_due, b.payment_status), (30.0, 30.0, 0.0, Paid));
    e.check();
}

#[test]
fn settled_later_bill_never_gets_an_invented_upfront() {
    let e = env();
    seed_party(&e.db, 1, "V", PartyType::Vendor);
    e.item(1);
    let p = e.buy(Some(1), Unpaid, 0.0, &[(1, 10.0, 10.0)]);
    let lot = e.lots(p)[0].id;
    e.acct.settle_bill_payment(settle_payload(1, TransactionType::Purchase, Some(p), 100.0)).unwrap();
    assert_eq!(e.pur(p).payment_status, Paid);
    let b = e.set(lot, 12.0).unwrap().bill.unwrap();
    assert_eq!((b.new_total, b.new_paid, b.balance_due, b.payment_status), (120.0, 100.0, 20.0, Partial));
    assert!(e.ledger(p).iter().all(|r| r.kind != PaymentKind::Upfront));
    e.check();
}

#[test]
fn total_below_applied_is_rejected_and_writes_nothing() {
    let e = env();
    seed_party(&e.db, 1, "V", PartyType::Vendor);
    e.item(1);
    let p = e.buy(Some(1), Unpaid, 0.0, &[(1, 10.0, 10.0)]);
    let lot = e.lots(p)[0].id;
    e.acct.settle_bill_payment(settle_payload(1, TransactionType::Purchase, Some(p), 80.0)).unwrap();
    let before = e.snap();
    assert_eq!(code(e.set(lot, 5.0)), "TOTAL_BELOW_APPLIED");
    assert_eq!(e.snap(), before);
    e.check();
}

#[test]
fn linked_vendor_return_blocks() {
    let e = env();
    seed_party(&e.db, 1, "V", PartyType::Vendor);
    e.item(1);
    let p = e.buy(Some(1), Paid, 100.0, &[(1, 10.0, 10.0)]);
    let lot = e.lots(p)[0].id;
    e.rets
        .create_vendor_return(VendorReturnPayload {
            original_purchase_id: Some(p),
            party_id: None,
            return_date: DAY.to_string(),
            settlement_type: SettlementType::Refund,
            reason: None,
            notes: None,
            items: vec![VendorReturnItemPayload { item_id: 1, purchase_item_id: Some(lot), quantity: 2.0 }],
        })
        .unwrap();
    let before = e.snap();
    assert_eq!(code(e.set(lot, 12.0)), "LINKED_RETURNS");
    assert_eq!(e.snap(), before);
    e.check();
}

#[test]
fn vendor_return_outflow_lot_blocks() {
    let e = env();
    seed_party(&e.db, 1, "V", PartyType::Vendor);
    e.item(1);
    let p = e.buy(Some(1), Paid, 100.0, &[(1, 10.0, 10.0)]);
    let lot = e.lots(p)[0].id;
    e.rets
        .create_vendor_return(VendorReturnPayload {
            original_purchase_id: None,
            party_id: Some(1),
            return_date: DAY.to_string(),
            settlement_type: SettlementType::Refund,
            reason: None,
            notes: None,
            items: vec![VendorReturnItemPayload { item_id: 1, purchase_item_id: None, quantity: 2.0 }],
        })
        .unwrap();
    let before = e.snap();
    assert_eq!(code(e.set(lot, 12.0)), "VENDOR_RETURNED_COST_CHANGE");
    assert_eq!(e.snap(), before);
    e.check();
}

#[test]
fn sold_lot_updates_cogs_but_the_sale_bill_is_unchanged() {
    let e = env();
    e.item(1);
    let p = e.buy(None, Paid, 100.0, &[(1, 10.0, 10.0)]);
    let lot = e.lots(p)[0].id;
    let s = e.sales.create_sale(sale_payload(None, Paid, 60.0, &[(1, None, 3.0, 20.0)])).unwrap().sale_id;
    let sale_before = e.db.begin_read().unwrap().sales().find_transaction_by_id(s).unwrap().unwrap();
    let cogs = || {
        e.f(&format!(
            "SELECT SUM(a.quantity_depleted * a.unit_cost) FROM sale_stock_allocations a \
             JOIN sale_items i ON i.id = a.sale_item_id WHERE i.sale_transaction_id = {s}"
        ))
    };
    assert_eq!(cogs(), 30.0);
    e.set(lot, 12.0).unwrap();
    assert_eq!(cogs(), 36.0);
    let sale_after = e.db.begin_read().unwrap().sales().find_transaction_by_id(s).unwrap().unwrap();
    assert_eq!(sale_before, sale_after);
    e.check();
}

#[test]
fn other_lines_and_charges_are_untouched() {
    let e = env();
    e.item(1);
    e.item(2);
    let p = e.buy(None, Paid, 130.0, &[(1, 10.0, 10.0), (2, 3.0, 10.0)]);
    let (a, b) = (e.lots(p)[0].clone(), e.lots(p)[1].clone());
    let r = e.set(a.id, 11.0).unwrap().bill.unwrap();
    assert_eq!(r.new_total, 140.0);
    assert_eq!(e.lot(b.id), b);
    let h = e.pur(p);
    assert_eq!((h.items_subtotal, h.total_amount, h.charges_total, h.discounts_total), (140.0, 140.0, 0.0, 0.0));
    e.check();
}

#[test]
fn opening_lot_is_recosted_only() {
    let e = env();
    e.item(1);
    let lot = {
        let mut tx = e.db.begin_transaction().unwrap();
        let id = e.inv.create_source_lot(&mut *tx, LotSource::Opening, DAY, 1, 5.0, Some(4.0), None).unwrap();
        tx.commit().unwrap();
        id
    };
    let r = e.set(lot, 6.0).unwrap();
    assert!(r.bill.is_none());
    assert_eq!((r.old_unit_cost, r.new_unit_cost), (4.0, 6.0));
    assert_eq!(e.lot(lot).unit_cost, 6.0);
    assert_eq!(e.f("SELECT COUNT(*) FROM payment_settlements"), 0.0);
    e.check();
}

#[test]
fn adjustment_lot_updates_the_adjustment_row() {
    let e = env();
    e.item(1);
    let res = e
        .inv
        .create_adjustment(StockAdjustmentPayload {
            adjustment_date: DAY.into(),
            item_id: 1,
            quantity: 4.0,
            reason: AdjustmentReason::UnrecordedStock,
            unit_cost: Some(5.0),
            notes: None,
        })
        .unwrap();
    let lot = e.inv.get_item_lots(1).unwrap().iter().find(|l| l.source == LotSource::Adjustment).unwrap().purchase_item_id;
    e.set(lot, 7.0).unwrap();
    assert_eq!(e.lot(lot).unit_cost, 7.0);
    assert_eq!(e.f(&format!("SELECT unit_cost FROM stock_adjustments WHERE id = {}", res.adjustment_id)), 7.0);
    e.check();
}

#[test]
fn return_lot_is_rejected() {
    let e = env();
    e.item(1);
    e.buy(None, Paid, 100.0, &[(1, 10.0, 10.0)]);
    e.sales.create_sale(sale_payload(None, Paid, 40.0, &[(1, None, 2.0, 20.0)])).unwrap();
    e.rets
        .create_customer_return(CustomerReturnPayload {
            original_sale_id: None,
            party_id: None,
            return_date: DAY.to_string(),
            settlement_type: SettlementType::Refund,
            reason: None,
            notes: None,
            items: vec![CustomerReturnItemPayload {
                item_id: 1,
                sale_item_id: None,
                quantity: 1.0,
                unit_price: 20.0,
                condition: StockCondition::Restockable,
                unit_cost_override: Some(10.0),
            }],
            replacement_items: vec![],
        })
        .unwrap();
    let lot = e.inv.get_item_lots(1).unwrap().iter().find(|l| l.source == LotSource::Return).unwrap().purchase_item_id;
    let before = e.snap();
    match e.set(lot, 9.0) {
        Err(AppError::ValidationError { field, .. }) => assert_eq!(field, "purchase_item_id"),
        other => panic!("expected ValidationError, got {other:?}"),
    }
    assert_eq!(e.snap(), before);
    e.check();
}

#[test]
fn unchanged_price_is_a_no_op() {
    let e = env();
    e.item(1);
    let p = e.buy(None, Paid, 100.0, &[(1, 10.0, 10.0)]);
    let lot = e.lots(p)[0].id;
    let before = e.snap();
    let r = e.set(lot, 10.0).unwrap();
    assert_eq!((r.old_unit_cost, r.new_unit_cost), (10.0, 10.0));
    assert_eq!(e.snap(), before);
    assert!(matches!(e.set(lot, -1.0), Err(AppError::ValidationError { .. })));
    e.check();
}
