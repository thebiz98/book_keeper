// tests/change_only_edit_test.rs
//! Test Suite: v1.7 H3 change-only bill edit (FR-55, DD-89, DD-90 item half; findings E1, E2, E4,
//! E5, E9). A sale edit diffs its lines by `sale_item_id` and writes only what changed; the
//! purchase edit keeps lot snapshots; charges and the UPFRONT row stay in place. Production
//! services on real SQLite; every step ends with the ledger, lot-flow and stock-cache invariants.

mod common;

use std::sync::Arc;

use book_keeper::core::dtos::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::data::contracts::FtsIndex;
use book_keeper::services::contracts::charge_type_service::ChargeTypeService;
use book_keeper::services::contracts::inventory_service::InventoryService;
use book_keeper::services::contracts::item_service::ItemService;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::return_service::ReturnService;
use book_keeper::services::contracts::sale_service::SaleService;
use common::invariants::*;
use common::ledger::*;
use common::*;

type Inv = InventoryServiceImpl<FifoStrategy>;
type Acct = AccountingServiceImpl<PaymentServiceImpl>;

use PaymentStatus::*;

struct E {
    db: TestDb,
    inv: Inv,
    items: ItemServiceImpl<Inv>,
    sales: SaleServiceImpl<Inv, PaymentServiceImpl>,
    purchases: PurchaseServiceImpl<Inv, PaymentServiceImpl>,
    rets: ReturnServiceImpl<Inv, PaymentServiceImpl, Acct>,
    types: ChargeTypeServiceImpl,
}

fn env() -> E {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let acct = AccountingServiceImpl::new(PaymentServiceImpl, db.clone());
    let index: Arc<dyn FtsIndex> = Arc::new(db.charge_type_index());
    E {
        items: ItemServiceImpl::new(db.clone(), Arc::new(db.indexer()), inv.clone()),
        sales: SaleServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        purchases: PurchaseServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        rets: ReturnServiceImpl::new(inv.clone(), PaymentServiceImpl, acct, db.clone()),
        types: ChargeTypeServiceImpl::new(db.clone(), index),
        inv,
        db,
    }
}

fn physical(name: &str, bundles: Option<Vec<CreateBundlePayload>>) -> CreateItemPayload {
    CreateItemPayload {
        item_type: None, service_cost: None, components: None, low_stock_threshold: None,
        name: name.into(), category: None, barcode: None, base_unit: "PCS".into(), mrp: None,
        default_sale_price: 10.0, initial_stock: None, initial_cost: None, bundles,
    }
}

fn comps(c: &[(i64, f64)]) -> Vec<ServiceComponentInput> {
    c.iter().map(|&(i, q)| ServiceComponentInput { component_item_id: i, quantity_per_unit: q }).collect()
}

fn update(name: &str, service: Option<(Option<f64>, &[(i64, f64)])>) -> UpdateItemPayload {
    UpdateItemPayload {
        name: name.into(), category: None, barcode: None, base_unit: "PCS".into(), mrp: None,
        default_sale_price: 10.0, item_type: None, low_stock_threshold: None,
        service_cost: service.and_then(|(c, _)| c),
        components: service.map(|(_, c)| comps(c)),
    }
}

/// One payload line: `(sale_item_id, item, bundle, qty, price)`.
type L = (Option<i64>, i64, Option<i64>, f64, f64);

