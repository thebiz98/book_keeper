//! Test Suite: FR-47 / DD-82 — a typed item total sets the line's rate (total ÷ qty), on real SQLite.

mod common;

use book_keeper::core::dtos::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::sale_service::SaleService;
use common::ledger::*;
use common::*;
use PaymentStatus::*;

type Inv = InventoryServiceImpl<FifoStrategy>;

fn env() -> (TestDb, SaleServiceImpl<Inv, PaymentServiceImpl>) {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    seed_party(&db, 1, "Cust", PartyType::Customer);
    seed_stocked_item(&db, &inv, 1, 50.0, 60.0, 100.0);
    let sales = SaleServiceImpl::new(inv, PaymentServiceImpl, db.clone());
    (db, sales)
}

fn line(s: &impl SaleService, q: f64, r: f64, t: Option<f64>) -> Result<SaleLinePreview, AppError> {
    s.preview_sale_line(PreviewSaleLinePayload { bundle_quantity: q, unit_price: r, entered_total: t })
}

fn field_of(e: AppError) -> String {
    match e {
        AppError::ValidationError { field, .. } => field,
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

#[test]
fn an_untouched_total_keeps_the_rate() {
    let (_db, s) = env();
    assert_eq!(line(&s, 10.0, 102.3, None).unwrap(), SaleLinePreview { line_total: 1023.0, unit_price: 102.3 });
    assert_eq!(line(&s, 0.0, 100.0, None).unwrap().line_total, 0.0);
}

#[test]
fn a_typed_total_sets_the_rate_and_gives_the_total_back() {
    let (_db, s) = env();
    assert_eq!(line(&s, 10.0, 102.3, Some(1000.0)).unwrap(), SaleLinePreview { line_total: 1000.0, unit_price: 100.0 });
    let third = line(&s, 3.0, 350.0, Some(1000.0)).unwrap();
    assert_eq!(third.line_total, 1000.0);
    assert!((third.unit_price - 1000.0 / 3.0).abs() < 1e-12, "full precision: {}", third.unit_price);
    assert_eq!(line(&s, 2.5, 10.0, Some(0.0)).unwrap(), SaleLinePreview { line_total: 0.0, unit_price: 0.0 });
}

#[test]
fn bad_input_is_rejected() {
    let (_db, s) = env();
    for bad in [-1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(field_of(line(&s, 1.0, 1.0, Some(bad)).unwrap_err()), "entered_total");
    }
    assert_eq!(field_of(line(&s, 0.0, 1.0, Some(5.0)).unwrap_err()), "entered_total");
    assert_eq!(field_of(line(&s, -1.0, 1.0, None).unwrap_err()), "bundle_quantity");
    assert_eq!(field_of(line(&s, 1.0, f64::NAN, None).unwrap_err()), "unit_price");
}

#[test]
fn a_sale_at_the_derived_rate_saves_the_typed_total_and_no_charge_lines() {
    let (db, s) = env();
    let rate = line(&s, 3.0, 100.0, Some(1000.0)).unwrap().unit_price;
    let id = s.create_sale(sale_payload(Some(1), Unpaid, 0.0, &[(1, None, 3.0, rate)])).unwrap().sale_id;
    let mut tx = db.begin_read().unwrap();
    let lines = tx.sales().list_items_by_transaction_id(id).unwrap();
    assert_eq!(lines.iter().map(|l| l.total_amount).collect::<Vec<_>>(), vec![1000.0]);
    let h = tx.sales().find_transaction_by_id(id).unwrap().unwrap();
    assert_eq!((h.items_subtotal, h.charges_total, h.discounts_total, h.total_amount), (1000.0, 0.0, 0.0, 1000.0));
    assert!(tx.bill_charges().list_by_transaction(TransactionType::Sale, id).unwrap().is_empty());
}
