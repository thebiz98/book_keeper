// tests/party_integrity_test.rs
//! Test Suite: v1.7 FR-54 party integrity (LLD 32, DD-88, DD-90 party half). Findings L1, R7, E6,
//! E7 on production services and real SQLite. Every write path ends with the ledger invariant.

mod common;

use std::sync::Arc;

use book_keeper::core::dtos::*;
use book_keeper::core::entities::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::accounting_service::AccountingService;
use book_keeper::services::contracts::party_service::PartyService;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::return_service::ReturnService;
use book_keeper::services::contracts::sale_service::SaleService;
use common::invariants::*;
use common::ledger::*;
use common::*;

type Inv = InventoryServiceImpl<FifoStrategy>;
type Acct = AccountingServiceImpl<PaymentServiceImpl>;

struct E {
    db: TestDb,
    parties: PartyServiceImpl,
    acct: Acct,
    sales: SaleServiceImpl<Inv, PaymentServiceImpl>,
    purchases: PurchaseServiceImpl<Inv, PaymentServiceImpl>,
    rets: ReturnServiceImpl<Inv, PaymentServiceImpl, Acct>,
}

fn env() -> E {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let acct = AccountingServiceImpl::new(PaymentServiceImpl, db.clone());
    // Stock arrives through a real, fully paid purchase so the ledger invariant holds.
    seed_stocked_item(&db, &inv, 1, 0.0, 0.0, 10.0);
    PurchaseServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone())
        .create_purchase(purchase_payload(None, PaymentStatus::Paid, 500.0, &[(1, None, 100.0, 5.0)]))
        .expect("stock");
    E {
        parties: PartyServiceImpl::new(db.clone(), Arc::new(db.party_index())),
        sales: SaleServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        purchases: PurchaseServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        rets: ReturnServiceImpl::new(inv, PaymentServiceImpl, acct.clone(), db.clone()),
        acct,
        db,
    }
}

use PaymentStatus::*;

impl E {
    fn party(&self, name: &str, t: PartyType) -> i64 {
        self.parties
            .create_party(CreatePartyPayload { name: name.into(), phone: None, party_type: t, notes: None, opening_balance: None })
            .expect("party")
    }
    fn retype(&self, id: i64, name: &str, t: PartyType) -> Result<(), AppError> {
        self.parties.update_party(
            id,
            UpdatePartyPayload { name: name.into(), phone: None, party_type: t, notes: None, opening_balance: None },
        )
    }
    /// Sale of `qty x 10`.
    fn sell(&self, party: Option<i64>, st: PaymentStatus, paid: f64, qty: f64) -> i64 {
        self.sales.create_sale(sale_payload(party, st, paid, &[(1, None, qty, 10.0)])).expect("sale").sale_id
    }
    fn sp(&self, party: Option<i64>, st: PaymentStatus, paid: f64, qty: f64) -> CreateSalePayload {
        sale_payload(party, st, paid, &[(1, None, qty, 10.0)])
    }
    fn settle(&self, party: i64, t: TransactionType, bill: i64, amount: f64) -> i64 {
        self.acct.settle_bill_payment(settle_payload(party, t, Some(bill), amount)).expect("settle").settlement_ids[0]
    }
    fn sale(&self, id: i64) -> SaleTransaction {
        self.db.begin_read().unwrap().sales().find_transaction_by_id(id).unwrap().unwrap()
    }
    fn by_party(&self, party: i64) -> Vec<PaymentSettlement> {
        self.db.begin_read().unwrap().settlements().list_by_party(party).unwrap()
    }
    fn snap(&self) -> String {
        let c = self.db.pool.get().unwrap();
        let q = |sql: &str| -> String { c.query_row(sql, [], |r| r.get::<_, Option<String>>(0)).unwrap().unwrap_or_default() };
        [
            q("SELECT group_concat(id||':'||total_amount||':'||paid_amount||':'||balance_due||':'||COALESCE(party_id,'')) FROM sale_transactions"),
            q("SELECT group_concat(id||':'||total_amount||':'||paid_amount||':'||balance_due||':'||COALESCE(party_id,'')) FROM purchase_transactions"),
            q("SELECT group_concat(id||':'||unit_cost||':'||remaining_quantity) FROM purchase_items"),
            q("SELECT group_concat(id||':'||kind||':'||amount_paid||':'||COALESCE(party_id,'')) FROM payment_settlements"),
            q("SELECT group_concat(id||':'||party_type||':'||is_active) FROM parties"),
            q("SELECT CAST(COUNT(*) AS TEXT) FROM return_transactions"),
        ]
        .join("|")
    }
}

