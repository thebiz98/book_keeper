//! Test Suite: v1.7 H1 — vendor returns take exact lots, net of the bill discount
//! (FR-53, DD-86, DD-87; findings R1, R2, R3, R10). Production services on real SQLite.

mod common;

use std::sync::Arc;

use common::invariants::{assert_ledger_invariant, assert_lot_flow_invariant};
use common::ledger::*;
use common::*;
use book_keeper::core::dtos::*;
use book_keeper::core::entities::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::data::contracts::FtsIndex;
use book_keeper::services::contracts::charge_type_service::ChargeTypeService;
use book_keeper::services::contracts::inventory_service::InventoryService;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::report_service::ReportService;
use book_keeper::services::contracts::return_service::ReturnService;

type Inv = InventoryServiceImpl<FifoStrategy>;
type Acct = AccountingServiceImpl<PaymentServiceImpl>;

const A: i64 = 5;
const B: i64 = 6;
const ITEM: i64 = 1;

struct Env {
    db: TestDb,
    inv: Inv,
    purchases: PurchaseServiceImpl<Inv, PaymentServiceImpl>,
    rets: ReturnServiceImpl<Inv, PaymentServiceImpl, Acct>,
    types: ChargeTypeServiceImpl,
}

fn env() -> Env {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let acct = AccountingServiceImpl::new(PaymentServiceImpl, db.clone());
    let index: Arc<dyn FtsIndex> = Arc::new(db.charge_type_index());
    seed_party(&db, A, "Vendor A", PartyType::Vendor);
    seed_party(&db, B, "Vendor B", PartyType::Vendor);
    {
        let mut tx = db.begin_transaction().unwrap();
        tx.items().create(&common::fixtures::create_test_item(ITEM, "Widget", None, None, 0.0, 150.0)).unwrap();
        tx.commit().unwrap();
    }
    Env {
        purchases: PurchaseServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        rets: ReturnServiceImpl::new(inv.clone(), PaymentServiceImpl, acct, db.clone()),
        types: ChargeTypeServiceImpl::new(db.clone(), index),
        inv,
        db,
    }
}

impl Env {
    /// A purchase of 10 × ₹100 from `party`; returns (bill id, lot id).
    fn buy(&self, party: i64, status: PaymentStatus, paid: f64, charges: Vec<BillChargeInput>) -> (i64, i64) {
        let mut p = purchase_payload(Some(party), status, paid, &[(ITEM, None, 10.0, 100.0)]);
        p.charges = charges;
        let id = self.purchases.create_purchase(p).expect("purchase").purchase_id;
        let lot = self.db.begin_read().unwrap().purchases().list_items_by_transaction_id(id).unwrap()[0].id;
        (id, lot)
    }
    fn charge_type(&self, name: &str, effect: ChargeEffect) -> i64 {
        self.types
            .create_charge_type(CreateChargeTypePayload { name: name.into(), effect, applies_to: ChargeAppliesTo::Purchase, default_amount: None })
            .unwrap()
            .id
    }
    /// ₹100 discount and ₹50 freight on a 10 × ₹100 bill (total ₹950).
    fn discounted(&self) -> Vec<BillChargeInput> {
        let d = self.charge_type("Trade discount", ChargeEffect::Discount);
        let f = self.charge_type("Freight", ChargeEffect::Charge);
        vec![BillChargeInput { charge_type_id: d, amount: 100.0 }, BillChargeInput { charge_type_id: f, amount: 50.0 }]
    }
    fn purchase(&self, id: i64) -> PurchaseTransaction {
        self.db.begin_read().unwrap().purchases().find_transaction_by_id(id).unwrap().unwrap()
    }
    fn ledger(&self, bill: i64) -> Vec<PaymentSettlement> {
        self.db.begin_read().unwrap().settlements().list_by_transaction(TransactionType::Purchase, bill).unwrap()
    }
    fn stock(&self) -> f64 {
        self.db.begin_read().unwrap().items().find_by_id(ITEM).unwrap().unwrap().current_stock
    }
    fn remaining(&self, lot: i64) -> f64 {
        self.db.begin_read().unwrap().purchases().find_purchase_item_by_id(lot).unwrap().unwrap().remaining_quantity
    }
    fn return_count(&self) -> usize {
        self.rets.list_returns(ReturnsFilter::default()).unwrap().total
    }
    fn pnl(&self) -> PnLReport {
        ReportServiceImpl::with_fixed_today(self.db.clone(), DAY).get_pnl_report(DAY, DAY).unwrap()
    }
    fn invariants(&self) {
        let mut tx = self.db.begin_read().unwrap();
        let bad = self.inv.verify_stock_cache(&mut *tx).unwrap();
        assert!(bad.is_empty(), "stock cache drift: {bad:?}");
        drop(tx);
        assert_ledger_invariant(&self.db);
        assert_lot_flow_invariant(&self.db);
    }
}

