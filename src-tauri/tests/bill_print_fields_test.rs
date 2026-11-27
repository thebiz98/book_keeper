// tests/bill_print_fields_test.rs
//! Test Suite: printed bill fields. Sale and purchase details carry each line's units per bundle
//! and the item's base unit, so the bill can print `Box (10 PCS)` with no math in the UI (DD-56).

mod common;

use std::sync::Arc;

use book_keeper::core::dtos::*;
use book_keeper::core::value_objects::*;
use book_keeper::services::contracts::item_service::ItemService;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::sale_service::SaleService;
use common::ledger::*;
use common::*;

type Inv = InventoryServiceImpl<FifoStrategy>;

fn boxed_item(items: &ItemServiceImpl<Inv>) -> (i64, i64) {
    let id = items
        .create_item(CreateItemPayload {
            item_type: None, service_cost: None, components: None, low_stock_threshold: None,
            name: "Paper".into(), category: None, barcode: None, base_unit: "PCS".into(), mrp: None,
            default_sale_price: 10.0, initial_stock: None, initial_cost: None,
            bundles: Some(vec![CreateBundlePayload {
                item_id: None, bundle_name: "Box".into(), units_per_bundle: 10.0, mrp: None,
                default_bundle_price: None, barcode: None,
            }]),
        })
        .unwrap();
    (id, items.get_item(id).unwrap().bundles[0].bundle_id)
}

#[test]
fn bill_lines_carry_units_per_bundle_and_base_unit() {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let items = ItemServiceImpl::new(db.clone(), Arc::new(db.indexer()), inv.clone());
    let sales = SaleServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone());
    let purchases = PurchaseServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone());
    let (a, boxb) = boxed_item(&items);

    let p = purchases
        .create_purchase(purchase_payload(None, PaymentStatus::Paid, 140.0, &[(a, Some(boxb), 2.0, 50.0), (a, None, 8.0, 5.0)]))
        .unwrap()
        .purchase_id;
    let pd = purchases.get_purchase_details(p).unwrap();
    let got: Vec<_> = pd.items.iter().map(|i| (i.units_per_bundle, i.base_unit.as_deref())).collect();
    assert_eq!(got, vec![(Some(10.0), Some("PCS")), (None, Some("PCS"))]);

    let s = sales
        .create_sale(sale_payload(None, PaymentStatus::Paid, 136.0, &[(a, Some(boxb), 1.0, 100.0), (a, None, 3.0, 12.0)]))
        .unwrap()
        .sale_id;
    let sd = sales.get_sale_details(s).unwrap();
    let got: Vec<_> = sd.items.iter().map(|i| (i.units_per_bundle, i.base_unit.as_deref())).collect();
    assert_eq!(got, vec![(Some(10.0), Some("PCS")), (None, Some("PCS"))]);
}
