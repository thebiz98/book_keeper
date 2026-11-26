// tests/preview_test.rs
//! Test Suite: DD-56 previews on real SQLite. A preview runs the same service code as its command in
//! a transaction that is dropped. For each preview: it equals the command (equivalence), it leaves
//! no row, sequence or stock change behind (no side effects), and it reports the command's error
//! (same errors).

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
use common::fixtures::*;
use common::invariants::*;
use common::ledger::*;
use common::*;
use PaymentStatus::*;

type Inv = InventoryServiceImpl<FifoStrategy>;
type Acct = AccountingServiceImpl<PaymentServiceImpl>;

const CUSTOMER: i64 = 1;
const VENDOR: i64 = 2;

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
    seed_party(&db, CUSTOMER, "Cust", PartyType::Customer);
    seed_party(&db, VENDOR, "Vend", PartyType::Vendor);
    // Item 1: 20 units @ 60, sale price 100, with a 6-unit pack. Item 2: no stock history.
    seed_stocked_item(&db, &inv, 1, 0.0, 0.0, 100.0);
    {
        let mut tx = db.begin_transaction().unwrap();
        tx.bundles().create(&create_test_bundle(1, 1, "Carton", 6.0, Some(600.0), None)).unwrap();
        tx.items().create(&create_test_item(2, "Item 2", None, None, 0.0, 50.0)).unwrap();
        tx.commit().unwrap();
    }
    let e = Env {
        sales: SaleServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        purchases: PurchaseServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        rets: ReturnServiceImpl::new(inv.clone(), PaymentServiceImpl, acct.clone(), db.clone()),
        types: ChargeTypeServiceImpl::new(db.clone(), index),
        acct,
        inv,
        db,
    };
    e.buy(None, Paid, 1200.0, &[(1, 20.0, 60.0)]);
    e
}

impl Env {
    /// Every table's row count, the code sequences, every item's stock and the lot remainders.
    fn snap(&self) -> String {
        let c = self.db.pool.get().unwrap();
        let names: Vec<String> = c
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        let mut out: Vec<String> = names
            .iter()
            .map(|n| {
                let k: i64 = c.query_row(&format!("SELECT COUNT(*) FROM \"{n}\""), [], |r| r.get(0)).unwrap();
                format!("{n}={k}")
            })
            .collect();
        let q = |sql: &str| -> String { c.query_row(sql, [], |r| r.get::<_, Option<String>>(0)).unwrap().unwrap_or_default() };
        out.push(q("SELECT group_concat(prefix||':'||last_value) FROM code_sequences"));
        out.push(q("SELECT group_concat(id||':'||current_stock) FROM items"));
        out.push(q("SELECT group_concat(id||':'||remaining_quantity||':'||unit_cost) FROM purchase_items"));
        out.join("|")
    }

    /// Runs a preview and proves it changed nothing.
    fn quiet<T>(&self, f: impl FnOnce() -> T) -> T {
        let before = self.snap();
        let r = f();
        assert_eq!(self.snap(), before, "a preview must leave no trace");
        r
    }

    fn charge(&self, name: &str, effect: ChargeEffect) -> i64 {
        self.types
            .create_charge_type(CreateChargeTypePayload {
                name: name.into(),
                effect,
                applies_to: ChargeAppliesTo::Both,
                default_amount: None,
            })
            .unwrap()
            .id
    }

    fn sale_row(&self, id: i64) -> (SaleTransaction, Vec<f64>) {
        let mut tx = self.db.begin_read().unwrap();
        let h = tx.sales().find_transaction_by_id(id).unwrap().unwrap();
        let lines = tx.sales().list_items_by_transaction_id(id).unwrap().iter().map(|l| l.total_amount).collect();
        (h, lines)
    }

    fn purchase_row(&self, id: i64) -> (PurchaseTransaction, Vec<f64>) {
        let mut tx = self.db.begin_read().unwrap();
        let h = tx.purchases().find_transaction_by_id(id).unwrap().unwrap();
        let lines = tx.purchases().list_items_by_transaction_id(id).unwrap().iter().map(|l| l.total_amount).collect();
        (h, lines)
    }

    fn buy(&self, party: Option<i64>, st: PaymentStatus, paid: f64, lines: &[(i64, f64, f64)]) -> i64 {
        let l: Vec<_> = lines.iter().map(|&(i, q, c)| (i, None, q, c)).collect();
        self.purchases.create_purchase(purchase_payload(party, st, paid, &l)).expect("purchase").purchase_id
    }

    fn pv_sale(&self, id: Option<i64>, p: &CreateSalePayload) -> BillPreview {
        self.quiet(|| self.sales.preview_sale(id, as_preview_sale(p, true)).unwrap())
    }

