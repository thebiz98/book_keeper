// tests/return_delete_test.rs
//! Test Suite: FR-20 return delete on real SQLite (schema v8): stock, lot, ledger and code effects
//! are reversed exactly, newest first, and impossible deletes are rejected without changes.

mod common;

use book_keeper::core::dtos::*;
use book_keeper::core::entities::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::inventory_service::InventoryService;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::return_service::ReturnService;
use book_keeper::services::contracts::sale_service::SaleService;
use common::invariants::*;
use common::ledger::*;
use common::*;

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

use PaymentStatus::*;

const R: StockCondition = StockCondition::Restockable;
const D: StockCondition = StockCondition::DamagedScrap;

fn rline(item: i64, sale_item: Option<i64>, qty: f64, price: f64, cond: StockCondition) -> CustomerReturnItemPayload {
    CustomerReturnItemPayload { item_id: item, sale_item_id: sale_item, quantity: qty, unit_price: price, condition: cond, unit_cost_override: None }
}

fn cust(sale: Option<i64>, party: Option<i64>, st: SettlementType, items: Vec<CustomerReturnItemPayload>) -> CustomerReturnPayload {
    CustomerReturnPayload {
        original_sale_id: sale,
        party_id: party,
        return_date: DAY.to_string(),
        settlement_type: st,
        reason: None,
        notes: None,
        items,
        replacement_items: vec![],
    }
}

fn vret(purchase: Option<i64>, party: Option<i64>, lot: Option<i64>, qty: f64) -> VendorReturnPayload {
    VendorReturnPayload {
        original_purchase_id: purchase,
        party_id: party,
        return_date: DAY.to_string(),
        settlement_type: SettlementType::Refund,
        reason: None,
        notes: None,
        items: vec![VendorReturnItemPayload { item_id: 1, purchase_item_id: lot, quantity: qty }],
    }
}