fn vr(bill: Option<i64>, party: Option<i64>, lines: &[(Option<i64>, f64)]) -> VendorReturnPayload {
    VendorReturnPayload {
        original_purchase_id: bill,
        party_id: party,
        return_date: DAY.to_string(),
        settlement_type: SettlementType::Refund,
        reason: None,
        notes: None,
        items: lines.iter().map(|&(lot, q)| VendorReturnItemPayload { item_id: ITEM, purchase_item_id: lot, quantity: q }).collect(),
    }
}

fn code_of(r: &Result<ReturnResult, AppError>) -> String {
    match r {
        Err(AppError::BillLocked { code, .. }) => code.clone(),
        other => panic!("expected BillLocked, got {other:?}"),
    }
}

fn kinds(rows: &[PaymentSettlement], k: PaymentKind) -> Vec<f64> {
    rows.iter().filter(|r| r.kind == k).map(|r| r.amount_paid).collect()
}

#[test]
fn r1_vendor_return_without_a_lot_is_refused_and_never_draws_other_lots() {
    let e = env();
    // Opening, adjustment and another vendor's lots of the item, plus vendor A's own lot.
    {
        let mut tx = e.db.begin_transaction().unwrap();
        e.inv.create_source_lot(&mut *tx, LotSource::Opening, DAY, ITEM, 4.0, Some(90.0), None).unwrap();
        tx.commit().unwrap();
    }
    e.inv.create_adjustment(StockAdjustmentPayload {
        adjustment_date: DAY.to_string(), item_id: ITEM, quantity: 3.0, reason: AdjustmentReason::CountCorrection, unit_cost: Some(80.0), notes: None,
    }).unwrap();
    e.buy(B, PaymentStatus::Unpaid, 0.0, vec![]);
    let (_, a_lot) = e.buy(A, PaymentStatus::Unpaid, 0.0, vec![]);
    let stock = e.stock();

    let r = e.rets.create_vendor_return(vr(None, Some(A), &[(None, 2.0)]));
    assert_eq!(code_of(&r), "VENDOR_LOT_REQUIRED");
    assert_eq!((e.stock(), e.return_count(), e.remaining(a_lot)), (stock, 0, 10.0), "nothing written");
    // The picker offers only vendor A's purchase lot.
    let lots = e.rets.list_vendor_return_lots(ITEM, A, None).unwrap();
    assert_eq!(lots.iter().map(|l| l.purchase_item_id).collect::<Vec<_>>(), vec![a_lot]);
    e.invariants();
}

#[test]
fn r1_pnl_ignores_old_fifo_vendor_returns_on_non_purchase_lots() {
    let e = env();
    let opening = {
        let mut tx = e.db.begin_transaction().unwrap();
        let id = e.inv.create_source_lot(&mut *tx, LotSource::Opening, DAY, ITEM, 4.0, Some(90.0), None).unwrap();
        tx.commit().unwrap();
        id
    };
    let (_, lot) = e.buy(A, PaymentStatus::Unpaid, 0.0, vec![]);
    // Old data: a FIFO vendor return drew 2 from the OPENING lot and 1 from the purchase lot.
    {
        let mut tx = e.db.begin_transaction().unwrap();
        let rid = tx.returns().create_return_transaction(&ReturnTransaction {
            id: 0, return_code: "RET-OLD".into(), return_date: DAY.into(), return_type: ReturnType::Vendor, party_id: Some(A),
            party_name: Some("Vendor A".into()), original_transaction_id: None, settlement_type: SettlementType::Refund,
            total_amount: 280.0, reason: None, notes: None, created_at: format!("{DAY}T10:00:00Z"),
        }).unwrap();
        tx.returns().create_return_item(&ReturnItem {
            id: 0, return_transaction_id: rid, line_type: ReturnLineType::Returned, item_id: ITEM, item_name: "Widget".into(),
            sale_item_id: None, purchase_item_id: None, quantity: 3.0, unit_price: 280.0 / 3.0, total_amount: 280.0, cost_amount: 280.0, condition: None,
        }).unwrap();
        for (l, q, c) in [(opening, 2.0, 90.0), (lot, 1.0, 100.0)] {
            tx.outflow_allocations().create_allocation(&StockOutflowAllocation {
                id: 0, source_type: OutflowSourceType::VendorReturn, source_id: rid, purchase_item_id: l, quantity_depleted: q, unit_cost: c,
            }).unwrap();
        }
        tx.commit().unwrap();
    }
    // Purchases 1000 less only the purchase-lot part (100); the OPENING draw (180) never was bought.
    let p = e.pnl();
    assert_eq!(p.total_purchases, 900.0);
    assert_eq!(p.purchase_discount_reversed, 0.0);
}

