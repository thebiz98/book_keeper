// tests/service_item_test.rs
//! Test Suite: service items with components, selling services, misc. cost (FR-36, FR-37,
//! DD-65..DD-67, schema v12). Production services on real SQLite.

mod common;

use std::sync::Arc;

use common::ledger::*;
use common::stock::assert_stock_invariant;
use common::*;
use book_keeper::core::dtos::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::{AdjustmentReason, ItemType, PaymentStatus};
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::inventory_service::InventoryService;
use book_keeper::services::contracts::item_service::ItemService;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::report_service::ReportService;
use book_keeper::services::contracts::return_service::ReturnService;
use book_keeper::services::contracts::sale_service::SaleService;

type Inv = InventoryServiceImpl<FifoStrategy>;
type Acct = AccountingServiceImpl<PaymentServiceImpl>;

struct E {
    db: TestDb,
    inv: Inv,
    items: ItemServiceImpl<Inv>,
    sales: SaleServiceImpl<Inv, PaymentServiceImpl>,
    purchases: PurchaseServiceImpl<Inv, PaymentServiceImpl>,
    rets: ReturnServiceImpl<Inv, PaymentServiceImpl, Acct>,
    report: ReportServiceImpl,
}

fn env() -> E {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let acct = AccountingServiceImpl::new(PaymentServiceImpl, db.clone());
    E {
        items: ItemServiceImpl::new(db.clone(), Arc::new(db.indexer()), inv.clone()),
        sales: SaleServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        purchases: PurchaseServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        rets: ReturnServiceImpl::new(inv.clone(), PaymentServiceImpl, acct, db.clone()),
        report: ReportServiceImpl::with_fixed_today(db.clone(), DAY),
        inv,
        db,
    }
}

fn physical(name: &str) -> CreateItemPayload {
    CreateItemPayload {
        item_type: None, service_cost: None, components: None, low_stock_threshold: None,
        name: name.into(), category: None, barcode: None, base_unit: "SHEET".into(), mrp: None,
        default_sale_price: 10.0, initial_stock: None, initial_cost: None, bundles: None,
    }
}

fn comp(id: i64, qty: f64) -> ServiceComponentInput {
    ServiceComponentInput { component_item_id: id, quantity_per_unit: qty }
}

fn service(name: &str, cost: Option<f64>, comps: &[(i64, f64)]) -> CreateItemPayload {
    CreateItemPayload {
        item_type: Some(ItemType::Service),
        service_cost: cost,
        components: Some(comps.iter().map(|&(i, q)| comp(i, q)).collect()),
        base_unit: "PRINT".into(),
        default_sale_price: 12.0,
        ..physical(name)
    }
}

