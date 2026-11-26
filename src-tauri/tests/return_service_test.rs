// tests/return_service_test.rs
//! Test Suite: Customer / Vendor Returns & Replacements (fix-phase F4 / T5).
//! Test Cases: TC-RET-001 through TC-RET-012. Production services on real SQLite.

mod common;

use common::fixtures::*;
use common::ledger::*;
use common::stock::seed_purchase_lot;
use common::*;
use book_keeper::core::dtos::*;
use book_keeper::core::entities::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::inventory_service::InventoryService;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::return_service::ReturnService;
use book_keeper::services::contracts::sale_service::SaleService;

type Inv = InventoryServiceImpl<FifoStrategy>;
type Acct = AccountingServiceImpl<PaymentServiceImpl>;

struct Env {
    db: TestDb,
    inv: Inv,
    sales: SaleServiceImpl<Inv, PaymentServiceImpl>,
    purchases: PurchaseServiceImpl<Inv, PaymentServiceImpl>,
    rets: ReturnServiceImpl<Inv, PaymentServiceImpl, Acct>,
}

fn env() -> Env {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let acct = AccountingServiceImpl::new(PaymentServiceImpl, db.clone());
    Env {
        sales: SaleServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        purchases: PurchaseServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        rets: ReturnServiceImpl::new(inv.clone(), PaymentServiceImpl, acct, db.clone()),
        inv,
        db,
    }
}

impl Env {
    fn sell(&self, party: Option<i64>, status: PaymentStatus, paid: f64, lines: &[(i64, f64, f64)]) -> i64 {
        let l: Vec<_> = lines.iter().map(|&(i, q, p)| (i, None, q, p)).collect();
        self.sales.create_sale(sale_payload(party, status, paid, &l)).expect("sale").sale_id
    }
    fn sale_lines(&self, sale_id: i64) -> Vec<SaleItem> {
        self.db.begin_read().unwrap().sales().list_items_by_transaction_id(sale_id).unwrap()
    }
    fn sale(&self, id: i64) -> SaleTransaction {
        self.db.begin_read().unwrap().sales().find_transaction_by_id(id).unwrap().unwrap()
    }
    fn purchase(&self, id: i64) -> PurchaseTransaction {
        self.db.begin_read().unwrap().purchases().find_transaction_by_id(id).unwrap().unwrap()
    }
    fn ledger(&self, t: TransactionType, bill: i64) -> Vec<PaymentSettlement> {
        self.db.begin_read().unwrap().settlements().list_by_transaction(t, bill).unwrap()
    }
    fn stock(&self, item: i64) -> f64 {
        self.db.begin_read().unwrap().items().find_by_id(item).unwrap().unwrap().current_stock
    }
    fn return_count(&self) -> usize {
        self.rets.list_returns(ReturnsFilter::default()).unwrap().total
    }
    /// `current_stock == Σ lot.remaining` for every item.
    fn assert_stock(&self) {
        let mut tx = self.db.begin_read().unwrap();
        let bad = self.inv.verify_stock_cache(&mut *tx).unwrap();
        assert!(bad.is_empty(), "stock cache drift: {bad:?}");
    }
    fn returned_qty(&self, sale_item_id: i64) -> f64 {
        let mut tx = self.db.begin_read().unwrap();
        tx.allocations().find_by_sale_item_id(sale_item_id).unwrap().iter().map(|a| a.returned_quantity).sum()
    }
}