#[test]
fn r2_another_vendors_lot_or_another_bills_lot_is_not_eligible() {
    let e = env();
    let (_, b_lot) = e.buy(B, PaymentStatus::Unpaid, 0.0, vec![]);
    let (bill1, _) = e.buy(A, PaymentStatus::Unpaid, 0.0, vec![]);
    let (_, lot2) = e.buy(A, PaymentStatus::Unpaid, 0.0, vec![]);
    let stock = e.stock();
    let r = e.rets.create_vendor_return(vr(None, Some(A), &[(Some(b_lot), 1.0)]));
    assert_eq!(code_of(&r), "VENDOR_LOT_NOT_ELIGIBLE");
    let r = e.rets.preview_vendor_return(vr(Some(bill1), None, &[(Some(lot2), 1.0)]));
    assert_eq!(code_of(&r), "VENDOR_LOT_NOT_ELIGIBLE");
    assert_eq!((e.stock(), e.return_count()), (stock, 0));
    // An opening lot is refused too.
    let opening = {
        let mut tx = e.db.begin_transaction().unwrap();
        let id = e.inv.create_source_lot(&mut *tx, LotSource::Opening, DAY, ITEM, 4.0, Some(90.0), None).unwrap();
        tx.commit().unwrap();
        id
    };
    let r = e.rets.create_vendor_return(vr(None, Some(A), &[(Some(opening), 1.0)]));
    assert_eq!(code_of(&r), "VENDOR_LOT_NOT_ELIGIBLE");
    // A vendor return needs a vendor.
    assert!(matches!(
        e.rets.create_vendor_return(vr(None, None, &[(Some(lot2), 1.0)])),
        Err(AppError::ValidationError { .. })
    ));
    e.invariants();
}

#[test]
fn r3_partial_return_of_a_discounted_bill_credits_the_net_value() {
    let e = env();
    let charges = e.discounted();
    let (bill, lot) = e.buy(A, PaymentStatus::Unpaid, 0.0, charges);
    assert_eq!(e.purchase(bill).total_amount, 950.0);
    let preview = e.rets.preview_vendor_return(vr(Some(bill), None, &[(Some(lot), 3.0)])).unwrap();
    assert_eq!((preview.total_amount, preview.discount_reversed), (270.0, 30.0));
    assert_eq!((e.return_count(), e.remaining(lot)), (0, 10.0), "preview writes nothing");

    let res = e.rets.create_vendor_return(vr(Some(bill), None, &[(Some(lot), 3.0)])).unwrap();
    assert_eq!((res.total_amount, res.credited_amount, res.cash_refunded, res.discount_reversed), (270.0, 270.0, 0.0, 30.0));
    let rows = e.ledger(bill);
    assert_eq!(kinds(&rows, PaymentKind::ReturnCredit), vec![270.0]);
    assert!(kinds(&rows, PaymentKind::RefundIn).is_empty());
    assert_eq!(e.purchase(bill).balance_due, 680.0);
    let d = e.rets.get_return_details(res.return_id).unwrap();
    let line = &d.items[0].return_item;
    assert_eq!((line.total_amount, line.cost_amount, line.unit_price), (270.0, 300.0, 90.0));
    assert_eq!(d.allocations[0].unit_cost, 100.0, "stock leaves at gross cost");
    assert_eq!(e.stock(), 7.0);
    let p = e.pnl();
    assert_eq!(p.total_purchases, 700.0);
    assert_eq!(p.purchase_discounts_income, 100.0);
    assert_eq!(p.purchase_discount_reversed, 30.0);
    assert_eq!(p.other_income, 70.0);
    assert_eq!(p.operating_expenses, 50.0, "freight stays a purchase expense");
    e.invariants();

    // Deleting the return restores the lot and both P&L lines vanish.
    e.rets.delete_return(res.return_id).unwrap();
    assert_eq!(e.remaining(lot), 10.0);
    let p = e.pnl();
    assert_eq!((p.total_purchases, p.purchase_discount_reversed, p.other_income), (1000.0, 0.0, 100.0));
    e.invariants();
}

