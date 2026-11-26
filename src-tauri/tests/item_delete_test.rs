// tests/item_delete_test.rs
//! Test Suite: delete and restore items (FR-45, DD-74..DD-76). Production services on real SQLite.

mod common;

use std::sync::Arc;

use book_keeper::core::dtos::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::{
    AdjustmentReason, ItemType, PaymentStatus, SettlementType, StockCondition,
};
use book_keeper::data::contracts::search_indexer::ItemSearchIndexer;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::inventory_service::InventoryService;
use book_keeper::services::contracts::item_import_service::ItemImportService;
use book_keeper::services::contracts::item_service::ItemService;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::report_service::ReportService;
use book_keeper::services::contracts::return_service::ReturnService;
use book_keeper::services::contracts::sale_service::SaleService;
use common::ledger::*;
use common::stock::assert_stock_invariant;
use common::*;

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
        name: name.into(), category: None, barcode: None, base_unit: "PCS".into(), mrp: None,
        default_sale_price: 10.0, initial_stock: None, initial_cost: None, bundles: None,
    }
}

fn update_payload(name: &str) -> UpdateItemPayload {
    UpdateItemPayload {
        item_type: None, service_cost: None, components: None, low_stock_threshold: None, name: name.into(),
        category: None, barcode: None, base_unit: "PCS".into(), mrp: None, default_sale_price: 10.0,
    }
}

fn bundle(item: i64, name: &str, barcode: Option<&str>) -> CreateBundlePayload {
    CreateBundlePayload {
        item_id: Some(item), bundle_name: name.into(), units_per_bundle: 12.0, mrp: None,
        default_bundle_price: None, barcode: barcode.map(String::from),
    }
}