fn line(item: i64, sale_item: Option<i64>, qty: f64, price: f64, cond: StockCondition) -> CustomerReturnItemPayload {
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

fn is_validation(r: &Result<ReturnResult, AppError>) -> bool {
    matches!(r, Err(AppError::ValidationError { .. }))
}

fn kinds(rows: &[PaymentSettlement], k: PaymentKind) -> Vec<f64> {
    rows.iter().filter(|r| r.kind == k).map(|r| r.amount_paid).collect()
}

const R: StockCondition = StockCondition::Restockable;
const D: StockCondition = StockCondition::DamagedScrap;

#[test]
fn tc_ret_001_linked_refund_on_unpaid_bill_credits_only() {
    let e = env();
    seed_party(&e.db, 1, "C", PartyType::Customer);
    seed_stocked_item(&e.db, &e.inv, 1, 20.0, 5.0, 10.0);
    let sale = e.sell(Some(1), PaymentStatus::Unpaid, 0.0, &[(1, 10.0, 10.0)]);
    let sl = e.sale_lines(sale)[0].id;
    let res = e.rets.create_customer_return(cust(Some(sale), None, SettlementType::Refund, vec![line(1, Some(sl), 3.0, 10.0, R)])).unwrap();
    assert_eq!((res.credited_amount, res.cash_refunded), (30.0, 0.0));
    let rows = e.ledger(TransactionType::Sale, sale);
    assert_eq!(kinds(&rows, PaymentKind::ReturnCredit), vec![30.0]);
    assert!(kinds(&rows, PaymentKind::RefundOut).is_empty());
    assert_eq!(e.sale(sale).balance_due, 70.0);
    // header snapshots, total, cost
    let d = e.rets.get_return_details(res.return_id).unwrap();
    assert_eq!(d.return_tx.total_amount, 30.0);
    assert_eq!(d.return_tx.party_id, Some(1), "party defaults from the sale");
    assert_eq!(d.return_tx.party_name.as_deref(), Some("C"));
    assert_eq!(d.items[0].return_item.cost_amount, 15.0);
    assert_eq!(d.items[0].return_item.item_name, "Item 1");
    assert_eq!(d.ledger.len(), 1);
    assert_eq!(e.stock(1), 13.0);
    e.assert_stock();
}

#[test]
fn tc_ret_002_linked_refund_partly_paid_credit_then_cash() {
    let e = env();
    seed_party(&e.db, 1, "C", PartyType::Customer);
    seed_stocked_item(&e.db, &e.inv, 1, 20.0, 5.0, 10.0);
    let sale = e.sell(Some(1), PaymentStatus::Partial, 80.0, &[(1, 10.0, 10.0)]);
    let sl = e.sale_lines(sale)[0].id;
    let res = e.rets.create_customer_return(cust(Some(sale), Some(1), SettlementType::Refund, vec![line(1, Some(sl), 5.0, 10.0, D)])).unwrap();
    assert_eq!((res.credited_amount, res.cash_refunded), (20.0, 30.0));
    let rows = e.ledger(TransactionType::Sale, sale);
    assert_eq!(kinds(&rows, PaymentKind::ReturnCredit), vec![20.0]);
    assert_eq!(kinds(&rows, PaymentKind::RefundOut), vec![30.0]);
    assert_eq!(e.sale(sale).payment_status, PaymentStatus::Paid);
    assert_eq!(e.stock(1), 10.0, "scrap does not restock");
    assert_eq!(e.returned_qty(sl), 5.0, "scrap still reverses COGS (TC-FIFO-005)");
    e.assert_stock();
}

#[test]
fn tc_ret_003_quantity_cap_and_duplicates() {
    let e = env();
    seed_stocked_item(&e.db, &e.inv, 1, 20.0, 5.0, 10.0);
    let sale = e.sell(None, PaymentStatus::Paid, 50.0, &[(1, 5.0, 10.0)]);
    let sl = e.sale_lines(sale)[0].id;
    e.rets.create_customer_return(cust(Some(sale), None, SettlementType::Refund, vec![line(1, Some(sl), 3.0, 10.0, R)])).unwrap();
    let (stock, rows, count) = (e.stock(1), ledger_row_count(&e.db), e.return_count());
    // TC-FIFO-007: repeated partial returns never over-restore.
    assert!(is_validation(&e.rets.create_customer_return(cust(Some(sale), None, SettlementType::Refund, vec![line(1, Some(sl), 3.0, 10.0, R)]))));
    // duplicate lines in one payload are summed: 2 + 2 > 2 remaining
    assert!(is_validation(&e.rets.create_customer_return(cust(Some(sale), None, SettlementType::Refund, vec![line(1, Some(sl), 1.0, 10.0, R), line(1, Some(sl), 2.0, 10.0, R)]))));
    assert_eq!((e.stock(1), ledger_row_count(&e.db), e.return_count()), (stock, rows, count), "nothing written");
    assert_eq!(e.returned_qty(sl), 3.0);
    e.rets.create_customer_return(cust(Some(sale), None, SettlementType::Refund, vec![line(1, Some(sl), 2.0, 10.0, R)])).unwrap();
    assert_eq!(e.returned_qty(sl), 5.0);
    e.assert_stock();
}

#[test]
fn tc_ret_004_item_mismatch_price_cap_and_bad_numbers() {
    let e = env();
    seed_stocked_item(&e.db, &e.inv, 1, 20.0, 5.0, 10.0);
    seed_stocked_item(&e.db, &e.inv, 2, 20.0, 5.0, 10.0);
    seed_party(&e.db, 1, "C", PartyType::Customer);
    seed_party(&e.db, 2, "Other", PartyType::Customer);
    let sale = e.sell(Some(1), PaymentStatus::Paid, 50.0, &[(1, 5.0, 10.0)]);
    let other = e.sell(None, PaymentStatus::Paid, 10.0, &[(1, 1.0, 10.0)]);
    let sl = e.sale_lines(sale)[0].id;
    let ol = e.sale_lines(other)[0].id;
    let try_ = |p: CustomerReturnPayload| e.rets.create_customer_return(p);
    assert!(is_validation(&try_(cust(Some(sale), None, SettlementType::Refund, vec![line(2, Some(sl), 1.0, 10.0, R)]))), "item mismatch");
    assert!(is_validation(&try_(cust(Some(sale), None, SettlementType::Refund, vec![line(1, Some(sl), 1.0, 12.0, R)]))), "price cap");
    assert!(is_validation(&try_(cust(Some(sale), None, SettlementType::Refund, vec![line(1, Some(ol), 1.0, 10.0, R)]))), "line of another sale");
    assert!(is_validation(&try_(cust(None, None, SettlementType::Refund, vec![line(1, Some(sl), 1.0, 10.0, R)]))), "linked needs a sale");
    assert!(is_validation(&try_(cust(Some(sale), Some(2), SettlementType::Refund, vec![line(1, Some(sl), 1.0, 10.0, R)]))), "party mismatch");
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(is_validation(&try_(cust(Some(sale), None, SettlementType::Refund, vec![line(1, Some(sl), bad, 10.0, R)]))), "qty {bad}");
    }
    assert!(is_validation(&try_(cust(Some(sale), None, SettlementType::Refund, vec![line(1, Some(sl), 1.0, f64::NAN, R)]))));
    assert!(is_validation(&try_(cust(Some(sale), None, SettlementType::Refund, vec![]))), "empty");
    assert_eq!(e.return_count(), 0);
    // price below the sold price is fine
    try_(cust(Some(sale), None, SettlementType::Refund, vec![line(1, Some(sl), 1.0, 9.0, R)])).unwrap();
    e.assert_stock();
}