#[test]
fn r3_full_return_of_a_discounted_bill_credits_900_without_cash() {
    let e = env();
    let charges = e.discounted();
    let (bill, lot) = e.buy(A, PaymentStatus::Unpaid, 0.0, charges);
    let res = e.rets.create_vendor_return(vr(Some(bill), None, &[(Some(lot), 10.0)])).unwrap();
    assert_eq!((res.total_amount, res.credited_amount, res.cash_refunded, res.discount_reversed), (900.0, 900.0, 0.0, 100.0));
    assert!(kinds(&e.ledger(bill), PaymentKind::RefundIn).is_empty());
    assert_eq!(e.purchase(bill).balance_due, 50.0, "the freight stays owed");
    let p = e.pnl();
    assert_eq!((p.total_purchases, p.purchase_discount_reversed, p.other_income), (0.0, 100.0, 0.0));
    e.invariants();
}

#[test]
fn r3_paid_bill_refunds_the_net_value_in_cash() {
    let e = env();
    let charges = e.discounted();
    let (bill, lot) = e.buy(A, PaymentStatus::Paid, 950.0, charges);
    let res = e.rets.create_vendor_return(vr(Some(bill), None, &[(Some(lot), 10.0)])).unwrap();
    assert_eq!((res.credited_amount, res.cash_refunded), (0.0, 900.0));
    assert_eq!(kinds(&e.ledger(bill), PaymentKind::RefundIn), vec![900.0]);
    e.invariants();
}

#[test]
fn r3_freight_only_bill_returns_at_gross_cost() {
    let e = env();
    let f = e.charge_type("Freight", ChargeEffect::Charge);
    let (bill, lot) = e.buy(A, PaymentStatus::Unpaid, 0.0, vec![BillChargeInput { charge_type_id: f, amount: 50.0 }]);
    let res = e.rets.create_vendor_return(vr(Some(bill), None, &[(Some(lot), 2.0)])).unwrap();
    assert_eq!((res.total_amount, res.discount_reversed), (200.0, 0.0));
    assert_eq!(e.pnl().operating_expenses, 50.0);
    e.invariants();
}

#[test]
fn r10_overdraw_across_lines_names_the_overdrawing_line() {
    let e = env();
    let (_, lot) = e.buy(A, PaymentStatus::Unpaid, 0.0, vec![]);
    let r = e.rets.create_vendor_return(vr(None, Some(A), &[(Some(lot), 6.0), (Some(lot), 6.0)]));
    match r {
        Err(AppError::ValidationError { field, .. }) => assert_eq!(field, "items[1].quantity"),
        other => panic!("expected ValidationError, got {other:?}"),
    }
    assert_eq!((e.remaining(lot), e.return_count()), (10.0, 0));
    // Within the lot's stock both lines go through.
    let res = e.rets.create_vendor_return(vr(None, Some(A), &[(Some(lot), 6.0), (Some(lot), 4.0)])).unwrap();
    assert_eq!(res.total_amount, 1000.0);
    assert_eq!(e.remaining(lot), 0.0);
    e.invariants();
}

#[test]
fn list_vendor_return_lots_returns_only_eligible_lots_in_id_order() {
    let e = env();
    let charges = e.discounted();
    let (bill1, lot1) = e.buy(A, PaymentStatus::Unpaid, 0.0, charges);
    e.buy(B, PaymentStatus::Unpaid, 0.0, vec![]);
    let (_, lot3) = e.buy(A, PaymentStatus::Unpaid, 0.0, vec![]);
    {
        let mut tx = e.db.begin_transaction().unwrap();
        e.inv.create_source_lot(&mut *tx, LotSource::Opening, DAY, ITEM, 4.0, Some(90.0), None).unwrap();
        tx.commit().unwrap();
    }
    let lots = e.rets.list_vendor_return_lots(ITEM, A, None).unwrap();
    assert_eq!(lots.iter().map(|l| l.purchase_item_id).collect::<Vec<_>>(), vec![lot1, lot3]);
    assert_eq!((lots[0].unit_cost, lots[0].net_unit_cost, lots[0].remaining_quantity), (100.0, 90.0, 10.0));
    assert_eq!((lots[1].net_unit_cost, lots[0].purchase_id), (100.0, bill1));
    assert_eq!(lots[0].base_unit, "PCS");
    let only_bill = e.rets.list_vendor_return_lots(ITEM, A, Some(bill1)).unwrap();
    assert_eq!(only_bill.len(), 1);
    // A used-up lot drops out.
    e.rets.create_vendor_return(vr(None, Some(A), &[(Some(lot3), 10.0)])).unwrap();
    assert_eq!(e.rets.list_vendor_return_lots(ITEM, A, None).unwrap().len(), 1);
    // A deleted item is refused.
    {
        let mut tx = e.db.begin_transaction().unwrap();
        let mut it = tx.items().find_by_id(ITEM).unwrap().unwrap();
        it.is_active = false;
        tx.items().update(&it).unwrap();
        tx.commit().unwrap();
    }
    assert!(e.rets.list_vendor_return_lots(ITEM, A, None).is_err());
}
