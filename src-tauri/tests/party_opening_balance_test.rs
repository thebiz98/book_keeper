//! Test Suite: party opening balance as an `OB-` opening bill (FR-35, DD-64, schema v11).
//! Real services on real SQLite. The exclusion test is the authoritative check of the LLD §22 list.

mod common;

use std::sync::Arc;

use book_keeper::core::dtos::*;
use book_keeper::core::entities::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::accounting_service::AccountingService;
use book_keeper::services::contracts::party_service::PartyService;
use book_keeper::services::contracts::report_service::ReportService;
use book_keeper::services::contracts::sale_service::SaleService;
use common::ledger::*;
use common::*;

type Acct = AccountingServiceImpl<PaymentServiceImpl>;

const TODAY: &str = "2026-11-20";

struct Env {
    db: TestDb,
    parties: PartyServiceImpl,
    acct: Acct,
    reports: ReportServiceImpl,
    sales: SaleServiceImpl<InventoryServiceImpl<FifoStrategy>, PaymentServiceImpl>,
}

fn env() -> Env {
    let db = TestDb::new();
    Env {
        parties: PartyServiceImpl::new(db.clone(), Arc::new(db.party_index())),
        acct: AccountingServiceImpl::new(PaymentServiceImpl, db.clone()),
        reports: ReportServiceImpl::with_fixed_today(db.clone(), TODAY),
        sales: SaleServiceImpl::new(InventoryServiceImpl::new(FifoStrategy, db.clone()), PaymentServiceImpl, db.clone()),
        db,
    }
}

fn ob(amount: f64, date: &str) -> Option<OpeningBalanceInput> {
    Some(OpeningBalanceInput { amount, as_of_date: date.to_string() })
}

impl Env {
    fn party(&self, name: &str, t: PartyType, opening: Option<OpeningBalanceInput>) -> i64 {
        self.parties
            .create_party(CreatePartyPayload {
                name: name.into(),
                phone: None,
                party_type: t,
                notes: None,
                opening_balance: opening,
            })
            .expect("party")
    }
    fn update(&self, id: i64, t: PartyType, opening: Option<OpeningBalanceInput>) -> Result<(), AppError> {
        self.parties.update_party(
            id,
            UpdatePartyPayload { name: "P".into(), phone: None, party_type: t, notes: None, opening_balance: opening },
        )
    }
    fn opening(&self, id: i64) -> Option<PartyOpeningBalance> {
        self.parties.get_party(id).unwrap().opening_balance
    }
    fn cash(&self) -> f64 {
        self.reports.get_dashboard_metrics().unwrap().cash_in_hand
    }
    fn count(&self, table: &str) -> i64 {
        let c = self.db.pool.get().unwrap();
        c.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap()
    }
}