fn msg_of<T: std::fmt::Debug>(r: Result<T, AppError>) -> String {
    match r {
        Err(e @ AppError::ValidationError { .. }) => e.to_string(),
        Err(AppError::BillLocked { message, .. }) => message,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

impl E {
    fn item(&self, name: &str) -> i64 {
        self.items.create_item(physical(name)).unwrap()
    }
    /// An item that has history (one purchase lot), so deleting it is a soft delete.
    fn used(&self, name: &str) -> i64 {
        let id = self.item(name);
        self.lot(id, 1.0, 1.0);
        id
    }
    fn lot(&self, item: i64, qty: f64, cost: f64) {
        self.purchases
            .create_purchase(purchase_payload(None, PaymentStatus::Paid, qty * cost, &[(item, None, qty, cost)]))
            .unwrap();
    }
    fn sell(&self, item: i64, qty: f64, price: f64) -> i64 {
        self.sales
            .create_sale(sale_payload(None, PaymentStatus::Paid, qty * price, &[(item, None, qty, price)]))
            .unwrap()
            .sale_id
    }
    fn row(&self, id: i64) -> book_keeper::core::entities::Item {
        self.db.begin_read().unwrap().items().find_by_id(id).unwrap().unwrap()
    }
    fn count(&self, sql: &str) -> i64 {
        self.db.pool.get().unwrap().query_row(sql, [], |r| r.get(0)).unwrap()
    }
    fn valuation(&self) -> f64 {
        self.report.get_stock_valuation_report().unwrap().total_valuation
    }
    fn hits(&self, q: &str) -> usize {
        self.items.search_items_fts(q, None).unwrap().len()
    }
}

#[test]
fn delete_and_restore_round_trip() {
    let e = env();
    let id = e.used("Pen");
    let before = e.row(id).updated_at;
    e.items.delete_item(id, false).unwrap();
    let r = e.row(id);
    assert!(!r.is_active);
    assert_ne!(r.updated_at, before, "updated_at is the delete time");
    assert!(!e.items.get_item(id).unwrap().is_active, "get_item still opens a deleted item");
    e.items.restore_item(id).unwrap();
    assert!(e.row(id).is_active);
    assert!(e.items.get_item(id).unwrap().is_active);
    assert_eq!(e.count("SELECT COUNT(*) FROM items"), 1, "never physically deleted");
}

#[test]
fn delete_restore_state_errors() {
    let e = env();
    let id = e.used("Pen");
    assert!(msg_of(e.items.restore_item(id)).contains("not deleted"));
    e.items.delete_item(id, false).unwrap();
    assert!(msg_of(e.items.delete_item(id, false)).contains("already deleted"));
    assert!(matches!(e.items.delete_item(999, false), Err(AppError::NotFound { .. })));
    assert!(matches!(e.items.restore_item(999), Err(AppError::NotFound { .. })));
    assert!(matches!(e.items.preview_item_delete(999), Err(AppError::NotFound { .. })));
}

#[test]
fn component_guard_and_service_restore() {
    let e = env();
    let paper = e.item("Paper");
    e.lot(paper, 10.0, 2.0);
    let svc = e
        .items
        .create_item(CreateItemPayload {
            item_type: Some(ItemType::Service),
            components: Some(vec![ServiceComponentInput { component_item_id: paper, quantity_per_unit: 1.0 }]),
            base_unit: "PRINT".into(),
            ..physical("Print")
        })
        .unwrap();
    let p = e.items.preview_item_delete(paper).unwrap();
    assert!(!p.can_delete);
    assert_eq!(p.blocking_services, vec!["Print".to_string()]);
    assert!(p.reason.unwrap().contains("Print"));
    match e.items.delete_item(paper, false) {
        Err(AppError::BillLocked { code, message }) => {
            assert_eq!(code, "ITEM_IS_COMPONENT");
            assert!(message.contains("Print"));
        }
        other => panic!("{other:?}"),
    }
    assert!(e.row(paper).is_active);
    // Sold once, so deleting the service is soft; then the component (used by that service row).
    e.sell(svc, 1.0, 12.0);
    assert_eq!(e.items.delete_item(svc, false).unwrap().mode, DeleteMode::Soft);
    assert!(e.items.preview_item_delete(paper).unwrap().can_delete);
    assert_eq!(e.items.delete_item(paper, false).unwrap().mode, DeleteMode::Soft);
    assert!(msg_of(e.items.restore_item(svc)).contains("Paper"));
    e.items.restore_item(paper).unwrap();
    e.items.restore_item(svc).unwrap();
    assert!(e.row(svc).is_active);
}

#[test]
fn every_write_to_a_deleted_item_is_refused() {
    let e = env();
    let id = e.item("Pen");
    e.lot(id, 10.0, 2.0);
    let b = e.items.create_bundle(bundle(id, "Box", None)).unwrap().bundle.id;
    e.items.delete_item(id, false).unwrap();
    let deleted = "'Pen' is deleted. Restore it first.";
    assert!(msg_of(e.items.update_item(id, update_payload("Pen"))).contains(deleted));
    assert!(msg_of(e.items.create_bundle(bundle(id, "Pack", None))).contains(deleted));
    assert!(msg_of(e.items.update_bundle(b, UpdateBundlePayload {
        bundle_name: "Box".into(), units_per_bundle: 6.0, mrp: None, default_bundle_price: None, barcode: None,
    })).contains(deleted));
    assert!(msg_of(e.items.delete_bundle(b)).contains(deleted));
    assert!(msg_of(e.items.update_low_stock_threshold(id, 3.0)).contains(deleted));
    for reason in [AdjustmentReason::UnrecordedStock, AdjustmentReason::CountCorrection] {
        let r = e.inv.create_adjustment(StockAdjustmentPayload {
            adjustment_date: DAY.into(), item_id: id, quantity: 1.0, reason, unit_cost: Some(1.0), notes: None,
        });
        assert!(msg_of(r).contains(deleted));
    }
    let r = e.inv.set_item_stock(SetItemStockPayload {
        item_id: id, counted_quantity: 3.0, unit_cost: Some(1.0), adjustment_date: DAY.into(), notes: None,
    });
    assert!(msg_of(r).contains(deleted));
    assert_eq!(e.row(id).current_stock, 10.0);
    // Restored, the same writes work again.
    e.items.restore_item(id).unwrap();
    e.items.update_item(id, update_payload("Pen")).unwrap();
}

#[test]
fn name_clash_with_a_deleted_item_on_create_and_import() {
    let e = env();
    let id = e.used("Pen");
    e.items.delete_item(id, false).unwrap();
    match e.items.create_item(physical("pen")) {
        Err(AppError::DeletedNameClash { field, code, existing_id, .. }) => {
            assert_eq!((field.as_str(), code.as_str(), existing_id), ("name", "ITEM_NAME_DELETED", id));
        }
        other => panic!("{other:?}"),
    }
    // Rename another item onto the deleted name: same clash.
    let other = e.item("Ink");
    assert!(matches!(e.items.update_item(other, update_payload("PEN")), Err(AppError::DeletedNameClash { .. })));

    let svc = ItemImportServiceImpl::new(e.db.clone(), e.inv.clone()).with_indexer(Arc::new(e.db.indexer()));
    let f = std::env::temp_dir().join(format!("bk_del_{}.csv", uuid::Uuid::new_v4()));
    std::fs::write(
        &f,
        "Item Name,Category,Barcode,Base Unit,MRP,Sale Price,Purchase Cost,Opening Stock,Packaging Bundle,Units Per Bundle\nPen,,,PCS,,10,1,,,\n",
    )
    .unwrap();
    let report = svc.validate_items_import(f.to_str().unwrap()).unwrap();
    assert_eq!(report.valid_count, 0);
    assert!(report.rows_with_errors[0].message.contains("ITEM_NAME_DELETED"), "{report:?}");
    assert!(report.duplicates.is_empty());
    assert!(!e.row(id).is_active, "an import never touches a deleted item");
}

#[test]
fn old_bills_keep_a_deleted_item_but_new_lines_cannot_add_it() {
    let e = env();
    let pen = e.item("Pen");
    let ink = e.item("Ink");
    e.lot(pen, 10.0, 2.0);
    e.lot(ink, 10.0, 3.0);
    let sale = e.sell(pen, 2.0, 10.0);
    e.items.delete_item(pen, false).unwrap();
    // Edit the old sale: qty and payment change; the deleted item stays.
    e.sales.update_sale(sale, sale_payload(None, PaymentStatus::Paid, 30.0, &[(pen, None, 3.0, 10.0)])).unwrap();
    assert_eq!(e.row(pen).current_stock, 7.0);
    // Adding the deleted item to another bill fails.
    let err = e.sales.create_sale(sale_payload(None, PaymentStatus::Paid, 10.0, &[(pen, None, 1.0, 10.0)])).unwrap_err();
    assert!(err.to_string().contains("Restore the item first"), "{err}");
    // Adding it to the old bill as an extra new item is allowed only if it was already there; a
    // different deleted item is not.
    e.items.delete_item(ink, false).unwrap();
    let err = e
        .sales
        .update_sale(sale, sale_payload(None, PaymentStatus::Paid, 40.0, &[(pen, None, 3.0, 10.0), (ink, None, 1.0, 10.0)]))
        .unwrap_err();
    assert!(err.to_string().contains("'Ink' is deleted"), "{err}");
    // Deleting the old bill restores the deleted item's lots.
    e.sales.delete_sale(sale).unwrap();
    assert_eq!(e.row(pen).current_stock, 10.0);
    assert!(!e.row(pen).is_active);
    assert_stock_invariant(&mut *e.db.begin_read().unwrap());
}

#[test]
fn purchase_edit_keeps_a_deleted_item_only_if_it_was_on_the_bill() {
    let e = env();
    let pen = e.item("Pen");
    let ink = e.used("Ink");
    let bill = e
        .purchases
        .create_purchase(purchase_payload(None, PaymentStatus::Paid, 20.0, &[(pen, None, 10.0, 2.0)]))
        .unwrap()
        .purchase_id;
    e.items.delete_item(pen, false).unwrap();
    e.items.delete_item(ink, false).unwrap();
    e.purchases
        .update_purchase(bill, purchase_payload(None, PaymentStatus::Paid, 22.0, &[(pen, None, 10.0, 2.2)]))
        .unwrap();
    let err = e
        .purchases
        .create_purchase(purchase_payload(None, PaymentStatus::Paid, 5.0, &[(pen, None, 1.0, 5.0)]))
        .unwrap_err();
    assert!(err.to_string().contains("Restore the item first"), "{err}");
    let err = e
        .purchases
        .update_purchase(bill, purchase_payload(None, PaymentStatus::Paid, 25.0, &[(pen, None, 10.0, 2.2), (ink, None, 1.0, 3.0)]))
        .unwrap_err();
    assert!(err.to_string().contains("'Ink' is deleted"), "{err}");
}

#[test]
fn returns_of_a_deleted_item_are_rejected_but_existing_returns_can_be_deleted() {
    let e = env();
    let pen = e.item("Pen");
    e.lot(pen, 10.0, 2.0);
    // DD-86: a vendor return names a lot bought from that vendor.
    seed_party(&e.db, 90, "V", book_keeper::core::value_objects::PartyType::Vendor);
    let vendor_bill = e
        .purchases
        .create_purchase(purchase_payload(Some(90), PaymentStatus::Paid, 20.0, &[(pen, None, 2.0, 10.0)]))
        .unwrap()
        .purchase_id;
    let vendor_lot = e.db.begin_read().unwrap().purchases().list_items_by_transaction_id(vendor_bill).unwrap()[0].id;
    let sale = e.sell(pen, 4.0, 10.0);
    let line = e.sales.get_sale_details(sale).unwrap().items[0].sale_item.clone();
    let ret = |qty: f64| CustomerReturnPayload {
        original_sale_id: Some(sale), party_id: None, return_date: DAY.into(),
        settlement_type: SettlementType::Refund, reason: None, notes: None,
        items: vec![CustomerReturnItemPayload {
            item_id: pen, sale_item_id: Some(line.id), quantity: qty, unit_price: 10.0,
            condition: StockCondition::Restockable, unit_cost_override: None,
        }],
        replacement_items: vec![],
    };
    let first = e.rets.create_customer_return(ret(1.0)).unwrap().return_id;
    e.items.delete_item(pen, false).unwrap();
    assert!(e.sales.get_sale_details(sale).unwrap().items[0].item_deleted, "pickers disable the line");
    let err = e.rets.create_customer_return(ret(1.0)).unwrap_err();
    assert!(err.to_string().contains("'Pen' is deleted. Restore the item first."), "{err}");
    let vr = VendorReturnPayload {
        original_purchase_id: None, party_id: Some(90), return_date: DAY.into(),
        settlement_type: SettlementType::Refund, reason: None, notes: None,
        items: vec![VendorReturnItemPayload { item_id: pen, purchase_item_id: Some(vendor_lot), quantity: 1.0 }],
    };
    let err = e.rets.create_vendor_return(vr).unwrap_err();
    assert!(matches!(err, AppError::DeletedReference { ref entity, .. } if entity == "ITEM"), "{err}");
    e.rets.delete_return(first).unwrap();
    assert_stock_invariant(&mut *e.db.begin_read().unwrap());
}

#[test]
fn valuation_drops_by_the_stock_value_and_comes_back() {
    let e = env();
    let pen = e.item("Pen");
    let ink = e.item("Ink");
    e.lot(pen, 10.0, 2.0);
    e.lot(pen, 5.0, 4.0);
    e.lot(ink, 3.0, 10.0);
    assert_eq!(e.valuation(), 70.0);
    let p = e.items.preview_item_delete(pen).unwrap();
    assert_eq!((p.stock, p.stock_value, p.can_delete), (15.0, 40.0, true));
    e.items.delete_item(pen, false).unwrap();
    assert_eq!(e.valuation(), 30.0);
    let rep = e.report.get_stock_valuation_report().unwrap();
    assert_eq!(rep.active_lots_count, 1);
    assert_eq!(rep.categories_breakdown.iter().map(|c| c.valuation).sum::<f64>(), 30.0);
    e.items.restore_item(pen).unwrap();
    assert_eq!(e.valuation(), 70.0);
}

#[test]
fn search_by_name_and_bundle_barcode_follows_delete_and_restore() {
    let e = env();
    let pen = e.used("Pen");
    e.item("Ink");
    e.items.create_bundle(bundle(pen, "Box", Some("PACK-777"))).unwrap();
    let idx = e.db.indexer();
    assert_eq!((e.hits("Pen"), e.hits("PACK-777")), (1, 1));
    let n = idx.indexed_item_count().unwrap();
    e.items.delete_item(pen, false).unwrap();
    assert_eq!((e.hits("Pen"), e.hits("PACK-777")), (0, 0));
    assert_eq!(idx.indexed_item_count().unwrap(), n - 1);
    assert_eq!(e.items.get_catalog(CatalogFilter { category: None, low_stock_only: None, search: Some("Pen".into()), page: None, limit: None }).unwrap().total, 0);
    e.items.restore_item(pen).unwrap();
    assert_eq!((e.hits("Pen"), e.hits("PACK-777")), (1, 1));
    assert_eq!(idx.indexed_item_count().unwrap(), n);
    // A rebuild keeps only active rows and runs the optimize step.
    e.items.delete_item(pen, false).unwrap();
    idx.rebuild_index().unwrap();
    assert_eq!((e.hits("Pen"), idx.indexed_item_count().unwrap()), (0, n - 1));
}

#[test]
fn previews_write_nothing() {
    let e = env();
    let pen = e.item("Pen");
    e.lot(pen, 5.0, 2.0);
    let rows = "SELECT (SELECT COUNT(*) FROM items WHERE is_active = 1) * 1000 + (SELECT COUNT(*) FROM purchase_items) + (SELECT COUNT(*) FROM stock_adjustments)";
    let before = (e.count(rows), e.row(pen).updated_at);
    e.items.preview_item_delete(pen).unwrap();
    e.items.list_deleted_items(None, None, None).unwrap();
    assert_eq!((e.count(rows), e.row(pen).updated_at), before);
    assert!(e.row(pen).is_active);
}

#[test]
fn list_deleted_items_search_paging_and_value() {
    let e = env();
    let names = ["Alpha pen", "Beta pen", "Gamma ink", "100%_x"];
    let ids: Vec<i64> = names.iter().map(|n| e.used(n)).collect();
    e.lot(ids[0], 4.0, 2.5);
    for id in &ids {
        e.items.delete_item(*id, false).unwrap();
    }
    let all = e.items.list_deleted_items(None, None, None).unwrap();
    assert_eq!((all.total, all.items.len()), (4, 4));
    let a = all.items.iter().find(|r| r.item_id == ids[0]).unwrap();
    assert_eq!((a.current_stock, a.stock_value, a.base_unit.as_str()), (5.0, 11.0, "PCS"));
    assert!(!a.deleted_at.is_empty());
    let pens = e.items.list_deleted_items(Some("PEN".into()), None, None).unwrap();
    assert_eq!(pens.total, 2);
    let p1 = e.items.list_deleted_items(None, Some(1), Some(3)).unwrap();
    let p2 = e.items.list_deleted_items(None, Some(2), Some(3)).unwrap();
    assert_eq!((p1.items.len(), p2.items.len(), p1.total), (3, 1, 4));
    // LIKE wildcards in the search are literal.
    assert_eq!(e.items.list_deleted_items(Some("%".into()), None, None).unwrap().total, 1);
    assert_eq!(e.items.list_deleted_items(Some("_".into()), None, None).unwrap().total, 1);
    e.items.restore_item(ids[1]).unwrap();
    assert_eq!(e.items.list_deleted_items(None, None, None).unwrap().total, 3);
}

// ------------------------------- A1: hard delete of never-used items (DD-79) -------------------------------

fn import_two(e: &E) -> (i64, i64) {
    let svc = ItemImportServiceImpl::new(e.db.clone(), e.inv.clone()).with_indexer(Arc::new(e.db.indexer()));
    let f = std::env::temp_dir().join(format!("bk_del_{}.csv", uuid::Uuid::new_v4()));
    std::fs::write(
        &f,
        "Item Name,Category,Barcode,Base Unit,MRP,Sale Price,Purchase Cost,Opening Stock,Packaging Bundle,Units Per Bundle\nImp A,,,PCS,,10,2,5,Box,6\nImp B,,,PCS,,10,3,4,,\n",
    )
    .unwrap();
    svc.execute_items_import(f.to_str().unwrap(), ConflictStrategy::Skip).unwrap();
    let mut r = e.db.begin_read().unwrap();
    (r.items().find_by_name("Imp A").unwrap().unwrap().id, r.items().find_by_name("Imp B").unwrap().unwrap().id)
}

#[test]
fn imported_item_is_removed_permanently_and_the_shared_opening_header_stays_until_empty() {
    let e = env();
    let (a, b) = import_two(&e);
    assert_eq!(e.count("SELECT COUNT(*) FROM purchase_transactions WHERE source = 'OPENING'"), 1, "one OPN header for both");
    let p = e.items.preview_item_delete(a).unwrap();
    assert_eq!((p.mode, p.can_delete, p.stock_value), (DeleteMode::Hard, true, 10.0));
    assert!(p.reasons_used.is_empty());
    assert_eq!(e.items.delete_item(a, false).unwrap().mode, DeleteMode::Hard);
    assert_eq!(e.count("SELECT COUNT(*) FROM items WHERE name = 'Imp A'"), 0);
    assert_eq!(e.count("SELECT COUNT(*) FROM item_bundles"), 0);
    assert_eq!(e.count("SELECT COUNT(*) FROM purchase_transactions WHERE source = 'OPENING'"), 1, "B still uses it");
    assert_eq!(e.count("SELECT COUNT(*) FROM purchase_items"), 1);
    assert_eq!(e.valuation(), 12.0);
    assert_eq!((e.hits("Imp A"), e.hits("Imp B")), (0, 1));
    assert_eq!(e.items.delete_item(b, false).unwrap().mode, DeleteMode::Hard);
    assert_eq!(e.count("SELECT COUNT(*) FROM purchase_transactions WHERE source = 'OPENING'"), 0, "last line drops the header");
    assert_eq!(e.count("SELECT COUNT(*) FROM purchase_items"), 0);
    assert!(e.items.list_deleted_items(None, None, None).unwrap().items.is_empty());
    // The name can be reused.
    e.item("Imp A");
    assert_stock_invariant(&mut *e.db.begin_read().unwrap());
}

#[test]
fn created_with_initial_stock_is_hard_but_a_sale_makes_it_soft() {
    let e = env();
    let id = e.items.create_item(CreateItemPayload { initial_stock: Some(5.0), initial_cost: Some(2.0), ..physical("Fresh") }).unwrap();
    assert_eq!(e.items.preview_item_delete(id).unwrap().mode, DeleteMode::Hard);
    let sold = e.items.create_item(CreateItemPayload { initial_stock: Some(5.0), initial_cost: Some(2.0), ..physical("Sold") }).unwrap();
    let sale = e.sell(sold, 1.0, 10.0);
    let p = e.items.preview_item_delete(sold).unwrap();
    assert_eq!(p.mode, DeleteMode::Soft);
    assert!(p.reasons_used.iter().any(|r| r.contains("sold")), "{:?}", p.reasons_used);
    // Delete the sale again: allocations are gone and the lot is whole, so it is never-used again.
    e.sales.delete_sale(sale).unwrap();
    assert_eq!(e.items.preview_item_delete(sold).unwrap().mode, DeleteMode::Hard);
    assert_eq!(e.items.delete_item(sold, false).unwrap().mode, DeleteMode::Hard);
    assert_eq!(e.items.delete_item(id, false).unwrap().mode, DeleteMode::Hard);
    assert_eq!(e.count("SELECT COUNT(*) FROM items"), 0);
}

#[test]
fn adjusted_items_are_soft_and_a_service_component_blocks_until_the_service_is_gone() {
    let e = env();
    let adj = e.items.create_item(CreateItemPayload { initial_stock: Some(5.0), initial_cost: Some(2.0), ..physical("Adj") }).unwrap();
    e.inv
        .create_adjustment(StockAdjustmentPayload {
            adjustment_date: DAY.into(), item_id: adj, quantity: -1.0, reason: AdjustmentReason::Damaged, unit_cost: None, notes: None,
        })
        .unwrap();
    assert_eq!(e.items.preview_item_delete(adj).unwrap().mode, DeleteMode::Soft);
    let paper = e.items.create_item(CreateItemPayload { initial_stock: Some(5.0), initial_cost: Some(2.0), ..physical("Paper") }).unwrap();
    let svc = e
        .items
        .create_item(CreateItemPayload {
            item_type: Some(ItemType::Service),
            components: Some(vec![ServiceComponentInput { component_item_id: paper, quantity_per_unit: 1.0 }]),
            base_unit: "PRINT".into(),
            ..physical("Print")
        })
        .unwrap();
    assert!(matches!(e.items.delete_item(paper, false), Err(AppError::BillLocked { .. })));
    assert_eq!(e.items.delete_item(svc, false).unwrap().mode, DeleteMode::Hard);
    assert_eq!(e.count("SELECT COUNT(*) FROM service_components"), 0);
    assert_eq!(e.items.delete_item(paper, false).unwrap().mode, DeleteMode::Hard);
}

#[test]
fn soft_deleted_item_that_is_unused_again_can_be_removed_permanently() {
    let e = env();
    let id = e.used("Gone");
    e.items.delete_item(id, false).unwrap();
    // Simulate history that vanished after the soft delete.
    e.db.pool.get().unwrap().execute("DELETE FROM purchase_items WHERE item_id = ?1", [id]).unwrap();
    e.db.pool.get().unwrap().execute("DELETE FROM purchase_transactions", []).unwrap();
    let probe = "SELECT (SELECT COUNT(*) FROM items) * 100 + (SELECT COUNT(*) FROM purchase_items)";
    let before = e.count(probe);
    let p = e.items.preview_item_delete(id).unwrap();
    assert_eq!((p.mode, p.can_delete), (DeleteMode::Hard, true));
    assert_eq!(e.count(probe), before, "preview writes nothing");
    assert_eq!(e.items.delete_item(id, false).unwrap().mode, DeleteMode::Hard);
    assert_eq!(e.count("SELECT COUNT(*) FROM items"), 0);
}

// ---- v1.7 H6: Just delete or Book as loss (FR-58, DD-93, finding S5) ----

fn losses(e: &E) -> f64 {
    e.report.get_pnl_report(DAY, DAY).unwrap().inventory_losses
}

fn adjustments_of(e: &E, item: i64) -> Vec<book_keeper::core::entities::StockAdjustment> {
    e.inv.list_adjustments(Some(item), None, None).unwrap().items
}

fn lots_remaining(e: &E, item: i64) -> Vec<f64> {
    let c = e.db.pool.get().unwrap();
    let mut st = c.prepare("SELECT remaining_quantity FROM purchase_items WHERE item_id = ?1 ORDER BY id").unwrap();
    let v: Vec<f64> = st.query_map([item], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect();
    v
}

fn invariants(e: &E) {
    assert_stock_invariant(&mut *e.db.begin_read().unwrap());
    common::invariants::assert_lot_flow_invariant(&e.db);
    common::invariants::assert_ledger_invariant(&e.db);
}

/// Pen: lots 10 @ 2 and 5 @ 4, 3 sold (FIFO from the first lot): 7 @ 2 + 5 @ 4 = 12 PCS worth 34.
fn pen_with_twelve(e: &E) -> i64 {
    let pen = e.item("Pen");
    e.lot(pen, 10.0, 2.0);
    e.lot(pen, 5.0, 4.0);
    e.sell(pen, 3.0, 10.0);
    pen
}

#[test]
fn s5_just_delete_books_no_loss_and_restore_brings_the_value_back() {
    let _today = book_keeper::core::clock::TodayGuard::new(DAY);
    let e = env();
    let pen = pen_with_twelve(&e);
    let p = e.items.preview_item_delete(pen).unwrap();
    assert_eq!(p.write_off, Some(WriteOffPreview { quantity: 12.0, base_unit: "PCS".into(), loss: 34.0 }));
    assert_eq!((e.valuation(), losses(&e)), (34.0, 0.0));

    let r = e.items.delete_item(pen, false).unwrap();
    assert_eq!((r.mode, r.adjustment_id), (DeleteMode::Soft, None));
    assert!(!e.row(pen).is_active);
    assert_eq!(e.row(pen).current_stock, 12.0, "lots untouched");
    assert_eq!(lots_remaining(&e, pen), vec![7.0, 5.0]);
    assert!(adjustments_of(&e, pen).is_empty());
    assert_eq!(losses(&e), 0.0, "no P&L entry");
    assert_eq!(e.valuation(), 0.0, "the stock leaves Inventory value while deleted");
    invariants(&e);

    e.items.restore_item(pen).unwrap();
    assert_eq!((e.valuation(), losses(&e), e.row(pen).current_stock), (34.0, 0.0, 12.0));
    invariants(&e);
}

#[test]
fn s5_book_as_loss_writes_off_the_whole_stock_at_fifo_cost_today() {
    let _today = book_keeper::core::clock::TodayGuard::new(DAY);
    let e = env();
    let pen = pen_with_twelve(&e);
    let ink = e.item("Ink");
    e.lot(ink, 3.0, 10.0);

    // The preview is the delete itself, rolled back: it writes nothing.
    let probe = "SELECT (SELECT COUNT(*) FROM items WHERE is_active = 1) * 1000000 + (SELECT COUNT(*) FROM stock_adjustments) * 1000 \
                 + (SELECT COUNT(*) FROM stock_outflow_allocations) * 100 + (SELECT CAST(SUM(remaining_quantity) AS INTEGER) FROM purchase_items)";
    let before = (e.count(probe), e.row(pen).updated_at);
    let preview = e.items.preview_item_delete(pen).unwrap().write_off.expect("stock to write off");
    assert_eq!((e.count(probe), e.row(pen).updated_at), before, "preview writes nothing");
    assert_eq!(e.row(pen).current_stock, 12.0);

    let r = e.items.delete_item(pen, true).unwrap();
    assert_eq!(r.mode, DeleteMode::Soft);
    let adj = adjustments_of(&e, pen);
    assert_eq!(adj.len(), 1);
    let a = &adj[0];
    assert_eq!(Some(a.id), r.adjustment_id);
    assert_eq!((a.quantity, a.reason, a.adjustment_date.as_str()), (-12.0, AdjustmentReason::TheftLost, DAY));
    assert_eq!(a.notes.as_deref(), Some("Written off on item delete"));
    assert!(!e.row(pen).is_active);
    assert_eq!(e.row(pen).current_stock, 0.0);
    assert_eq!(lots_remaining(&e, pen), vec![0.0, 0.0]);
    assert_eq!(losses(&e), 34.0, "loss = FIFO cost 7 x 2 + 5 x 4");
    assert_eq!(preview.loss, losses(&e), "preview figure equals the committed loss");
    assert_eq!(preview.quantity, 12.0);
    assert_eq!(e.valuation(), 30.0, "only Ink is left");
    assert_eq!(e.count("SELECT COUNT(*) FROM stock_outflow_allocations WHERE source_type = 'ADJUSTMENT'"), 2);
    invariants(&e);

    // G7: undoing the write-off of a deleted item says restore first, and changes nothing.
    let aid = r.adjustment_id.unwrap();
    let g = e.inv.get_adjustment_undo_guard(aid).unwrap();
    assert!(!g.can_undo);
    assert_eq!(g.reason.unwrap().code, "ITEM_DELETED");
    match e.inv.undo_stock_adjustment(aid) {
        Err(AppError::BillLocked { code, message }) => {
            assert_eq!(code, "ITEM_DELETED");
            assert!(message.contains("Restore the item first"), "{message}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!((losses(&e), e.row(pen).current_stock), (34.0, 0.0));

    // Restore keeps the write-off (stock 0, loss stays); then the undo puts the units back.
    e.items.restore_item(pen).unwrap();
    assert_eq!((e.row(pen).current_stock, losses(&e), e.valuation()), (0.0, 34.0, 30.0));
    invariants(&e);
    let undone = e.inv.undo_stock_adjustment(aid).unwrap();
    assert_eq!(undone.loss_removed, 34.0);
    assert_eq!((e.row(pen).current_stock, losses(&e), e.valuation()), (12.0, 0.0, 64.0));
    assert_eq!(lots_remaining(&e, pen), vec![7.0, 5.0]);
    assert!(adjustments_of(&e, pen).is_empty());
    invariants(&e);
}

#[test]
fn write_off_without_stock_or_on_a_hard_delete_posts_nothing() {
    let _today = book_keeper::core::clock::TodayGuard::new(DAY);
    let e = env();
    // Used, no stock left.
    let empty = e.item("Empty");
    e.lot(empty, 2.0, 3.0);
    e.sell(empty, 2.0, 10.0);
    let p = e.items.preview_item_delete(empty).unwrap();
    assert_eq!((p.mode, p.write_off), (DeleteMode::Soft, None));
    let r = e.items.delete_item(empty, true).unwrap();
    assert_eq!((r.mode, r.adjustment_id), (DeleteMode::Soft, None));
    assert!(adjustments_of(&e, empty).is_empty());

    // Never used (opening stock only): HARD, write_off ignored.
    let fresh = e.items.create_item(CreateItemPayload { initial_stock: Some(5.0), initial_cost: Some(2.0), ..physical("Fresh") }).unwrap();
    let p = e.items.preview_item_delete(fresh).unwrap();
    assert_eq!((p.mode, p.write_off), (DeleteMode::Hard, None));
    let r = e.items.delete_item(fresh, true).unwrap();
    assert_eq!((r.mode, r.adjustment_id), (DeleteMode::Hard, None));
    assert_eq!(e.count("SELECT COUNT(*) FROM stock_adjustments"), 0);
    assert_eq!(losses(&e), 0.0);
    invariants(&e);
}

#[test]
fn a_blocked_delete_has_no_write_off_and_writes_nothing() {
    let _today = book_keeper::core::clock::TodayGuard::new(DAY);
    let e = env();
    let paper = e.used("Paper");
    e.items
        .create_item(CreateItemPayload {
            item_type: Some(ItemType::Service),
            components: Some(vec![ServiceComponentInput { component_item_id: paper, quantity_per_unit: 1.0 }]),
            base_unit: "PRINT".into(),
            ..physical("Print")
        })
        .unwrap();
    let p = e.items.preview_item_delete(paper).unwrap();
    assert_eq!((p.can_delete, p.write_off), (false, None));
    assert!(matches!(e.items.delete_item(paper, true), Err(AppError::BillLocked { .. })));
    assert_eq!(e.count("SELECT COUNT(*) FROM stock_adjustments"), 0);
    assert_eq!(e.row(paper).current_stock, 1.0);
    assert!(e.row(paper).is_active);
}

