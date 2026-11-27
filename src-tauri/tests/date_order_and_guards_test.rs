// tests/date_order_and_guards_test.rs
//! Test Suite: v1.7 H7 (FR-59, DD-94..DD-96) on real SQLite: date order (E10), the lot floor with
//! restocked return units (S1), reversal clamps become errors (S2), the lot reprice payment rule
//! (E8), return-guard errors (R9), deleted items in returns (D10) and the opening lot date (S6).

mod common;

use std::sync::Arc;

use book_keeper::core::dtos::*;
use book_keeper::core::entities::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::accounting_service::AccountingService;
use book_keeper::services::contracts::inventory_service::InventoryService;
use book_keeper::services::contracts::item_service::ItemService;
use book_keeper::services::contracts::party_service::PartyService;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::return_service::ReturnService;
use book_keeper::services::contracts::sale_service::SaleService;
use common::invariants::*;
use common::ledger::*;
use common::*;
use PaymentStatus::*;

type Inv = InventoryServiceImpl<FifoStrategy>;
type Acct = AccountingServiceImpl<PaymentServiceImpl>;

const BEFORE: &str = "2026-11-19";
const AFTER: &str = "2026-11-22";
const LATER: &str = "2026-11-23";
const CUST: i64 = 50;
const VEND: i64 = 60;

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
    let e = Env {
        sales: SaleServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        purchases: PurchaseServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        rets: ReturnServiceImpl::new(inv.clone(), PaymentServiceImpl, acct.clone(), db.clone()),
        acct,
        inv,
        db,
    };
    seed_party(&e.db, CUST, "Asha", PartyType::Customer);
    seed_party(&e.db, VEND, "Vik", PartyType::Vendor);
    seed_stocked_item(&e.db, &e.inv, 1, 0.0, 0.0, 100.0);
    e
}

fn rline(sale_item: Option<i64>, qty: f64, price: f64, cond: StockCondition) -> CustomerReturnItemPayload {
    CustomerReturnItemPayload { item_id: 1, sale_item_id: sale_item, quantity: qty, unit_price: price, condition: cond, unit_cost_override: None }
}

fn cust(sale: Option<i64>, party: Option<i64>, date: &str, items: Vec<CustomerReturnItemPayload>) -> CustomerReturnPayload {
    CustomerReturnPayload {
        original_sale_id: sale,
        party_id: party,
        return_date: date.to_string(),
        settlement_type: SettlementType::Refund,
        reason: None,
        notes: None,
        items,
        replacement_items: vec![],
    }
}

fn vret(purchase: Option<i64>, lot: i64, qty: f64, date: &str) -> VendorReturnPayload {
    VendorReturnPayload {
        original_purchase_id: purchase,
        party_id: Some(VEND),
        return_date: date.to_string(),
        settlement_type: SettlementType::Refund,
        reason: None,
        notes: None,
        items: vec![VendorReturnItemPayload { item_id: 1, purchase_item_id: Some(lot), quantity: qty }],
    }
}