#[test]
fn tc_ret_005_unlinked_restockable_walk_in() {
    let e = env();
    seed_stocked_item(&e.db, &e.inv, 1, 5.0, 6.0, 10.0);
    let res = e.rets.create_customer_return(cust(None, None, SettlementType::Refund, vec![line(1, None, 2.0, 10.0, R)])).unwrap();
    assert_eq!((res.credited_amount, res.cash_refunded), (0.0, 20.0));
    let lots = e.inv.get_item_lots(1).unwrap();
    let rl = lots.iter().find(|l| l.source == LotSource::Return).expect("RETURN lot");
    assert!(rl.transaction_code.starts_with("RTL-"));
    assert_eq!((rl.quantity, rl.unit_cost), (2.0, 6.0));
    let d = e.rets.get_return_details(res.return_id).unwrap();
    assert_eq!(kinds(&d.ledger, PaymentKind::RefundOut), vec![20.0]);
    assert_eq!(d.items[0].return_item.cost_amount, 12.0);
    assert_eq!(e.stock(1), 7.0);
    // unlinked scrap: no stock change, cost 0
    e.rets.create_customer_return(cust(None, None, SettlementType::Refund, vec![line(1, None, 1.0, 10.0, D)])).unwrap();
    assert_eq!(e.stock(1), 7.0);
    e.assert_stock();
}

