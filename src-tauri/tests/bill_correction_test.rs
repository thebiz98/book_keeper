// tests/bill_correction_test.rs
//! Test Suite: FR-20 bill edit / delete on real SQLite (schema v8). Every operation, successful or
//! rejected, ends with the ledger, stock-cache and lot-flow invariants.

mod common;

use std::sync::Arc;

use book_keeper::core::dtos::*;
use book_keeper::core::entities::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::data::contracts::FtsIndex;
use book_keeper::services::contracts::accounting_service::AccountingService;
use book_keeper::services::contracts::charge_type_service::ChargeTypeService;
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
    types: ChargeTypeServiceImpl,
}

fn env() -> Env {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let acct = AccountingServiceImpl::new(PaymentServiceImpl, db.clone());
    let index: Arc<dyn FtsIndex> = Arc::new(db.charge_type_index());
    Env {
        sales: SaleServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        purchases: PurchaseServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        rets: ReturnServiceImpl::new(inv.clone(), PaymentServiceImpl, acct.clone(), db.clone()),
        types: ChargeTypeServiceImpl::new(db.clone(), index),
        acct,
        inv,
        db,
    }
}

use PaymentStatus::*;

impl Env {
    /// Item with no stock yet.
    fn item(&self, id: i64) {
        seed_stocked_item(&self.db, &self.inv, id, 0.0, 0.0, 100.0);
    }
    /// New item whose stock arrives through a real, fully paid purchase. Returns the purchase id.
    fn stocked(&self, item: i64, qty: f64, cost: f64) -> i64 {
        self.item(item);
        self.buy(None, Paid, qty * cost, &[(item, qty, cost)])
    }
    fn buy(&self, party: Option<i64>, st: PaymentStatus, paid: f64, lines: &[(i64, f64, f64)]) -> i64 {
        let l: Vec<_> = lines.iter().map(|&(i, q, c)| (i, None, q, c)).collect();
        self.purchases.create_purchase(purchase_payload(party, st, paid, &l)).expect("purchase").purchase_id
    }
    fn sell(&self, party: Option<i64>, st: PaymentStatus, paid: f64, lines: &[(i64, f64, f64)]) -> i64 {
        let l: Vec<_> = lines.iter().map(|&(i, q, p)| (i, None, q, p)).collect();
        self.sales.create_sale(sale_payload(party, st, paid, &l)).expect("sale").sale_id
    }
    fn sp(&self, party: Option<i64>, st: PaymentStatus, paid: f64, lines: &[(i64, f64, f64)]) -> CreateSalePayload {
        let l: Vec<_> = lines.iter().map(|&(i, q, p)| (i, None, q, p)).collect();
        sale_payload(party, st, paid, &l)
    }
    /// Edit payload for one purchase line that keeps `lot` (or a new line when `None`).
    fn pp(&self, party: Option<i64>, st: PaymentStatus, paid: f64, lines: &[(Option<i64>, i64, f64, f64)]) -> CreatePurchasePayload {
        let l: Vec<_> = lines.iter().map(|&(_, i, q, c)| (i, None, q, c)).collect();
        let mut p = purchase_payload(party, st, paid, &l);
        for (it, &(lot, ..)) in p.items.iter_mut().zip(lines) {
            it.purchase_item_id = lot;
        }
        p
    }
    fn sale(&self, id: i64) -> SaleTransaction {
        self.db.begin_read().unwrap().sales().find_transaction_by_id(id).unwrap().unwrap()
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
    fn stock(&self, item: i64) -> f64 {
        self.db.begin_read().unwrap().items().find_by_id(item).unwrap().unwrap().current_stock
    }
    fn ledger(&self, t: TransactionType, id: i64) -> Vec<PaymentSettlement> {
        self.db.begin_read().unwrap().settlements().list_by_transaction(t, id).unwrap()
    }
    fn sale_lines(&self, id: i64) -> Vec<SaleItem> {
        self.db.begin_read().unwrap().sales().list_items_by_transaction_id(id).unwrap()
    }
    fn f(&self, sql: &str) -> f64 {
        self.db.pool.get().unwrap().query_row(sql, [], |r| r.get::<_, Option<f64>>(0)).unwrap().unwrap_or(0.0)
    }
    fn cogs(&self, sale: i64) -> f64 {
        self.f(&format!(
            "SELECT SUM((a.quantity_depleted - a.returned_quantity) * a.unit_cost) FROM sale_stock_allocations a \
             JOIN sale_items i ON i.id = a.sale_item_id WHERE i.sale_transaction_id = {sale}"
        ))
    }
    fn count(&self, table: &str) -> i64 {
        self.db.pool.get().unwrap().query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap()
    }
    /// Everything a rejected edit must leave untouched.
    fn snap(&self) -> String {
        let c = self.db.pool.get().unwrap();
        let q = |sql: &str| -> String { c.query_row(sql, [], |r| r.get::<_, Option<String>>(0)).unwrap().unwrap_or_default() };
        [
            q("SELECT group_concat(id||':'||total_amount||':'||paid_amount||':'||COALESCE(party_id,'')||':'||transaction_date) FROM sale_transactions"),
            q("SELECT group_concat(id||':'||total_amount||':'||paid_amount||':'||COALESCE(party_id,'')) FROM purchase_transactions"),
            q("SELECT group_concat(id||':'||item_id||':'||quantity||':'||remaining_quantity||':'||unit_cost) FROM purchase_items"),
            q("SELECT group_concat(id||':'||current_stock) FROM items"),
            q("SELECT group_concat(id||':'||kind||':'||amount_paid||':'||COALESCE(party_id,'')||':'||settlement_date) FROM payment_settlements"),
            q("SELECT group_concat(id||':'||sale_item_id||':'||purchase_item_id||':'||quantity_depleted||':'||returned_quantity||':'||unit_cost) FROM sale_stock_allocations"),
            q("SELECT group_concat(id||':'||quantity) FROM sale_items"),
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

fn field<T: std::fmt::Debug>(r: Result<T, AppError>) -> String {
    match r {
        Err(AppError::ValidationError { field, .. }) => field,
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

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

// ================================================================ sale edit

#[test]
fn sale_edit_raising_quantity_redepletes_fifo_and_keeps_id_and_code() {
    let e = env();
    e.stocked(1, 10.0, 5.0);
    e.buy(None, Paid, 60.0, &[(1, 10.0, 6.0)]);
    let id = e.sell(None, Paid, 40.0, &[(1, 4.0, 10.0)]);
    let code0 = e.sale(id).transaction_code;
    assert!(e.sale(id).updated_at.is_none());
    let r = e.sales.update_sale(id, e.sp(None, Paid, 120.0, &[(1, 12.0, 10.0)])).unwrap();
    assert_eq!((r.sale_id, r.transaction_code.as_str()), (id, code0.as_str()));
    let s = e.sale(id);
    assert_eq!((s.total_amount, s.paid_amount, s.balance_due, s.payment_status), (120.0, 120.0, 0.0, Paid));
    assert!(s.updated_at.is_some());
    assert_eq!(e.cogs(id), 62.0, "10 x 5 from the oldest lot + 2 x 6");
    assert_eq!(e.stock(1), 8.0);
    e.check();
}

#[test]
fn sale_edit_lowering_quantity_restores_lots() {
    let e = env();
    let p = e.stocked(1, 10.0, 5.0);
    let id = e.sell(None, Paid, 60.0, &[(1, 6.0, 10.0)]);
    e.sales.update_sale(id, e.sp(None, Paid, 20.0, &[(1, 2.0, 10.0)])).unwrap();
    assert_eq!(e.lots(p)[0].remaining_quantity, 8.0);
    assert_eq!(e.stock(1), 8.0);
    assert_eq!(e.cogs(id), 10.0);
    e.check();
}

#[test]
fn sale_edit_changing_item_restores_old_and_depletes_new() {
    let e = env();
    e.stocked(1, 10.0, 5.0);
    e.stocked(2, 10.0, 5.0);
    let id = e.sell(None, Paid, 30.0, &[(1, 3.0, 10.0)]);
    e.sales.update_sale(id, e.sp(None, Paid, 20.0, &[(2, 2.0, 10.0)])).unwrap();
    assert_eq!((e.stock(1), e.stock(2)), (10.0, 8.0));
    assert_eq!(e.sale_lines(id)[0].item_id, 2);
    e.check();
}

#[test]
fn sale_edit_can_use_its_own_units_and_a_failed_edit_changes_nothing() {
    let e = env();
    e.stocked(1, 5.0, 5.0);
    let id = e.sell(None, Paid, 50.0, &[(1, 5.0, 10.0)]);
    e.sales.update_sale(id, e.sp(None, Paid, 50.0, &[(1, 5.0, 10.0)])).unwrap();
    let before = e.snap();
    let r = e.sales.update_sale(id, e.sp(None, Paid, 60.0, &[(1, 6.0, 10.0)]));
    assert!(matches!(r, Err(AppError::InsufficientStockError { .. })), "{r:?}");
    assert_eq!(e.snap(), before, "rollback leaves every row as it was");
    e.check();
}

#[test]
fn sale_edit_moves_upfront_cash_to_the_new_date() {
    let e = env();
    e.stocked(1, 10.0, 5.0);
    let id = e.sell(None, Paid, 100.0, &[(1, 1.0, 100.0)]);
    let mut p = e.sp(None, Paid, 100.0, &[(1, 1.0, 100.0)]);
    p.transaction_date = "2026-11-21".to_string();
    e.sales.update_sale(id, p).unwrap();
    let up = e.ledger(TransactionType::Sale, id);
    assert_eq!(up.len(), 1);
    assert_eq!(up[0].settlement_date, "2026-11-21");
    assert_eq!(e.acct.ledger_cash_summary(DAY, DAY).unwrap().sale_upfront, 0.0);
    assert_eq!(e.acct.ledger_cash_summary("2026-11-21", "2026-11-21").unwrap().sale_upfront, 100.0);
    e.check();
}

#[test]
fn sale_edit_payment_transitions_follow_the_ledger() {
    let e = env();
    seed_party(&e.db, 1, "A", PartyType::Customer);
    e.stocked(1, 10.0, 5.0);
    let id = e.sell(Some(1), Paid, 100.0, &[(1, 1.0, 100.0)]);
    let r = e.sales.update_sale(id, e.sp(Some(1), Partial, 40.0, &[(1, 1.0, 100.0)])).unwrap();
    assert_eq!((r.paid_amount, r.balance_due, r.payment_status), (40.0, 60.0, Partial));
    assert_eq!(e.ledger(TransactionType::Sale, id).len(), 1, "one UPFRONT row, replaced");
    let r = e.sales.update_sale(id, e.sp(Some(1), Unpaid, 0.0, &[(1, 1.0, 100.0)])).unwrap();
    assert_eq!((r.paid_amount, r.balance_due, r.payment_status), (0.0, 100.0, Unpaid));
    assert!(e.ledger(TransactionType::Sale, id).is_empty());
    let r = e.sales.update_sale(id, e.sp(Some(1), Paid, 130.0, &[(1, 1.0, 100.0)])).unwrap();
    assert_eq!((r.paid_amount, r.change_amount, r.payment_status), (100.0, 30.0, Paid));
    e.check();
}

#[test]
fn sale_edit_party_change_repoints_settlements() {
    let e = env();
    seed_party(&e.db, 1, "A", PartyType::Customer);
    seed_party(&e.db, 2, "B", PartyType::Customer);
    e.stocked(1, 10.0, 5.0);
    let id = e.sell(Some(1), Unpaid, 0.0, &[(1, 1.0, 100.0)]);
    e.acct.settle_bill_payment(settle_payload(1, TransactionType::Sale, Some(id), 30.0)).unwrap();
    e.sales.update_sale(id, e.sp(Some(2), Unpaid, 0.0, &[(1, 1.0, 100.0)])).unwrap();
    let rows = e.ledger(TransactionType::Sale, id);
    assert_eq!((rows.len(), rows[0].party_id), (1, Some(2)));
    let mut tx = e.db.begin_read().unwrap();
    assert!(tx.settlements().list_by_party(1).unwrap().is_empty());
    assert_eq!(tx.settlements().list_by_party(2).unwrap().len(), 1);
    let h = e.sale(id);
    assert_eq!((h.party_id, h.party_name.as_deref(), h.balance_due), (Some(2), Some("B"), 70.0));
    drop(tx);
    e.check();
}

#[test]
fn unlinked_return_credit_locks_delete_and_party_change_but_counts_to_the_floor() {
    let e = env();
    seed_party(&e.db, 1, "A", PartyType::Customer);
    seed_party(&e.db, 2, "B", PartyType::Customer);
    e.stocked(1, 20.0, 5.0);
    let id = e.sell(Some(1), Unpaid, 0.0, &[(1, 1.0, 100.0)]);
    let res = e.rets.create_customer_return(cust(None, Some(1), SettlementType::Refund, vec![rline(1, None, 2.0, 10.0, R)])).unwrap();
    assert_eq!(res.credited_amount, 20.0);
    let g = e.sales.get_sale_edit_guard(id).unwrap();
    assert!(g.can_edit && !g.can_delete && !g.can_change_party);
    assert_eq!(g.applied_amount, 20.0);
    assert!(g.reasons.iter().any(|r| r.code == "RETURN_CREDITS" && r.blocks_delete && !r.blocks_edit));
    let before = e.snap();
    assert_eq!(code(e.sales.update_sale(id, e.sp(Some(2), Unpaid, 0.0, &[(1, 1.0, 100.0)]))), "PARTY_CHANGE_WITH_CREDITS");
    assert_eq!(code(e.sales.delete_sale(id)), "RETURN_CREDITS");
    assert_eq!(code(e.sales.update_sale(id, e.sp(Some(1), Unpaid, 0.0, &[(1, 1.0, 10.0)]))), "TOTAL_BELOW_APPLIED");
    assert_eq!(e.snap(), before);
    let r = e.sales.update_sale(id, e.sp(Some(1), Unpaid, 0.0, &[(1, 1.0, 100.0)])).unwrap();
    assert_eq!(r.balance_due, 80.0, "the credit stays");
    e.check();
}

#[test]
fn sale_edit_total_floor_is_the_applied_amount() {
    let e = env();
    seed_party(&e.db, 1, "A", PartyType::Customer);
    e.stocked(1, 10.0, 5.0);
    let id = e.sell(Some(1), Unpaid, 0.0, &[(1, 1.0, 100.0)]);
    e.acct.settle_bill_payment(settle_payload(1, TransactionType::Sale, Some(id), 60.0)).unwrap();
    assert_eq!(code(e.sales.update_sale(id, e.sp(Some(1), Unpaid, 0.0, &[(1, 1.0, 50.0)]))), "TOTAL_BELOW_APPLIED");
    let r = e.sales.update_sale(id, e.sp(Some(1), Paid, 0.0, &[(1, 1.0, 60.0)])).unwrap();
    assert_eq!((r.payment_status, r.balance_due), (Paid, 0.0));
    let r = e.sales.update_sale(id, e.sp(Some(1), Unpaid, 0.0, &[(1, 1.0, 80.0)])).unwrap();
    assert_eq!((r.payment_status, r.balance_due, r.paid_amount), (Partial, 20.0, 60.0));
    e.check();
}

#[test]
fn sale_edit_cannot_clear_the_party_when_later_payments_exist() {
    let e = env();
    seed_party(&e.db, 1, "A", PartyType::Customer);
    e.stocked(1, 10.0, 5.0);
    let id = e.sell(Some(1), Unpaid, 0.0, &[(1, 1.0, 100.0)]);
    e.acct.settle_bill_payment(settle_payload(1, TransactionType::Sale, Some(id), 30.0)).unwrap();
    let before = e.snap();
    assert_eq!(code(e.sales.update_sale(id, e.sp(None, Paid, 70.0, &[(1, 1.0, 100.0)]))), "PARTY_REQUIRED_WITH_PAYMENTS");
    assert_eq!(e.snap(), before);
    e.check();
}

#[test]
fn linked_return_locks_a_sale_until_the_return_is_deleted() {
    let e = env();
    e.stocked(1, 10.0, 5.0);
    let id = e.sell(None, Paid, 50.0, &[(1, 5.0, 10.0)]);
    let sl = e.sale_lines(id)[0].id;
    let ret = e.rets.create_customer_return(cust(Some(id), None, SettlementType::Refund, vec![rline(1, Some(sl), 2.0, 10.0, R)])).unwrap();
    let g = e.sales.get_sale_edit_guard(id).unwrap();
    assert!(!g.can_edit && !g.can_delete);
    assert_eq!(g.reasons[0].code, "LINKED_RETURNS");
    assert_eq!(code(e.sales.update_sale(id, e.sp(None, Paid, 50.0, &[(1, 5.0, 10.0)]))), "LINKED_RETURNS");
    assert_eq!(code(e.sales.delete_sale(id)), "LINKED_RETURNS");
    e.rets.delete_return(ret.return_id).unwrap();
    e.sales.update_sale(id, e.sp(None, Paid, 30.0, &[(1, 3.0, 10.0)])).unwrap();
    e.sales.delete_sale(id).unwrap();
    e.check();
}

#[test]
fn sale_delete_restores_lots_stock_and_cash() {
    let e = env();
    let p = e.stocked(1, 10.0, 5.0);
    let id = e.sell(None, Paid, 40.0, &[(1, 4.0, 10.0)]);
    assert_eq!(e.acct.ledger_cash_summary(DAY, DAY).unwrap().sale_upfront, 40.0);
    let r = e.sales.delete_sale(id).unwrap();
    assert_eq!(r.removed_cash_amount, 40.0);
    assert_eq!(e.lots(p)[0].remaining_quantity, 10.0);
    assert_eq!(e.stock(1), 10.0);
    assert_eq!((e.count("sale_transactions"), e.count("sale_items"), e.count("sale_stock_allocations")), (0, 0, 0));
    assert_eq!(e.count("payment_settlements") as usize, e.ledger(TransactionType::Purchase, p).len());
    assert_eq!(e.acct.ledger_cash_summary(DAY, DAY).unwrap().sale_upfront, 0.0);
    e.check();
}

#[test]
fn deleting_one_bill_leaves_the_other_bills_share_of_a_lump_sum() {
    let e = env();
    seed_party(&e.db, 1, "A", PartyType::Customer);
    e.stocked(1, 10.0, 5.0);
    let a = e.sell(Some(1), Unpaid, 0.0, &[(1, 1.0, 100.0)]);
    let b = e.sell(Some(1), Unpaid, 0.0, &[(1, 1.0, 100.0)]);
    e.acct.settle_bill_payment(settle_payload(1, TransactionType::Sale, None, 150.0)).unwrap();
    let r = e.sales.delete_sale(b).unwrap();
    assert_eq!(r.removed_cash_amount, 50.0);
    let rows = e.ledger(TransactionType::Sale, a);
    assert_eq!((rows.len(), rows[0].amount_paid), (1, 100.0));
    assert_eq!(e.sale(a).payment_status, Paid);
    e.check();
}

#[test]
fn deleted_sale_codes_are_not_reused() {
    let e = env();
    e.stocked(1, 10.0, 5.0);
    e.sell(None, Paid, 10.0, &[(1, 1.0, 10.0)]);
    let b = e.sell(None, Paid, 10.0, &[(1, 1.0, 10.0)]);
    assert_eq!(e.sale(b).transaction_code, "SAL-1002");
    e.sales.delete_sale(b).unwrap();
    assert_eq!(e.sales.get_next_sale_code().unwrap(), "SAL-1003");
    let c = e.sell(None, Paid, 10.0, &[(1, 1.0, 10.0)]);
    assert_eq!(e.sale(c).transaction_code, "SAL-1003");
    e.check();
}

#[test]
fn sale_edit_reposts_charges() {
    let e = env();
    e.stocked(1, 10.0, 5.0);
    let t = e
        .types
        .create_charge_type(CreateChargeTypePayload {
            name: "Transport".into(),
            effect: ChargeEffect::Charge,
            applies_to: ChargeAppliesTo::Both,
            default_amount: None,
        })
        .unwrap()
        .id;
    let id = e.sell(None, Paid, 100.0, &[(1, 1.0, 100.0)]);
    let mut p = e.sp(None, Paid, 120.0, &[(1, 1.0, 100.0)]);
    p.charges = vec![BillChargeInput { charge_type_id: t, amount: 20.0 }];
    e.sales.update_sale(id, p).unwrap();
    let h = e.sale(id);
    assert_eq!((h.total_amount, h.charges_total, h.items_subtotal), (120.0, 20.0, 100.0));
    assert_eq!(e.count("bill_charges"), 1);
    e.sales.update_sale(id, e.sp(None, Paid, 100.0, &[(1, 1.0, 100.0)])).unwrap();
    assert_eq!((e.count("bill_charges"), e.sale(id).total_amount), (0, 100.0));
    e.check();
}

// ================================================================ purchase edit

#[test]
fn purchase_edit_updates_lots_in_place_adds_and_removes_lines() {
    let e = env();
    e.item(1);
    e.item(2);
    e.item(3);
    let p = e.buy(None, Paid, 62.0, &[(1, 10.0, 5.0), (2, 4.0, 3.0)]);
    let lots = e.lots(p);
    let (l1, l2) = (lots[0].id, lots[1].id);
    let payload = e.pp(None, Paid, 92.0, &[(Some(l1), 1, 12.0, 6.0), (None, 3, 2.0, 10.0)]);
    let r = e.purchases.update_purchase(p, payload).unwrap();
    assert_eq!(r.purchase_id, p);
    let now = e.lots(p);
    assert_eq!(now.len(), 2);
    assert_eq!(now[0].id, l1, "id kept");
    assert_eq!((now[0].quantity, now[0].remaining_quantity, now[0].unit_cost), (12.0, 12.0, 6.0));
    assert!(e.db.begin_read().unwrap().purchases().find_purchase_item_by_id(l2).unwrap().is_none());
    assert_eq!((e.stock(1), e.stock(2), e.stock(3)), (12.0, 0.0, 2.0));
    let h = e.pur(p);
    assert_eq!((h.total_amount, h.paid_amount, h.payment_status), (92.0, 92.0, Paid));
    assert!(h.updated_at.is_some());
    e.check();
}

#[test]
fn purchase_lot_quantity_cannot_drop_below_what_left_it() {
    let e = env();
    let p = e.stocked(1, 10.0, 5.0);
    let lot = e.lots(p)[0].id;
    e.sell(None, Paid, 40.0, &[(1, 4.0, 10.0)]);
    let before = e.snap();
    assert_eq!(code(e.purchases.update_purchase(p, e.pp(None, Paid, 15.0, &[(Some(lot), 1, 3.0, 5.0)]))), "LOT_BELOW_CONSUMED");
    assert_eq!(e.snap(), before);
    e.purchases.update_purchase(p, e.pp(None, Paid, 20.0, &[(Some(lot), 1, 4.0, 5.0)])).unwrap();
    assert_eq!((e.lot(lot).quantity, e.lot(lot).remaining_quantity), (4.0, 0.0));
    e.purchases.update_purchase(p, e.pp(None, Paid, 60.0, &[(Some(lot), 1, 12.0, 5.0)])).unwrap();
    assert_eq!((e.lot(lot).quantity, e.lot(lot).remaining_quantity), (12.0, 8.0));
    assert_eq!(e.stock(1), 8.0);
    e.check();
}

#[test]
fn referenced_lot_cannot_change_item_or_be_removed() {
    let e = env();
    e.item(1);
    e.item(2);
    let p = e.buy(None, Paid, 75.0, &[(1, 10.0, 5.0), (2, 5.0, 5.0)]);
    let (l1, l2) = (e.lots(p)[0].id, e.lots(p)[1].id);
    e.sell(None, Paid, 10.0, &[(1, 1.0, 10.0)]);
    let before = e.snap();
    let changed = e.pp(None, Paid, 75.0, &[(Some(l1), 2, 10.0, 5.0), (Some(l2), 2, 5.0, 5.0)]);
    assert_eq!(code(e.purchases.update_purchase(p, changed)), "CONSUMED_LINE_CHANGED");
    let removed = e.pp(None, Paid, 25.0, &[(Some(l2), 2, 5.0, 5.0)]);
    assert_eq!(code(e.purchases.update_purchase(p, removed)), "CONSUMED_LINE_REMOVED");
    assert_eq!(e.snap(), before);
    e.check();
}

#[test]
fn fully_returned_lot_is_still_referenced() {
    let e = env();
    let p = e.stocked(1, 10.0, 5.0);
    let lot = e.lots(p)[0].id;
    let s = e.sell(None, Paid, 40.0, &[(1, 4.0, 10.0)]);
    let sl = e.sale_lines(s)[0].id;
    e.rets.create_customer_return(cust(Some(s), None, SettlementType::Refund, vec![rline(1, Some(sl), 4.0, 10.0, R)])).unwrap();
    assert_eq!(e.lot(lot).remaining_quantity, 10.0, "nothing consumed");
    e.item(2);
    assert_eq!(code(e.purchases.update_purchase(p, e.pp(None, Paid, 50.0, &[(Some(lot), 2, 10.0, 5.0)]))), "CONSUMED_LINE_CHANGED");
    assert_eq!(code(e.purchases.delete_purchase(p)), "CONSUMED_LOTS");
    e.check();
}

#[test]
fn purchase_cost_change_recosts_cogs_and_valuation() {
    let e = env();
    let p = e.stocked(1, 10.0, 5.0);
    let lot = e.lots(p)[0].id;
    let s = e.sell(None, Paid, 40.0, &[(1, 4.0, 10.0)]);
    assert_eq!(e.cogs(s), 20.0);
    e.purchases.update_purchase(p, e.pp(None, Paid, 60.0, &[(Some(lot), 1, 10.0, 6.0)])).unwrap();
    assert_eq!(e.cogs(s), 24.0);
    assert_eq!(e.f("SELECT SUM(remaining_quantity * unit_cost) FROM purchase_items"), 36.0);
    e.check();
}

#[test]
fn purchase_cost_change_recosts_adjustment_losses() {
    let e = env();
    let p = e.stocked(1, 10.0, 5.0);
    let lot = e.lots(p)[0].id;
    e.inv
        .create_adjustment(StockAdjustmentPayload {
            adjustment_date: DAY.to_string(),
            item_id: 1,
            quantity: -2.0,
            reason: AdjustmentReason::Damaged,
            unit_cost: None,
            notes: None,
        })
        .unwrap();
    assert_eq!(e.f("SELECT SUM(quantity_depleted * unit_cost) FROM stock_outflow_allocations"), 10.0);
    e.purchases.update_purchase(p, e.pp(None, Paid, 60.0, &[(Some(lot), 1, 10.0, 6.0)])).unwrap();
    assert_eq!(e.f("SELECT SUM(quantity_depleted * unit_cost) FROM stock_outflow_allocations"), 12.0);
    e.check();
}

#[test]
fn purchase_cost_change_recomputes_stored_return_costs() {
    let e = env();
    let p = e.stocked(1, 10.0, 5.0);
    let lot = e.lots(p)[0].id;
    let s = e.sell(None, Paid, 50.0, &[(1, 5.0, 10.0)]);
    let sl = e.sale_lines(s)[0].id;
    e.rets.create_customer_return(cust(Some(s), None, SettlementType::Refund, vec![rline(1, Some(sl), 2.0, 10.0, D)])).unwrap();
    let mut repl = cust(Some(s), None, SettlementType::Replacement, vec![rline(1, Some(sl), 1.0, 10.0, R)]);
    repl.replacement_items = vec![ReplacementItemPayload { item_id: 1, quantity: 1.0 }];
    e.rets.create_customer_return(repl).unwrap();
    let costs = |e: &Env| -> Vec<f64> {
        let c = e.db.pool.get().unwrap();
        let mut st = c.prepare("SELECT cost_amount FROM return_items ORDER BY id").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    assert_eq!(costs(&e), vec![10.0, 5.0, 5.0]);
    e.purchases.update_purchase(p, e.pp(None, Paid, 60.0, &[(Some(lot), 1, 10.0, 6.0)])).unwrap();
    assert_eq!(costs(&e), vec![12.0, 6.0, 6.0], "scrap line, restocked line, replacement issue");
    e.check();
}

#[test]
fn recost_replays_two_partial_returns_over_two_lots() {
    let e = env();
    let a = e.stocked(1, 3.0, 5.0);
    e.buy(None, Paid, 40.0, &[(1, 5.0, 8.0)]);
    let lot_a = e.lots(a)[0].id;
    let s = e.sell(None, Paid, 120.0, &[(1, 6.0, 20.0)]); // 3 from A, 3 from B
    let sl = e.sale_lines(s)[0].id;
    for _ in 0..2 {
        e.rets.create_customer_return(cust(Some(s), None, SettlementType::Refund, vec![rline(1, Some(sl), 2.0, 20.0, D)])).unwrap();
    }
    let costs = |e: &Env| -> Vec<f64> {
        let c = e.db.pool.get().unwrap();
        let mut st = c.prepare("SELECT cost_amount FROM return_items ORDER BY id").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    assert_eq!(costs(&e), vec![16.0, 13.0], "2 x 8, then 1 x 8 + 1 x 5");
    e.purchases.update_purchase(a, e.pp(None, Paid, 18.0, &[(Some(lot_a), 1, 3.0, 6.0)])).unwrap();
    assert_eq!(costs(&e), vec![16.0, 14.0]);
    e.check();
}

#[test]
fn cost_change_on_a_vendor_returned_lot_is_rejected() {
    let e = env();
    seed_party(&e.db, 1, "V", PartyType::Vendor);
    e.item(1);
    let p = e.buy(Some(1), Paid, 50.0, &[(1, 10.0, 5.0)]);
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
    let g = e.purchases.get_purchase_edit_guard(p).unwrap();
    assert!(g.lot_limits[0].vendor_returned && g.lot_limits[0].referenced);
    assert_eq!(g.lot_limits[0].consumed_quantity, 2.0);
    let before = e.snap();
    assert_eq!(code(e.purchases.update_purchase(p, e.pp(Some(1), Paid, 60.0, &[(Some(lot), 1, 10.0, 6.0)]))), "VENDOR_RETURNED_COST_CHANGE");
    assert_eq!(e.snap(), before);
    e.purchases.update_purchase(p, e.pp(Some(1), Paid, 60.0, &[(Some(lot), 1, 12.0, 5.0)])).unwrap();
    assert_eq!((e.lot(lot).quantity, e.lot(lot).remaining_quantity), (12.0, 10.0));
    e.check();
}

#[test]
fn linked_vendor_return_locks_the_purchase() {
    let e = env();
    seed_party(&e.db, 1, "V", PartyType::Vendor);
    e.item(1);
    let p = e.buy(Some(1), Paid, 50.0, &[(1, 10.0, 5.0)]);
    let lot = e.lots(p)[0].id;
    let r = e
        .rets
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
    assert_eq!(code(e.purchases.update_purchase(p, e.pp(Some(1), Paid, 50.0, &[(Some(lot), 1, 10.0, 5.0)]))), "LINKED_RETURNS");
    assert_eq!(code(e.purchases.delete_purchase(p)), "LINKED_RETURNS");
    // A sale with the same numeric id is not blocked by a vendor return (return_type filter).
    e.stocked(2, 5.0, 5.0);
    let s = e.sell(None, Paid, 10.0, &[(2, 1.0, 10.0)]);
    assert_eq!(s, p);
    e.sales.delete_sale(s).unwrap();
    e.rets.delete_return(r.return_id).unwrap();
    e.purchases.update_purchase(p, e.pp(Some(1), Paid, 50.0, &[(Some(lot), 1, 10.0, 5.0)])).unwrap();
    e.check();
}

#[test]
fn purchase_vendor_change_repoints_settlements_and_credits_lock_it() {
    let e = env();
    seed_party(&e.db, 1, "V1", PartyType::Vendor);
    seed_party(&e.db, 2, "V2", PartyType::Vendor);
    e.item(1);
    let p = e.buy(Some(1), Unpaid, 0.0, &[(1, 10.0, 10.0)]);
    let lot = e.lots(p)[0].id;
    e.acct.settle_bill_payment(settle_payload(1, TransactionType::Purchase, Some(p), 30.0)).unwrap();
    e.purchases.update_purchase(p, e.pp(Some(2), Unpaid, 0.0, &[(Some(lot), 1, 10.0, 10.0)])).unwrap();
    assert_eq!(e.ledger(TransactionType::Purchase, p)[0].party_id, Some(2));
    assert_eq!(e.pur(p).balance_due, 70.0);
    // An unlinked vendor return credits the vendor's open bill and pins the vendor.
    e.rets
        .create_vendor_return(VendorReturnPayload {
            original_purchase_id: None,
            party_id: Some(2),
            return_date: DAY.to_string(),
            settlement_type: SettlementType::Refund,
            reason: None,
            notes: None,
            items: vec![VendorReturnItemPayload { item_id: 1, purchase_item_id: None, quantity: 2.0 }],
        })
        .unwrap();
    assert_eq!(code(e.purchases.update_purchase(p, e.pp(Some(1), Unpaid, 0.0, &[(Some(lot), 1, 10.0, 10.0)]))), "PARTY_CHANGE_WITH_CREDITS");
    e.check();
}

#[test]
fn purchase_delete_removes_lots_stock_and_cash_and_never_reuses_the_code() {
    let e = env();
    e.item(1);
    let p = e.buy(None, Paid, 50.0, &[(1, 10.0, 5.0)]);
    assert_eq!(e.pur(p).transaction_code, "PUR-1001");
    let r = e.purchases.delete_purchase(p).unwrap();
    assert_eq!(r.removed_cash_amount, 50.0);
    assert_eq!((e.stock(1), e.count("purchase_items"), e.count("purchase_transactions"), e.count("payment_settlements")), (0.0, 0, 0, 0));
    assert_eq!(e.purchases.get_next_purchase_code().unwrap(), "PUR-1002");
    e.check();
}

#[test]
fn purchase_delete_is_blocked_by_any_referenced_lot() {
    let e = env();
    let p = e.stocked(1, 10.0, 5.0);
    e.sell(None, Paid, 10.0, &[(1, 1.0, 10.0)]);
    let before = e.snap();
    assert_eq!(code(e.purchases.delete_purchase(p)), "CONSUMED_LOTS");
    assert_eq!(e.snap(), before);
}

#[test]
fn stock_record_headers_cannot_be_edited_or_deleted() {
    let e = env();
    e.item(1);
    e.inv
        .create_adjustment(StockAdjustmentPayload {
            adjustment_date: DAY.to_string(),
            item_id: 1,
            quantity: 3.0,
            reason: AdjustmentReason::UnrecordedStock,
            unit_cost: Some(5.0),
            notes: None,
        })
        .unwrap();
    let id = e.f("SELECT id FROM purchase_transactions WHERE source = 'ADJUSTMENT'") as i64;
    let g = e.purchases.get_purchase_edit_guard(id).unwrap();
    assert!(!g.can_edit && !g.can_delete);
    assert_eq!(code(e.purchases.update_purchase(id, e.pp(None, Paid, 0.0, &[(None, 1, 1.0, 1.0)]))), "NOT_A_PURCHASE");
    assert_eq!(code(e.purchases.delete_purchase(id)), "NOT_A_PURCHASE");
}

#[test]
fn purchase_item_id_is_only_valid_on_edit_and_must_belong_to_the_bill() {
    let e = env();
    e.item(1);
    let a = e.buy(None, Paid, 50.0, &[(1, 10.0, 5.0)]);
    let b = e.buy(None, Paid, 50.0, &[(1, 10.0, 5.0)]);
    let lot_a = e.lots(a)[0].id;
    assert_eq!(field(e.purchases.create_purchase(e.pp(None, Paid, 50.0, &[(Some(lot_a), 1, 10.0, 5.0)]))), "purchase_item_id");
    let before = e.snap();
    assert_eq!(field(e.purchases.update_purchase(b, e.pp(None, Paid, 50.0, &[(Some(lot_a), 1, 10.0, 5.0)]))), "purchase_item_id");
    assert_eq!(e.snap(), before);
}
