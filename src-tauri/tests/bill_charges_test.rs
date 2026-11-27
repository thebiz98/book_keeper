//! Test Suite: bill-level charges and discounts (FR-19, schema v7) on real SQLite.

mod common;

use std::sync::Arc;

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
use book_keeper::services::contracts::sale_service::SaleService;
use common::fixtures::*;
use common::ledger::*;
use common::*;

type Inv = InventoryServiceImpl<FifoStrategy>;

struct Env {
    db: TestDb,
    sales: SaleServiceImpl<Inv, PaymentServiceImpl>,
    purchases: PurchaseServiceImpl<Inv, PaymentServiceImpl>,
    types: ChargeTypeServiceImpl,
    index: Arc<dyn FtsIndex>,
    inv: Inv,
}

fn env() -> Env {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let index: Arc<dyn FtsIndex> = Arc::new(db.charge_type_index());
    Env {
        sales: SaleServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        purchases: PurchaseServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        types: ChargeTypeServiceImpl::new(db.clone(), index.clone()),
        index,
        inv,
        db,
    }
}

impl Env {
    fn ty(&self, name: &str, effect: ChargeEffect, applies_to: ChargeAppliesTo, default: Option<f64>) -> i64 {
        self.types
            .create_charge_type(CreateChargeTypePayload { name: name.into(), effect, applies_to, default_amount: default })
            .unwrap()
            .id
    }
    fn stocked(&self) {
        seed_party(&self.db, 1, "Cust", PartyType::Customer);
        seed_stocked_item(&self.db, &self.inv, 1, 10.0, 60.0, 100.0);
    }
    fn sale(&self, status: PaymentStatus, paid: f64, charges: Vec<BillChargeInput>) -> Result<i64, AppError> {
        let mut p = sale_payload(Some(1), status, paid, &[(1, None, 2.0, 100.0)]);
        p.charges = charges;
        self.sales.create_sale(p).map(|r| r.sale_id)
    }
    fn header(&self, id: i64) -> SaleTransaction {
        self.db.begin_read().unwrap().sales().find_transaction_by_id(id).unwrap().unwrap()
    }
    fn count(&self, table: &str) -> i64 {
        self.db.pool.get().unwrap().query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap()
    }
}

fn ci(id: i64, amount: f64) -> BillChargeInput {
    BillChargeInput { charge_type_id: id, amount }
}