#[test]
fn tc_ret_006_unlinked_return_for_known_customer_fifo_dues() {
    let e = env();
    seed_party(&e.db, 1, "C", PartyType::Customer);
    seed_stocked_item(&e.db, &e.inv, 1, 20.0, 5.0, 10.0);
    let a = e.sell(Some(1), PaymentStatus::Unpaid, 0.0, &[(1, 1.0, 15.0)]);
    let b = e.sell(Some(1), PaymentStatus::Unpaid, 0.0, &[(1, 4.0, 10.0)]);
    let res = e.rets.create_customer_return(cust(None, Some(1), SettlementType::Refund, vec![line(1, None, 3.0, 10.0, R)])).unwrap();
    assert_eq!((res.credited_amount, res.cash_refunded), (30.0, 0.0));
    assert_eq!(kinds(&e.ledger(TransactionType::Sale, a), PaymentKind::ReturnCredit), vec![15.0]);
    assert_eq!(kinds(&e.ledger(TransactionType::Sale, b), PaymentKind::ReturnCredit), vec![15.0]);
    assert_eq!((e.sale(a).balance_due, e.sale(b).balance_due), (0.0, 25.0));
    assert!(kinds(&e.rets.get_return_details(res.return_id).unwrap().ledger, PaymentKind::RefundOut).is_empty());
    e.assert_stock();
}

#[test]
fn tc_ret_007_defective_replacement_same_item() {
    let e = env();
    seed_stocked_item(&e.db, &e.inv, 1, 2.0, 8.0, 20.0); // lot 1: 2 @ 8
    {
        let mut tx = e.db.begin_transaction().unwrap();
        seed_purchase_lot(&e.inv, &mut *tx, 1001, 1, 5.0, 9.0); // lot 2: 5 @ 9
        tx.commit().unwrap();
    }
    let sale = e.sell(None, PaymentStatus::Paid, 40.0, &[(1, 2.0, 20.0)]);
    let sl = e.sale_lines(sale)[0].id;
    let mut p = cust(Some(sale), None, SettlementType::Replacement, vec![line(1, Some(sl), 1.0, 20.0, D)]);
    p.replacement_items = vec![ReplacementItemPayload { item_id: 1, quantity: 1.0 }];
    let rows_before = ledger_row_count(&e.db);
    let res = e.rets.create_customer_return(p).unwrap();
    assert_eq!((res.credited_amount, res.cash_refunded), (0.0, 0.0));
    assert_eq!(ledger_row_count(&e.db), rows_before, "no ledger rows");
    assert_eq!(e.returned_qty(sl), 1.0);
    let d = e.rets.get_return_details(res.return_id).unwrap();
    let ret = d.items.iter().find(|i| i.return_item.line_type == ReturnLineType::Returned).unwrap();
    let iss = d.items.iter().find(|i| i.return_item.line_type == ReturnLineType::ReplacementIssued).unwrap();
    assert_eq!(ret.return_item.cost_amount, 8.0, "scrap cost -> inventory loss");
    assert_eq!((iss.return_item.cost_amount, iss.return_item.unit_price, iss.return_item.total_amount), (9.0, 0.0, 0.0));
    assert_eq!(d.allocations.len(), 1);
    assert_eq!((d.allocations[0].source_type, d.allocations[0].unit_cost), (OutflowSourceType::Replacement, 9.0));
    assert_eq!(d.return_tx.settlement_type, SettlementType::Replacement);
    assert_eq!(e.stock(1), 4.0);
    e.assert_stock();
}