impl Env {
    /// A paid purchase of item 1 from the vendor on `DAY`; returns `(purchase_id, lot_id)`.
    fn buy(&self, qty: f64, cost: f64) -> (i64, i64) {
        let id = self
            .purchases
            .create_purchase(purchase_payload(Some(VEND), Paid, qty * cost, &[(1, None, qty, cost)]))
            .unwrap()
            .purchase_id;
        (id, self.lots(id)[0].id)
    }
    fn lots(&self, purchase: i64) -> Vec<PurchaseItem> {
        self.db.begin_read().unwrap().purchases().list_items_by_transaction_id(purchase).unwrap()
    }
    fn lot(&self, id: i64) -> PurchaseItem {
        self.db.begin_read().unwrap().purchases().find_purchase_item_by_id(id).unwrap().unwrap()
    }
    /// An unpaid credit sale of item 1 to the customer on `DAY`; returns `(sale_id, line_id)`.
    fn sell(&self, qty: f64, price: f64) -> (i64, i64) {
        let id = self.sales.create_sale(sale_payload(Some(CUST), Unpaid, 0.0, &[(1, None, qty, price)])).unwrap().sale_id;
        let line = self.db.begin_read().unwrap().sales().list_items_by_transaction_id(id).unwrap()[0].id;
        (id, line)
    }
    fn stock(&self) -> f64 {
        self.db.begin_read().unwrap().items().find_by_id(1).unwrap().unwrap().current_stock
    }
    fn exec(&self, sql: &str) {
        self.db.pool.get().unwrap().execute_batch(sql).unwrap();
    }
    fn snap(&self) -> String {
        let c = self.db.pool.get().unwrap();
        let q = |sql: &str| -> String { c.query_row(sql, [], |r| r.get::<_, Option<String>>(0)).unwrap().unwrap_or_default() };
        [
            q("SELECT group_concat(id||':'||quantity||':'||remaining_quantity||':'||unit_cost) FROM purchase_items"),
            q("SELECT group_concat(id||':'||current_stock) FROM items"),
            q("SELECT group_concat(id||':'||kind||':'||amount_paid||':'||settlement_date) FROM payment_settlements"),
            q("SELECT group_concat(id||':'||quantity_depleted||':'||returned_quantity) FROM sale_stock_allocations"),
            q("SELECT group_concat(id||':'||quantity_depleted) FROM stock_outflow_allocations"),
            q("SELECT group_concat(id||':'||transaction_date) FROM sale_transactions"),
            q("SELECT group_concat(id||':'||transaction_date) FROM purchase_transactions"),
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

fn locked<T: std::fmt::Debug>(r: Result<T, AppError>) -> (String, String) {
    match r {
        Err(AppError::BillLocked { code, message }) => (code, message),
        other => panic!("expected BillLocked, got {other:?}"),
    }
}

fn date_order<T: std::fmt::Debug>(r: Result<T, AppError>) -> String {
    let (code, message) = locked(r);
    assert_eq!(code, "DATE_ORDER", "{message}");
    message
}

// =========================================================================
// E10 / DD-94: date order
// =========================================================================

#[test]
fn e10_customer_return_cannot_be_dated_before_its_bill() {
    let e = env();
    e.buy(10.0, 5.0);
    let (sale, line) = e.sell(4.0, 10.0);
    let p = cust(Some(sale), None, BEFORE, vec![rline(Some(line), 1.0, 10.0, StockCondition::Restockable)]);
    let before = e.snap();
    let m = date_order(e.rets.preview_customer_return(p.clone()));
    assert_eq!(m, "Return date can't be before the bill date (20 Nov 2026)");
    date_order(e.rets.create_customer_return(p));
    assert_eq!(e.snap(), before, "nothing written");
    // The bill's own day is allowed.
    e.rets.create_customer_return(cust(Some(sale), None, DAY, vec![rline(Some(line), 1.0, 10.0, StockCondition::Restockable)])).unwrap();
    e.check();
}

#[test]
fn e10_vendor_return_cannot_be_dated_before_its_bill_or_lot() {
    let e = env();
    let (pur, lot) = e.buy(10.0, 5.0);
    let before = e.snap();
    let m = date_order(e.rets.preview_vendor_return(vret(Some(pur), lot, 2.0, BEFORE)));
    assert!(m.contains("20 Nov 2026"), "{m}");
    date_order(e.rets.create_vendor_return(vret(Some(pur), lot, 2.0, BEFORE)));
    // Unlinked: the lot's own bill still limits it.
    date_order(e.rets.create_vendor_return(vret(None, lot, 2.0, BEFORE)));
    assert_eq!(e.snap(), before);
    e.rets.create_vendor_return(vret(Some(pur), lot, 2.0, DAY)).unwrap();
    e.check();
}

#[test]
fn e10_settlement_cannot_be_dated_before_its_bill_single_batch_lump_and_edit() {
    let e = env();
    e.buy(10.0, 5.0);
    let (sale, _) = e.sell(4.0, 10.0);
    let before = e.snap();
    // Single bill.
    let mut p = settle_payload(CUST, TransactionType::Sale, Some(sale), 10.0);
    p.settlement_date = BEFORE.into();
    let m = date_order(e.acct.settle_bill_payment(p.clone()));
    assert_eq!(m, "Payment date can't be before the bill date (20 Nov 2026)");
    // Lump sum (FIFO picks the same bill).
    p.transaction_id = None;
    date_order(e.acct.settle_bill_payment(p));
    // Batch.
    let batch = SettleBatchPayload {
        party_id: CUST,
        transaction_type: TransactionType::Sale,
        settlement_date: BEFORE.into(),
        payment_mode: Some("CASH".into()),
        notes: None,
        allocations: vec![SettleAllocation { transaction_id: sale, amount_paid: 10.0 }],
    };
    date_order(e.acct.settle_bills_batch(batch));
    assert_eq!(e.snap(), before, "nothing written");
    // Edit: a settlement moved before the bill.
    let sid = e.acct.settle_bill_payment(settle_payload(CUST, TransactionType::Sale, Some(sale), 10.0)).unwrap().settlement_ids[0];
    let before = e.snap();
    let edit = UpdateSettlementPayload { amount_paid: 10.0, settlement_date: BEFORE.into(), payment_mode: None, notes: None };
    date_order(e.acct.update_settlement(sid, edit.clone()));
    assert_eq!(e.snap(), before);
    e.acct.update_settlement(sid, UpdateSettlementPayload { settlement_date: AFTER.into(), ..edit }).unwrap();
    e.check();
}

#[test]
fn e10_sale_edit_cannot_move_the_bill_after_its_earliest_payment() {
    let e = env();
    e.buy(10.0, 5.0);
    let (sale, line) = e.sell(4.0, 10.0);
    let mut p = settle_payload(CUST, TransactionType::Sale, Some(sale), 10.0);
    p.settlement_date = AFTER.into();
    e.acct.settle_bill_payment(p).unwrap();
    let mut edit = sale_payload(Some(CUST), Unpaid, 0.0, &[(1, None, 4.0, 10.0)]);
    edit.items[0].sale_item_id = Some(line);
    edit.transaction_date = LATER.into();
    let before = e.snap();
    let m = date_order(e.sales.update_sale(sale, edit.clone()));
    assert_eq!(m, "This bill has a payment on 22 Nov 2026, so its date can't be later");
    // The preview shows the same error.
    let preview = e
        .sales
        .preview_sale(
            Some(sale),
            PreviewSalePayload {
                transaction_date: LATER.into(),
                party_id: Some(CUST),
                payment_status: None,
                paid_amount: None,
                payment_mode: None,
                notes: None,
                items: edit.items.clone(),
                charges: vec![],
                confirm_move_settlements: false,
            },
        )
        .unwrap();
    assert_eq!(preview.errors[0].field, "DATE_ORDER");
    assert_eq!(e.snap(), before);
    // Up to the payment's day, and any earlier day, are allowed.
    edit.transaction_date = AFTER.into();
    e.sales.update_sale(sale, edit.clone()).unwrap();
    edit.transaction_date = BEFORE.into();
    e.sales.update_sale(sale, edit).unwrap();
    e.check();
}

#[test]
fn e10_purchase_edit_cannot_move_the_bill_after_its_earliest_payment() {
    let e = env();
    let pur = e
        .purchases
        .create_purchase(purchase_payload(Some(VEND), Unpaid, 0.0, &[(1, None, 10.0, 5.0)]))
        .unwrap()
        .purchase_id;
    let lot = e.lots(pur)[0].id;
    let mut p = settle_payload(VEND, TransactionType::Purchase, Some(pur), 10.0);
    p.settlement_date = AFTER.into();
    e.acct.settle_bill_payment(p).unwrap();
    let mut edit = purchase_payload(Some(VEND), Unpaid, 0.0, &[(1, None, 10.0, 5.0)]);
    edit.items[0].purchase_item_id = Some(lot);
    edit.transaction_date = LATER.into();
    let before = e.snap();
    date_order(e.purchases.update_purchase(pur, edit.clone()));
    assert_eq!(e.snap(), before);
    edit.transaction_date = AFTER.into();
    e.purchases.update_purchase(pur, edit).unwrap();
    e.check();
}

#[test]
fn e10_opening_balance_as_of_cannot_move_after_its_earliest_settlement() {
    let e = env();
    let parties = PartyServiceImpl::new(e.db.clone(), Arc::new(e.db.party_index()));
    let pid = parties
        .create_party(CreatePartyPayload {
            name: "Old".into(),
            phone: None,
            party_type: PartyType::Customer,
            notes: None,
            opening_balance: Some(OpeningBalanceInput { amount: 500.0, as_of_date: BEFORE.into() }),
        })
        .unwrap();
    let mut p = settle_payload(pid, TransactionType::Sale, None, 100.0);
    p.settlement_date = DAY.into();
    e.acct.settle_bill_payment(p).unwrap();
    let upd = |date: &str| {
        parties.update_party(
            pid,
            UpdatePartyPayload {
                name: "Old".into(),
                phone: None,
                party_type: PartyType::Customer,
                notes: None,
                opening_balance: Some(OpeningBalanceInput { amount: 500.0, as_of_date: date.into() }),
            },
        )
    };
    let m = date_order(upd(AFTER));
    assert!(m.contains("20 Nov 2026"), "{m}");
    assert_eq!(parties.get_party(pid).unwrap().opening_balance.unwrap().as_of_date, BEFORE);
    upd(DAY).unwrap();
    assert_eq!(parties.get_party(pid).unwrap().opening_balance.unwrap().as_of_date, DAY);
    e.check();
}

// =========================================================================
// S1 / DD-95: the lot floor counts restocked return units
// =========================================================================

#[test]
fn s1_purchase_lot_cannot_shrink_below_restocked_units_and_the_return_stays_deletable() {
    let e = env();
    let (pur, lot) = e.buy(10.0, 5.0);
    let (sale, line) = e.sell(6.0, 10.0);
    let ret = e
        .rets
        .create_customer_return(cust(Some(sale), None, DAY, vec![rline(Some(line), 2.0, 10.0, StockCondition::Restockable)]))
        .unwrap()
        .return_id;
    assert_eq!(e.lot(lot).remaining_quantity, 6.0);
    let edit = |q: f64| {
        let mut p = purchase_payload(Some(VEND), Paid, q * 5.0, &[(1, None, q, 5.0)]);
        p.items[0].purchase_item_id = Some(lot);
        p
    };
    let before = e.snap();
    let (code, _) = locked(e.purchases.update_purchase(pur, edit(5.0)));
    assert_eq!(code, "LOT_BELOW_CONSUMED");
    assert_eq!(e.snap(), before);
    e.purchases.update_purchase(pur, edit(6.0)).unwrap();
    assert_eq!(e.lot(lot).remaining_quantity, 2.0);
    e.check();
    assert!(e.rets.get_return_delete_guard(ret).unwrap().can_delete);
    e.rets.delete_return(ret).unwrap();
    assert_eq!((e.lot(lot).remaining_quantity, e.stock()), (0.0, 0.0));
    e.check();
}

#[test]
fn s1_opening_lot_floor_holds_for_the_quantity_edit_and_set_stock() {
    let e = env();
    let lot = {
        let mut tx = e.db.begin_transaction().unwrap();
        let id = e.inv.create_source_lot(&mut *tx, LotSource::Opening, DAY, 1, 10.0, Some(5.0), None).unwrap();
        tx.commit().unwrap();
        id
    };
    let (sale, line) = e.sell(6.0, 10.0);
    let ret = e
        .rets
        .create_customer_return(cust(Some(sale), None, DAY, vec![rline(Some(line), 2.0, 10.0, StockCondition::Restockable)]))
        .unwrap()
        .return_id;
    let before = e.snap();
    // G6 quantity edit.
    let q = |quantity: f64| UpdateOpeningQuantityPayload { purchase_item_id: lot, quantity };
    let (code, _) = locked(e.inv.preview_opening_lot_quantity(q(5.0)));
    assert_eq!(code, "LOT_BELOW_CONSUMED");
    locked(e.inv.update_opening_lot_quantity(q(5.0)));
    assert_eq!(e.inv.preview_opening_lot_quantity(q(10.0)).unwrap().consumed, 6.0, "restocked units count as used");
    // H5 Set stock opening correction: only 10 - 6 = 4 is unused.
    let set = |counted: f64, mode: Option<SetStockMode>| SetItemStockPayload {
        item_id: 1,
        counted_quantity: counted,
        unit_cost: None,
        adjustment_date: DAY.into(),
        notes: None,
        mode,
    };
    let preview = e.inv.preview_set_item_stock(set(0.0, None)).unwrap();
    let opening = preview.options.iter().find(|o| o.mode == SetStockMode::OpeningCorrection).unwrap();
    assert!(!opening.enabled, "6 cannot come off: 2 of them are restocked return units");
    assert!(e.inv.set_item_stock(set(0.0, Some(SetStockMode::OpeningCorrection))).is_err());
    assert_eq!(e.snap(), before);
    // 4 can come off (stock 6 -> 2), and the return stays deletable.
    e.inv.set_item_stock(set(2.0, Some(SetStockMode::OpeningCorrection))).unwrap();
    assert_eq!((e.lot(lot).quantity, e.lot(lot).remaining_quantity), (6.0, 2.0));
    e.check();
    e.rets.delete_return(ret).unwrap();
    assert_eq!(e.stock(), 0.0);
    // Quantity 6 is now exactly what was drawn.
    e.inv.update_opening_lot_quantity(q(6.0)).unwrap();
    e.check();
}

// =========================================================================
// S2 / DD-95: reversal overflow is an error, never clamped
// =========================================================================

#[test]
fn s2_sale_delete_with_an_allocation_larger_than_the_lot_room_fails_and_writes_nothing() {
    let e = env();
    let (_, lot) = e.buy(10.0, 5.0);
    let (sale, _) = e.sell(6.0, 10.0);
    // Corrupt: the allocation claims 8, but the lot only has room for 6 more.
    e.exec(&format!("UPDATE sale_stock_allocations SET quantity_depleted = 8 WHERE purchase_item_id = {lot}"));
    let before = e.snap();
    let (code, message) = locked(e.sales.delete_sale(sale));
    assert_eq!(code, "STOCK_INVARIANT");
    assert!(message.contains("Nothing was changed"), "{message}");
    assert_eq!(e.snap(), before);
}

#[test]
fn s2_vendor_return_delete_with_an_outflow_larger_than_the_lot_room_fails_and_writes_nothing() {
    let e = env();
    let (pur, lot) = e.buy(10.0, 5.0);
    let ret = e.rets.create_vendor_return(vret(Some(pur), lot, 3.0, DAY)).unwrap().return_id;
    e.exec(&format!("UPDATE stock_outflow_allocations SET quantity_depleted = 5 WHERE source_id = {ret}"));
    let before = e.snap();
    let (code, _) = locked(e.rets.delete_return(ret));
    assert_eq!(code, "STOCK_INVARIANT");
    assert_eq!(e.snap(), before);
    assert!(!e.rets.get_return_delete_guard(ret).unwrap().can_delete, "the guard shows it too");
}

// =========================================================================
// E8 / DD-96: a fully paid bill stays paid on a reprice, however it was paid
// =========================================================================

#[test]
fn e8_upfront_plus_settlement_bill_stays_paid_and_the_extra_goes_to_upfront() {
    let e = env();
    let pur = e
        .purchases
        .create_purchase(purchase_payload(Some(VEND), Partial, 600.0, &[(1, None, 10.0, 100.0)]))
        .unwrap()
        .purchase_id;
    e.acct.settle_bill_payment(settle_payload(VEND, TransactionType::Purchase, Some(pur), 400.0)).unwrap();
    let lot = e.lots(pur)[0].id;
    let b = e
        .purchases
        .update_lot_purchase_price(UpdateLotPricePayload { purchase_item_id: lot, unit_price: 110.0 })
        .unwrap()
        .bill
        .unwrap();
    assert_eq!((b.new_total, b.new_paid, b.balance_due, b.payment_status), (1100.0, 1100.0, 0.0, Paid));
    let rows = e.db.begin_read().unwrap().settlements().list_by_transaction(TransactionType::Purchase, pur).unwrap();
    let up: f64 = rows.iter().filter(|r| r.kind == PaymentKind::Upfront).map(|r| r.amount_paid).sum();
    assert_eq!(up, 700.0);
    e.check();
}

#[test]
fn e8_part_paid_bill_takes_the_change_in_its_balance() {
    let e = env();
    let pur = e
        .purchases
        .create_purchase(purchase_payload(Some(VEND), Partial, 600.0, &[(1, None, 10.0, 100.0)]))
        .unwrap()
        .purchase_id;
    let lot = e.lots(pur)[0].id;
    let b = e
        .purchases
        .update_lot_purchase_price(UpdateLotPricePayload { purchase_item_id: lot, unit_price: 110.0 })
        .unwrap()
        .bill
        .unwrap();
    assert_eq!((b.new_total, b.new_paid, b.balance_due, b.payment_status), (1100.0, 600.0, 500.0, Partial));
    e.check();
}

// =========================================================================
// R9: any error of the return undo dry run blocks delete
// =========================================================================

#[test]
fn r9_return_guard_blocks_delete_on_a_non_bill_locked_error() {
    let e = env();
    let (_, lot) = e.buy(10.0, 5.0);
    let (sale, line) = e.sell(4.0, 10.0);
    let ret = e
        .rets
        .create_customer_return(cust(Some(sale), None, DAY, vec![rline(Some(line), 1.0, 10.0, StockCondition::Restockable)]))
        .unwrap()
        .return_id;
    // Corrupt: the lot the allocation points at is gone, so the dry run fails with NotFound.
    e.exec(&format!("PRAGMA foreign_keys = OFF; DELETE FROM purchase_items WHERE id = {lot}; PRAGMA foreign_keys = ON;"));
    let g = e.rets.get_return_delete_guard(ret).unwrap();
    assert!(!g.can_delete);
    assert_eq!(g.reasons[0].code, "RETURN_UNDO_FAILED");
    assert!(g.reasons[0].blocks_delete);
}

// =========================================================================
// D10 / DD-90: a return never uses a deleted item
// =========================================================================

#[test]
fn d10_return_lines_and_replacements_of_a_deleted_item_are_deleted_references() {
    let e = env();
    e.buy(10.0, 5.0);
    seed_stocked_item(&e.db, &e.inv, 2, 5.0, 3.0, 30.0);
    let (sale, line) = e.sell(4.0, 10.0);
    let is_deleted_ref = |r: Result<ReturnResult, AppError>, id: i64| match r {
        Err(AppError::DeletedReference { entity, id: got, .. }) => assert_eq!((entity.as_str(), got), ("ITEM", id)),
        other => panic!("expected DeletedReference, got {other:?}"),
    };
    // A replacement of a deleted item.
    e.exec("UPDATE items SET is_active = 0 WHERE id = 2");
    let mut repl = cust(Some(sale), None, DAY, vec![rline(Some(line), 1.0, 10.0, StockCondition::Restockable)]);
    repl.settlement_type = SettlementType::Replacement;
    repl.replacement_items = vec![ReplacementItemPayload { item_id: 2, quantity: 1.0 }];
    is_deleted_ref(e.rets.preview_customer_return(repl), 2);
    // Linked and unlinked lines of a deleted item.
    e.exec("UPDATE items SET is_active = 0 WHERE id = 1");
    is_deleted_ref(e.rets.create_customer_return(cust(Some(sale), None, DAY, vec![rline(Some(line), 1.0, 10.0, StockCondition::Restockable)])), 1);
    is_deleted_ref(e.rets.create_customer_return(cust(None, Some(CUST), DAY, vec![rline(None, 1.0, 10.0, StockCondition::Restockable)])), 1);
}

// =========================================================================
// S6: the opening lot of a new item is dated local today
// =========================================================================

#[test]
fn s6_opening_lot_on_item_create_uses_the_local_date() {
    let e = env();
    let _today = book_keeper::core::clock::TodayGuard::new("2030-01-02");
    let items = ItemServiceImpl::new(e.db.clone(), Arc::new(e.db.indexer()), e.inv.clone());
    let id = items
        .create_item(CreateItemPayload {
            item_type: None,
            service_cost: None,
            components: None,
            low_stock_threshold: None,
            name: "Fresh".into(),
            category: None,
            barcode: None,
            base_unit: "PCS".into(),
            mrp: None,
            default_sale_price: 10.0,
            initial_stock: Some(3.0),
            initial_cost: Some(2.0),
            bundles: None,
        })
        .unwrap();
    let lots = e.inv.get_item_lots(id).unwrap();
    assert_eq!(lots[0].transaction_date, "2030-01-02");
}