    fn pv_purchase(&self, id: Option<i64>, p: &CreatePurchasePayload) -> BillPreview {
        self.quiet(|| self.purchases.preview_purchase(id, as_preview_purchase(p, true)).unwrap())
    }

    fn lots_of(&self, purchase: i64) -> Vec<PurchaseItem> {
        self.db.begin_read().unwrap().purchases().list_items_by_transaction_id(purchase).unwrap()
    }

    fn sale_line_ids(&self, sale: i64) -> Vec<SaleItem> {
        self.db.begin_read().unwrap().sales().list_items_by_transaction_id(sale).unwrap()
    }

    fn check(&self) {
        assert_ledger_invariant(&self.db);
        assert_lot_flow_invariant(&self.db);
        let mut tx = self.db.begin_read().unwrap();
        let bad = self.inv.verify_stock_cache(&mut *tx).unwrap();
        assert!(bad.is_empty(), "stock cache drift: {bad:?}");
    }
}

fn as_preview_sale(p: &CreateSalePayload, with_payment: bool) -> PreviewSalePayload {
    PreviewSalePayload {
        transaction_date: p.transaction_date.clone(),
        party_id: p.party_id,
        payment_status: with_payment.then_some(p.payment_status),
        paid_amount: with_payment.then_some(p.paid_amount),
        payment_mode: p.payment_mode.clone(),
        notes: p.notes.clone(),
        items: p.items.clone(),
        charges: p.charges.clone(),
    }
}

fn as_preview_purchase(p: &CreatePurchasePayload, with_payment: bool) -> PreviewPurchasePayload {
    PreviewPurchasePayload {
        transaction_date: p.transaction_date.clone(),
        party_id: p.party_id,
        supplier_invoice_ref: p.supplier_invoice_ref.clone(),
        payment_status: with_payment.then_some(p.payment_status),
        paid_amount: with_payment.then_some(p.paid_amount),
        payment_mode: p.payment_mode.clone(),
        notes: p.notes.clone(),
        items: p.items.clone(),
        charges: p.charges.clone(),
    }
}

fn ci(id: i64, amount: f64) -> BillChargeInput {
    BillChargeInput { charge_type_id: id, amount }
}

fn mapped(e: &AppError) -> PreviewError {
    PreviewError::from_app_error(e).expect("a business error")
}

/// The preview of a sale/purchase equals what the saved bill ended up as.
fn assert_bill_matches(
    p: &BillPreview,
    total: f64,
    subtotal: f64,
    charges: f64,
    discounts: f64,
    lines: &[f64],
    payment: BillPaymentPreview,
) {
    assert!(p.errors.is_empty(), "unexpected errors {:?}", p.errors);
    assert_eq!(p.line_totals, lines);
    assert_eq!(
        (p.items_subtotal, p.charges_total, p.discounts_total, p.total_amount),
        (subtotal, charges, discounts, total)
    );
    assert_eq!(p.payment, Some(payment));
}

// ---------------------------------------------------------------------------------------------
// Sales: equivalence
// ---------------------------------------------------------------------------------------------

/// Previews, creates, and compares the preview with the saved header, lines and summary.
fn sale_case(e: &Env, p: CreateSalePayload) -> i64 {
    let pv = e.pv_sale(None, &p);
    let s = e.sales.create_sale(p).unwrap();
    let (h, lines) = e.sale_row(s.sale_id);
    assert_bill_matches(
        &pv,
        h.total_amount,
        h.items_subtotal,
        h.charges_total,
        h.discounts_total,
        &lines,
        BillPaymentPreview {
            paid_amount: s.paid_amount,
            balance_due: s.balance_due,
            change_amount: s.change_amount,
            payment_status: s.payment_status,
        },
    );
    assert_eq!((pv.applied_amount, pv.open_amount), (0.0, h.total_amount));
    e.check();
    s.sale_id
}

#[test]
fn sale_preview_equals_create_cash_with_change() {
    let e = env();
    let id = sale_case(&e, sale_payload(None, Paid, 250.0, &[(1, None, 2.0, 100.0)]));
    let s = e.sale_row(id).0;
    assert_eq!((s.total_amount, s.payment_status), (200.0, Paid));
}

#[test]
fn sale_preview_equals_create_partial_and_unpaid() {
    let e = env();
    sale_case(&e, sale_payload(Some(CUSTOMER), Partial, 50.0, &[(1, None, 2.0, 100.0)]));
    sale_case(&e, sale_payload(Some(CUSTOMER), Unpaid, 0.0, &[(1, None, 1.0, 100.0)]));
}

