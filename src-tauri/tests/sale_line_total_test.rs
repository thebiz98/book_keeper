//! Test Suite: FR-47 / DD-81 — an edited sale item total becomes an "Item extra charge" or
//! "Item discount" bill charge line (schema v13), on real SQLite.

mod common;

use std::sync::Arc;

use book_keeper::core::dtos::*;
use book_keeper::core::entities::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::data::contracts::FtsIndex;
use book_keeper::services::contracts::charge_type_service::ChargeTypeService;
use book_keeper::services::contracts::sale_service::SaleService;
use common::ledger::*;
use common::*;
use PaymentStatus::*;

type Inv = InventoryServiceImpl<FifoStrategy>;

struct Env {
    db: TestDb,
    sales: SaleServiceImpl<Inv, PaymentServiceImpl>,
    types: ChargeTypeServiceImpl,
}

fn env() -> Env {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let index: Arc<dyn FtsIndex> = Arc::new(db.charge_type_index());
    seed_party(&db, 1, "Cust", PartyType::Customer);
    seed_stocked_item(&db, &inv, 1, 50.0, 60.0, 100.0);
    seed_stocked_item(&db, &inv, 2, 50.0, 10.0, 20.0);
    Env {
        sales: SaleServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        types: ChargeTypeServiceImpl::new(db.clone(), index),
        db,
    }
}

/// Lines are (item, qty, rate, entered total).
fn payload(lines: &[(i64, f64, f64, Option<f64>)]) -> CreateSalePayload {
    let plain: Vec<_> = lines.iter().map(|&(i, q, r, _)| (i, None, q, r)).collect();
    let mut p = sale_payload(Some(1), Unpaid, 0.0, &plain);
    for (line, &(_, _, _, t)) in p.items.iter_mut().zip(lines) {
        line.entered_total = t;
    }
    p
}

fn preview(p: &CreateSalePayload) -> PreviewSalePayload {
    PreviewSalePayload {
        transaction_date: p.transaction_date.clone(),
        party_id: p.party_id,
        payment_status: None,
        paid_amount: None,
        payment_mode: p.payment_mode.clone(),
        notes: p.notes.clone(),
        items: p.items.clone(),
        charges: p.charges.clone(),
    }
}

impl Env {
    fn header(&self, id: i64) -> SaleTransaction {
        self.db.begin_read().unwrap().sales().find_transaction_by_id(id).unwrap().unwrap()
    }
    fn line_totals(&self, id: i64) -> Vec<f64> {
        let mut tx = self.db.begin_read().unwrap();
        tx.sales().list_items_by_transaction_id(id).unwrap().iter().map(|l| l.total_amount).collect()
    }
    /// (name, effect, amount) of the bill's charge lines.
    fn charges(&self, id: i64) -> Vec<(String, ChargeEffect, f64)> {
        let mut tx = self.db.begin_read().unwrap();
        tx.bill_charges()
            .list_by_transaction(TransactionType::Sale, id)
            .unwrap()
            .into_iter()
            .map(|c| (c.charge_name, c.effect, c.amount))
            .collect()
    }
    fn count(&self, table: &str) -> i64 {
        self.db.pool.get().unwrap().query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap()
    }
}