#[test]
fn tc_ret_008_colour_swap_different_item() {
    let e = env();
    seed_stocked_item(&e.db, &e.inv, 1, 3.0, 5.0, 10.0); // Blue
    seed_stocked_item(&e.db, &e.inv, 2, 3.0, 5.5, 10.0); // Red
    let sale = e.sell(None, PaymentStatus::Paid, 10.0, &[(1, 1.0, 10.0)]);
    let sl = e.sale_lines(sale)[0].id;
    let mut p = cust(Some(sale), None, SettlementType::Replacement, vec![line(1, Some(sl), 1.0, 10.0, R)]);
    p.replacement_items = vec![ReplacementItemPayload { item_id: 2, quantity: 1.0 }];
    let rows_before = ledger_row_count(&e.db);
    let res = e.rets.create_customer_return(p).unwrap();
    assert_eq!(ledger_row_count(&e.db), rows_before);
    assert_eq!((e.stock(1), e.stock(2)), (3.0, 2.0));
    assert_eq!(e.returned_qty(sl), 1.0);
    let d = e.rets.get_return_details(res.return_id).unwrap();
    let iss = d.items.iter().find(|i| i.return_item.line_type == ReplacementIssued()).unwrap();
    assert_eq!((iss.return_item.item_id, iss.return_item.cost_amount), (2, 5.5));
    e.assert_stock();
}

#[allow(non_snake_case)]
fn ReplacementIssued() -> ReturnLineType {
    ReturnLineType::ReplacementIssued
}

#[test]
fn tc_ret_009_vendor_return_credit_then_refund_in() {
    let e = env();
    seed_party(&e.db, 5, "V", PartyType::Vendor);
    {
        let mut tx = e.db.begin_transaction().unwrap();
        tx.items().create(&create_test_item(1, "Item 1", None, None, 0.0, 20.0)).unwrap();
        tx.commit().unwrap();
    }
    let pur = e.purchases.create_purchase(purchase_payload(Some(5), PaymentStatus::Partial, 700.0, &[(1, None, 100.0, 10.0)])).unwrap().purchase_id;
    let lot = e.db.begin_read().unwrap().purchases().list_items_by_transaction_id(pur).unwrap()[0].id;
    let res = e.rets.create_vendor_return(VendorReturnPayload {
        original_purchase_id: Some(pur),
        party_id: None,
        return_date: DAY.to_string(),
        settlement_type: SettlementType::Refund,
        reason: None,
        notes: None,
        items: vec![VendorReturnItemPayload { item_id: 1, purchase_item_id: Some(lot), quantity: 50.0 }],
    }).unwrap();
    assert_eq!((res.credited_amount, res.cash_refunded), (300.0, 200.0));
    let rows = e.ledger(TransactionType::Purchase, pur);
    assert_eq!(kinds(&rows, PaymentKind::ReturnCredit), vec![300.0]);
    assert_eq!(kinds(&rows, PaymentKind::RefundIn), vec![200.0]);
    assert_eq!(e.purchase(pur).balance_due, 0.0);
    let d = e.rets.get_return_details(res.return_id).unwrap();
    assert_eq!((d.return_tx.total_amount, d.return_tx.party_id), (500.0, Some(5)));
    assert_eq!((d.items[0].return_item.total_amount, d.items[0].return_item.unit_price), (500.0, 10.0));
    assert_eq!(d.allocations.len(), 1);
    assert_eq!(e.stock(1), 50.0);
    // cap: only 50 remain on the lot
    let over = e.rets.create_vendor_return(VendorReturnPayload {
        original_purchase_id: Some(pur), party_id: None, return_date: DAY.to_string(), settlement_type: SettlementType::Refund, reason: None, notes: None,
        items: vec![VendorReturnItemPayload { item_id: 1, purchase_item_id: Some(lot), quantity: 51.0 }],
    });
    assert!(is_validation(&over));
    e.assert_stock();
}