#[test]
fn sale_preview_equals_create_with_charges_and_discounts() {
    let e = env();
    let freight = e.charge("Freight", ChargeEffect::Charge);
    let disc = e.charge("Discount", ChargeEffect::Discount);
    let mut p = sale_payload(Some(CUSTOMER), Paid, 205.0, &[(1, None, 2.0, 100.0)]);
    p.charges = vec![ci(freight, 20.0), ci(disc, 15.0)];
    let id = sale_case(&e, p);
    assert_eq!(e.sale_row(id).0.total_amount, 205.0);
}

#[test]
fn sale_preview_equals_create_with_packs() {
    let e = env();
    let id = sale_case(&e, sale_payload(None, Paid, 1300.0, &[(1, Some(1), 2.0, 600.0), (1, None, 1.0, 100.0)]));
    assert_eq!(e.sale_row(id).1, vec![1200.0, 100.0]);
}

#[test]
fn sale_edit_preview_equals_update_with_applied_money() {
    let e = env();
    let id = e.sales.create_sale(sale_payload(Some(CUSTOMER), Partial, 50.0, &[(1, None, 2.0, 100.0)])).unwrap().sale_id;
    e.acct.settle_bill_payment(settle_payload(CUSTOMER, TransactionType::Sale, Some(id), 30.0)).unwrap();
    let edit = sale_payload(Some(CUSTOMER), Partial, 20.0, &[(1, None, 3.0, 100.0)]);
    let pv = e.pv_sale(Some(id), &edit);
    assert_eq!((pv.total_amount, pv.applied_amount, pv.open_amount), (300.0, 30.0, 270.0));
    let s = e.sales.update_sale(id, edit).unwrap();
    let (h, lines) = e.sale_row(id);
    assert_bill_matches(
        &pv,
        h.total_amount,
        h.items_subtotal,
        h.charges_total,
        h.discounts_total,
        &lines,
        BillPaymentPreview {
            paid_amount: s.paid_amount,
            balance_due: s.balance_due,
            change_amount: s.change_amount,
            payment_status: s.payment_status,
        },
    );
    assert_eq!((h.paid_amount, h.balance_due), (50.0, 250.0)); // 20 upfront + 30 settled
    e.check();
}

#[test]
fn sale_preview_without_payment_gives_totals_only() {
    let e = env();
    let p = sale_payload(None, Paid, 0.0, &[(1, None, 2.0, 100.0)]);
    let pv = e.quiet(|| e.sales.preview_sale(None, as_preview_sale(&p, false)).unwrap());
    assert_eq!((pv.total_amount, pv.open_amount, pv.line_totals.clone()), (200.0, 200.0, vec![200.0]));
    assert!(pv.payment.is_none() && pv.errors.is_empty());
    // Stock is still checked without a payment.
    let big = sale_payload(None, Paid, 0.0, &[(1, None, 50.0, 100.0)]);
    let pv = e.quiet(|| e.sales.preview_sale(None, as_preview_sale(&big, false)).unwrap());
    assert_eq!(pv.errors.len(), 1);
    assert_eq!(pv.total_amount, 5000.0);
}

#[test]
fn sale_preview_does_not_consume_the_next_code() {
    let e = env();
    let before = e.sales.get_next_sale_code().unwrap();
    e.pv_sale(None, &sale_payload(None, Paid, 200.0, &[(1, None, 2.0, 100.0)]));
    assert_eq!(e.sales.get_next_sale_code().unwrap(), before);
    let s = e.sales.create_sale(sale_payload(None, Paid, 200.0, &[(1, None, 2.0, 100.0)])).unwrap();
    assert_eq!(s.transaction_code, before);
}

// ---------------------------------------------------------------------------------------------
// Sales: same errors
// ---------------------------------------------------------------------------------------------

#[test]
fn sale_preview_reports_the_commands_errors() {
    let e = env();
    // Invalid payment: PAID with too little cash. The totals still come back.
    let bad_pay = sale_payload(None, Paid, 10.0, &[(1, None, 2.0, 100.0)]);
    let pv = e.pv_sale(None, &bad_pay);
    let err = e.sales.create_sale(bad_pay).unwrap_err();
    assert_eq!(pv.errors, vec![mapped(&err)]);
    assert_eq!((pv.total_amount, pv.line_totals.clone(), pv.payment.clone()), (200.0, vec![200.0], None));

    // Credit without a customer.
    let walk_in_credit = sale_payload(None, Unpaid, 0.0, &[(1, None, 1.0, 100.0)]);
    let pv = e.pv_sale(None, &walk_in_credit);
    assert_eq!(pv.errors, vec![mapped(&e.sales.create_sale(walk_in_credit).unwrap_err())]);

    // Insufficient stock.
    let big = sale_payload(None, Paid, 5000.0, &[(1, None, 50.0, 100.0)]);
    let pv = e.pv_sale(None, &big);
    let err = e.sales.create_sale(big).unwrap_err();
    assert!(matches!(err, AppError::InsufficientStockError { .. }));
    assert_eq!(pv.errors, vec![mapped(&err)]);
    assert_eq!(pv.total_amount, 5000.0);

    // Payload shape.
    let empty = sale_payload(None, Paid, 0.0, &[]);
    let pv = e.pv_sale(None, &empty);
    assert_eq!(pv.errors, vec![mapped(&e.sales.create_sale(empty).unwrap_err())]);
    e.check();
}

