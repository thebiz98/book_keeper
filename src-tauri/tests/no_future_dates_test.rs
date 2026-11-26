// tests/no_future_dates_test.rs
//! FR-28 / DD-58: an entry may be dated today or earlier, never later. "Today" is pinned with
//! `core::clock::with_today`. Production services on real SQLite.

mod common;

use common::ledger::*;
use common::*;
use book_keeper::core::clock::with_today;
use book_keeper::core::dtos::*;
use book_keeper::core::error::AppError;
use book_keeper::core::money::check_entry_date;
use book_keeper::core::value_objects::*;
use book_keeper::services::contracts::accounting_service::AccountingService;
use book_keeper::services::contracts::expense_service::ExpenseService;
use book_keeper::services::contracts::inventory_service::InventoryService;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::return_service::ReturnService;
use book_keeper::services::contracts::sale_service::SaleService;
use PaymentStatus::*;

type Inv = InventoryServiceImpl<FifoStrategy>;
type Acct = AccountingServiceImpl<PaymentServiceImpl>;

const TODAY: &str = "2026-11-21";
const TOMORROW: &str = "2026-11-22";
const CUSTOMER: i64 = 1;
const VENDOR: i64 = 2;
const MSG: &str = "Date cannot be in the future";

struct Env {
    db: TestDb,
    inv: Inv,
    sales: SaleServiceImpl<Inv, PaymentServiceImpl>,
    purchases: PurchaseServiceImpl<Inv, PaymentServiceImpl>,
    rets: ReturnServiceImpl<Inv, PaymentServiceImpl, Acct>,
    acct: Acct,
    exp: ExpenseServiceImpl,
}

fn env() -> Env {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let acct = AccountingServiceImpl::new(PaymentServiceImpl, db.clone());
    seed_party(&db, CUSTOMER, "Cust", PartyType::Customer);
    seed_party(&db, VENDOR, "Vend", PartyType::Vendor);
    seed_stocked_item(&db, &inv, 1, 20.0, 60.0, 100.0);
    let index: std::sync::Arc<dyn book_keeper::data::contracts::FtsIndex> = std::sync::Arc::new(db.expense_type_index());
    Env {
        sales: SaleServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        purchases: PurchaseServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        rets: ReturnServiceImpl::new(inv.clone(), PaymentServiceImpl, acct.clone(), db.clone()),
        exp: ExpenseServiceImpl::new(db.clone(), index),
        acct,
        inv,
        db,
    }
}

/// The error must be the date rule on `field`.
fn assert_future<T: std::fmt::Debug>(r: Result<T, AppError>, field: &str) {
    match r {
        Err(AppError::ValidationError { field: f, message }) => {
            assert_eq!((f.as_str(), message.as_str()), (field, MSG));
        }
        other => panic!("expected a future-date error on {field}, got {other:?}"),
    }
}

fn sale_on(date: &str) -> CreateSalePayload {
    let mut p = sale_payload(None, Paid, 100.0, &[(1, None, 1.0, 100.0)]);
    p.transaction_date = date.into();
    p
}

fn purchase_on(date: &str) -> CreatePurchasePayload {
    let mut p = purchase_payload(None, Paid, 60.0, &[(1, None, 1.0, 60.0)]);
    p.transaction_date = date.into();
    p
}

fn preview_of(p: &CreateSalePayload) -> PreviewSalePayload {
    PreviewSalePayload {
        confirm_move_settlements: false,
        transaction_date: p.transaction_date.clone(),
        party_id: p.party_id,
        payment_status: Some(p.payment_status),
        paid_amount: Some(p.paid_amount),
        payment_mode: p.payment_mode.clone(),
        notes: p.notes.clone(),
        items: p.items.clone(),
        charges: p.charges.clone(),
    }
}