fn field_of<T: std::fmt::Debug>(r: Result<T, AppError>) -> String {
    match r {
        Err(AppError::ValidationError { field, .. }) => field,
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

#[test]
fn creates_one_opening_bill_per_side_with_ob_codes() {
    let e = env();
    let c = e.party("Cust", PartyType::Customer, ob(500.0, DAY));
    let v = e.party("Vend", PartyType::Vendor, ob(300.0, "2026-10-01"));
    let co = e.opening(c).expect("customer opening");
    let vo = e.opening(v).expect("vendor opening");
    assert!(co.transaction_code.starts_with("OB-") && vo.transaction_code.starts_with("OB-"));
    assert_ne!(co.transaction_code, vo.transaction_code);
    assert_eq!((co.amount, co.as_of_date.as_str(), co.settled_amount), (500.0, DAY, 0.0));
    assert_eq!(vo.as_of_date, "2026-10-01");
    let mut tx = e.db.begin_read().unwrap();
    let s = tx.sales().find_transaction_by_id(co.bill_id).unwrap().unwrap();
    assert!(s.is_opening_balance);
    assert_eq!((s.total_amount, s.items_subtotal, s.balance_due, s.payment_status), (500.0, 500.0, 500.0, PaymentStatus::Unpaid));
    assert!(tx.sales().list_items_by_transaction_id(s.id).unwrap().is_empty());
    let p = tx.purchases().find_transaction_by_id(vo.bill_id).unwrap().unwrap();
    assert!(p.is_opening_balance);
    assert_eq!(p.source, LotSource::Purchase);
    assert!(tx.purchases().list_items_by_transaction_id(p.id).unwrap().is_empty());
}

#[test]
fn zero_or_absent_opening_creates_no_bill() {
    let e = env();
    let a = e.party("A", PartyType::Customer, None);
    let b = e.party("B", PartyType::Customer, ob(0.0, DAY));
    assert!(e.opening(a).is_none() && e.opening(b).is_none());
    assert_eq!(e.count("sale_transactions"), 0);
}

#[test]
fn at_most_one_opening_bill_per_party() {
    let e = env();
    let c = e.party("Cust", PartyType::Customer, ob(500.0, DAY));
    let first = e.opening(c).unwrap();
    e.update(c, PartyType::Customer, ob(700.0, DAY)).unwrap();
    let second = e.opening(c).unwrap();
    assert_eq!(first.bill_id, second.bill_id, "update changes the same bill");
    assert_eq!(second.amount, 700.0);
    assert_eq!(e.count("sale_transactions"), 1);
    // The index backs it up at the schema level.
    let conn = e.db.pool.get().unwrap();
    let dup = conn.execute(
        "INSERT INTO sale_transactions (transaction_code, transaction_date, party_id, payment_status, total_amount, paid_amount, balance_due, created_at, is_opening_balance) \
         VALUES ('OB-9999', '2026-01-01', ?1, 'UNPAID', 1, 0, 1, 'x', 1)",
        [c],
    );
    assert!(dup.is_err());
}

#[test]
fn update_without_opening_leaves_the_bill_alone() {
    let e = env();
    let c = e.party("Cust", PartyType::Customer, ob(500.0, DAY));
    e.update(c, PartyType::Customer, None).unwrap();
    assert_eq!(e.opening(c).unwrap().amount, 500.0);
}

#[test]
fn removing_an_unsettled_opening_bill_works() {
    let e = env();
    let v = e.party("Vend", PartyType::Vendor, ob(300.0, DAY));
    e.update(v, PartyType::Vendor, ob(0.0, DAY)).unwrap();
    assert!(e.opening(v).is_none());
    assert_eq!(e.count("purchase_transactions"), 0);
}

#[test]
fn reducing_below_the_settled_amount_is_rejected() {
    let e = env();
    let c = e.party("Cust", PartyType::Customer, ob(500.0, DAY));
    let bill = e.opening(c).unwrap().bill_id;
    e.acct.settle_bill_payment(settle_payload(c, TransactionType::Sale, Some(bill), 200.0)).unwrap();
    assert_eq!(e.opening(c).unwrap().settled_amount, 200.0);
    let err = e.update(c, PartyType::Customer, ob(150.0, DAY));
    assert_eq!(field_of(err), "opening_balance.amount");
    let msg = match e.update(c, PartyType::Customer, ob(150.0, DAY)) {
        Err(AppError::ValidationError { message, .. }) => message,
        other => panic!("{other:?}"),
    };
    assert_eq!(msg, "₹200 is already settled against the opening balance");
    // Removing it entirely is the same error.
    assert_eq!(field_of(e.update(c, PartyType::Customer, ob(0.0, DAY))), "opening_balance.amount");
    // At or above the settled amount is fine and recomputes the balance.
    e.update(c, PartyType::Customer, ob(200.0, DAY)).unwrap();
    let mut tx = e.db.begin_read().unwrap();
    let s = tx.sales().find_transaction_by_id(bill).unwrap().unwrap();
    assert_eq!((s.balance_due, s.payment_status), (0.0, PaymentStatus::Paid));
}

#[test]
fn settling_an_opening_bill_moves_cash() {
    let e = env();
    let c = e.party("Cust", PartyType::Customer, ob(500.0, DAY));
    let v = e.party("Vend", PartyType::Vendor, ob(300.0, DAY));
    assert_eq!(e.cash(), 0.0, "the opening bills alone move no cash");
    e.acct.settle_bill_payment(settle_payload(c, TransactionType::Sale, None, 120.0)).unwrap();
    assert_eq!(e.cash(), 120.0);
    e.acct.settle_bill_payment(settle_payload(v, TransactionType::Purchase, None, 100.0)).unwrap();
    assert_eq!(e.cash(), 20.0);
}

#[test]
fn settlement_preview_and_batch_include_the_opening_bill_oldest_first() {
    let e = env();
    let c = e.party("Cust", PartyType::Customer, ob(500.0, "2026-01-05"));
    {
        let mut tx = e.db.begin_transaction().unwrap();
        common::ledger::seed_sale_bill(&mut *tx, 50, Some(c), 300.0, 0.0, DAY);
        tx.commit().unwrap();
    }
    let bill = e.opening(c).unwrap().bill_id;
    let preview = e.acct.preview_settlement_allocation(c, TransactionType::Sale, 600.0).unwrap();
    assert_eq!(preview.len(), 2);
    assert_eq!((preview[0].transaction_id, preview[0].amount_paid), (bill, 500.0), "opening bill first (as-of date)");
    assert_eq!((preview[1].transaction_id, preview[1].amount_paid), (50, 100.0));
    let before = ledger_row_count(&e.db);
    let _ = e.acct.preview_settlement_allocation(c, TransactionType::Sale, 600.0).unwrap();
    assert_eq!(ledger_row_count(&e.db), before, "preview writes nothing");
    let res = e.acct.settle_bills_batch(SettleBatchPayload {
        party_id: c,
        transaction_type: TransactionType::Sale,
        settlement_date: DAY.into(),
        payment_mode: None,
        notes: None,
        allocations: vec![SettleAllocation { transaction_id: bill, amount_paid: 500.0 }],
    });
    assert!(res.is_ok());
    assert_eq!(e.opening(c).unwrap().settled_amount, 500.0);
}

#[test]
fn party_type_change_with_an_opening_bill_is_rejected() {
    let e = env();
    let c = e.party("Cust", PartyType::Customer, ob(500.0, DAY));
    assert_eq!(field_of(e.update(c, PartyType::Vendor, None)), "party_type");
}

#[test]
fn invalid_opening_input_is_rejected() {
    let e = env();
    let bad = |o: Option<OpeningBalanceInput>| {
        field_of(e.parties.create_party(CreatePartyPayload {
            name: "X".into(),
            phone: None,
            party_type: PartyType::Customer,
            notes: None,
            opening_balance: o,
        }))
    };
    assert_eq!(bad(ob(-1.0, DAY)), "opening_balance.amount");
    assert_eq!(bad(ob(f64::NAN, DAY)), "opening_balance.amount");
    assert_eq!(bad(ob(10.0, "2999-01-01")), "opening_balance.as_of_date");
    assert_eq!(bad(ob(10.0, "nope")), "opening_balance.as_of_date");
    assert_eq!(e.count("parties"), 0, "a rejected create leaves no party behind");
}

#[test]
fn opening_bill_cannot_be_edited_deleted_or_returned() {
    let e = env();
    let c = e.party("Cust", PartyType::Customer, ob(500.0, DAY));
    let bill = e.opening(c).unwrap().bill_id;
    let g = e.sales.get_sale_edit_guard(bill).unwrap();
    assert!(!g.can_edit && !g.can_delete);
    assert_eq!(g.reasons[0].code, "OPENING_BALANCE");
    assert!(matches!(e.sales.delete_sale(bill), Err(AppError::BillLocked { code, .. }) if code == "OPENING_BALANCE"));
    let v = e.party("Vend", PartyType::Vendor, ob(300.0, DAY));
    let pbill = e.opening(v).unwrap().bill_id;
    let mut tx = e.db.begin_read().unwrap();
    let pg = book_keeper::services::r#impl::bill_guard::evaluate_purchase_guard(&mut *tx, pbill).unwrap();
    assert!(!pg.can_edit && !pg.can_delete);
    assert_eq!(pg.reasons[0].code, "OPENING_BALANCE");
}

/// The authoritative exclusion test: with one opening bill per side, every sales / purchase /
/// P&L / profit / dashboard figure equals the empty-book value, while dues, ledger and the party
/// balance include the bills.
#[test]
fn opening_bills_are_excluded_from_every_trade_report_and_included_in_dues() {
    let e = env();
    let baseline = |e: &Env| {
        let m = e.reports.get_dashboard_metrics().unwrap();
        let pnl = e.reports.get_pnl_report("2000-01-01", TODAY).unwrap();
        let trend: f64 = e.reports.get_sales_purchase_trend("1y").unwrap().iter().map(|t| t.sales_amount + t.purchase_amount).sum();
        let recent = e.reports.get_recent_transactions(20).unwrap().len();
        let sales = e.sales.get_sales_history(SalesHistoryFilter::default()).unwrap();
        let mut tx = e.db.begin_read().unwrap();
        let (purchases, pcount) = tx.purchases().list_history(&PurchaseHistoryFilter::default(), 0, 50).unwrap();
        let (_, item_bills) = tx.sales().list_bills_for_item(1, 0, 50).unwrap();
        (
            (m.today_sales_amount, m.today_sales_count, m.today_purchase_amount, m.today_purchase_count, m.cash_in_hand),
            (pnl.gross_sales, pnl.net_sales, pnl.gross_profit, pnl.net_profit, pnl.total_purchases),
            (trend, recent, sales.items.len(), sales.total, purchases.len(), pcount, item_bills),
            e.reports.get_stock_valuation_report().unwrap().total_valuation,
        )
    };
    // Stock first: the seed helper uses fixed purchase ids.
    let inv = InventoryServiceImpl::new(FifoStrategy, e.db.clone());
    seed_stocked_item(&e.db, &inv, 1, 10.0, 6.0, 10.0);
    let empty = baseline(&e);
    let c = e.party("Cust", PartyType::Customer, ob(500.0, TODAY));
    let v = e.party("Vend", PartyType::Vendor, ob(300.0, TODAY));
    assert_eq!(baseline(&e), empty, "opening bills change no trade figure");

    // Dues include them.
    let m = e.reports.get_dashboard_metrics().unwrap();
    assert_eq!((m.customer_receivables, m.vendor_payables), (500.0, 300.0));
    let cd = e.acct.get_party_dues_summary(PartyType::Customer).unwrap();
    assert_eq!((cd.len(), cd[0].balance_due), (1, 500.0));
    assert_eq!(e.acct.get_party_dues_summary(PartyType::Vendor).unwrap()[0].balance_due, 300.0);
    // Ledger includes them, flagged.
    let l = e.acct.get_party_ledger(c).unwrap();
    assert_eq!((l.bills.len(), l.current_balance, l.bills[0].is_opening_balance), (1, 500.0, true));
    assert_eq!(l.entries[0].debit, 500.0);
    assert_eq!(e.acct.get_party_ledger(v).unwrap().current_balance, 300.0);
    // Party directory: balance includes, bill count excludes.
    let rows = e.parties.list_parties(None, None).unwrap();
    for r in &rows {
        assert_eq!(r.transaction_count, 0);
        assert!(r.outstanding_balance > 0.0 && r.party.opening_balance.is_some());
    }
    // Profit of a flagged bill is 0 and reads no allocation.
    let bill = e.opening(c).unwrap().bill_id;
    let d = e.sales.get_sale_details(bill).unwrap();
    assert!(d.sale.is_opening_balance && d.items.is_empty());
    assert_eq!(d.profit_summary.total, 0.0);

    // A real sale still shows, alone.
    let s = e.sales.create_sale(sale_payload(None, PaymentStatus::Paid, 100.0, &[(1, None, 2.0, 10.0)])).unwrap();
    let h = e.sales.get_sales_history(SalesHistoryFilter { include_profit: Some(true), ..Default::default() }).unwrap();
    assert_eq!((h.total, h.items.len(), h.items[0].id), (1, 1, s.sale_id));
    assert_eq!(e.reports.get_recent_transactions(20).unwrap().len(), 2, "the stocking purchase and the sale only");
}

#[test]
fn transactions_reset_clears_opening_bills_and_the_ob_sequence() {
    let e = env();
    let c = e.party("Cust", PartyType::Customer, ob(500.0, DAY));
    e.party("Vend", PartyType::Vendor, ob(300.0, DAY));
    assert!(e.opening(c).is_some());
    {
        let mut tx = e.db.begin_transaction().unwrap();
        tx.maintenance().clear_transactions().unwrap();
        tx.commit().unwrap();
    }
    assert_eq!((e.count("sale_transactions"), e.count("purchase_transactions")), (0, 0));
    assert!(e.opening(c).is_none());
    let again = e.party("Cust2", PartyType::Customer, ob(10.0, DAY));
    assert_eq!(e.opening(again).unwrap().transaction_code, "OB-0001", "the OB- sequence restarted");
}

#[test]
fn dues_paths_can_list_the_opening_bill_flagged() {
    let e = env();
    let c = e.party("Cust", PartyType::Customer, ob(500.0, DAY));
    let v = e.party("Vend", PartyType::Vendor, ob(300.0, DAY));
    let f = |p: i64, on: bool| SalesHistoryFilter { party_id: Some(p), include_opening_balance: Some(on), ..Default::default() };
    assert_eq!(e.sales.get_sales_history(f(c, false)).unwrap().total, 0);
    let rows = e.sales.get_sales_history(f(c, true)).unwrap();
    assert_eq!((rows.total, rows.items[0].is_opening_balance, rows.items[0].balance_due), (1, true, 500.0));
    let mut tx = e.db.begin_read().unwrap();
    assert_eq!(tx.purchases().list_history(&f(v, false), 0, 50).unwrap().1, 0);
    let (prows, n) = tx.purchases().list_history(&f(v, true), 0, 50).unwrap();
    assert_eq!((n, prows[0].is_opening_balance), (1, true));
}