#[test]
fn tc_ret_010_vendor_named_lot_must_be_purchase_source() {
    let e = env();
    seed_party(&e.db, 5, "V", PartyType::Vendor);
    seed_stocked_item(&e.db, &e.inv, 1, 5.0, 10.0, 20.0);
    e.db.pool.get().unwrap().execute("UPDATE purchase_transactions SET party_id = 5 WHERE id = 1", []).unwrap();
    e.inv.create_adjustment(StockAdjustmentPayload {
        adjustment_date: DAY.to_string(), item_id: 1, quantity: 3.0, reason: AdjustmentReason::CountCorrection, unit_cost: Some(10.0), notes: None,
    }).unwrap();
    let adj_lot = e.inv.get_item_lots(1).unwrap().into_iter().find(|l| l.source == LotSource::Adjustment).unwrap().purchase_item_id;
    let vr = |lot: Option<i64>, item: i64| VendorReturnPayload {
        original_purchase_id: None, party_id: Some(5), return_date: DAY.to_string(), settlement_type: SettlementType::Refund, reason: None, notes: None,
        items: vec![VendorReturnItemPayload { item_id: item, purchase_item_id: lot, quantity: 1.0 }],
    };
    let stock = e.stock(1);
    assert!(matches!(
        e.rets.create_vendor_return(vr(Some(adj_lot), 1)),
        Err(AppError::BillLocked { code, .. }) if code == "VENDOR_LOT_NOT_ELIGIBLE"
    ));
    assert_eq!((e.stock(1), e.return_count()), (stock, 0));
    // an ADJ bill is not a returnable purchase
    let adj_bill = e.inv.get_item_lots(1).unwrap().into_iter().find(|l| l.source == LotSource::Adjustment).unwrap().purchase_transaction_id;
    let mut p = vr(None, 1);
    p.original_purchase_id = Some(adj_bill);
    assert!(is_validation(&e.rets.create_vendor_return(p)));
    // vendor replacement is not allowed
    let mut p = vr(None, 1);
    p.settlement_type = SettlementType::Replacement;
    assert!(is_validation(&e.rets.create_vendor_return(p)));
    // DD-86: no lot named is refused, never a FIFO draw.
    assert!(matches!(
        e.rets.create_vendor_return(vr(None, 1)),
        Err(AppError::BillLocked { code, .. }) if code == "VENDOR_LOT_REQUIRED"
    ));
    assert_eq!((e.stock(1), e.return_count()), (stock, 0));
    // The vendor's purchase lot is returned; nothing is open, so all cash.
    let res = e.rets.create_vendor_return(vr(Some(1), 1)).unwrap();
    assert_eq!((res.credited_amount, res.cash_refunded), (0.0, 10.0));
    e.assert_stock();
}

#[test]
fn tc_ret_011_replacement_validation() {
    let e = env();
    seed_stocked_item(&e.db, &e.inv, 1, 2.0, 5.0, 10.0);
    seed_stocked_item(&e.db, &e.inv, 2, 0.0, 5.0, 10.0);
    let sale = e.sell(None, PaymentStatus::Paid, 20.0, &[(1, 2.0, 10.0)]);
    let sl = e.sale_lines(sale)[0].id;
    let issue = |item: i64, q: f64| vec![ReplacementItemPayload { item_id: item, quantity: q }];
    let mk = |lines: Vec<CustomerReturnItemPayload>, orig: Option<i64>, iss: Vec<ReplacementItemPayload>| {
        let mut p = cust(orig, None, SettlementType::Replacement, lines);
        p.replacement_items = iss;
        p
    };
    let stock = (e.stock(1), e.stock(2));
    assert!(is_validation(&e.rets.create_customer_return(mk(vec![line(1, None, 1.0, 10.0, R)], Some(sale), issue(1, 1.0)))), "unlinked returned line");
    assert!(is_validation(&e.rets.create_customer_return(mk(vec![line(1, Some(sl), 1.0, 10.0, R)], Some(sale), vec![]))), "empty issue list");
    assert!(is_validation(&e.rets.create_customer_return(mk(vec![line(1, Some(sl), 1.0, 10.0, R)], None, issue(1, 1.0)))), "no original sale");
    assert!(is_validation(&e.rets.create_customer_return(mk(vec![line(1, Some(sl), 1.0, 10.0, R)], Some(sale), issue(1, 0.0)))), "zero issue qty");
    // REFUND must not carry replacement items
    let mut p = cust(Some(sale), None, SettlementType::Refund, vec![line(1, Some(sl), 1.0, 10.0, R)]);
    p.replacement_items = issue(1, 1.0);
    assert!(is_validation(&e.rets.create_customer_return(p)));
    // issued item out of stock: fails after the restock step, everything rolls back
    let r = e.rets.create_customer_return(mk(vec![line(1, Some(sl), 1.0, 10.0, R)], Some(sale), issue(2, 1.0)));
    assert!(matches!(r, Err(AppError::InsufficientStockError { .. })), "{r:?}");
    assert_eq!((e.stock(1), e.stock(2), e.return_count(), e.returned_qty(sl)), (stock.0, stock.1, 0, 0.0));
    // inactive issued item
    {
        let mut tx = e.db.begin_transaction().unwrap();
        let mut it = tx.items().find_by_id(2).unwrap().unwrap();
        it.is_active = false;
        tx.items().update(&it).unwrap();
        tx.commit().unwrap();
    }
    assert!(is_validation(&e.rets.create_customer_return(mk(vec![line(1, Some(sl), 1.0, 10.0, R)], Some(sale), issue(2, 1.0)))));
    e.assert_stock();
}