impl E {
    fn item(&self, name: &str) -> i64 {
        self.items.create_item(physical(name, None)).unwrap()
    }
    /// Physical item with one "Box" bundle of 10. Returns `(item, bundle)`.
    fn boxed(&self, name: &str) -> (i64, i64) {
        let b = CreateBundlePayload {
            item_id: None, bundle_name: "Box".into(), units_per_bundle: 10.0, mrp: None, default_bundle_price: None, barcode: None,
        };
        let id = self.items.create_item(physical(name, Some(vec![b]))).unwrap();
        let bundle = self.items.get_item(id).unwrap().bundles[0].bundle_id;
        (id, bundle)
    }
    fn svc(&self, name: &str, cost: Option<f64>, c: &[(i64, f64)]) -> i64 {
        self.items
            .create_item(CreateItemPayload {
                item_type: Some(ItemType::Service),
                service_cost: cost,
                components: Some(comps(c)),
                ..physical(name, None)
            })
            .unwrap()
    }
    /// A fully paid purchase of one lot; returns `(purchase, lot)`.
    fn lot(&self, item: i64, qty: f64, cost: f64) -> (i64, i64) {
        let p = self
            .purchases
            .create_purchase(purchase_payload(None, Paid, qty * cost, &[(item, None, qty, cost)]))
            .unwrap()
            .purchase_id;
        (p, self.purchases.get_purchase_details(p).unwrap().items[0].purchase_item.id)
    }
    fn payload(&self, party: Option<i64>, st: PaymentStatus, paid: f64, lines: &[L]) -> CreateSalePayload {
        let l: Vec<_> = lines.iter().map(|&(_, i, b, q, p)| (i, b, q, p)).collect();
        let mut p = sale_payload(party, st, paid, &l);
        for (it, &(sid, ..)) in p.items.iter_mut().zip(lines) {
            it.sale_item_id = sid;
        }
        p
    }
    fn sell(&self, lines: &[L]) -> i64 {
        let total: f64 = lines.iter().map(|l| (l.3 * l.4 * 100.0).round() / 100.0).sum();
        self.sales.create_sale(self.payload(None, Paid, total, lines)).unwrap().sale_id
    }
    fn ids(&self, sale: i64) -> Vec<i64> {
        self.sales.get_sale_details(sale).unwrap().items.iter().map(|d| d.sale_item.id).collect()
    }
    fn stock(&self, item: i64) -> f64 {
        self.db.begin_read().unwrap().items().find_by_id(item).unwrap().unwrap().current_stock
    }
    fn remaining(&self, lot: i64) -> f64 {
        self.db.begin_read().unwrap().purchases().find_purchase_item_by_id(lot).unwrap().unwrap().remaining_quantity
    }
    fn q(&self, sql: &str) -> String {
        let c = self.db.pool.get().unwrap();
        c.query_row(sql, [], |r| r.get::<_, Option<String>>(0)).unwrap().unwrap_or_default()
    }
    fn f(&self, sql: &str) -> f64 {
        self.db.pool.get().unwrap().query_row(sql, [], |r| r.get::<_, Option<f64>>(0)).unwrap().unwrap_or(0.0)
    }
    /// Allocations of one sale: `id:line:lot:depleted:returned:cost`, id order.
    fn allocs(&self, sale: i64) -> String {
        self.q(&format!(
            "SELECT group_concat(x, '|') FROM (SELECT a.id||':'||a.sale_item_id||':'||a.purchase_item_id||':'||a.quantity_depleted \
             ||':'||a.returned_quantity||':'||a.unit_cost AS x FROM sale_stock_allocations a JOIN sale_items i ON i.id = a.sale_item_id \
             WHERE i.sale_transaction_id = {sale} ORDER BY a.id)"
        ))
    }
    /// Every stored column of the sale's lines.
    fn lines(&self, sale: i64) -> String {
        self.q(&format!(
            "SELECT group_concat(x, '|') FROM (SELECT id||':'||item_id||':'||item_name||':'||COALESCE(pack_label,'-')||':'||pack_quantity \
             ||':'||quantity||':'||unit_price||':'||total_amount||':'||COALESCE(service_cost_total,'-')||':'||transaction_date AS x \
             FROM sale_items WHERE sale_transaction_id = {sale} ORDER BY id)"
        ))
    }
    /// The bill's ledger rows: `id:kind:amount:date:party:mode`.
    fn ledger(&self, t: &str, bill: i64) -> String {
        self.q(&format!(
            "SELECT group_concat(x, '|') FROM (SELECT id||':'||kind||':'||amount_paid||':'||settlement_date||':'||COALESCE(party_id,'-') \
             ||':'||COALESCE(payment_mode,'-') AS x FROM payment_settlements WHERE transaction_type = '{t}' AND transaction_id = {bill} ORDER BY id)"
        ))
    }
    fn charges(&self, t: &str, bill: i64) -> String {
        self.q(&format!(
            "SELECT group_concat(x, '|') FROM (SELECT id||':'||charge_type_id||':'||charge_name||':'||effect||':'||amount AS x \
             FROM bill_charges WHERE transaction_type = '{t}' AND transaction_id = {bill} ORDER BY id)"
        ))
    }
    /// Everything a sale edit may touch, except the header.
    fn fingerprint(&self, sale: i64) -> String {
        [self.lines(sale), self.allocs(sale), self.ledger("SALE", sale), self.charges("SALE", sale)].join(" # ")
    }
    fn cogs(&self, sale: i64) -> f64 {
        self.f(&format!(
            "SELECT SUM((a.quantity_depleted - a.returned_quantity) * a.unit_cost) FROM sale_stock_allocations a \
             JOIN sale_items i ON i.id = a.sale_item_id WHERE i.sale_transaction_id = {sale}"
        ))
    }
    /// The whole database's money and stock rows (a rejected edit leaves them as they were).
    fn snap(&self) -> String {
        [
            self.q("SELECT group_concat(id||':'||total_amount||':'||paid_amount||':'||balance_due||':'||COALESCE(notes,'')) FROM sale_transactions"),
            self.q("SELECT group_concat(id||':'||remaining_quantity) FROM purchase_items"),
            self.q("SELECT group_concat(id||':'||current_stock) FROM items"),
            self.q("SELECT group_concat(id||':'||amount_paid) FROM payment_settlements"),
            self.q("SELECT group_concat(id||':'||quantity_depleted) FROM sale_stock_allocations"),
            self.q("SELECT group_concat(id||':'||quantity||':'||total_amount) FROM sale_items"),
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

fn deleted_ref<T: std::fmt::Debug>(r: Result<T, AppError>, id: i64) {
    match r {
        Err(AppError::DeletedReference { entity, id: got, .. }) => assert_eq!((entity.as_str(), got), ("ITEM", id)),
        other => panic!("expected DeletedReference(ITEM {id}), got {other:?}"),
    }
}

fn field<T: std::fmt::Debug>(r: Result<T, AppError>) -> String {
    match r {
        Err(AppError::ValidationError { field, .. }) => field,
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

// ================================================================ E1: unchanged lines

#[test]
fn e1_notes_only_edit_leaves_lines_allocations_service_cost_and_upfront_identical() {
    let e = env();
    let (a, boxb) = e.boxed("Paper");
    let c = e.item("Ink");
    let s = e.svc("Print", Some(4.0), &[(c, 2.0)]);
    e.lot(a, 50.0, 5.0);
    e.lot(c, 20.0, 1.0);
    let sale = e.sell(&[(None, a, Some(boxb), 1.0, 100.0), (None, a, None, 3.0, 12.0), (None, s, None, 2.0, 30.0)]);
    // After the sale: a newer purchase, the service re-defined, the item and its bundle renamed.
    let (newer, _) = e.lot(a, 10.0, 9.0);
    e.items.update_item(s, update("Print", Some((Some(9.0), &[(c, 5.0)])))).unwrap();
    e.items.update_item(a, update("Paper A4", None)).unwrap();
    e.db.pool.get().unwrap().execute(&format!("UPDATE item_bundles SET bundle_name = 'Carton' WHERE id = {boxb}"), []).unwrap();
    let before = e.fingerprint(sale);
    let cogs = e.cogs(sale);
    // E4: the detail names the bundle by id even after the rename; the form sends the ids back.
    let d = e.sales.get_sale_details(sale).unwrap();
    assert_eq!(d.items.iter().map(|i| i.bundle_id).collect::<Vec<_>>(), vec![Some(boxb), None, None]);
    let ids = e.ids(sale);
    let mut p = e.payload(None, Paid, 196.0, &[
        (Some(ids[0]), a, d.items[0].bundle_id, 1.0, 100.0),
        (Some(ids[1]), a, None, 3.0, 12.0),
        (Some(ids[2]), s, None, 2.0, 30.0),
    ]);
    p.notes = Some("called back".into());
    e.sales.update_sale(sale, p).unwrap();
    assert_eq!(e.fingerprint(sale), before, "lines, allocations, UPFRONT row and charges byte-identical");
    assert_eq!(e.cogs(sale), cogs);
    assert_eq!(e.sales.get_sale_details(sale).unwrap().sale.notes.as_deref(), Some("called back"));
    // The newer purchase was not drawn, so it can still be deleted.
    assert!(e.purchases.get_purchase_edit_guard(newer).unwrap().can_delete);
    e.check();
}

#[test]
fn e1_preview_of_a_notes_only_edit_is_clean_and_writes_nothing() {
    let e = env();
    let a = e.item("Paper");
    e.lot(a, 10.0, 5.0);
    let sale = e.sell(&[(None, a, None, 2.0, 10.0)]);
    let ids = e.ids(sale);
    let before = e.snap();
    let p = e.payload(None, Paid, 20.0, &[(Some(ids[0]), a, None, 2.0, 10.0)]);
    let pv = e
        .sales
        .preview_sale(Some(sale), PreviewSalePayload {
            transaction_date: p.transaction_date.clone(), party_id: None, payment_status: Some(Paid), paid_amount: Some(20.0),
            payment_mode: p.payment_mode.clone(), notes: Some("x".into()), items: p.items.clone(), charges: vec![],
            confirm_move_settlements: false,
        })
        .unwrap();
    assert!(pv.errors.is_empty(), "{:?}", pv.errors);
    assert_eq!(pv.total_amount, 20.0);
    assert_eq!(e.snap(), before);
    e.check();
}

#[test]
fn e1_rate_only_edit_writes_only_the_price_fields() {
    let e = env();
    let a = e.item("Paper");
    let b = e.item("Pen");
    e.lot(a, 10.0, 5.0);
    e.lot(b, 10.0, 2.0);
    let sale = e.sell(&[(None, a, None, 2.0, 10.0), (None, b, None, 1.0, 5.0)]);
    let ids = e.ids(sale);
    let allocs = e.allocs(sale);
    let up = e.ledger("SALE", sale);
    e.sales.update_sale(sale, e.payload(None, Paid, 29.0, &[(Some(ids[0]), a, None, 2.0, 12.0), (Some(ids[1]), b, None, 1.0, 5.0)])).unwrap();
    assert_eq!(e.allocs(sale), allocs, "no stock moved");
    let lines = e.sales.get_sale_details(sale).unwrap().items;
    assert_eq!((lines[0].sale_item.id, lines[0].sale_item.unit_price, lines[0].sale_item.total_amount), (ids[0], 12.0, 24.0));
    assert_eq!((lines[1].sale_item.id, lines[1].sale_item.total_amount), (ids[1], 5.0));
    // The UPFRONT row keeps its id; only its amount follows the new total.
    let up_id = up.split(':').next().unwrap().to_string();
    let now = e.ledger("SALE", sale);
    assert!(now.starts_with(&format!("{up_id}:UPFRONT:29")), "{now}");
    assert_eq!(e.sales.get_sale_details(sale).unwrap().sale.total_amount, 29.0);
    e.check();
}

// ================================================================ E1: quantity changes

#[test]
fn e1_raising_quantity_draws_only_the_extra_and_keeps_the_original_allocations() {
    let e = env();
    let a = e.item("Paper");
    let (_, l1) = e.lot(a, 5.0, 5.0);
    let (_, l2) = e.lot(a, 10.0, 6.0);
    let s1 = e.sell(&[(None, a, None, 5.0, 10.0)]); // all of L1
    let s2 = e.sell(&[(None, a, None, 3.0, 10.0)]); // 3 of L2
    // A restockable return of S1 puts 2 units back on the older lot L1.
    let s1_line = e.ids(s1)[0];
    e.rets
        .create_customer_return(CustomerReturnPayload {
            original_sale_id: Some(s1), party_id: None, return_date: DAY.into(), settlement_type: SettlementType::Refund,
            reason: None, notes: None, replacement_items: vec![],
            items: vec![CustomerReturnItemPayload {
                item_id: a, sale_item_id: Some(s1_line), quantity: 2.0, unit_price: 10.0,
                condition: StockCondition::Restockable, unit_cost_override: None,
            }],
        })
        .unwrap();
    assert_eq!(e.remaining(l1), 2.0);
    let before = e.allocs(s2);
    let line = e.ids(s2)[0];
    e.sales.update_sale(s2, e.payload(None, Paid, 40.0, &[(Some(line), a, None, 4.0, 10.0)])).unwrap();
    let after = e.allocs(s2);
    assert!(after.starts_with(&before), "the original allocation stays: {before} -> {after}");
    assert_eq!(e.cogs(s2), 23.0, "3 x 6 kept from L2 + 1 x 5 extra from the oldest open lot (a re-draw would give 22)");
    assert_eq!((e.remaining(l1), e.remaining(l2)), (1.0, 7.0));
    assert_eq!(e.ids(s2), vec![line]);
    e.check();
}

#[test]
fn e1_lowering_quantity_releases_the_newest_allocation_first() {
    let e = env();
    let a = e.item("Paper");
    let (_, l1) = e.lot(a, 3.0, 5.0);
    let (_, l2) = e.lot(a, 10.0, 6.0);
    let sale = e.sell(&[(None, a, None, 5.0, 10.0)]); // L1 3, L2 2
    let line = e.ids(sale)[0];
    let ids: Vec<i64> = e.sales.get_sale_details(sale).unwrap().items[0].allocations.iter().map(|x| x.id).collect();
    e.sales.update_sale(sale, e.payload(None, Paid, 40.0, &[(Some(line), a, None, 4.0, 10.0)])).unwrap();
    assert_eq!(e.allocs(sale), format!("{}:{line}:{l1}:3.0:0.0:5.0|{}:{line}:{l2}:1.0:0.0:6.0", ids[0], ids[1]));
    assert_eq!((e.remaining(l1), e.remaining(l2), e.stock(a)), (0.0, 9.0, 9.0));
    e.check();
    e.sales.update_sale(sale, e.payload(None, Paid, 20.0, &[(Some(line), a, None, 2.0, 10.0)])).unwrap();
    assert_eq!(e.allocs(sale), format!("{}:{line}:{l1}:2.0:0.0:5.0", ids[0]), "the newest row goes, then the older shrinks");
    assert_eq!((e.remaining(l1), e.remaining(l2), e.cogs(sale)), (1.0, 10.0, 10.0));
    e.check();
}

#[test]
fn e1_changing_the_item_keeps_the_line_id_and_draws_the_new_item_fresh() {
    let e = env();
    let a = e.item("Paper");
    let b = e.item("Pen");
    e.lot(a, 10.0, 5.0);
    e.lot(b, 10.0, 2.0);
    let sale = e.sell(&[(None, a, None, 3.0, 10.0)]);
    let line = e.ids(sale)[0];
    e.sales.update_sale(sale, e.payload(None, Paid, 20.0, &[(Some(line), b, None, 2.0, 10.0)])).unwrap();
    let l = &e.sales.get_sale_details(sale).unwrap().items[0];
    assert_eq!((l.sale_item.id, l.sale_item.item_id, l.item_name.as_str(), l.sale_item.quantity), (line, b, "Pen", 2.0));
    assert_eq!((e.stock(a), e.stock(b), e.cogs(sale)), (10.0, 8.0, 4.0));
    e.check();
}

#[test]
fn e1_removed_and_new_lines_release_and_draw_while_the_other_lines_stay() {
    let e = env();
    let a = e.item("Paper");
    let b = e.item("Pen");
    let c = e.item("Clip");
    e.lot(a, 10.0, 5.0);
    e.lot(b, 10.0, 2.0);
    e.lot(c, 10.0, 1.0);
    let sale = e.sell(&[(None, a, None, 3.0, 10.0), (None, b, None, 2.0, 5.0)]);
    let ids = e.ids(sale);
    let kept = e.lines(sale).split('|').next().unwrap().to_string();
    let kept_alloc = e.allocs(sale).split('|').next().unwrap().to_string();
    e.sales.update_sale(sale, e.payload(None, Paid, 34.0, &[(Some(ids[0]), a, None, 3.0, 10.0), (None, c, None, 4.0, 1.0)])).unwrap();
    assert!(e.lines(sale).starts_with(&kept), "the kept line is untouched");
    assert!(e.allocs(sale).starts_with(&kept_alloc));
    let now = e.ids(sale);
    assert_eq!(now.len(), 2);
    assert_eq!(now[0], ids[0]);
    assert!(!now.contains(&ids[1]), "the removed line is gone");
    assert_eq!((e.stock(a), e.stock(b), e.stock(c)), (7.0, 10.0, 6.0));
    e.check();
}

#[test]
fn e1_header_only_date_change_touches_no_line_and_moves_the_upfront_row_in_place() {
    let e = env();
    let a = e.item("Paper");
    e.lot(a, 10.0, 5.0);
    let sale = e.sell(&[(None, a, None, 2.0, 10.0)]);
    let ids = e.ids(sale);
    let allocs = e.allocs(sale);
    let up_id: String = e.ledger("SALE", sale).split(':').next().unwrap().to_string();
    let mut p = e.payload(None, Paid, 20.0, &[(Some(ids[0]), a, None, 2.0, 10.0)]);
    p.transaction_date = "2026-11-19".into();
    e.sales.update_sale(sale, p).unwrap();
    assert_eq!(e.allocs(sale), allocs);
    assert!(e.lines(sale).ends_with(":2026-11-19"), "only the line date follows the header (§15)");
    assert_eq!(e.ledger("SALE", sale), format!("{up_id}:UPFRONT:20.0:2026-11-19:-:CASH"));
    e.check();
}

#[test]
fn e1_upfront_row_is_updated_in_place_deleted_at_zero_and_created_when_none() {
    let e = env();
    seed_party(&e.db, 1, "A", PartyType::Customer);
    let a = e.item("Paper");
    e.lot(a, 10.0, 5.0);
    let sale = e.sales.create_sale(e.payload(Some(1), Paid, 100.0, &[(None, a, None, 1.0, 100.0)])).unwrap().sale_id;
    let line = e.ids(sale)[0];
    let up_id: String = e.ledger("SALE", sale).split(':').next().unwrap().to_string();
    let r = e.sales.update_sale(sale, e.payload(Some(1), Paid, 80.0, &[(Some(line), a, None, 1.0, 80.0)])).unwrap();
    assert_eq!((r.paid_amount, r.balance_due), (80.0, 0.0));
    assert_eq!(e.ledger("SALE", sale), format!("{up_id}:UPFRONT:80.0:{DAY}:1:CASH"));
    e.check();
    e.sales.update_sale(sale, e.payload(Some(1), Partial, 30.0, &[(Some(line), a, None, 1.0, 80.0)])).unwrap();
    assert_eq!(e.ledger("SALE", sale), format!("{up_id}:UPFRONT:30.0:{DAY}:1:CASH"));
    e.sales.update_sale(sale, e.payload(Some(1), Unpaid, 0.0, &[(Some(line), a, None, 1.0, 80.0)])).unwrap();
    assert_eq!(e.ledger("SALE", sale), "", "deleted at 0");
    e.check();
    let r = e.sales.update_sale(sale, e.payload(Some(1), Paid, 100.0, &[(Some(line), a, None, 1.0, 80.0)])).unwrap();
    assert_eq!((r.paid_amount, r.change_amount), (80.0, 20.0));
    assert_eq!(e.ledger("SALE", sale).matches("UPFRONT").count(), 1, "created when none");
    e.check();
}

#[test]
fn e1_stock_short_on_the_extra_fails_and_writes_nothing() {
    let e = env();
    let a = e.item("Paper");
    e.lot(a, 5.0, 5.0);
    let sale = e.sell(&[(None, a, None, 4.0, 10.0)]);
    let line = e.ids(sale)[0];
    let before = e.snap();
    let r = e.sales.update_sale(sale, e.payload(None, Paid, 60.0, &[(Some(line), a, None, 6.0, 10.0)]));
    match r {
        Err(AppError::InsufficientStockError { required, available, .. }) => assert_eq!((required, available), (2.0, 1.0)),
        other => panic!("expected InsufficientStockError, got {other:?}"),
    }
    assert_eq!(e.snap(), before);
    // The line's own units are enough for 5 (4 kept + the 1 left).
    e.sales.update_sale(sale, e.payload(None, Paid, 50.0, &[(Some(line), a, None, 5.0, 10.0)])).unwrap();
    assert_eq!(e.stock(a), 0.0);
    e.check();
}

#[test]
fn e1_line_ids_must_belong_to_the_sale_and_appear_once_and_create_rejects_them() {
    let e = env();
    let a = e.item("Paper");
    e.lot(a, 10.0, 5.0);
    let s1 = e.sell(&[(None, a, None, 1.0, 10.0)]);
    let s2 = e.sell(&[(None, a, None, 1.0, 10.0)]);
    let (l1, l2) = (e.ids(s1)[0], e.ids(s2)[0]);
    let before = e.snap();
    assert_eq!(field(e.sales.update_sale(s1, e.payload(None, Paid, 10.0, &[(Some(l2), a, None, 1.0, 10.0)]))), "sale_item_id");
    assert_eq!(
        field(e.sales.update_sale(s1, e.payload(None, Paid, 20.0, &[(Some(l1), a, None, 1.0, 10.0), (Some(l1), a, None, 1.0, 10.0)]))),
        "sale_item_id"
    );
    assert_eq!(field(e.sales.create_sale(e.payload(None, Paid, 10.0, &[(Some(l1), a, None, 1.0, 10.0)]))), "sale_item_id");
    assert_eq!(e.snap(), before);
    e.check();
}

// ================================================================ E1: service lines

#[test]
fn e1_service_quantity_change_scales_the_stored_cost_and_component_ratio() {
    let e = env();
    let c = e.item("Ink");
    let other = e.item("Toner");
    e.lot(c, 20.0, 1.0);
    e.lot(other, 20.0, 1.0);
    let s = e.svc("Print", Some(10.0), &[(c, 2.0)]);
    let sale = e.sell(&[(None, s, None, 2.0, 30.0)]);
    let line = e.ids(sale)[0];
    assert_eq!(e.stock(c), 16.0);
    // The live definition changes after the sale: cost 15, 5 Toner per unit instead of 2 Ink.
    e.items.update_item(s, update("Print", Some((Some(15.0), &[(other, 5.0)])))).unwrap();
    e.sales.update_sale(sale, e.payload(None, Paid, 90.0, &[(Some(line), s, None, 3.0, 30.0)])).unwrap();
    let l = &e.sales.get_sale_details(sale).unwrap().items[0];
    assert_eq!((l.sale_item.quantity, l.sale_item.service_cost_total), (3.0, Some(30.0)), "stored 20 for 2, scaled");
    assert_eq!((e.stock(c), e.stock(other)), (14.0, 20.0), "the stored ratio (2 Ink) draws the extra unit");
    e.check();
    e.sales.update_sale(sale, e.payload(None, Paid, 30.0, &[(Some(line), s, None, 1.0, 30.0)])).unwrap();
    let l = &e.sales.get_sale_details(sale).unwrap().items[0];
    assert_eq!((l.sale_item.quantity, l.sale_item.service_cost_total), (1.0, Some(10.0)));
    assert_eq!(e.stock(c), 18.0);
    e.check();
}

// ================================================================ E5 / DD-90: deleted items

#[test]
fn e5_deleted_item_line_may_stay_unchanged_but_never_change() {
    let e = env();
    let a = e.item("Paper");
    let b = e.item("Pen");
    e.lot(a, 10.0, 5.0);
    e.lot(b, 10.0, 2.0);
    let sale = e.sell(&[(None, a, None, 2.0, 10.0), (None, b, None, 1.0, 5.0)]);
    let ids = e.ids(sale);
    e.items.delete_item(a, false).unwrap();
    let before = e.snap();
    // Unchanged line of the deleted item, the other line changed: allowed.
    e.sales.update_sale(sale, e.payload(None, Paid, 30.0, &[(Some(ids[0]), a, None, 2.0, 10.0), (Some(ids[1]), b, None, 2.0, 5.0)])).unwrap();
    e.check();
    let before2 = e.snap();
    assert_ne!(before, before2);
    deleted_ref(e.sales.update_sale(sale, e.payload(None, Paid, 40.0, &[(Some(ids[0]), a, None, 3.0, 10.0), (Some(ids[1]), b, None, 2.0, 5.0)])), a);
    deleted_ref(e.sales.update_sale(sale, e.payload(None, Paid, 32.0, &[(Some(ids[0]), a, None, 2.0, 11.0), (Some(ids[1]), b, None, 2.0, 5.0)])), a);
    deleted_ref(e.sales.update_sale(sale, e.payload(None, Paid, 10.0, &[(Some(ids[1]), b, None, 2.0, 5.0)])), a);
    deleted_ref(
        e.sales.update_sale(sale, e.payload(None, Paid, 40.0, &[(Some(ids[0]), a, None, 2.0, 10.0), (Some(ids[1]), b, None, 2.0, 5.0), (None, a, None, 1.0, 10.0)])),
        a,
    );
    assert_eq!(e.snap(), before2, "nothing written");
    // After a restore the same change goes through.
    e.items.restore_item(a).unwrap();
    e.sales.update_sale(sale, e.payload(None, Paid, 40.0, &[(Some(ids[0]), a, None, 3.0, 10.0), (Some(ids[1]), b, None, 2.0, 5.0)])).unwrap();
    e.check();
}

#[test]
fn e5_kept_service_line_cannot_draw_a_deleted_component_or_change_once_deleted() {
    let e = env();
    let c = e.item("Ink");
    e.lot(c, 20.0, 1.0);
    let s = e.svc("Print", None, &[(c, 2.0)]);
    let sale = e.sell(&[(None, s, None, 1.0, 30.0)]);
    let line = e.ids(sale)[0];
    // The component leaves the service and is deleted; its hidden stock stays.
    e.items.update_item(s, update("Print", Some((None, &[])))).unwrap();
    e.items.delete_item(c, false).unwrap();
    let mut notes = e.payload(None, Paid, 30.0, &[(Some(line), s, None, 1.0, 30.0)]);
    notes.notes = Some("ok".into());
    e.sales.update_sale(sale, notes).unwrap();
    let before = e.snap();
    deleted_ref(e.sales.update_sale(sale, e.payload(None, Paid, 60.0, &[(Some(line), s, None, 2.0, 30.0)])), c);
    assert_eq!(e.snap(), before, "the deleted component's hidden stock is not consumed");
    // A deleted service line may not change either.
    e.items.restore_item(c).unwrap();
    e.items.delete_item(s, false).unwrap();
    deleted_ref(e.sales.update_sale(sale, e.payload(None, Paid, 35.0, &[(Some(line), s, None, 1.0, 35.0)])), s);
    e.check();
}

// ================================================================ E9: charges kept

#[test]
fn e9_renamed_charge_type_keeps_the_stored_charge_row_unless_it_changed() {
    let e = env();
    let a = e.item("Paper");
    e.lot(a, 10.0, 5.0);
    let t = e
        .types
        .create_charge_type(CreateChargeTypePayload {
            name: "Transport".into(), effect: ChargeEffect::Charge, applies_to: ChargeAppliesTo::Both, default_amount: None,
        })
        .unwrap()
        .id;
    let mut p = e.payload(None, Paid, 120.0, &[(None, a, None, 1.0, 100.0)]);
    p.charges = vec![BillChargeInput { charge_type_id: t, amount: 20.0 }];
    let sale = e.sales.create_sale(p).unwrap().sale_id;
    let line = e.ids(sale)[0];
    let stored = e.charges("SALE", sale);
    // The type is renamed and flipped to a discount afterwards.
    e.db.pool.get().unwrap().execute(&format!("UPDATE charge_types SET name = 'Freight', effect = 'DISCOUNT' WHERE id = {t}"), []).unwrap();
    let mut p = e.payload(None, Paid, 120.0, &[(Some(line), a, None, 1.0, 100.0)]);
    p.charges = vec![BillChargeInput { charge_type_id: t, amount: 20.0 }];
    p.notes = Some("n".into());
    e.sales.update_sale(sale, p.clone()).unwrap();
    assert_eq!(e.charges("SALE", sale), stored, "kept row: id, name and effect unchanged");
    assert_eq!(e.sales.get_sale_details(sale).unwrap().sale.total_amount, 120.0);
    e.check();
    // A changed amount re-resolves that row from the live type.
    p.charges[0].amount = 25.0;
    p.paid_amount = 75.0;
    e.sales.update_sale(sale, p).unwrap();
    let now = e.charges("SALE", sale);
    assert!(now.ends_with(&format!(":{t}:Freight:DISCOUNT:25.0")), "{now}");
    assert_eq!(e.sales.get_sale_details(sale).unwrap().sale.total_amount, 75.0);
    e.check();
}

#[test]
fn e9_purchase_edit_keeps_stored_charge_rows_too() {
    let e = env();
    let a = e.item("Paper");
    let t = e
        .types
        .create_charge_type(CreateChargeTypePayload {
            name: "Transport".into(), effect: ChargeEffect::Charge, applies_to: ChargeAppliesTo::Both, default_amount: None,
        })
        .unwrap()
        .id;
    let mut p = purchase_payload(None, Paid, 60.0, &[(a, None, 10.0, 5.0)]);
    p.charges = vec![BillChargeInput { charge_type_id: t, amount: 10.0 }];
    let bill = e.purchases.create_purchase(p.clone()).unwrap().purchase_id;
    let lot = e.purchases.get_purchase_details(bill).unwrap().items[0].purchase_item.id;
    let stored = e.charges("PURCHASE", bill);
    let up = e.ledger("PURCHASE", bill);
    e.db.pool.get().unwrap().execute(&format!("UPDATE charge_types SET name = 'Freight' WHERE id = {t}"), []).unwrap();
    p.items[0].purchase_item_id = Some(lot);
    p.notes = Some("n".into());
    e.purchases.update_purchase(bill, p).unwrap();
    assert_eq!(e.charges("PURCHASE", bill), stored);
    assert_eq!(e.ledger("PURCHASE", bill), up, "UPFRONT row untouched");
    e.check();
}

// ================================================================ E2 / E4: purchase snapshots

#[test]
fn e2_purchase_notes_edit_after_a_rename_keeps_the_lot_snapshots() {
    let e = env();
    let (a, boxb) = e.boxed("Paper");
    let bill = e
        .purchases
        .create_purchase(purchase_payload(None, Paid, 100.0, &[(a, Some(boxb), 2.0, 50.0)]))
        .unwrap()
        .purchase_id;
    e.items.update_item(a, update("Paper A4", None)).unwrap();
    e.db.pool.get().unwrap().execute(&format!("UPDATE item_bundles SET bundle_name = 'Carton' WHERE id = {boxb}"), []).unwrap();
    let d = e.purchases.get_purchase_details(bill).unwrap();
    let lot = d.items[0].purchase_item.clone();
    assert_eq!((lot.item_name.as_str(), lot.pack_label.as_deref(), d.items[0].bundle_id), ("Paper", Some("Box"), Some(boxb)));
    let mut p = purchase_payload(None, Paid, 100.0, &[(a, d.items[0].bundle_id, 2.0, 50.0)]);
    p.items[0].purchase_item_id = Some(lot.id);
    p.notes = Some("n".into());
    e.purchases.update_purchase(bill, p.clone()).unwrap();
    let now = e.purchases.get_purchase_details(bill).unwrap().items[0].purchase_item.clone();
    assert_eq!(now, lot, "lot row unchanged");
    // A cost change rewrites the price fields only; the snapshots stay.
    p.items[0].unit_cost = 60.0;
    p.paid_amount = 120.0;
    e.purchases.update_purchase(bill, p).unwrap();
    let now = e.purchases.get_purchase_details(bill).unwrap().items[0].purchase_item.clone();
    assert_eq!((now.item_name.as_str(), now.pack_label.as_deref(), now.unit_cost), ("Paper", Some("Box"), 6.0));
    e.check();
}