fn field_of(e: AppError) -> String {
    match e {
        AppError::ValidationError { field, .. } => field,
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

fn s(n: &str) -> String {
    n.to_string()
}

#[test]
fn a_lower_total_books_an_item_discount_and_the_line_keeps_qty_times_rate() {
    let e = env();
    let id = e.sales.create_sale(payload(&[(1, 10.0, 102.3, Some(1000.0))])).unwrap().sale_id;
    assert_eq!(e.line_totals(id), vec![1023.0]);
    assert_eq!(e.charges(id), vec![(s("Item discount"), ChargeEffect::Discount, 23.0)]);
    let h = e.header(id);
    assert_eq!((h.items_subtotal, h.charges_total, h.discounts_total, h.total_amount), (1023.0, 0.0, 23.0, 1000.0));
}

#[test]
fn a_higher_total_books_an_item_extra_charge() {
    let e = env();
    let id = e.sales.create_sale(payload(&[(1, 2.0, 100.0, Some(210.5))])).unwrap().sale_id;
    assert_eq!(e.charges(id), vec![(s("Item extra charge"), ChargeEffect::Charge, 10.5)]);
    assert_eq!(e.header(id).total_amount, 210.5);
}

#[test]
fn adjustments_are_summed_into_one_line_per_effect_next_to_manual_charges() {
    let e = env();
    let delivery = e
        .types
        .create_charge_type(CreateChargeTypePayload {
            name: "Delivery".into(),
            effect: ChargeEffect::Charge,
            applies_to: ChargeAppliesTo::Sale,
            default_amount: None,
        })
        .unwrap()
        .id;
    let mut p = payload(&[
        (1, 1.0, 100.0, Some(95.0)),
        (2, 3.0, 20.0, Some(58.0)),
        (1, 1.0, 100.0, Some(101.0)),
        (2, 1.0, 20.0, Some(20.0)),
        (2, 1.0, 20.0, None),
    ]);
    p.charges = vec![BillChargeInput { charge_type_id: delivery, amount: 30.0 }];
    let pv = e.sales.preview_sale(None, preview(&p)).unwrap();
    assert_eq!(pv.line_adjustments, vec![-5.0, -2.0, 1.0, 0.0, 0.0]);
    assert_eq!(
        pv.adjustment_lines,
        vec![
            PreviewChargeLine { name: s("Item extra charge"), effect: ChargeEffect::Charge, amount: 1.0 },
            PreviewChargeLine { name: s("Item discount"), effect: ChargeEffect::Discount, amount: 7.0 },
        ]
    );
    let id = e.sales.create_sale(p).unwrap().sale_id;
    assert_eq!(
        e.charges(id),
        vec![
            (s("Delivery"), ChargeEffect::Charge, 30.0),
            (s("Item extra charge"), ChargeEffect::Charge, 1.0),
            (s("Item discount"), ChargeEffect::Discount, 7.0),
        ]
    );
    let h = e.header(id);
    assert_eq!((h.items_subtotal, h.charges_total, h.discounts_total, h.total_amount), (300.0, 31.0, 7.0, 324.0));
    assert_eq!((pv.items_subtotal, pv.charges_total, pv.discounts_total, pv.total_amount), (300.0, 31.0, 7.0, 324.0));
}

#[test]
fn the_preview_equals_the_command_and_leaves_nothing_behind() {
    let e = env();
    let p = payload(&[(1, 10.0, 102.3, Some(1000.0))]);
    let counts = || (e.count("sale_transactions"), e.count("bill_charges"), e.count("sale_items"));
    let before = counts();
    let pv = e.sales.preview_sale(None, preview(&p)).unwrap();
    assert_eq!(counts(), before);
    assert!(pv.errors.is_empty());
    assert_eq!(pv.line_totals, vec![1023.0]);
    assert_eq!(pv.line_adjustments, vec![-23.0]);
    let h = e.header(e.sales.create_sale(p).unwrap().sale_id);
    assert_eq!(
        (pv.items_subtotal, pv.charges_total, pv.discounts_total, pv.total_amount),
        (h.items_subtotal, h.charges_total, h.discounts_total, h.total_amount)
    );
}

#[test]
fn a_bad_entered_total_is_rejected() {
    let e = env();
    for bad in [-1.0, f64::NAN, f64::INFINITY] {
        let r = e.sales.create_sale(payload(&[(1, 1.0, 100.0, Some(bad))]));
        assert_eq!(field_of(r.unwrap_err()), "entered_total");
    }
    assert_eq!(e.count("sale_transactions"), 0);
}

#[test]
fn a_zero_total_is_a_full_discount() {
    let e = env();
    let mut p = payload(&[(1, 1.0, 100.0, Some(0.0))]);
    p.payment_status = Paid; // a zero-value bill must be PAID (existing rule)
    let id = e.sales.create_sale(p).unwrap().sale_id;
    let h = e.header(id);
    assert_eq!((h.discounts_total, h.total_amount, h.payment_status), (100.0, 0.0, Paid));
}

#[test]
fn an_edit_keeps_the_saved_adjustment_lines_as_ordinary_charges() {
    let e = env();
    let id = e.sales.create_sale(payload(&[(1, 10.0, 102.3, Some(1000.0))])).unwrap().sale_id;
    let discount_type = {
        let mut tx = e.db.begin_read().unwrap();
        tx.bill_charges().list_by_transaction(TransactionType::Sale, id).unwrap()[0].charge_type_id
    };
    // The edit screen sends the saved lines back as charge inputs, with no entered totals.
    let mut p = payload(&[(1, 10.0, 102.3, None)]);
    p.charges = vec![BillChargeInput { charge_type_id: discount_type, amount: 23.0 }];
    e.sales.update_sale(id, p).unwrap();
    assert_eq!(e.charges(id), vec![(s("Item discount"), ChargeEffect::Discount, 23.0)]);
    assert_eq!(e.header(id).total_amount, 1000.0);
}

#[test]
fn preview_sale_line_gives_the_total_and_the_signed_adjustment() {
    let e = env();
    let line = |q: f64, r: f64, t: Option<f64>| {
        e.sales.preview_sale_line(PreviewSaleLinePayload { bundle_quantity: q, unit_price: r, entered_total: t })
    };
    assert_eq!(line(10.0, 102.3, None).unwrap(), SaleLinePreview { line_total: 1023.0, adjustment: 0.0 });
    assert_eq!(line(10.0, 102.3, Some(1000.0)).unwrap(), SaleLinePreview { line_total: 1023.0, adjustment: -23.0 });
    assert_eq!(line(2.0, 100.0, Some(200.01)).unwrap().adjustment, 0.01);
    assert_eq!(line(2.0, 100.0, Some(199.99)).unwrap().adjustment, -0.01);
    assert_eq!(line(2.0, 100.0, Some(200.004)).unwrap().adjustment, 0.0);
    assert_eq!(line(0.0, 100.0, None).unwrap().line_total, 0.0);
    assert_eq!(field_of(line(1.0, 1.0, Some(-0.5)).unwrap_err()), "entered_total");
    assert_eq!(field_of(line(-1.0, 1.0, None).unwrap_err()), "bundle_quantity");
    assert_eq!(field_of(line(1.0, f64::NAN, None).unwrap_err()), "unit_price");
}

#[test]
fn the_charge_picker_hides_the_system_types() {
    let e = env();
    let names: Vec<String> = e
        .types
        .search_charge_types("", TransactionType::Sale, None)
        .unwrap()
        .into_iter()
        .map(|c| c.name)
        .collect();
    assert!(names.is_empty(), "{names:?}");
    assert!(e.types.search_charge_types("item", TransactionType::Sale, None).unwrap().is_empty());
}