fn field_of(e: AppError) -> String {
    match e {
        AppError::ValidationError { field, .. } => field,
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

use ChargeAppliesTo::*;
use ChargeEffect::*;

fn both(e: &Env) -> (i64, i64) {
    (e.ty("Transport", Charge, Both, Some(50.0)), e.ty("Cash Discount", Discount, Both, None))
}

#[test]
fn sale_total_includes_charges_minus_discounts() {
    let e = env();
    e.stocked();
    let (t, d) = both(&e);
    let id = e.sale(PaymentStatus::Paid, 230.0, vec![ci(t, 50.0), ci(d, 20.0)]).unwrap();
    let h = e.header(id);
    assert_eq!((h.items_subtotal, h.charges_total, h.discounts_total), (200.0, 50.0, 20.0));
    assert_eq!((h.total_amount, h.paid_amount, h.balance_due), (230.0, 230.0, 0.0));
    let det = e.sales.get_sale_details(id).unwrap();
    assert_eq!(det.charges.len(), 2);
    assert_eq!(det.charges[0].charge_name, "Transport");
    assert_eq!(det.charges[1].effect, Discount);
    assert_eq!(det.items[0].net_unit_price, 90.0);
    let hist = e.sales.get_sales_history(SalesHistoryFilter::default()).unwrap();
    assert_eq!((hist.items[0].charges_total, hist.items[0].discounts_total), (50.0, 20.0));
}

#[test]
fn payment_cross_check_uses_the_grand_total() {
    let e = env();
    e.stocked();
    let (t, d) = both(&e);
    let ch = || vec![ci(t, 50.0), ci(d, 20.0)];
    assert_eq!(field_of(e.sale(PaymentStatus::Paid, 200.0, ch()).unwrap_err()), "paid_amount");
    assert!(e.sale(PaymentStatus::Partial, 229.99, ch()).is_ok());
    assert_eq!(field_of(e.sale(PaymentStatus::Partial, 230.0, ch()).unwrap_err()), "paid_amount");
    let un = e.sale(PaymentStatus::Unpaid, 0.0, ch()).unwrap();
    assert_eq!(e.header(un).balance_due, 230.0);
    // Paid with change.
    let r = e.sales.create_sale({
        let mut p = sale_payload(Some(1), PaymentStatus::Paid, 300.0, &[(1, None, 1.0, 100.0)]);
        p.charges = vec![ci(t, 50.0)];
        p
    });
    let r = r.unwrap();
    assert_eq!((r.total_amount, r.paid_amount, r.change_amount), (150.0, 150.0, 150.0));
}

#[test]
fn discounts_exceeding_the_bill_are_rejected_atomically() {
    let e = env();
    e.stocked();
    let d = e.ty("Big Discount", Discount, Both, None);
    let err = e.sale(PaymentStatus::Paid, 0.0, vec![ci(d, 250.0)]).unwrap_err();
    assert_eq!(field_of(err), "charges");
    assert_eq!(e.count("sale_transactions"), 0);
    assert_eq!(e.count("bill_charges"), 0);
    assert_eq!(e.db.begin_read().unwrap().items().find_by_id(1).unwrap().unwrap().current_stock, 10.0);
}

#[test]
fn discount_equal_to_the_bill_gives_a_zero_total() {
    let e = env();
    e.stocked();
    let d = e.ty("Full Discount", Discount, Both, None);
    let id = e.sale(PaymentStatus::Paid, 0.0, vec![ci(d, 200.0)]).unwrap();
    assert_eq!(e.header(id).total_amount, 0.0);
    assert_eq!(ledger_row_count(&e.db), 0);
    let err = e.sale(PaymentStatus::Unpaid, 0.0, vec![ci(d, 200.0)]).unwrap_err();
    assert_eq!(field_of(err), "payment_status");
}

#[test]
fn amounts_are_rounded_and_bad_inputs_rejected() {
    let e = env();
    e.stocked();
    let (t, d) = both(&e);
    let id = e.sale(PaymentStatus::Paid, 201.0, vec![ci(t, 0.105)]).unwrap();
    assert_eq!(e.db.begin_read().unwrap().bill_charges().list_by_transaction(TransactionType::Sale, id).unwrap()[0].amount, 0.11);
    for bad in [0.004, 0.0, -5.0, f64::NAN] {
        assert_eq!(field_of(e.sale(PaymentStatus::Paid, 200.0, vec![ci(d, bad)]).unwrap_err()), "charges.amount");
    }
    let many: Vec<_> = (0..51).map(|_| ci(t, 1.0)).collect();
    assert_eq!(field_of(e.sale(PaymentStatus::Paid, 300.0, many).unwrap_err()), "charges");
}

#[test]
fn applies_to_and_unknown_types() {
    let e = env();
    e.stocked();
    let only_sale = e.ty("Delivery", Charge, Sale, None);
    let only_pur = e.ty("Carrier Fee", Charge, Purchase, None);
    assert!(e.sale(PaymentStatus::Paid, 210.0, vec![ci(only_sale, 10.0)]).is_ok());
    assert_eq!(field_of(e.sale(PaymentStatus::Paid, 210.0, vec![ci(only_pur, 10.0)]).unwrap_err()), "charge_type_id");
    let mut p = purchase_payload(None, PaymentStatus::Paid, 110.0, &[(1, None, 10.0, 10.0)]);
    p.charges = vec![ci(only_sale, 10.0)];
    assert_eq!(field_of(e.purchases.create_purchase(p).unwrap_err()), "charge_type_id");
    assert!(matches!(e.sale(PaymentStatus::Paid, 200.0, vec![ci(999, 1.0)]).unwrap_err(), AppError::NotFound { .. }));
}

#[test]
fn purchase_charges_do_not_change_lot_cost() {
    let e = env();
    let carrier = e.ty("Carrier Fee", Charge, Purchase, None);
    let disc = e.ty("Early Payment", Discount, Both, None);
    {
        let mut tx = e.db.begin_transaction().unwrap();
        tx.items().create(&create_test_item(1, "Item 1", None, None, 0.0, 100.0)).unwrap();
        tx.commit().unwrap();
    }
    let mk = |status, paid| {
        let mut p = purchase_payload(None, status, paid, &[(1, None, 10.0, 30.0)]);
        p.charges = vec![ci(carrier, 40.0), ci(disc, 10.0)];
        p
    };
    assert!(e.purchases.create_purchase(mk(PaymentStatus::Paid, 300.0)).is_err());
    let r = e.purchases.create_purchase(mk(PaymentStatus::Paid, 330.0)).unwrap();
    let det = e.purchases.get_purchase_details(r.purchase_id).unwrap();
    assert_eq!(det.purchase.total_amount, 330.0);
    assert_eq!(det.purchase.items_subtotal, 300.0);
    assert_eq!(det.items[0].purchase_item.unit_cost, 30.0, "DD-38: lot cost unaffected");
    assert_eq!(det.charges.len(), 2);
    assert_eq!(e.db.begin_read().unwrap().reports().stock_valuation().unwrap().total_valuation, 300.0);
}

#[test]
fn source_lot_headers_have_zero_subtotal_and_no_charges() {
    let e = env();
    e.stocked();
    e.inv
        .set_item_stock(SetItemStockPayload { item_id: 1, counted_quantity: 30.0, unit_cost: Some(3.0), adjustment_date: DAY.into(), notes: None, mode: None })
        .unwrap();
    let conn = e.db.pool.get().unwrap();
    let (n, sum): (i64, f64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(items_subtotal + charges_total + discounts_total), 0) FROM purchase_transactions WHERE source <> 'PURCHASE'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(n >= 1);
    assert_eq!(sum, 0.0);
    assert_eq!(e.count("bill_charges"), 0);
}

#[test]
fn charge_type_service_rules() {
    let e = env();
    let t = e.types.create_charge_type(CreateChargeTypePayload {
        name: "  Transport ".into(), effect: Charge, applies_to: Both, default_amount: Some(50.0),
    }).unwrap();
    assert_eq!(t.name, "Transport");
    let dup = e.types.create_charge_type(CreateChargeTypePayload {
        name: "transport".into(), effect: Discount, applies_to: Sale, default_amount: None,
    });
    assert_eq!(field_of(dup.unwrap_err()), "name");
    for bad in [Some(0.0), Some(-1.0)] {
        let r = e.types.create_charge_type(CreateChargeTypePayload { name: "X".into(), effect: Charge, applies_to: Both, default_amount: bad });
        assert_eq!(field_of(r.unwrap_err()), "default_amount");
    }
    let names = |v: Vec<ChargeType>| v.into_iter().map(|c| c.name).collect::<Vec<_>>();
    assert_eq!(names(e.types.search_charge_types("tra", TransactionType::Sale, None).unwrap()), vec!["Transport"]);
    e.ty("Carrier Fee", Charge, Purchase, None);
    e.ty("Delivery", Charge, Sale, None);
    assert_eq!(
        names(e.types.search_charge_types("", TransactionType::Sale, None).unwrap()),
        vec!["Delivery", "Transport"]
    );
    assert!(e.types.search_charge_types("carr", TransactionType::Sale, None).unwrap().is_empty());
    assert_eq!(names(e.types.search_charge_types("carr", TransactionType::Purchase, None).unwrap()), vec!["Carrier Fee"]);
    for l in [0, 51] {
        assert_eq!(field_of(e.types.search_charge_types("", TransactionType::Sale, Some(l)).unwrap_err()), "limit");
    }
    // Startup repair: a wiped index is rebuilt.
    e.db.pool.get().unwrap().execute("DELETE FROM charge_type_search_fts", []).unwrap();
    assert!(book_keeper::app_state::ensure_fts_index(&*e.index).unwrap());
    assert_eq!(names(e.types.search_charge_types("tra", TransactionType::Sale, None).unwrap()), vec!["Transport"]);
}

#[test]
fn linked_refund_is_capped_at_the_net_rate_and_charges_are_not_refunded() {
    use book_keeper::services::contracts::return_service::ReturnService;
    let e = env();
    e.stocked();
    let acct = AccountingServiceImpl::new(PaymentServiceImpl, e.db.clone());
    let rets = ReturnServiceImpl::new(e.inv.clone(), PaymentServiceImpl, acct, e.db.clone());
    let (t, d) = both(&e);
    let ret = |sale: i64, qty: f64, price: f64, st: SettlementType| CustomerReturnPayload {
        original_sale_id: Some(sale),
        party_id: Some(1),
        return_date: DAY.to_string(),
        settlement_type: st,
        reason: None,
        notes: None,
        items: vec![CustomerReturnItemPayload {
            item_id: 1, sale_item_id: Some(e.db.begin_read().unwrap().sales().list_items_by_transaction_id(sale).unwrap()[0].id),
            quantity: qty, unit_price: price, condition: StockCondition::Restockable, unit_cost_override: None,
        }],
        replacement_items: vec![],
    };
    let discounted = e.sale(PaymentStatus::Paid, 180.0, vec![ci(d, 20.0)]).unwrap();
    assert_eq!(field_of(rets.create_customer_return(ret(discounted, 1.0, 100.0, SettlementType::Refund)).unwrap_err()), "unit_price");
    assert!(rets.create_customer_return(ret(discounted, 1.0, 90.0, SettlementType::Refund)).is_ok());
    let charged = e.sale(PaymentStatus::Unpaid, 0.0, vec![ci(t, 50.0)]).unwrap();
    assert_eq!(e.header(charged).total_amount, 250.0);
    rets.create_customer_return(ret(charged, 2.0, 100.0, SettlementType::Refund)).unwrap();
    let h = e.header(charged);
    assert_eq!(h.balance_due, 50.0, "charges are not refunded");
}

#[test]
fn pnl_and_dashboard_use_items_subtotal_and_fill_new_fields() {
    let e = env();
    let carrier = e.ty("Carrier Fee", Charge, Purchase, None);
    let disc = e.ty("Cash Discount", Discount, Both, None);
    let transport = e.ty("Transport", Charge, Both, None);
    {
        let mut tx = e.db.begin_transaction().unwrap();
        tx.items().create(&create_test_item(1, "Item 1", None, None, 0.0, 100.0)).unwrap();
        tx.commit().unwrap();
    }
    let mut p = purchase_payload(None, PaymentStatus::Paid, 330.0, &[(1, None, 10.0, 30.0)]);
    p.charges = vec![ci(carrier, 40.0), ci(disc, 10.0)];
    e.purchases.create_purchase(p).unwrap();
    let mut s = sale_payload(None, PaymentStatus::Paid, 230.0, &[(1, None, 2.0, 100.0)]);
    s.charges = vec![ci(transport, 50.0), ci(disc, 20.0)];
    e.sales.create_sale(s).unwrap();
    let rs = ReportServiceImpl::with_fixed_today(e.db.clone(), DAY);
    let p = rs.get_pnl_report(DAY, DAY).unwrap();
    assert_eq!((p.gross_sales, p.sales_discounts, p.net_sales, p.cogs, p.gross_profit), (200.0, 20.0, 180.0, 60.0, 120.0));
    assert_eq!((p.sale_charges_income, p.purchase_discounts_income, p.other_income), (50.0, 10.0, 60.0));
    assert_eq!((p.purchase_charges_expense, p.operating_expenses), (40.0, 40.0));
    assert_eq!(p.net_profit, 120.0 + 60.0 - 0.0 - 40.0);
    assert_eq!(p.total_purchases, 300.0);
    let dash = rs.get_dashboard_metrics().unwrap();
    assert_eq!((dash.today_sales_amount, dash.today_purchase_amount), (200.0, 300.0));
    let cash = rs.get_cash_flow_report(DAY, DAY).unwrap();
    assert_eq!((cash.sales_cash_inflow, cash.purchase_cash_outflow), (230.0, 330.0));
}