#[test]
fn helper_accepts_today_and_past_and_rejects_later() {
    assert!(check_entry_date("d", "2026-11-21", "2026-11-21").is_ok());
    assert!(check_entry_date("d", "2026-11-20", "2026-11-21").is_ok());
    assert_future(check_entry_date("d", "2026-11-22", "2026-11-21"), "d");
    // A malformed date keeps the format message.
    match check_entry_date("d", "nope", "2026-11-21") {
        Err(AppError::ValidationError { message, .. }) => assert!(message.contains("YYYY-MM-DD")),
        other => panic!("{other:?}"),
    }
}

#[test]
fn sale_today_passes_tomorrow_is_rejected_and_preview_agrees() {
    let e = env();
    with_today(TODAY, || {
        assert!(e.sales.create_sale(sale_on(TODAY)).is_ok());
        let bad = sale_on(TOMORROW);
        assert_future(e.sales.create_sale(bad.clone()), "transaction_date");
        let pv = e.sales.preview_sale(None, preview_of(&bad)).unwrap();
        assert_eq!(
            pv.errors,
            vec![PreviewError::new("transaction_date", MSG)]
        );
    });
}

#[test]
fn editing_a_future_dated_sale_without_moving_the_date_is_rejected() {
    let e = env();
    // The bill was legitimately dated later (clock one day ahead when it was saved).
    let id = with_today(TOMORROW, || e.sales.create_sale(sale_on(TOMORROW)).unwrap().sale_id);
    with_today(TODAY, || {
        assert_future(e.sales.update_sale(id, sale_on(TOMORROW)), "transaction_date");
        let pv = e.sales.preview_sale(Some(id), preview_of(&sale_on(TOMORROW))).unwrap();
        assert_eq!(pv.errors.len(), 1);
        assert_eq!(pv.errors[0].field, "transaction_date");
        // Moving it to today is fine.
        assert!(e.sales.update_sale(id, sale_on(TODAY)).is_ok());
    });
}

#[test]
fn purchase_today_passes_tomorrow_is_rejected_edit_and_preview_too() {
    let e = env();
    with_today(TODAY, || {
        assert!(e.purchases.create_purchase(purchase_on(TODAY)).is_ok());
        assert_future(e.purchases.create_purchase(purchase_on(TOMORROW)), "transaction_date");
        let pp = purchase_on(TOMORROW);
        let pv = e
            .purchases
            .preview_purchase(
                None,
                PreviewPurchasePayload {
                    confirm_move_settlements: false,
                    transaction_date: pp.transaction_date.clone(),
                    party_id: pp.party_id,
                    supplier_invoice_ref: None,
                    payment_status: Some(pp.payment_status),
                    paid_amount: Some(pp.paid_amount),
                    payment_mode: pp.payment_mode.clone(),
                    notes: None,
                    items: pp.items.clone(),
                    charges: vec![],
                },
            )
            .unwrap();
        assert_eq!(pv.errors[0].field, "transaction_date");
    });
    let id = with_today(TOMORROW, || e.purchases.create_purchase(purchase_on(TOMORROW)).unwrap().purchase_id);
    with_today(TODAY, || {
        assert_future(e.purchases.update_purchase(id, purchase_on(TOMORROW)), "transaction_date");
    });
}

#[test]
fn expense_date_rule() {
    let e = env();
    let ty = e.exp.create_expense_type(CreateExpenseTypePayload { name: "Rent".into() }).unwrap().id;
    let pay = |d: &str| ExpensePayload { expense_date: d.into(), expense_type_id: ty, amount: 10.0, notes: None };
    with_today(TODAY, || {
        let ok = e.exp.create_expense(pay(TODAY)).unwrap();
        assert_future(e.exp.create_expense(pay(TOMORROW)), "expense_date");
        assert_future(e.exp.update_expense(ok.id, pay(TOMORROW)), "expense_date");
        assert!(e.exp.update_expense(ok.id, pay(DAY)).is_ok());
    });
}