#[test]
fn sale_edit_preview_reports_total_below_applied_like_update() {
    let e = env();
    let id = e.sales.create_sale(sale_payload(Some(CUSTOMER), Partial, 50.0, &[(1, None, 2.0, 100.0)])).unwrap().sale_id;
    e.acct.settle_bill_payment(settle_payload(CUSTOMER, TransactionType::Sale, Some(id), 100.0)).unwrap();
    let edit = sale_payload(Some(CUSTOMER), Paid, 0.0, &[(1, None, 1.0, 50.0)]);
    let pv = e.pv_sale(Some(id), &edit);
    let err = e.sales.update_sale(id, edit).unwrap_err();
    assert!(matches!(&err, AppError::BillLocked { code, .. } if code == "TOTAL_BELOW_APPLIED"));
    assert_eq!(pv.errors, vec![mapped(&err)]);
    assert_eq!((pv.total_amount, pv.applied_amount), (50.0, 100.0));
    e.check();
}

// ---------------------------------------------------------------------------------------------
// Purchases
// ---------------------------------------------------------------------------------------------

fn purchase_case(e: &Env, p: CreatePurchasePayload) -> i64 {
    let pv = e.pv_purchase(None, &p);
    let s = e.purchases.create_purchase(p).unwrap();
    let (h, lines) = e.purchase_row(s.purchase_id);
    assert_bill_matches(
        &pv,
        h.total_amount,
        h.items_subtotal,
        h.charges_total,
        h.discounts_total,
        &lines,
        BillPaymentPreview {
            paid_amount: h.paid_amount,
            balance_due: h.balance_due,
            change_amount: 0.0,
            payment_status: h.payment_status,
        },
    );
    e.check();
    s.purchase_id
}

#[test]
fn purchase_preview_equals_create() {
    let e = env();
    purchase_case(&e, purchase_payload(None, Paid, 100.0, &[(1, None, 10.0, 10.0)]));
    purchase_case(&e, purchase_payload(Some(VENDOR), Partial, 40.0, &[(1, None, 10.0, 10.0)]));
    purchase_case(&e, purchase_payload(Some(VENDOR), Unpaid, 0.0, &[(1, None, 10.0, 10.0)]));
    purchase_case(&e, purchase_payload(None, Paid, 3000.0, &[(1, Some(1), 5.0, 600.0)]));
    let freight = e.charge("Freight", ChargeEffect::Charge);
    let disc = e.charge("Discount", ChargeEffect::Discount);
    let mut p = purchase_payload(Some(VENDOR), Paid, 105.0, &[(1, None, 10.0, 10.0)]);
    p.charges = vec![ci(freight, 20.0), ci(disc, 15.0)];
    let id = purchase_case(&e, p);
    assert_eq!(e.purchase_row(id).0.total_amount, 105.0);
}

#[test]
fn purchase_edit_preview_equals_update_with_applied_money() {
    let e = env();
    let id = e.buy(Some(VENDOR), Partial, 40.0, &[(1, 10.0, 10.0)]);
    e.acct.settle_bill_payment(settle_payload(VENDOR, TransactionType::Purchase, Some(id), 20.0)).unwrap();
    let lot = e.lots_of(id)[0].id;
    let mut edit = purchase_payload(Some(VENDOR), Partial, 10.0, &[(1, None, 12.0, 10.0)]);
    edit.items[0].purchase_item_id = Some(lot);
    let pv = e.pv_purchase(Some(id), &edit);
    assert_eq!((pv.total_amount, pv.applied_amount, pv.open_amount), (120.0, 20.0, 100.0));
    e.purchases.update_purchase(id, edit).unwrap();
    let (h, lines) = e.purchase_row(id);
    assert_bill_matches(
        &pv,
        h.total_amount,
        h.items_subtotal,
        h.charges_total,
        h.discounts_total,
        &lines,
        BillPaymentPreview {
            paid_amount: h.paid_amount,
            balance_due: h.balance_due,
            change_amount: 0.0,
            payment_status: h.payment_status,
        },
    );
    e.check();
}