fn field_of<T: std::fmt::Debug>(r: Result<T, AppError>) -> String {
    match r {
        Err(AppError::ValidationError { field, .. }) => field,
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

fn msg_of<T: std::fmt::Debug>(r: Result<T, AppError>) -> String {
    match r {
        Err(AppError::ValidationError { message, .. }) => message,
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

impl E {
    fn item(&self, name: &str) -> i64 {
        self.items.create_item(physical(name)).unwrap()
    }
    fn svc(&self, name: &str, cost: Option<f64>, comps: &[(i64, f64)]) -> i64 {
        self.items.create_item(service(name, cost, comps)).unwrap()
    }
    /// One purchase bill with one lot (a PURCHASE-source lot, so FR-25 re-costing applies).
    fn lot(&self, item: i64, qty: f64, cost: f64) {
        self.purchases
            .create_purchase(purchase_payload(None, PaymentStatus::Paid, qty * cost, &[(item, None, qty, cost)]))
            .unwrap();
    }
    fn sell(&self, lines: &[(i64, f64, f64)]) -> Result<i64, AppError> {
        let total: f64 = lines.iter().map(|&(_, q, p)| q * p).sum();
        let l: Vec<(i64, Option<i64>, f64, f64)> = lines.iter().map(|&(i, q, p)| (i, None, q, p)).collect();
        self.sales.create_sale(sale_payload(None, PaymentStatus::Paid, total, &l)).map(|r| r.sale_id)
    }
    fn stock(&self, id: i64) -> f64 {
        self.db.begin_read().unwrap().items().find_by_id(id).unwrap().unwrap().current_stock
    }
    fn detail(&self, sale: i64) -> SaleDetailView {
        self.sales.get_sale_details(sale).unwrap()
    }
    fn remaining(&self, lot: i64) -> f64 {
        let mut tx = self.db.begin_read().unwrap();
        tx.purchases().find_purchase_item_by_id(lot).unwrap().unwrap().remaining_quantity
    }
    fn invariant(&self) {
        assert_stock_invariant(&mut *self.db.begin_read().unwrap());
    }
}

// ------------------------------------ create / update ------------------------------------

#[test]
fn service_create_stores_components_cost_and_has_no_stock() {
    let e = env();
    let paper = e.item("Paper");
    let id = e.svc("Print", Some(2.456), &[(paper, 1.0)]);
    let mut tx = e.db.begin_read().unwrap();
    let it = tx.items().find_by_id(id).unwrap().unwrap();
    assert_eq!((it.item_type, it.current_stock, it.is_low_stock, it.service_cost), (ItemType::Service, 0.0, false, Some(2.46)));
    assert_eq!(tx.service_components().list_for_service(id).unwrap().len(), 1);
    assert_eq!(tx.service_components().services_using(paper).unwrap(), vec![id]);
    drop(tx);
    let v = e.items.get_service_components(id).unwrap();
    assert_eq!((v[0].name.as_str(), v[0].base_unit.as_str(), v[0].quantity_per_unit), ("Paper", "SHEET", 1.0));
}

#[test]
fn service_create_validations() {
    let e = env();
    let paper = e.item("Paper");
    let svc0 = e.svc("Lamination", None, &[]);
    assert!(e.items.get_service_components(svc0).unwrap().is_empty());
    assert_eq!(field_of(e.items.create_item(service("S1", Some(-1.0), &[]))), "service_cost");
    assert_eq!(field_of(e.items.create_item(service("S2", Some(f64::NAN), &[]))), "service_cost");
    assert_eq!(field_of(e.items.create_item(service("S3", None, &[(paper, 0.0)]))), "components");
    assert_eq!(field_of(e.items.create_item(service("S4", None, &[(paper, f64::INFINITY)]))), "components");
    assert_eq!(field_of(e.items.create_item(service("S5", None, &[(paper, 1.0), (paper, 2.0)]))), "components");
    assert_eq!(field_of(e.items.create_item(service("S6", None, &[(svc0, 1.0)]))), "components", "a service cannot be a component");
    assert!(matches!(e.items.create_item(service("S7", None, &[(999, 1.0)])), Err(AppError::NotFound { .. })));
    let mut p = service("S8", None, &[]);
    p.initial_stock = Some(5.0);
    assert_eq!(field_of(e.items.create_item(p)), "initial_stock");
    let mut p = service("S9", None, &[]);
    p.bundles = Some(vec![CreateBundlePayload {
        item_id: None, bundle_name: "Box".into(), units_per_bundle: 10.0, mrp: None, default_bundle_price: None, barcode: None,
    }]);
    assert_eq!(field_of(e.items.create_item(p)), "bundles");
    // Inactive physical item cannot be a component.
    e.db.pool.get().unwrap().execute("UPDATE items SET is_active = 0 WHERE id = ?1", [paper]).unwrap();
    assert_eq!(field_of(e.items.create_item(service("S10", None, &[(paper, 1.0)]))), "components");
    // Nothing was written by the failed attempts.
    let n: i64 = e.db.pool.get().unwrap().query_row("SELECT COUNT(*) FROM items WHERE name LIKE 'S%' AND name <> 'Svc'", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0);
}

#[test]
fn physical_items_reject_service_fields() {
    let e = env();
    let paper = e.item("Paper");
    let mut p = physical("Pen");
    p.service_cost = Some(1.0);
    assert_eq!(field_of(e.items.create_item(p)), "service_cost");
    let mut p = physical("Pen");
    p.components = Some(vec![comp(paper, 1.0)]);
    assert_eq!(field_of(e.items.create_item(p)), "components");
    let mut u = UpdateItemPayload {
        item_type: None, service_cost: Some(1.0), components: None, low_stock_threshold: None, name: "Paper".into(),
        category: None, barcode: None, base_unit: "SHEET".into(), mrp: None, default_sale_price: 10.0,
    };
    assert_eq!(field_of(e.items.update_item(paper, u.clone())), "service_cost");
    u.service_cost = None;
    u.components = Some(vec![comp(paper, 1.0)]);
    assert_eq!(field_of(e.items.update_item(paper, u)), "components");
}

#[test]
fn item_type_cannot_change_and_components_are_replaced_or_kept() {
    let e = env();
    let paper = e.item("Paper");
    let ink = e.item("Ink");
    let id = e.svc("Print", Some(2.0), &[(paper, 1.0)]);
    let mut u = UpdateItemPayload {
        item_type: Some(ItemType::Physical), service_cost: Some(2.0), components: None, low_stock_threshold: None,
        name: "Print".into(), category: None, barcode: None, base_unit: "PRINT".into(), mrp: None, default_sale_price: 12.0,
    };
    assert_eq!(msg_of(e.items.update_item(id, u.clone())), "Item type cannot be changed");
    // A physical item cannot become a service either.
    let mut pu = u.clone();
    pu.item_type = Some(ItemType::Service);
    pu.service_cost = None;
    assert_eq!(msg_of(e.items.update_item(paper, pu)), "Item type cannot be changed");
    // Same type + None components keeps the list; Some replaces it; service_cost None clears.
    u.item_type = Some(ItemType::Service);
    u.service_cost = None;
    e.items.update_item(id, u.clone()).unwrap();
    assert_eq!(e.items.get_service_components(id).unwrap().len(), 1);
    assert_eq!(e.db.begin_read().unwrap().items().find_by_id(id).unwrap().unwrap().service_cost, None);
    u.components = Some(vec![comp(ink, 0.5), comp(paper, 2.0)]);
    u.service_cost = Some(3.0);
    e.items.update_item(id, u.clone()).unwrap();
    let v = e.items.get_service_components(id).unwrap();
    assert_eq!(v.iter().map(|c| (c.component_item_id, c.quantity_per_unit)).collect::<Vec<_>>(), vec![(ink, 0.5), (paper, 2.0)]);
    u.components = Some(vec![comp(id, 1.0)]);
    assert_eq!(field_of(e.items.update_item(id, u.clone())), "components", "self reference");
    u.components = Some(vec![]);
    e.items.update_item(id, u).unwrap();
    assert!(e.items.get_service_components(id).unwrap().is_empty());
}

#[test]
fn can_make_comes_from_component_stock_and_shows_in_catalog_and_search() {
    let e = env();
    let paper = e.item("Paper");
    let ink = e.item("Ink");
    let print = e.svc("Photo print", None, &[(paper, 1.0), (ink, 0.25)]);
    let bare = e.svc("Lamination", Some(5.0), &[]);
    e.lot(paper, 10.0, 2.0);
    e.lot(ink, 1.0, 40.0);
    let page = e.items.get_catalog(CatalogFilter::default()).unwrap();
    let row = |name: &str| page.items.iter().find(|r| r.name == name).unwrap().clone();
    assert_eq!(row("Photo print").can_make, Some(4), "ink limits: floor(1 / 0.25)");
    assert_eq!(row("Photo print").item_type, ItemType::Service);
    assert_eq!(row("Lamination").can_make, None, "no components");
    assert_eq!(row("Lamination").service_cost, Some(5.0));
    assert_eq!(row("Paper").can_make, None);
    let hit = e.items.search_items_fts("Photo", None).unwrap();
    assert_eq!(hit[0].can_make, Some(4));
    // Float division guard: 0.3 / 0.1 must be 3.
    let a = e.item("A");
    e.lot(a, 0.3, 1.0);
    let s = e.svc("Tenth", None, &[(a, 0.1)]);
    let n = e.db.begin_read().unwrap().service_components().can_make(&[s, bare]).unwrap();
    assert_eq!(n.get(&s), Some(&3));
    assert_eq!(n.get(&bare), None);
    let _ = print;
}

#[test]
fn preview_service_cost_uses_fifo_and_the_misc_floor_and_writes_nothing() {
    let e = env();
    let paper = e.item("Paper");
    e.lot(paper, 2.0, 2.0);
    e.lot(paper, 10.0, 4.0);
    let snap = |e: &E| -> (i64, i64) {
        let c = e.db.pool.get().unwrap();
        (
            c.query_row("SELECT COUNT(*) FROM service_components", [], |r| r.get(0)).unwrap(),
            c.query_row("SELECT COUNT(*) FROM items", [], |r| r.get(0)).unwrap(),
        )
    };
    let before = snap(&e);
    // One unit = 3 sheets: 2 @ 2 + 1 @ 4 = 8.
    let p = e.inv.preview_service_cost(PreviewServiceCostPayload { components: vec![comp(paper, 3.0)], service_cost: Some(10.0) }).unwrap();
    assert_eq!((p.components_cost, p.misc_cost, p.unit_cost), (8.0, 2.0, 10.0));
    let p = e.inv.preview_service_cost(PreviewServiceCostPayload { components: vec![comp(paper, 3.0)], service_cost: Some(5.0) }).unwrap();
    assert_eq!((p.components_cost, p.misc_cost, p.unit_cost), (8.0, 0.0, 8.0));
    let p = e.inv.preview_service_cost(PreviewServiceCostPayload { components: vec![comp(paper, 3.0)], service_cost: None }).unwrap();
    assert_eq!((p.misc_cost, p.unit_cost), (0.0, 8.0));
    let p = e.inv.preview_service_cost(PreviewServiceCostPayload { components: vec![], service_cost: Some(4.0) }).unwrap();
    assert_eq!((p.components_cost, p.misc_cost, p.unit_cost), (0.0, 4.0, 4.0));
    assert_eq!(field_of(e.inv.preview_service_cost(PreviewServiceCostPayload { components: vec![comp(paper, -1.0)], service_cost: None })), "components");
    assert_eq!(before, snap(&e));
}

// ------------------------------------ X5 guards ------------------------------------

#[test]
fn services_are_never_stocked_each_path_is_rejected() {
    let e = env();
    let paper = e.item("Paper");
    e.lot(paper, 10.0, 2.0);
    let s = e.svc("Print", Some(2.0), &[(paper, 1.0)]);
    // purchase line
    assert_eq!(
        field_of(e.purchases.create_purchase(purchase_payload(None, PaymentStatus::Paid, 10.0, &[(s, None, 1.0, 10.0)]))),
        "item_id"
    );
    // bundle
    assert_eq!(
        field_of(e.items.create_bundle(CreateBundlePayload {
            item_id: Some(s), bundle_name: "Box".into(), units_per_bundle: 10.0, mrp: None, default_bundle_price: None, barcode: None,
        })),
        "item_id"
    );
    // stock adjustment (positive and negative) and set stock
    for (q, reason) in [(5.0, AdjustmentReason::UnrecordedStock), (-1.0, AdjustmentReason::Damaged)] {
        assert_eq!(
            field_of(e.inv.create_adjustment(StockAdjustmentPayload {
                adjustment_date: DAY.into(), item_id: s, quantity: q, reason, unit_cost: Some(1.0), notes: None,
            })),
            "item_id"
        );
    }
    assert_eq!(
        field_of(e.inv.set_item_stock(SetItemStockPayload { item_id: s, counted_quantity: 3.0, unit_cost: Some(1.0), adjustment_date: DAY.into(), notes: None })),
        "item_id"
    );
    // opening stock / unrecorded stock lots, and lot-less outflows
    let mut tx = e.db.begin_transaction().unwrap();
    assert!(e.inv.create_source_lot(&mut *tx, book_keeper::core::value_objects::LotSource::Opening, DAY, s, 2.0, Some(1.0), None).is_err());
    assert!(e.inv.create_source_lot(&mut *tx, book_keeper::core::value_objects::LotSource::Adjustment, DAY, s, 2.0, Some(1.0), None).is_err());
    assert!(e.inv.issue_stock_outflow(&mut *tx, book_keeper::core::value_objects::OutflowSourceType::Adjustment, 1, s, 1.0).is_err());
    drop(tx);
    // vendor return line
    let vr = VendorReturnPayload {
        original_purchase_id: None, party_id: None, return_date: DAY.into(),
        settlement_type: book_keeper::core::value_objects::SettlementType::Refund, reason: None, notes: None,
        items: vec![VendorReturnItemPayload { item_id: s, purchase_item_id: None, quantity: 1.0 }],
    };
    assert_eq!(field_of(e.rets.create_vendor_return(vr)), "item_id");
    // still no stock, no lots, never low
    let it = e.db.begin_read().unwrap().items().find_by_id(s).unwrap().unwrap();
    assert_eq!((it.current_stock, it.is_low_stock), (0.0, false));
    let lots: i64 = e.db.pool.get().unwrap().query_row("SELECT COUNT(*) FROM purchase_items WHERE item_id = ?1", [s], |r| r.get(0)).unwrap();
    assert_eq!(lots, 0);
    e.invariant();
}

#[test]
fn services_cannot_be_returned_or_issued_as_replacement() {
    let e = env();
    let paper = e.item("Paper");
    let pen = e.item("Pen");
    e.lot(paper, 10.0, 2.0);
    e.lot(pen, 10.0, 3.0);
    let s = e.svc("Print", Some(2.0), &[(paper, 1.0)]);
    let sale = e.sell(&[(s, 2.0, 12.0), (pen, 1.0, 5.0)]).unwrap();
    let d = e.detail(sale);
    let svc_line = d.items.iter().find(|l| l.is_service).unwrap().sale_item.clone();
    assert_eq!(d.items.iter().filter(|l| l.is_service).count(), 1, "return pickers can skip service lines");
    let ret = |items: Vec<CustomerReturnItemPayload>, repl: Vec<ReplacementItemPayload>, st| CustomerReturnPayload {
        original_sale_id: Some(sale), party_id: None, return_date: DAY.into(), settlement_type: st, reason: None, notes: None,
        items, replacement_items: repl,
    };
    let linked = CustomerReturnItemPayload {
        item_id: s, sale_item_id: Some(svc_line.id), quantity: 1.0, unit_price: 12.0,
        condition: book_keeper::core::value_objects::StockCondition::Restockable, unit_cost_override: None,
    };
    let err = e.rets.create_customer_return(ret(vec![linked.clone()], vec![], book_keeper::core::value_objects::SettlementType::Refund)).unwrap_err();
    assert!(err.to_string().contains("Services cannot be returned"), "{err}");
    // unlinked service line
    let mut unlinked = linked.clone();
    unlinked.sale_item_id = None;
    unlinked.unit_cost_override = Some(1.0);
    assert!(e.rets.create_customer_return(ret(vec![unlinked], vec![], book_keeper::core::value_objects::SettlementType::Refund)).is_err());
    // replacement issue of a service
    let phys = d.items.iter().find(|l| !l.is_service).unwrap().sale_item.clone();
    let ok_line = CustomerReturnItemPayload {
        item_id: pen, sale_item_id: Some(phys.id), quantity: 1.0, unit_price: 5.0,
        condition: book_keeper::core::value_objects::StockCondition::Restockable, unit_cost_override: None,
    };
    let err = e
        .rets
        .create_customer_return(ret(vec![ok_line], vec![ReplacementItemPayload { item_id: s, quantity: 1.0 }], book_keeper::core::value_objects::SettlementType::Replacement))
        .unwrap_err();
    assert!(err.to_string().contains("Services cannot be returned"), "{err}");
    e.invariant();
}

// ------------------------------------ selling ------------------------------------

#[test]
fn service_and_physical_lines_share_component_lots_in_line_order() {
    let e = env();
    let paper = e.item("Paper");
    e.lot(paper, 5.0, 2.0); // lot 1
    e.lot(paper, 10.0, 3.0); // lot 2
    let print = e.svc("Print", None, &[(paper, 1.0)]);
    // Paper sold first (3 @ lot 1), then 4 prints: 2 more from lot 1, 2 from lot 2.
    let sale = e.sell(&[(paper, 3.0, 10.0), (print, 4.0, 12.0)]).unwrap();
    assert_eq!(e.stock(paper), 8.0);
    assert_eq!((e.remaining(1), e.remaining(2)), (0.0, 8.0));
    let d = e.detail(sale);
    let l = &d.items[1];
    assert!(l.is_service);
    assert_eq!(l.allocations.iter().map(|a| (a.purchase_item_id, a.quantity_depleted)).collect::<Vec<_>>(), vec![(1, 2.0), (2, 2.0)]);
    assert_eq!((l.cost_amount, l.misc_cost, l.profit), (10.0, 0.0, 38.0));
    assert_eq!(l.sale_item.service_cost_total, None);
    assert_eq!(d.items[0].allocations.iter().map(|a| a.quantity_depleted).sum::<f64>(), 3.0);
    // Bill profit: 30 + 48 - (6 + 10)
    assert_eq!(d.profit_summary.total, 62.0);
    assert_eq!(d.profit_summary.lines_profit, 62.0);
    e.invariant();
}

#[test]
fn shortfall_names_the_component_the_service_and_the_line() {
    let e = env();
    let paper = e.item("Paper");
    e.lot(paper, 3.0, 2.0);
    let print = e.svc("Photo print", None, &[(paper, 1.0)]);
    let err = e.sell(&[(paper, 1.0, 10.0), (print, 5.0, 12.0)]).unwrap_err();
    let m = msg_of::<i64>(Err(err));
    assert!(m.contains("Paper") && m.contains("Photo print") && m.contains("line 2"), "{m}");
    assert_eq!(e.stock(paper), 3.0, "nothing written");
    // A physical-only shortfall keeps the original error type.
    assert!(matches!(e.sell(&[(paper, 9.0, 10.0), (print, 1.0, 12.0)]), Err(AppError::InsufficientStockError { .. })));
    // Together the lines need 4 > 3 even though each alone fits.
    assert!(e.sell(&[(print, 2.0, 12.0), (paper, 2.0, 10.0)]).is_err());
    e.invariant();
}

#[test]
fn services_reject_bundles_on_sale_lines() {
    let e = env();
    let paper = e.item("Paper");
    e.lot(paper, 3.0, 2.0);
    let print = e.svc("Print", None, &[(paper, 1.0)]);
    let b = e.items.create_bundle(CreateBundlePayload {
        item_id: Some(paper), bundle_name: "Ream".into(), units_per_bundle: 5.0, mrp: None, default_bundle_price: None, barcode: None,
    })
    .unwrap();
    let r = e.sales.create_sale(sale_payload(None, PaymentStatus::Paid, 12.0, &[(print, Some(b.id), 1.0, 12.0)]));
    assert_eq!(field_of(r), "bundle_id");
}

#[test]
fn misc_cost_cases_no_target_target_below_and_above_the_fifo_cost() {
    let e = env();
    let paper = e.item("Paper");
    e.lot(paper, 100.0, 2.0);
    let none = e.svc("NoTarget", None, &[(paper, 3.0)]); // C = 6 / unit
    let low = e.svc("Low", Some(4.0), &[(paper, 3.0)]); // target 4 < 6
    let high = e.svc("High", Some(10.0), &[(paper, 3.0)]); // target 10 > 6
    let labour = e.svc("Labour", Some(7.0), &[]); // C = 0
    let sale = e.sell(&[(none, 2.0, 20.0), (low, 2.0, 20.0), (high, 2.0, 20.0), (labour, 2.0, 20.0)]).unwrap();
    let d = e.detail(sale);
    let by = |n: usize| (d.items[n].cost_amount, d.items[n].misc_cost, d.items[n].profit, d.items[n].sale_item.service_cost_total);
    assert_eq!(by(0), (12.0, 0.0, 28.0, None));
    assert_eq!(by(1), (12.0, 0.0, 28.0, Some(8.0)));
    assert_eq!(by(2), (20.0, 8.0, 20.0, Some(20.0)));
    assert_eq!(by(3), (14.0, 14.0, 26.0, Some(14.0)));
    // Bill profit = total 80*... : 160 - (12*3 FIFO + misc 8 + 14)
    assert_eq!(d.profit_summary.total, 160.0 - 36.0 - 8.0 - 14.0);
    assert_eq!(d.profit_summary.lines_profit, d.profit_summary.total);
    // History profit column agrees with the detail.
    let h = e
        .sales
        .get_sales_history(SalesHistoryFilter { include_profit: Some(true), ..Default::default() })
        .unwrap();
    assert_eq!(h.items[0].profit, Some(d.profit_summary.total));
    // P&L counts the same misc cost as its own line.
    let pnl = e.report.get_pnl_report(DAY, DAY).unwrap();
    assert_eq!(pnl.service_misc_costs, 22.0);
    assert_eq!(pnl.cogs, 36.0);
    assert_eq!(pnl.gross_profit, pnl.net_sales - pnl.cogs - 22.0);
    e.invariant();
}

#[test]
fn lot_recost_changes_misc_and_keeps_line_cost_at_max_of_fifo_and_target() {
    let e = env();
    let paper = e.item("Paper");
    e.lot(paper, 10.0, 2.0); // lot 1
    let s = e.svc("Print", Some(5.0), &[(paper, 2.0)]); // target 5 per unit
    let sale = e.sell(&[(s, 2.0, 20.0)]).unwrap(); // C = 8, target 10 -> misc 2, cost 10
    let l = &e.detail(sale).items[0];
    assert_eq!((l.cost_amount, l.misc_cost), (10.0, 2.0));
    // Lot cost 2 -> 3: C = 12 > target 10 -> misc 0, line cost = C = 12.
    e.purchases.update_lot_purchase_price(UpdateLotPricePayload { purchase_item_id: 1, unit_price: 3.0 }).unwrap();
    let l = &e.detail(sale).items[0];
    assert_eq!((l.cost_amount, l.misc_cost), (12.0, 0.0));
    // Lot cost 3 -> 2.5: C = 10 = target.
    e.purchases.update_lot_purchase_price(UpdateLotPricePayload { purchase_item_id: 1, unit_price: 2.5 }).unwrap();
    let l = &e.detail(sale).items[0];
    assert_eq!((l.cost_amount, l.misc_cost), (10.0, 0.0));
    // Lot cost back to 1: C = 4, misc 6, cost stays at the target 10.
    e.purchases.update_lot_purchase_price(UpdateLotPricePayload { purchase_item_id: 1, unit_price: 1.0 }).unwrap();
    let l = &e.detail(sale).items[0];
    assert_eq!((l.cost_amount, l.misc_cost), (10.0, 6.0));
    let pnl = e.report.get_pnl_report(DAY, DAY).unwrap();
    assert_eq!((pnl.cogs, pnl.service_misc_costs), (4.0, 6.0));
}

#[test]
fn bill_edit_and_delete_restore_component_lots_and_repost_recosts() {
    let e = env();
    let paper = e.item("Paper");
    e.lot(paper, 5.0, 2.0); // lot 1
    e.lot(paper, 10.0, 3.0); // lot 2
    let s = e.svc("Print", Some(1.0), &[(paper, 1.0)]);
    let sale = e.sell(&[(s, 7.0, 12.0)]).unwrap();
    assert_eq!((e.stock(paper), e.remaining(1), e.remaining(2)), (8.0, 0.0, 8.0));
    // Edit down to 2 prints: lots restored then 2 drawn again from lot 1.
    let payload = sale_payload(None, PaymentStatus::Paid, 24.0, &[(s, None, 2.0, 12.0)]);
    e.sales.update_sale(sale, payload).unwrap();
    assert_eq!((e.stock(paper), e.remaining(1), e.remaining(2)), (13.0, 3.0, 10.0));
    let d = e.detail(sale);
    assert_eq!(d.items.len(), 1);
    assert_eq!((d.items[0].cost_amount, d.items[0].sale_item.service_cost_total), (4.0, Some(2.0)));
    // Service cost raised before the edit: the repost uses the cost in force at edit time.
    let u = UpdateItemPayload {
        item_type: Some(ItemType::Service), service_cost: Some(5.0), components: None, low_stock_threshold: None,
        name: "Print".into(), category: None, barcode: None, base_unit: "PRINT".into(), mrp: None, default_sale_price: 12.0,
    };
    e.items.update_item(s, u).unwrap();
    assert_eq!(e.detail(sale).items[0].sale_item.service_cost_total, Some(2.0), "past sales are not touched by the item edit");
    e.sales.update_sale(sale, sale_payload(None, PaymentStatus::Paid, 24.0, &[(s, None, 2.0, 12.0)])).unwrap();
    let d = e.detail(sale);
    assert_eq!((d.items[0].sale_item.service_cost_total, d.items[0].misc_cost, d.items[0].cost_amount), (Some(10.0), 6.0, 10.0));
    // Delete puts everything back.
    e.sales.delete_sale(sale).unwrap();
    assert_eq!((e.stock(paper), e.remaining(1), e.remaining(2)), (15.0, 5.0, 10.0));
    e.invariant();
}

#[test]
fn preview_matches_the_command_and_writes_nothing() {
    let e = env();
    let paper = e.item("Paper");
    e.lot(paper, 5.0, 2.0);
    e.lot(paper, 10.0, 3.0);
    let s = e.svc("Print", Some(3.0), &[(paper, 2.0)]);
    // After a physical line of 1 and a service line of 1 (3 sheets promised), a service line of 2.
    let prior = vec![PriorLine { item_id: paper, quantity: 1.0 }, PriorLine { item_id: s, quantity: 1.0 }];
    let counts = |e: &E| -> (f64, i64, i64) {
        let c = e.db.pool.get().unwrap();
        (
            c.query_row("SELECT SUM(remaining_quantity) FROM purchase_items", [], |r| r.get(0)).unwrap(),
            c.query_row("SELECT COUNT(*) FROM sale_stock_allocations", [], |r| r.get(0)).unwrap(),
            c.query_row("SELECT COUNT(*) FROM sale_items", [], |r| r.get(0)).unwrap(),
        )
    };
    let before = counts(&e);
    let draw = e.inv.preview_lot_draw(s, 2.0, &prior).unwrap();
    assert_eq!(before, counts(&e));
    assert_eq!(draw.components.len(), 1);
    let c = &draw.components[0];
    assert_eq!((c.item_id, c.quantity), (paper, 4.0));
    // 3 sheets are taken first (lot 1 has 5): the 4 sheets are 2 @ 2 (lot 1) + 2 @ 3 (lot 2).
    assert_eq!(c.lots.iter().map(|l| l.draw_quantity).collect::<Vec<_>>(), vec![2.0, 2.0]);
    assert_eq!((draw.components_cost, draw.misc_cost, draw.line_cost), (10.0, 0.0, 10.0));
    assert_eq!(draw.shortfall, 0.0);
    // The real bill in the same order costs the same.
    let sale = e
        .sales
        .create_sale(sale_payload(None, PaymentStatus::Paid, 12.0 + 12.0 + 24.0 + 10.0, &[(paper, None, 1.0, 10.0), (s, None, 1.0, 12.0), (s, None, 2.0, 12.0)]))
        .unwrap()
        .sale_id;
    let d = e.detail(sale);
    assert_eq!(d.items[2].cost_amount, draw.line_cost);
    assert_eq!(d.items[2].misc_cost, draw.misc_cost);
    // Physical line preview skips the service line's sheets too.
    let phys = e.inv.preview_lot_draw(paper, 1.0, &[PriorLine { item_id: s, quantity: 2.0 }]).unwrap();
    assert_eq!(phys.line_cost, phys.draw_cost);
    assert_eq!(phys.lots.iter().map(|l| l.draw_quantity).sum::<f64>(), 1.0);
    // preview_sale of a service bill writes nothing and prices like the command.
    let before = counts(&e);
    let pv = e
        .sales
        .preview_sale(None, PreviewSalePayload {
            confirm_move_settlements: false,
            transaction_date: DAY.into(), party_id: None, payment_status: None, paid_amount: None, payment_mode: None, notes: None,
            items: vec![CreateSaleItemPayload { item_id: s, bundle_id: None, bundle_quantity: 1.0, unit_price: 12.0 }],
            charges: vec![],
        })
        .unwrap();
    assert_eq!(pv.total_amount, 12.0);
    assert_eq!(before, counts(&e));
    // Shortfall in preview, in service units.
    let big = e.inv.preview_lot_draw(s, 100.0, &[]).unwrap();
    assert!(big.shortfall > 0.0);
    assert!(big.components[0].shortfall > 0.0);
    assert_eq!(field_of(e.inv.preview_lot_draw(s, -1.0, &[])), "quantity");
}

#[test]
fn x4_audit_recost_and_reversal_credit_the_component_not_the_service() {
    let e = env();
    let paper = e.item("Paper");
    e.lot(paper, 10.0, 2.0);
    let s = e.svc("Print", None, &[(paper, 1.0)]);
    let sale = e.sell(&[(s, 4.0, 12.0)]).unwrap();
    assert_eq!(e.stock(s), 0.0, "the service never holds stock");
    e.sales.delete_sale(sale).unwrap();
    assert_eq!((e.stock(paper), e.stock(s)), (10.0, 0.0));
    e.invariant();
}