#[test]
fn settlement_single_batch_and_edit() {
    let e = env();
    let bill = e.sales.create_sale({
        let mut p = sale_payload(Some(CUSTOMER), Unpaid, 0.0, &[(1, None, 2.0, 100.0)]);
        p.transaction_date = DAY.into();
        p
    });
    let bill = bill.unwrap().sale_id;
    let single = |d: &str| SettlePaymentPayload {
        party_id: CUSTOMER,
        transaction_type: TransactionType::Sale,
        transaction_id: Some(bill),
        settlement_date: d.into(),
        amount_paid: 50.0,
        payment_mode: Some("CASH".into()),
        notes: None,
    };
    let batch = |d: &str| SettleBatchPayload {
        party_id: CUSTOMER,
        transaction_type: TransactionType::Sale,
        settlement_date: d.into(),
        payment_mode: Some("CASH".into()),
        notes: None,
        allocations: vec![SettleAllocation { transaction_id: bill, amount_paid: 10.0 }],
    };
    with_today(TODAY, || {
        assert_future(e.acct.settle_bill_payment(single(TOMORROW)), "settlement_date");
        assert_future(e.acct.settle_bills_batch(batch(TOMORROW)), "settlement_date");
        let ok = e.acct.settle_bill_payment(single(TODAY)).unwrap();
        let sid = ok.settlement_ids[0];
        let upd = |d: &str| UpdateSettlementPayload {
            amount_paid: 50.0,
            settlement_date: d.into(),
            payment_mode: Some("CASH".into()),
            notes: None,
        };
        assert_future(e.acct.update_settlement(sid, upd(TOMORROW)), "settlement_date");
        assert!(e.acct.update_settlement(sid, upd(TODAY)).is_ok());
        assert!(e.acct.settle_bills_batch(batch(TODAY)).is_ok());
    });
}

#[test]
fn return_dates_and_previews() {
    let e = env();
    let cust = |d: &str| CustomerReturnPayload {
        original_sale_id: None,
        party_id: Some(CUSTOMER),
        return_date: d.into(),
        settlement_type: SettlementType::Refund,
        reason: None,
        notes: None,
        items: vec![],
        replacement_items: vec![],
    };
    let vend = |d: &str| VendorReturnPayload {
        original_purchase_id: None,
        party_id: Some(VENDOR),
        return_date: d.into(),
        settlement_type: SettlementType::Refund,
        reason: None,
        notes: None,
        items: vec![],
    };
    with_today(TODAY, || {
        assert_future(e.rets.create_customer_return(cust(TOMORROW)), "return_date");
        assert_future(e.rets.preview_customer_return(cust(TOMORROW)), "return_date");
        assert_future(e.rets.create_vendor_return(vend(TOMORROW)), "return_date");
        assert_future(e.rets.preview_vendor_return(vend(TOMORROW)), "return_date");
        // Today passes the date rule (it then fails on the empty item list, not on the date).
        match e.rets.create_customer_return(cust(TODAY)) {
            Err(AppError::ValidationError { message, .. }) => assert_ne!(message, MSG),
            other => panic!("{other:?}"),
        }
    });
}

#[test]
fn stock_adjustment_and_set_stock() {
    let e = env();
    let adj = |d: &str| StockAdjustmentPayload {
        adjustment_date: d.into(),
        item_id: 1,
        quantity: -1.0,
        reason: AdjustmentReason::Damaged,
        unit_cost: None,
        notes: None,
    };
    let set = |d: &str| SetItemStockPayload {
        item_id: 1,
        counted_quantity: 19.0,
        unit_cost: None,
        adjustment_date: d.into(),
        notes: None,
    };
    with_today(TODAY, || {
        assert_future(e.inv.create_adjustment(adj(TOMORROW)), "adjustment_date");
        assert_future(e.inv.preview_adjustment(adj(TOMORROW)), "adjustment_date");
        assert_future(e.inv.set_item_stock(set(TOMORROW)), "adjustment_date");
        assert_future(e.inv.preview_set_item_stock(set(TOMORROW)), "adjustment_date");
        assert!(e.inv.create_adjustment(adj(TODAY)).is_ok());
        assert!(e.inv.set_item_stock(set(TODAY)).is_ok());
    });
}