#[test]
fn purchase_preview_without_payment_and_with_errors() {
    let e = env();
    let p = purchase_payload(None, Paid, 0.0, &[(1, None, 10.0, 10.0)]);
    let pv = e.quiet(|| e.purchases.preview_purchase(None, as_preview_purchase(&p, false)).unwrap());
    assert_eq!((pv.total_amount, pv.line_totals.clone()), (100.0, vec![100.0]));
    assert!(pv.payment.is_none() && pv.errors.is_empty());

    let over = purchase_payload(Some(VENDOR), Paid, 500.0, &[(1, None, 10.0, 10.0)]);
    let pv = e.pv_purchase(None, &over);
    assert_eq!(pv.errors, vec![mapped(&e.purchases.create_purchase(over).unwrap_err())]);
    assert_eq!(pv.total_amount, 100.0);

    let no_vendor = purchase_payload(None, Unpaid, 0.0, &[(1, None, 10.0, 10.0)]);
    let pv = e.pv_purchase(None, &no_vendor);
    assert_eq!(pv.errors, vec![mapped(&e.purchases.create_purchase(no_vendor).unwrap_err())]);
}

// ---------------------------------------------------------------------------------------------
// Lot price
// ---------------------------------------------------------------------------------------------

fn lot_case(e: &Env, bill: i64, lot: i64, price: f64) {
    let up = UpdateLotPricePayload { purchase_item_id: lot, unit_price: price };
    let pv = e.quiet(|| e.purchases.preview_lot_purchase_price(up.clone()).unwrap());
    let done = e.purchases.update_lot_purchase_price(up).unwrap();
    assert_eq!(pv, done);
    assert_eq!(e.purchase_row(bill).0.total_amount, done.bill.unwrap().new_total);
    e.check();
}

#[test]
fn lot_price_preview_equals_command_cash_partial_and_settled_later() {
    let e = env();
    let cash = e.buy(None, Paid, 100.0, &[(1, 10.0, 10.0)]);
    let cash_lot = e.lots_of(cash)[0].id;
    lot_case(&e, cash, cash_lot, 12.0);
    lot_case(&e, cash, cash_lot, 8.0);

    let partial = e.buy(Some(VENDOR), Partial, 40.0, &[(1, 10.0, 10.0)]);
    lot_case(&e, partial, e.lots_of(partial)[0].id, 6.0);

    let later = e.buy(Some(VENDOR), Unpaid, 0.0, &[(1, 10.0, 10.0)]);
    e.acct.settle_bill_payment(settle_payload(VENDOR, TransactionType::Purchase, Some(later), 30.0)).unwrap();
    lot_case(&e, later, e.lots_of(later)[0].id, 15.0);
}

#[test]
fn lot_price_preview_reports_the_commands_errors() {
    let e = env();
    let bill = e.buy(Some(VENDOR), Unpaid, 0.0, &[(1, 10.0, 10.0)]);
    let lot = e.lots_of(bill)[0].id;
    e.acct.settle_bill_payment(settle_payload(VENDOR, TransactionType::Purchase, Some(bill), 80.0)).unwrap();
    for price in [5.0, -1.0, f64::NAN] {
        let up = UpdateLotPricePayload { purchase_item_id: lot, unit_price: price };
        let pv = e.quiet(|| e.purchases.preview_lot_purchase_price(up.clone()).unwrap_err());
        assert_eq!(pv, e.purchases.update_lot_purchase_price(up).unwrap_err(), "price {price}");
    }
    let missing = UpdateLotPricePayload { purchase_item_id: 9999, unit_price: 1.0 };
    assert_eq!(
        e.purchases.preview_lot_purchase_price(missing.clone()).unwrap_err(),
        e.purchases.update_lot_purchase_price(missing).unwrap_err()
    );
}

// ---------------------------------------------------------------------------------------------
// Settlement allocation
// ---------------------------------------------------------------------------------------------