impl Env {
    /// Item 1 stocked with one paid purchase lot; returns that lot's id.
    fn stocked(&self, qty: f64, cost: f64) -> i64 {
        seed_stocked_item(&self.db, &self.inv, 1, 0.0, 0.0, 100.0);
        let id = self.purchases.create_purchase(purchase_payload(None, Paid, qty * cost, &[(1, None, qty, cost)])).unwrap().purchase_id;
        self.lots(id)[0].id
    }
    fn lots(&self, purchase: i64) -> Vec<PurchaseItem> {
        self.db.begin_read().unwrap().purchases().list_items_by_transaction_id(purchase).unwrap()
    }
    fn lot(&self, id: i64) -> PurchaseItem {
        self.db.begin_read().unwrap().purchases().find_purchase_item_by_id(id).unwrap().unwrap()
    }
    fn sell(&self, party: Option<i64>, st: PaymentStatus, paid: f64, qty: f64, price: f64) -> i64 {
        self.sales.create_sale(sale_payload(party, st, paid, &[(1, None, qty, price)])).unwrap().sale_id
    }
    fn line(&self, sale: i64) -> i64 {
        self.db.begin_read().unwrap().sales().list_items_by_transaction_id(sale).unwrap()[0].id
    }
    fn sale(&self, id: i64) -> SaleTransaction {
        self.db.begin_read().unwrap().sales().find_transaction_by_id(id).unwrap().unwrap()
    }
    fn stock(&self) -> f64 {
        self.db.begin_read().unwrap().items().find_by_id(1).unwrap().unwrap().current_stock
    }
    fn returned(&self, sale_item: i64) -> f64 {
        let mut tx = self.db.begin_read().unwrap();
        tx.allocations().find_by_sale_item_id(sale_item).unwrap().iter().map(|a| a.returned_quantity).sum()
    }
    fn count(&self, table: &str) -> i64 {
        self.db.pool.get().unwrap().query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap()
    }
    fn exec(&self, sql: &str) {
        self.db.pool.get().unwrap().execute_batch(sql).unwrap();
    }
    fn snap(&self) -> String {
        let c = self.db.pool.get().unwrap();
        let q = |sql: &str| -> String { c.query_row(sql, [], |r| r.get::<_, Option<String>>(0)).unwrap().unwrap_or_default() };
        [
            q("SELECT group_concat(id||':'||remaining_quantity||':'||quantity) FROM purchase_items"),
            q("SELECT group_concat(id||':'||current_stock) FROM items"),
            q("SELECT group_concat(id||':'||kind||':'||amount_paid) FROM payment_settlements"),
            q("SELECT group_concat(id||':'||returned_quantity) FROM sale_stock_allocations"),
            q("SELECT group_concat(id) FROM return_transactions"),
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

#[test]
fn linked_restockable_refund_delete_restores_lot_balance_and_credit() {
    let e = env();
    let lot = e.stocked(20.0, 5.0);
    seed_party(&e.db, 1, "C", PartyType::Customer);
    let s = e.sell(Some(1), Unpaid, 0.0, 10.0, 10.0);
    let sl = e.line(s);
    let r = e.rets.create_customer_return(cust(Some(s), None, SettlementType::Refund, vec![rline(1, Some(sl), 3.0, 10.0, R)])).unwrap();
    assert_eq!((e.lot(lot).remaining_quantity, e.sale(s).balance_due), (13.0, 70.0));
    let d = e.rets.delete_return(r.return_id).unwrap();
    assert_eq!((d.removed_credit_amount, d.removed_cash_amount), (30.0, 0.0));
    assert_eq!((e.lot(lot).remaining_quantity, e.stock(), e.returned(sl)), (10.0, 10.0, 0.0));
    let h = e.sale(s);
    assert_eq!((h.balance_due, h.payment_status), (100.0, Unpaid));
    assert_eq!(e.count("return_transactions") + e.count("return_items"), 0);
    e.check();
}

#[test]
fn linked_refund_with_cash_delete_removes_the_cash_row() {
    let e = env();
    e.stocked(20.0, 5.0);
    let s = e.sell(None, Paid, 100.0, 10.0, 10.0);
    let sl = e.line(s);
    let r = e.rets.create_customer_return(cust(Some(s), None, SettlementType::Refund, vec![rline(1, Some(sl), 3.0, 10.0, R)])).unwrap();
    assert_eq!(e.acct.ledger_cash_summary(DAY, DAY).unwrap().customer_refund_out, 30.0);
    let d = e.rets.delete_return(r.return_id).unwrap();
    assert_eq!((d.removed_cash_amount, d.removed_credit_amount), (30.0, 0.0));
    assert_eq!(e.acct.ledger_cash_summary(DAY, DAY).unwrap().customer_refund_out, 0.0);
    e.check();
}

#[test]
fn linked_scrap_delete_reverses_returned_quantity_only() {
    let e = env();
    e.stocked(20.0, 5.0);
    let s = e.sell(None, Paid, 100.0, 10.0, 10.0);
    let sl = e.line(s);
    let r = e.rets.create_customer_return(cust(Some(s), None, SettlementType::Refund, vec![rline(1, Some(sl), 2.0, 10.0, D)])).unwrap();
    assert_eq!((e.returned(sl), e.stock()), (2.0, 10.0));
    e.rets.delete_return(r.return_id).unwrap();
    assert_eq!((e.returned(sl), e.stock()), (0.0, 10.0));
    e.check();
}

#[test]
fn delete_is_rejected_when_the_restocked_units_were_sold_again() {
    let e = env();
    e.stocked(10.0, 5.0);
    let s = e.sell(None, Paid, 100.0, 10.0, 10.0);
    let sl = e.line(s);
    let r = e.rets.create_customer_return(cust(Some(s), None, SettlementType::Refund, vec![rline(1, Some(sl), 3.0, 10.0, R)])).unwrap();
    e.sell(None, Paid, 30.0, 3.0, 10.0);
    let g = e.rets.get_return_delete_guard(r.return_id).unwrap();
    assert!(!g.can_delete && !g.can_edit);
    assert_eq!(g.reasons[0].code, "RESTOCKED_CONSUMED");
    let before = e.snap();
    assert_eq!(code(e.rets.delete_return(r.return_id)), "RESTOCKED_CONSUMED");
    assert_eq!(e.snap(), before);
    e.check();
}

#[test]
fn unlinked_restock_delete_removes_the_return_lot_and_header_and_retires_codes() {
    let e = env();
    e.stocked(10.0, 5.0);
    let r = e.rets.create_customer_return(cust(None, None, SettlementType::Refund, vec![rline(1, None, 4.0, 10.0, R)])).unwrap();
    assert_eq!(e.stock(), 14.0);
    let lots_before = e.count("purchase_items");
    let link: Option<i64> = e.db.pool.get().unwrap().query_row("SELECT purchase_item_id FROM return_items", [], |x| x.get(0)).unwrap();
    assert!(link.is_some(), "the RETURN lot is recorded on its line");
    let d = e.rets.delete_return(r.return_id).unwrap();
    assert_eq!(d.removed_cash_amount, 40.0);
    assert_eq!((e.stock(), e.count("purchase_items"), e.count("purchase_transactions")), (10.0, lots_before - 1, 1));
    let again = e.rets.create_customer_return(cust(None, None, SettlementType::Refund, vec![rline(1, None, 1.0, 10.0, R)])).unwrap();
    assert_eq!(again.return_code, "RET-1002", "RET code not reused");
    let rtl: String = e.db.pool.get().unwrap().query_row("SELECT transaction_code FROM purchase_transactions WHERE source = 'RETURN'", [], |x| x.get(0)).unwrap();
    assert_eq!(rtl, "RTL-0002", "RTL code not reused");
    e.check();
}

#[test]
fn unlinked_delete_is_rejected_once_its_lot_was_sold_from() {
    let e = env();
    e.stocked(2.0, 5.0);
    e.sell(None, Paid, 20.0, 2.0, 10.0);
    let r = e.rets.create_customer_return(cust(None, None, SettlementType::Refund, vec![rline(1, None, 3.0, 10.0, R)])).unwrap();
    e.sell(None, Paid, 10.0, 1.0, 10.0);
    let before = e.snap();
    assert_eq!(code(e.rets.delete_return(r.return_id)), "RETURN_LOT_USED");
    assert_eq!(e.snap(), before);
    e.check();
}

#[test]
fn unlinked_return_credit_over_two_bills_recomputes_both_on_delete() {
    let e = env();
    e.stocked(30.0, 5.0);
    seed_party(&e.db, 1, "C", PartyType::Customer);
    let a = e.sell(Some(1), Unpaid, 0.0, 5.0, 10.0);
    let b = e.sell(Some(1), Unpaid, 0.0, 5.0, 10.0);
    let r = e.rets.create_customer_return(cust(None, Some(1), SettlementType::Refund, vec![rline(1, None, 8.0, 10.0, R)])).unwrap();
    assert_eq!((e.sale(a).balance_due, e.sale(b).balance_due), (0.0, 20.0));
    let d = e.rets.delete_return(r.return_id).unwrap();
    assert_eq!((d.removed_credit_amount, d.removed_cash_amount), (80.0, 0.0));
    assert_eq!((e.sale(a).balance_due, e.sale(b).balance_due), (50.0, 50.0));
    assert_eq!((e.sale(a).payment_status, e.sale(b).payment_status), (Unpaid, Unpaid));
    e.check();
}

#[test]
fn legacy_unlinked_return_without_lot_link_is_found_by_header_notes() {
    let e = env();
    e.stocked(10.0, 5.0);
    let r = e.rets.create_customer_return(cust(None, None, SettlementType::Refund, vec![rline(1, None, 4.0, 10.0, R)])).unwrap();
    e.exec("UPDATE return_items SET purchase_item_id = NULL");
    e.rets.delete_return(r.return_id).unwrap();
    assert_eq!((e.stock(), e.count("purchase_transactions")), (10.0, 1));
    e.check();
}

#[test]
fn replacement_return_delete_puts_issued_units_back() {
    let e = env();
    let lot = e.stocked(10.0, 5.0);
    let s = e.sell(None, Paid, 20.0, 2.0, 10.0);
    let sl = e.line(s);
    let mut p = cust(Some(s), None, SettlementType::Replacement, vec![rline(1, Some(sl), 1.0, 10.0, R)]);
    p.replacement_items = vec![ReplacementItemPayload { item_id: 1, quantity: 1.0 }];
    let r = e.rets.create_customer_return(p).unwrap();
    assert_eq!((e.stock(), e.count("stock_outflow_allocations")), (8.0, 1));
    e.rets.delete_return(r.return_id).unwrap();
    assert_eq!((e.stock(), e.lot(lot).remaining_quantity, e.returned(sl), e.count("stock_outflow_allocations")), (8.0, 8.0, 0.0, 0));
    e.check();
}

#[test]
fn vendor_return_delete_restores_named_lot_and_fifo_returns() {
    let e = env();
    seed_party(&e.db, 1, "V", PartyType::Vendor);
    seed_stocked_item(&e.db, &e.inv, 1, 0.0, 0.0, 100.0);
    let p = e.purchases.create_purchase(purchase_payload(Some(1), Unpaid, 0.0, &[(1, None, 10.0, 5.0)])).unwrap().purchase_id;
    let lot = e.lots(p)[0].id;
    let named = e.rets.create_vendor_return(vret(Some(p), None, Some(lot), 3.0)).unwrap();
    let fifo = e.rets.create_vendor_return(vret(None, Some(1), None, 2.0)).unwrap();
    assert_eq!((e.lot(lot).remaining_quantity, e.stock()), (5.0, 5.0));
    e.rets.delete_return(named.return_id).unwrap();
    assert_eq!((e.lot(lot).remaining_quantity, e.stock()), (8.0, 8.0));
    e.rets.delete_return(fifo.return_id).unwrap();
    assert_eq!((e.lot(lot).remaining_quantity, e.stock()), (10.0, 10.0));
    assert_eq!(e.count("payment_settlements"), 0, "credit and refund rows are gone");
    let h = e.db.begin_read().unwrap().purchases().find_transaction_by_id(p).unwrap().unwrap();
    assert_eq!((h.balance_due, h.payment_status), (50.0, Unpaid));
    e.check();
}

#[test]
fn returns_on_one_bill_are_deleted_newest_first_and_the_sale_unlocks() {
    let e = env();
    e.stocked(20.0, 5.0);
    let s = e.sell(None, Paid, 100.0, 10.0, 10.0);
    let sl = e.line(s);
    let first = e.rets.create_customer_return(cust(Some(s), None, SettlementType::Refund, vec![rline(1, Some(sl), 2.0, 10.0, R)])).unwrap();
    let second = e.rets.create_customer_return(cust(Some(s), None, SettlementType::Refund, vec![rline(1, Some(sl), 3.0, 10.0, D)])).unwrap();
    let before = e.snap();
    assert_eq!(code(e.rets.delete_return(first.return_id)), "NEWER_RETURN");
    assert_eq!(e.snap(), before);
    e.rets.delete_return(second.return_id).unwrap();
    assert_eq!(e.returned(sl), 2.0);
    e.rets.delete_return(first.return_id).unwrap();
    assert_eq!((e.returned(sl), e.stock()), (0.0, 10.0));
    assert!(e.sales.get_sale_edit_guard(s).unwrap().can_edit);
    e.sales.update_sale(s, sale_payload(None, Paid, 50.0, &[(1, None, 5.0, 10.0)])).unwrap();
    e.check();
}