fn locked_code<T: std::fmt::Debug>(r: Result<T, AppError>) -> (String, String) {
    match r {
        Err(AppError::BillLocked { code, message }) => (code, message),
        other => panic!("expected BillLocked, got {other:?}"),
    }
}

fn validation<T: std::fmt::Debug>(r: Result<T, AppError>) -> (String, String) {
    match r {
        Err(AppError::ValidationError { field, message }) => (field, message),
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

fn deleted_party<T: std::fmt::Debug>(r: Result<T, AppError>, party: i64) -> String {
    match r {
        Err(AppError::DeletedReference { entity, id, message, .. }) => {
            assert_eq!((entity.as_str(), id), ("PARTY", party));
            message
        }
        other => panic!("expected DeletedReference, got {other:?}"),
    }
}

// ---- L1: the type is locked once anything references the party ----

#[test]
fn l1_party_type_is_locked_once_a_bill_references_it() {
    let e = env();
    let c = e.party("Ravi", PartyType::Customer);
    e.sell(Some(c), Paid, 10.0, 1.0);
    let before = e.snap();
    let (field, msg) = validation(e.retype(c, "Ravi", PartyType::Vendor));
    assert_eq!(field, "party_type");
    assert_eq!(msg, "'Ravi' has bills or payments, so its type can't change.");
    assert_eq!(e.snap(), before);
    // other fields still edit
    e.retype(c, "Ravi Kumar", PartyType::Customer).unwrap();

    // an unreferenced party may change type
    let fresh = e.party("Fresh", PartyType::Customer);
    e.retype(fresh, "Fresh", PartyType::Vendor).unwrap();
    assert_eq!(e.parties.get_party(fresh).unwrap().party_type, PartyType::Vendor);
}

#[test]
fn l1_a_return_alone_locks_the_type() {
    let e = env();
    let c = e.party("Walk", PartyType::Customer);
    e.rets
        .create_customer_return(unlinked_customer_return(Some(c)))
        .expect("unlinked return");
    validation(e.retype(c, "Walk", PartyType::Vendor));
}

// ---- R7: an unlinked return takes only a party of the matching type ----

fn unlinked_customer_return(party: Option<i64>) -> CustomerReturnPayload {
    CustomerReturnPayload {
        original_sale_id: None,
        party_id: party,
        return_date: DAY.into(),
        settlement_type: SettlementType::Refund,
        reason: None,
        notes: None,
        items: vec![CustomerReturnItemPayload {
            item_id: 1,
            sale_item_id: None,
            quantity: 1.0,
            unit_price: 10.0,
            condition: StockCondition::Restockable,
            unit_cost_override: None,
        }],
        replacement_items: vec![],
    }
}

#[test]
fn r7_unlinked_returns_reject_a_party_of_the_other_type() {
    let e = env();
    let c = e.party("Cust", PartyType::Customer);
    let v = e.party("Vend", PartyType::Vendor);
    let before = e.snap();
    let (field, msg) = validation(e.rets.create_customer_return(unlinked_customer_return(Some(v))));
    assert_eq!((field.as_str(), msg.as_str()), ("party_id", "'Vend' is a vendor. Pick a customer."));
    let (field, msg) = validation(e.rets.create_vendor_return(VendorReturnPayload {
        original_purchase_id: None,
        party_id: Some(c),
        return_date: DAY.into(),
        settlement_type: SettlementType::Refund,
        reason: None,
        notes: None,
        items: vec![VendorReturnItemPayload { item_id: 1, purchase_item_id: None, quantity: 1.0 }],
    }));
    assert_eq!((field.as_str(), msg.as_str()), ("party_id", "'Cust' is a customer. Pick a vendor."));
    assert_eq!(e.snap(), before);
    // the matching type passes
    e.rets.create_customer_return(unlinked_customer_return(Some(c))).unwrap();
    assert_ledger_invariant(&e.db);
}

// ---- E6: no write raises a deleted party's due or pays it ----

#[test]
fn e6_editing_a_deleted_customers_bill_to_a_higher_due_is_refused() {
    let e = env();
    let c = e.party("Gone", PartyType::Customer);
    let bill = e.sell(Some(c), Paid, 30.0, 3.0);
    e.parties.delete_party(c).unwrap();

    let before = e.snap();
    // higher total, paid as before -> a due appears
    let m = deleted_party(e.sales.update_sale(bill, e.sp(Some(c), Partial, 30.0, 5.0)), c);
    assert_eq!(m, "'Gone' is deleted. Restore it first.");
    assert_eq!(e.snap(), before);
    // the preview says the same, with what the Restore button needs
    let p = e
        .sales
        .preview_sale(
            Some(bill),
            PreviewSalePayload {
                transaction_date: DAY.into(),
                party_id: Some(c),
                payment_status: None,
                paid_amount: None,
                payment_mode: None,
                notes: None,
                items: vec![CreateSaleItemPayload { item_id: 1, bundle_id: None, bundle_quantity: 5.0, unit_price: 10.0 }],
                charges: vec![],
                confirm_move_settlements: false,
            },
        )
        .unwrap();
    assert_eq!(p.errors.len(), 1, "{:?}", p.errors);
    assert_eq!(p.errors[0].field, "party_id");
    assert_eq!(
        p.errors[0].deleted,
        Some(DeletedRef { entity: "PARTY".into(), id: c, name: "Gone".into() })
    );
    assert_eq!(e.snap(), before, "a preview writes nothing");

    // lowering the total (refund the difference as paid) keeps the deleted customer
    let r = e.sales.update_sale(bill, e.sp(Some(c), Paid, 20.0, 2.0)).expect("lower total passes");
    assert_eq!((r.total_amount, r.balance_due), (20.0, 0.0));
    assert_eq!(e.sale(bill).party_id, Some(c));
    // after a restore the raise passes
    e.parties.restore_party(c).unwrap();
    e.sales.update_sale(bill, e.sp(Some(c), Partial, 20.0, 5.0)).unwrap();
    assert_eq!(e.sale(bill).balance_due, 30.0);
    assert_ledger_invariant(&e.db);
}

#[test]
fn e6_purchase_edit_and_lot_reprice_cannot_raise_a_deleted_vendors_due() {
    let e = env();
    let v = e.party("Old Vendor", PartyType::Vendor);
    let p = e
        .purchases
        .create_purchase(purchase_payload(Some(v), Unpaid, 0.0, &[(1, None, 10.0, 10.0)]))
        .unwrap()
        .purchase_id;
    e.settle(v, TransactionType::Purchase, p, 100.0);
    e.parties.delete_party(v).unwrap();
    let lot = e.db.begin_read().unwrap().purchases().list_items_by_transaction_id(p).unwrap()[0].id;

    let before = e.snap();
    let mut edit = purchase_payload(Some(v), Unpaid, 0.0, &[(1, None, 10.0, 12.0)]);
    edit.items[0].purchase_item_id = Some(lot);
    deleted_party(e.purchases.update_purchase(p, edit), v);
    let reprice = UpdateLotPricePayload { purchase_item_id: lot, unit_price: 12.0 };
    deleted_party(e.purchases.preview_lot_purchase_price(reprice.clone()), v);
    deleted_party(e.purchases.update_lot_purchase_price(reprice), v);
    assert_eq!(e.snap(), before);
    assert_ledger_invariant(&e.db);
}

#[test]
fn e6_settlements_of_a_deleted_party_cannot_be_created_edited_or_deleted() {
    let e = env();
    let c = e.party("Gone", PartyType::Customer);
    let bill = e.sell(Some(c), Unpaid, 0.0, 2.0);
    let sid = e.settle(c, TransactionType::Sale, bill, 20.0);
    e.parties.delete_party(c).unwrap();

    let before = e.snap();
    deleted_party(e.acct.settle_bill_payment(settle_payload(c, TransactionType::Sale, Some(bill), 1.0)), c);
    deleted_party(e.acct.settle_bill_payment(settle_payload(c, TransactionType::Sale, None, 1.0)), c);
    deleted_party(e.acct.preview_settlement_allocation(c, TransactionType::Sale, 1.0), c);
    deleted_party(
        e.acct.update_settlement(
            sid,
            UpdateSettlementPayload { amount_paid: 10.0, settlement_date: DAY.into(), payment_mode: None, notes: None },
        ),
        c,
    );
    deleted_party(e.acct.delete_settlement(sid), c);
    assert_eq!(e.snap(), before);

    e.parties.restore_party(c).unwrap();
    e.acct.delete_settlement(sid).unwrap();
    assert_ledger_invariant(&e.db);
}

// ---- E7 / DD-88: a party change moves SETTLEMENT rows only when confirmed ----

#[test]
fn e7_party_change_with_settlements_needs_the_confirm_flag() {
    let e = env();
    let a = e.party("A", PartyType::Customer);
    let b = e.party("B", PartyType::Customer);
    let bill = e.sell(Some(a), Unpaid, 0.0, 10.0);
    e.settle(a, TransactionType::Sale, bill, 30.0);
    e.settle(a, TransactionType::Sale, bill, 20.0);

    let g = e.sales.get_sale_edit_guard(bill).unwrap();
    assert_eq!(g.settlements_to_move, SettlementsToMove { count: 2, amount: 50.0 });
    assert!(g.can_change_party);

    let before = e.snap();
    let (code, msg) = locked_code(e.sales.update_sale(bill, e.sp(Some(b), Unpaid, 0.0, 10.0)));
    assert_eq!(code, "SETTLEMENTS_ON_BILL");
    assert_eq!(msg, "This bill has 2 later payments (\u{20b9}50.00). Confirm moving them to 'B'.");
    assert_eq!(e.snap(), before, "nothing written");

    // the preview runs the same body: the error, and nothing written
    let pv = e
        .sales
        .preview_sale(
            Some(bill),
            PreviewSalePayload {
                transaction_date: DAY.into(),
                party_id: Some(b),
                payment_status: None,
                paid_amount: None,
                payment_mode: None,
                notes: None,
                items: vec![CreateSaleItemPayload { item_id: 1, bundle_id: None, bundle_quantity: 10.0, unit_price: 10.0 }],
                charges: vec![],
                confirm_move_settlements: false,
            },
        )
        .unwrap();
    assert_eq!(pv.errors.iter().map(|x| x.field.as_str()).collect::<Vec<_>>(), vec!["SETTLEMENTS_ON_BILL"]);
    assert_eq!(e.snap(), before);

    // confirmed: the rows move, both ledgers and both dues summaries follow
    let mut p = e.sp(Some(b), Unpaid, 0.0, 10.0);
    p.confirm_move_settlements = true;
    let r = e.sales.update_sale(bill, p).unwrap();
    assert_eq!(r.balance_due, 50.0);
    assert!(e.by_party(a).is_empty());
    assert_eq!(e.by_party(b).len(), 2);
    let dues = e.acct.get_party_dues_summary(PartyType::Customer).unwrap();
    assert_eq!(dues.iter().map(|d| (d.party_id, d.balance_due)).collect::<Vec<_>>(), vec![(b, 50.0)]);
    assert!(e.acct.get_party_ledger(a).unwrap().bills.is_empty());
    assert_eq!(e.acct.get_party_ledger(b).unwrap().bills.len(), 1);
    assert_eq!(e.sales.get_sale_edit_guard(bill).unwrap().settlements_to_move.count, 2);
    assert_ledger_invariant(&e.db);
}

#[test]
fn e7_purchase_vendor_change_needs_the_flag_too() {
    let e = env();
    let v1 = e.party("V1", PartyType::Vendor);
    let v2 = e.party("V2", PartyType::Vendor);
    let p = e.purchases.create_purchase(purchase_payload(Some(v1), Unpaid, 0.0, &[(1, None, 10.0, 5.0)])).unwrap().purchase_id;
    e.settle(v1, TransactionType::Purchase, p, 10.0);
    let lot = e.db.begin_read().unwrap().purchases().list_items_by_transaction_id(p).unwrap()[0].id;
    let g = e.purchases.get_purchase_edit_guard(p).unwrap();
    assert_eq!(g.settlements_to_move, SettlementsToMove { count: 1, amount: 10.0 });

    let mut edit = purchase_payload(Some(v2), Unpaid, 0.0, &[(1, None, 10.0, 5.0)]);
    edit.items[0].purchase_item_id = Some(lot);
    let (code, msg) = locked_code(e.purchases.update_purchase(p, edit.clone()));
    assert_eq!(code, "SETTLEMENTS_ON_BILL");
    assert!(msg.contains("1 later payment (\u{20b9}10.00)"), "{msg}");
    edit.confirm_move_settlements = true;
    e.purchases.update_purchase(p, edit).unwrap();
    assert!(e.by_party(v1).is_empty());
    assert_eq!(e.by_party(v2).len(), 1);
    assert_ledger_invariant(&e.db);
}

#[test]
fn e7_no_settlements_no_flag_needed_and_return_credit_still_blocks() {
    let e = env();
    let a = e.party("A", PartyType::Customer);
    let b = e.party("B", PartyType::Customer);
    let bill = e.sell(Some(a), Unpaid, 0.0, 10.0);
    assert_eq!(e.sales.get_sale_edit_guard(bill).unwrap().settlements_to_move, SettlementsToMove::default());
    e.sales.update_sale(bill, e.sp(Some(b), Unpaid, 0.0, 10.0)).expect("no payments to move");

    // unlinked return credit on the bill: still PARTY_CHANGE_WITH_CREDITS, even with the flag
    e.rets.create_customer_return(unlinked_customer_return(Some(b))).unwrap();
    e.settle(b, TransactionType::Sale, bill, 5.0);
    let mut p = e.sp(Some(a), Unpaid, 0.0, 10.0);
    p.confirm_move_settlements = true;
    let before = e.snap();
    assert_eq!(locked_code(e.sales.update_sale(bill, p)).0, "PARTY_CHANGE_WITH_CREDITS");
    assert_eq!(e.snap(), before);
    assert_ledger_invariant(&e.db);
}