#[test]
fn settlement_allocation_preview_equals_the_lump_sum_command() {
    let e = env();
    let mut bills = Vec::new();
    for qty in [1.0, 2.0, 3.0] {
        bills.push(e.sales.create_sale(sale_payload(Some(CUSTOMER), Unpaid, 0.0, &[(1, None, qty, 100.0)])).unwrap().sale_id);
    }
    // 100 + 200 + 300 outstanding; 250 covers bill 1 fully and 150 of bill 2.
    let pv = e.quiet(|| e.acct.preview_settlement_allocation(CUSTOMER, TransactionType::Sale, 250.0).unwrap());
    assert_eq!(
        pv,
        vec![
            SettleAllocation { transaction_id: bills[0], amount_paid: 100.0 },
            SettleAllocation { transaction_id: bills[1], amount_paid: 150.0 },
        ]
    );
    let done = e.acct.settle_bill_payment(settle_payload(CUSTOMER, TransactionType::Sale, None, 250.0)).unwrap();
    assert_eq!(done.bills_touched, bills[..2].to_vec());
    let booked: Vec<(i64, f64)> = {
        let mut tx = e.db.begin_read().unwrap();
        done.settlement_ids
            .iter()
            .map(|id| tx.settlements().find_by_id(*id).unwrap().unwrap())
            .map(|r| (r.transaction_id.unwrap(), r.amount_paid))
            .collect()
    };
    assert_eq!(booked, pv.iter().map(|a| (a.transaction_id, a.amount_paid)).collect::<Vec<_>>());
    e.check();
}

#[test]
fn settlement_allocation_preview_reports_the_commands_errors() {
    let e = env();
    e.sales.create_sale(sale_payload(Some(CUSTOMER), Unpaid, 0.0, &[(1, None, 1.0, 100.0)])).unwrap();
    for amount in [500.0, 0.0, -5.0, f64::NAN] {
        let pv = e.quiet(|| e.acct.preview_settlement_allocation(CUSTOMER, TransactionType::Sale, amount).unwrap_err());
        let cmd = e.acct.settle_bill_payment(settle_payload(CUSTOMER, TransactionType::Sale, None, amount)).unwrap_err();
        assert_eq!(pv, cmd, "amount {amount}");
    }
    // Wrong party side and unknown party.
    for party in [VENDOR, 777] {
        let pv = e.acct.preview_settlement_allocation(party, TransactionType::Sale, 10.0).unwrap_err();
        let cmd = e.acct.settle_bill_payment(settle_payload(party, TransactionType::Sale, None, 10.0)).unwrap_err();
        assert_eq!(pv, cmd, "party {party}");
    }
}

// ---------------------------------------------------------------------------------------------
// Returns
// ---------------------------------------------------------------------------------------------

fn cust_line(item: i64, sale_item: Option<i64>, qty: f64, price: f64, cost: Option<f64>) -> CustomerReturnItemPayload {
    CustomerReturnItemPayload {
        item_id: item,
        sale_item_id: sale_item,
        quantity: qty,
        unit_price: price,
        condition: StockCondition::Restockable,
        unit_cost_override: cost,
    }
}

fn cust_return(sale: Option<i64>, party: Option<i64>, items: Vec<CustomerReturnItemPayload>) -> CustomerReturnPayload {
    CustomerReturnPayload {
        original_sale_id: sale,
        party_id: party,
        return_date: DAY.to_string(),
        settlement_type: SettlementType::Refund,
        reason: None,
        notes: None,
        items,
        replacement_items: vec![],
    }
}

fn customer_case(e: &Env, p: CustomerReturnPayload) -> ReturnResult {
    let pv = e.quiet(|| e.rets.preview_customer_return(p.clone()).unwrap());
    let done = e.rets.create_customer_return(p).unwrap();
    assert_eq!(pv, done);
    e.check();
    done
}

#[test]
fn customer_return_preview_equals_command_linked_and_unlinked() {
    let e = env();
    // Linked to a partially paid bill: the refund credits the dues first, the rest is cash.
    let sale = e.sales.create_sale(sale_payload(Some(CUSTOMER), Partial, 50.0, &[(1, None, 4.0, 100.0)])).unwrap().sale_id;
    let line = e.sale_line_ids(sale)[0].id;
    let r = customer_case(&e, cust_return(Some(sale), None, vec![cust_line(1, Some(line), 3.0, 100.0, None)]));
    assert_eq!((r.total_amount, r.credited_amount, r.cash_refunded), (300.0, 300.0, 0.0));

    // Linked, bigger than the open balance: 100 open, refund 100 more than that is cash.
    let rest = e.sale_line_ids(sale)[0].quantity - 3.0;
    assert_eq!(rest, 1.0);
    let r = customer_case(&e, cust_return(Some(sale), None, vec![cust_line(1, Some(line), 1.0, 100.0, None)]));
    assert_eq!((r.total_amount, r.credited_amount, r.cash_refunded), (100.0, 50.0, 50.0));

    // Unlinked walk-in return: all cash; unlinked with a customer and no bills: all cash too.
    let r = customer_case(&e, cust_return(None, None, vec![cust_line(1, None, 2.0, 90.0, None)]));
    assert_eq!((r.total_amount, r.credited_amount, r.cash_refunded), (180.0, 0.0, 180.0));
    let r = customer_case(&e, cust_return(None, Some(CUSTOMER), vec![cust_line(2, None, 1.0, 40.0, Some(25.0))]));
    assert_eq!((r.total_amount, r.cash_refunded), (40.0, 40.0));
}