#[test]
fn tc_ret_012_linked_return_never_touches_other_bills() {
    let e = env();
    seed_party(&e.db, 1, "C", PartyType::Customer);
    seed_stocked_item(&e.db, &e.inv, 1, 20.0, 5.0, 10.0);
    let a = e.sell(Some(1), PaymentStatus::Partial, 0.0 + 0.01, &[(1, 5.0, 10.0)]); // SAL-A: due ~50
    let b = e.sell(Some(1), PaymentStatus::Paid, 100.0, &[(1, 10.0, 10.0)]);
    let sl = e.sale_lines(b)[0].id;
    let a_due = e.sale(a).balance_due;
    let res = e.rets.create_customer_return(cust(Some(b), None, SettlementType::Refund, vec![line(1, Some(sl), 3.0, 10.0, R)])).unwrap();
    assert_eq!((res.credited_amount, res.cash_refunded), (0.0, 30.0));
    assert!(kinds(&e.ledger(TransactionType::Sale, a), PaymentKind::ReturnCredit).is_empty());
    assert_eq!(e.sale(a).balance_due, a_due);
    assert_eq!(kinds(&e.ledger(TransactionType::Sale, b), PaymentKind::RefundOut), vec![30.0]);
    e.assert_stock();
}

#[test]
fn list_returns_is_paginated_newest_first_and_filtered() {
    let e = env();
    seed_party(&e.db, 5, "V", PartyType::Vendor);
    seed_stocked_item(&e.db, &e.inv, 1, 20.0, 5.0, 10.0);
    e.db.pool.get().unwrap().execute("UPDATE purchase_transactions SET party_id = 5 WHERE id = 1", []).unwrap();
    for (i, date) in ["2026-11-01", "2026-11-05", "2026-11-03"].iter().enumerate() {
        let mut p = cust(None, None, SettlementType::Refund, vec![line(1, None, 1.0 + i as f64, 10.0, R)]);
        p.return_date = date.to_string();
        e.rets.create_customer_return(p).unwrap();
    }
    e.rets.create_vendor_return(VendorReturnPayload {
        original_purchase_id: None, party_id: Some(5), return_date: "2026-11-04".to_string(), settlement_type: SettlementType::Refund, reason: None, notes: None,
        items: vec![VendorReturnItemPayload { item_id: 1, purchase_item_id: Some(1), quantity: 1.0 }],
    }).unwrap();
    let all = e.rets.list_returns(ReturnsFilter { page: Some(1), limit: Some(3), ..Default::default() }).unwrap();
    assert_eq!((all.total, all.items.len(), all.limit), (4, 3, 3));
    let dates: Vec<_> = all.items.iter().map(|r| r.return_date.as_str()).collect();
    assert_eq!(dates, vec!["2026-11-05", "2026-11-04", "2026-11-03"]);
    let p2 = e.rets.list_returns(ReturnsFilter { page: Some(2), limit: Some(3), ..Default::default() }).unwrap();
    assert_eq!(p2.items.len(), 1);
    let v = e.rets.list_returns(ReturnsFilter { return_type: Some(ReturnType::Vendor), ..Default::default() }).unwrap();
    assert_eq!(v.total, 1);
    let r = e.rets.list_returns(ReturnsFilter { start_date: Some("2026-11-03".into()), end_date: Some("2026-11-04".into()), ..Default::default() }).unwrap();
    assert_eq!(r.total, 2);
    assert!(matches!(e.rets.get_return_details(9999), Err(AppError::NotFound { .. })));
    e.assert_stock();
}