#[test]
fn customer_replacement_preview_reports_the_issued_value() {
    let e = env();
    let sale = e.sales.create_sale(sale_payload(Some(CUSTOMER), Paid, 200.0, &[(1, None, 2.0, 100.0)])).unwrap().sale_id;
    let line = e.sale_line_ids(sale)[0].id;
    let mut p = cust_return(Some(sale), None, vec![cust_line(1, Some(line), 1.0, 100.0, None)]);
    p.settlement_type = SettlementType::Replacement;
    p.replacement_items = vec![ReplacementItemPayload { item_id: 1, quantity: 2.0 }];
    let r = customer_case(&e, p);
    assert_eq!((r.total_amount, r.issued_value, r.cash_refunded), (100.0, 200.0, 0.0));
}

#[test]
fn customer_return_preview_reports_the_commands_errors() {
    let e = env();
    let sale = e.sales.create_sale(sale_payload(Some(CUSTOMER), Paid, 200.0, &[(1, None, 2.0, 100.0)])).unwrap().sale_id;
    let line = e.sale_line_ids(sale)[0].id;
    let cases = vec![
        cust_return(Some(sale), None, vec![cust_line(1, Some(line), 5.0, 100.0, None)]), // more than sold
        cust_return(Some(sale), None, vec![cust_line(1, Some(line), 1.0, 500.0, None)]), // above the sold rate
        cust_return(None, None, vec![cust_line(2, None, 1.0, 10.0, None)]),              // no cost, no lot
        cust_return(None, None, vec![]),
        cust_return(None, None, vec![cust_line(1, None, 0.0, 10.0, None)]),
    ];
    for p in cases {
        let pv = e.quiet(|| e.rets.preview_customer_return(p.clone()).unwrap_err());
        assert_eq!(pv, e.rets.create_customer_return(p).unwrap_err());
    }
}

fn vendor_return(purchase: Option<i64>, party: Option<i64>, items: Vec<VendorReturnItemPayload>) -> VendorReturnPayload {
    VendorReturnPayload {
        original_purchase_id: purchase,
        party_id: party,
        return_date: DAY.to_string(),
        settlement_type: SettlementType::Refund,
        reason: None,
        notes: None,
        items,
    }
}

fn vendor_case(e: &Env, p: VendorReturnPayload) -> ReturnResult {
    let pv = e.quiet(|| e.rets.preview_vendor_return(p.clone()).unwrap());
    let done = e.rets.create_vendor_return(p).unwrap();
    assert_eq!(pv, done);
    e.check();
    done
}

#[test]
fn vendor_return_preview_equals_command_linked_and_unlinked() {
    let e = env();
    let bill = e.buy(Some(VENDOR), Partial, 40.0, &[(1, 10.0, 10.0)]);
    let lot = e.lots_of(bill)[0].id;
    // Named lot on its bill: credits the 60 payable first, rest is cash from the vendor.
    let r = vendor_case(
        &e,
        vendor_return(Some(bill), None, vec![VendorReturnItemPayload { item_id: 1, purchase_item_id: Some(lot), quantity: 8.0 }]),
    );
    assert_eq!((r.total_amount, r.credited_amount, r.cash_refunded), (80.0, 60.0, 20.0));
    // DD-86: no bill, the vendor's lot of a paid bill; nothing is open, so all cash.
    let paid = e.buy(Some(VENDOR), Paid, 100.0, &[(1, 10.0, 10.0)]);
    let paid_lot = e.lots_of(paid)[0].id;
    let r = vendor_case(
        &e,
        vendor_return(None, Some(VENDOR), vec![VendorReturnItemPayload { item_id: 1, purchase_item_id: Some(paid_lot), quantity: 2.0 }]),
    );
    assert_eq!((r.total_amount, r.credited_amount, r.cash_refunded), (20.0, 0.0, 20.0));
}

#[test]
fn vendor_return_preview_reports_the_commands_errors() {
    let e = env();
    let line = |qty| vec![VendorReturnItemPayload { item_id: 1, purchase_item_id: None, quantity: qty }];
    for p in [
        vendor_return(None, None, line(500.0)),
        vendor_return(None, Some(VENDOR), line(1.0)), // VENDOR_LOT_REQUIRED
        vendor_return(None, None, vec![]),
        vendor_return(Some(9999), None, line(1.0)),
    ] {
        let pv = e.quiet(|| e.rets.preview_vendor_return(p.clone()).unwrap_err());
        assert_eq!(pv, e.rets.create_vendor_return(p).unwrap_err());
    }
}

// ---------------------------------------------------------------------------------------------
// Stock adjustment and set stock
// ---------------------------------------------------------------------------------------------

fn adj(item: i64, qty: f64, reason: AdjustmentReason, cost: Option<f64>) -> StockAdjustmentPayload {
    StockAdjustmentPayload {
        adjustment_date: DAY.to_string(),
        item_id: item,
        quantity: qty,
        reason,
        unit_cost: cost,
        notes: None,
    }
}

#[test]
fn adjustment_preview_equals_command() {
    let e = env();
    // Positive with the latest-lot default cost (60) and with an explicit cost.
    for (qty, cost, want_cost) in [(5.0, None, 60.0), (2.0, Some(70.0), 70.0)] {
        let p = adj(1, qty, AdjustmentReason::UnrecordedStock, cost);
        let pv = e.quiet(|| e.inv.preview_adjustment(p.clone()).unwrap());
        let done = e.inv.create_adjustment(p).unwrap();
        assert_eq!((pv.unit_cost, pv.loss_value, pv.value_change), (Some(want_cost), 0.0, qty * want_cost));
        assert_eq!(pv.loss_value, done.loss_value);
        let stored: f64 = e.db.pool.get().unwrap()
            .query_row("SELECT unit_cost FROM stock_adjustments WHERE id = ?1", [done.adjustment_id], |r| r.get(0)).unwrap();
        assert_eq!(stored, want_cost);
    }
    // Write-off: FIFO loss (oldest lot 60 first).
    let p = adj(1, -4.0, AdjustmentReason::Damaged, None);
    let before = e.inv.get_item_lots(1).unwrap().len();
    let pv = e.quiet(|| e.inv.preview_adjustment(p.clone()).unwrap());
    let done = e.inv.create_adjustment(p).unwrap();
    assert_eq!((pv.unit_cost, pv.loss_value, pv.value_change), (None, done.loss_value, -done.loss_value));
    assert_eq!(done.loss_value, 240.0);
    assert_eq!((pv.previous_stock, pv.new_stock), (27.0, 23.0));
    assert_eq!(e.inv.get_item_lots(1).unwrap().len(), before);
    e.check();
}

#[test]
fn adjustment_preview_reports_the_commands_errors() {
    let e = env();
    let cases = vec![
        adj(1, -500.0, AdjustmentReason::Damaged, None),           // more than the stock
        adj(2, 3.0, AdjustmentReason::UnrecordedStock, None),     // no prior lot, no cost
        adj(1, 0.0, AdjustmentReason::Damaged, None),
        adj(1, -1.0, AdjustmentReason::UnrecordedStock, None),
        adj(9999, 1.0, AdjustmentReason::UnrecordedStock, Some(1.0)),
    ];
    for p in cases {
        let pv = e.quiet(|| e.inv.preview_adjustment(p.clone()).unwrap_err());
        assert_eq!(pv, e.inv.create_adjustment(p).unwrap_err());
    }
}

fn count(item: i64, counted: f64, cost: Option<f64>) -> SetItemStockPayload {
    SetItemStockPayload { item_id: item, counted_quantity: counted, unit_cost: cost, adjustment_date: DAY.to_string(), notes: None }
}

#[test]
fn set_item_stock_preview_equals_command() {
    let e = env();
    for p in [count(1, 25.0, Some(65.0)), count(1, 25.0, None), count(1, 10.0, None), count(1, 10.0, None)] {
        let pv = e.quiet(|| e.inv.preview_set_item_stock(p.clone()).unwrap());
        let done = e.inv.set_item_stock(p).unwrap();
        assert_eq!(pv, done);
    }
    // Up: unit cost used is the explicit one, then the latest-lot default; down: the FIFO loss.
    let up = e.quiet(|| e.inv.preview_set_item_stock(count(1, 30.0, None)).unwrap());
    assert_eq!((up.delta, up.unit_cost, up.value_change), (20.0, Some(65.0), 1300.0));
    let down = e.quiet(|| e.inv.preview_set_item_stock(count(1, 0.0, None)).unwrap());
    assert_eq!(down.delta, -10.0);
    assert!(down.value_change < 0.0 && down.unit_cost.is_none());
    e.check();
}

#[test]
fn set_item_stock_preview_reports_the_commands_errors() {
    let e = env();
    for p in [count(2, 5.0, None), count(1, -1.0, None), count(1, f64::NAN, None), count(9999, 1.0, None)] {
        let pv = e.quiet(|| e.inv.preview_set_item_stock(p.clone()).unwrap_err());
        assert_eq!(pv, e.inv.set_item_stock(p).unwrap_err());
    }
}
