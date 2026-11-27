// tests/found_stock_test.rs
//! Test Suite: v1.7 H5 found stock rules (FR-57, DD-92, LLD 35) on real SQLite: increase reasons,
//! cost 0, the latest-cost default, the Undo hint, FIFO rank, the reprice note, and Set stock as
//! the opening-correction entry point (scope B). Every mutating step ends with the invariants.

mod common;

use std::sync::Arc;
use book_keeper::core::dtos::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::inventory_service::InventoryService;
use book_keeper::services::contracts::item_service::ItemService;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::report_service::ReportService;
use book_keeper::services::contracts::return_service::ReturnService;
use book_keeper::services::contracts::sale_service::SaleService;
use common::invariants::*;
use common::ledger::*;
use common::stock::*;
use common::*;
use PaymentStatus::*;

type Inv = InventoryServiceImpl<FifoStrategy>;
type Acct = AccountingServiceImpl<PaymentServiceImpl>;

const ADJ_DAY: &str = "2026-11-26";

struct Env {
    db: TestDb,
    inv: Inv,
    items: ItemServiceImpl<Inv>,
    sales: SaleServiceImpl<Inv, PaymentServiceImpl>,
    purchases: PurchaseServiceImpl<Inv, PaymentServiceImpl>,
    rets: ReturnServiceImpl<Inv, PaymentServiceImpl, Acct>,
}

fn env() -> Env {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let acct = AccountingServiceImpl::new(PaymentServiceImpl, db.clone());
    Env {
        items: ItemServiceImpl::new(db.clone(), Arc::new(db.indexer()), inv.clone()),
        sales: SaleServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        purchases: PurchaseServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        rets: ReturnServiceImpl::new(inv.clone(), PaymentServiceImpl, acct, db.clone()),
        inv,
        db,
    }
}

impl Env {
    /// Item "Pen" with opening stock `qty` @ `cost` (an OPENING lot); returns (item id, lot id).
    fn opening_item(&self, qty: f64, cost: f64) -> (i64, i64) {
        let id = self
            .items
            .create_item(CreateItemPayload { item_type: None, service_cost: None, components: None, low_stock_threshold: None,
                name: "Pen".into(), category: None, barcode: None, base_unit: "PCS".into(), mrp: None,
                default_sale_price: 15.0, initial_stock: Some(qty), initial_cost: Some(cost), bundles: None })
            .unwrap();
        (id, self.inv.get_item_lots(id).unwrap()[0].purchase_item_id)
    }
    fn buy(&self, item: i64, qty: f64, cost: f64) -> i64 {
        let p = self.purchases.create_purchase(purchase_payload(None, Paid, qty * cost, &[(item, None, qty, cost)])).unwrap().purchase_id;
        self.db.begin_read().unwrap().purchases().list_items_by_transaction_id(p).unwrap()[0].id
    }
    fn sell(&self, item: i64, qty: f64) -> (i64, i64) {
        let s = self.sales.create_sale(sale_payload(None, Paid, qty * 15.0, &[(item, None, qty, 15.0)])).unwrap().sale_id;
        let line = self.db.begin_read().unwrap().sales().list_items_by_transaction_id(s).unwrap()[0].id;
        (s, line)
    }
    fn stock(&self, item: i64) -> f64 {
        self.db.begin_read().unwrap().items().find_by_id(item).unwrap().unwrap().current_stock
    }
    fn lot(&self, id: i64) -> Option<book_keeper::core::entities::PurchaseItem> {
        self.db.begin_read().unwrap().purchases().find_purchase_item_by_id(id).unwrap()
    }
    fn adjustments(&self, item: i64) -> usize {
        self.inv.list_adjustments(Some(item), None, None).unwrap().items.len()
    }
    fn losses(&self) -> f64 {
        let rep = ReportServiceImpl::with_fixed_today(self.db.clone(), "2026-11-27");
        rep.get_pnl_report(DAY, "2026-11-27").unwrap().inventory_losses
    }
    fn check(&self) {
        assert_ledger_invariant(&self.db);
        assert_lot_flow_invariant(&self.db);
        let mut r = self.db.begin_read().unwrap();
        assert_stock_invariant(&mut *r);
    }
}

fn adj(item_id: i64, qty: f64, reason: AdjustmentReason, unit_cost: Option<f64>) -> StockAdjustmentPayload {
    StockAdjustmentPayload { adjustment_date: ADJ_DAY.into(), item_id, quantity: qty, reason, unit_cost, notes: None }
}

fn set(item_id: i64, counted: f64, mode: Option<SetStockMode>) -> SetItemStockPayload {
    SetItemStockPayload { item_id, counted_quantity: counted, adjustment_date: ADJ_DAY.into(), mode, ..Default::default() }
}

fn field<T: std::fmt::Debug>(r: Result<T, AppError>) -> (String, String) {
    match r {
        Err(AppError::ValidationError { field, message }) => (field, message),
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

// ---- Scope A ----

#[test]
fn s3_increase_allows_only_found_reasons() {
    let e = env();
    let (id, _) = e.opening_item(6.0, 10.0);
    for r in [AdjustmentReason::Damaged, AdjustmentReason::Expired, AdjustmentReason::TheftLost] {
        let (f, m) = field(e.inv.preview_adjustment(adj(id, 5.0, r, None)));
        assert_eq!((f.as_str(), m.as_str()), ("reason", "Damaged, Expired and Theft can only reduce stock."));
        assert_eq!(field(e.inv.create_adjustment(adj(id, 5.0, r, None))).0, "reason");
    }
    assert_eq!(e.stock(id), 6.0);
    e.inv.create_adjustment(adj(id, 5.0, AdjustmentReason::CountCorrection, None)).unwrap();
    e.inv.create_adjustment(adj(id, 1.0, AdjustmentReason::UnrecordedStock, None)).unwrap();
    // Reductions keep every reason.
    e.inv.create_adjustment(adj(id, -1.0, AdjustmentReason::Damaged, None)).unwrap();
    assert_eq!(e.stock(id), 11.0);
    // Found stock is never a gain: the only P&L line is the Damaged loss.
    assert_eq!(e.losses(), 10.0);
    e.check();
}

#[test]
fn s4_typed_cost_zero_is_honoured() {
    let e = env();
    let (id, _) = e.opening_item(6.0, 10.0);
    let p = e.inv.preview_adjustment(adj(id, 5.0, AdjustmentReason::UnrecordedStock, Some(0.0))).unwrap();
    assert_eq!((p.unit_cost, p.value_change), (Some(0.0), 0.0));
    e.inv.create_adjustment(adj(id, 5.0, AdjustmentReason::UnrecordedStock, Some(0.0))).unwrap();
    let lots = e.inv.get_item_lots(id).unwrap();
    assert_eq!((lots[1].source, lots[1].unit_cost, lots[1].quantity), (LotSource::Adjustment, 0.0, 5.0));

    // An item with no lots at all can take zero-cost stock through Set stock.
    let mut tx = e.db.begin_transaction().unwrap();
    seed_empty_item(&mut *tx, 50);
    tx.commit().unwrap();
    let r = e.inv.set_item_stock(SetItemStockPayload { unit_cost: Some(0.0), ..set(50, 4.0, None) }).unwrap();
    assert_eq!((r.new_stock, r.unit_cost, r.value_change, r.mode), (4.0, Some(0.0), 0.0, Some(SetStockMode::Adjustment)));
    e.check();
}

#[test]
fn s8_default_cost_prefers_the_latest_purchase_or_opening_lot() {
    let e = env();
    let (id, _) = e.opening_item(6.0, 10.0);
    e.buy(id, 4.0, 12.0);
    // A mistyped adjustment cost, newer than the purchase lot.
    e.inv.create_adjustment(adj(id, 1.0, AdjustmentReason::UnrecordedStock, Some(999.0))).unwrap();
    let p = e.inv.preview_adjustment(adj(id, 2.0, AdjustmentReason::UnrecordedStock, None)).unwrap();
    assert_eq!((p.unit_cost, p.value_change), (Some(12.0), 24.0));

    // Only adjustment lots: the latest of them.
    let mut tx = e.db.begin_transaction().unwrap();
    seed_empty_item(&mut *tx, 60);
    tx.commit().unwrap();
    e.inv.create_adjustment(adj(60, 1.0, AdjustmentReason::UnrecordedStock, Some(3.0))).unwrap();
    e.inv.create_adjustment(adj(60, 1.0, AdjustmentReason::UnrecordedStock, Some(4.0))).unwrap();
    assert_eq!(e.inv.preview_adjustment(adj(60, 1.0, AdjustmentReason::UnrecordedStock, None)).unwrap().unit_cost, Some(4.0));
    e.check();
}

#[test]
fn undo_hint_lists_undoable_writeoffs_until_undone() {
    let e = env();
    let (id, _) = e.opening_item(6.0, 10.0);
    let aid = e.inv.set_item_stock(set(id, 3.0, Some(SetStockMode::Adjustment))).unwrap().adjustment_id.unwrap();

    let p = e.inv.preview_adjustment(adj(id, 3.0, AdjustmentReason::UnrecordedStock, None)).unwrap();
    assert_eq!(p.undoable_writeoffs.len(), 1);
    let w = &p.undoable_writeoffs[0];
    assert_eq!((w.adjustment_id, w.quantity, w.reason, w.loss, w.adjustment_date.as_str()), (aid, 3.0, AdjustmentReason::CountCorrection, 30.0, ADJ_DAY));
    // Set stock up shows it too; a decrease never does; the probe writes nothing.
    let up = e.inv.preview_set_item_stock(set(id, 6.0, Some(SetStockMode::Adjustment))).unwrap();
    assert_eq!(up.undoable_writeoffs.iter().map(|w| w.adjustment_id).collect::<Vec<_>>(), vec![aid]);
    assert!(e.inv.preview_adjustment(adj(id, -1.0, AdjustmentReason::Damaged, None)).unwrap().undoable_writeoffs.is_empty());
    assert_eq!((e.stock(id), e.adjustments(id), e.losses()), (3.0, 1, 30.0));

    e.inv.undo_stock_adjustment(aid).unwrap();
    assert!(e.inv.preview_adjustment(adj(id, 3.0, AdjustmentReason::UnrecordedStock, None)).unwrap().undoable_writeoffs.is_empty());
    e.check();
}

#[test]
fn s7_fifo_rank_follows_entry_order_not_date() {
    let e = env();
    let (id, opening) = e.opening_item(2.0, 10.0);
    // Entered later but dated earlier than the opening lot's date.
    let mut payload = purchase_payload(None, Paid, 12.0, &[(id, None, 1.0, 12.0)]);
    payload.transaction_date = "2026-01-01".into();
    e.purchases.create_purchase(payload).unwrap();
    let lots = e.inv.get_item_lots(id).unwrap();
    assert_eq!(lots.iter().map(|l| l.fifo_rank).collect::<Vec<_>>(), vec![Some(1), Some(2)]);
    assert_eq!(lots[0].purchase_item_id, opening);
    e.sell(id, 2.0); // empties the opening lot, which FIFO draws first
    let lots = e.inv.get_item_lots(id).unwrap();
    assert_eq!(lots.iter().map(|l| l.fifo_rank).collect::<Vec<_>>(), vec![None, Some(1)]);
    assert_eq!(e.lot(opening).unwrap().remaining_quantity, 0.0);
}

#[test]
fn s11_reprice_says_when_history_is_recosted() {
    let e = env();
    let (id, _) = e.opening_item(1.0, 10.0);
    let lot = e.buy(id, 5.0, 10.0);
    let price = |p: f64| UpdateLotPricePayload { purchase_item_id: lot, unit_price: p };
    assert!(!e.purchases.preview_lot_purchase_price(price(11.0)).unwrap().recosts_history);
    e.sell(id, 2.0); // 1 from the opening lot, 1 from this one
    assert!(e.purchases.preview_lot_purchase_price(price(11.0)).unwrap().recosts_history);
    assert!(e.purchases.update_lot_purchase_price(price(11.0)).unwrap().recosts_history);
    e.check();
}

// ---- Scope B: Set stock corrects opening stock ----

#[test]
fn set_stock_down_and_up_as_opening_correction_books_no_loss() {
    let e = env();
    let (id, lot) = e.opening_item(6.0, 10.0);
    e.sell(id, 1.0);
    assert_eq!(e.stock(id), 5.0);

    // Both paths are open, so nothing is applied until the user picks one.
    let p = e.inv.preview_set_item_stock(set(id, 2.0, None)).unwrap();
    assert!(p.choice_required);
    assert_eq!((p.mode, p.delta, p.new_stock, p.adjustment_id), (None, -3.0, 5.0, None));
    let modes: Vec<_> = p.options.iter().map(|o| (o.mode, o.enabled)).collect();
    assert_eq!(modes, vec![(SetStockMode::OpeningCorrection, true), (SetStockMode::Adjustment, true)]);
    assert_eq!(p.options[0].label, "Opening stock was entered wrong (no loss)");
    assert_eq!(field(e.inv.set_item_stock(set(id, 2.0, None))).0, "mode");
    assert_eq!(e.stock(id), 5.0);

    // Down: the preview equals the command, the opening lot shrinks, no adjustment, no loss.
    let pv = e.inv.preview_set_item_stock(set(id, 2.0, Some(SetStockMode::OpeningCorrection))).unwrap();
    let done = e.inv.set_item_stock(set(id, 2.0, Some(SetStockMode::OpeningCorrection))).unwrap();
    assert_eq!(pv, done);
    assert_eq!((done.mode, done.reason, done.adjustment_id, done.new_stock, done.value_change), (Some(SetStockMode::OpeningCorrection), None, None, 2.0, -30.0));
    let l = e.lot(lot).unwrap();
    assert_eq!((l.quantity, l.remaining_quantity, l.unit_cost, l.total_amount), (3.0, 2.0, 10.0, 30.0));
    assert_eq!((e.adjustments(id), e.losses()), (0, 0.0));
    e.check();

    // Up: the newest opening lot grows at its own cost; a typed cost is ignored.
    let up = e.inv.set_item_stock(SetItemStockPayload { unit_cost: Some(99.0), ..set(id, 4.0, Some(SetStockMode::OpeningCorrection)) }).unwrap();
    assert_eq!((up.new_stock, up.unit_cost, up.value_change), (4.0, Some(10.0), 20.0));
    assert!(!up.options[0].takes_unit_cost && up.options[1].takes_unit_cost);
    let l = e.lot(lot).unwrap();
    assert_eq!((l.quantity, l.remaining_quantity), (5.0, 4.0));
    assert_eq!((e.adjustments(id), e.losses()), (0, 0.0));
    e.check();
}

#[test]
fn opening_correction_takes_the_newest_opening_lot_first() {
    let e = env();
    let (id, first) = e.opening_item(4.0, 10.0);
    // A second opening lot (e.g. an import) for the same item.
    let mut tx = e.db.begin_transaction().unwrap();
    let second = e.inv.create_source_lot(&mut *tx, LotSource::Opening, DAY, id, 3.0, Some(8.0), None).unwrap();
    tx.commit().unwrap();
    let r = e.inv.set_item_stock(set(id, 2.0, Some(SetStockMode::OpeningCorrection))).unwrap();
    assert_eq!((r.new_stock, r.value_change), (2.0, -24.0 - 20.0));
    assert!(e.lot(second).is_none(), "the unused newest lot is removed");
    let l = e.lot(first).unwrap();
    assert_eq!((l.quantity, l.remaining_quantity), (2.0, 2.0));
    assert_eq!((e.adjustments(id), e.losses()), (0, 0.0));
    e.check();
}

#[test]
fn opening_correction_unavailable_when_write_offs_use_the_opening_stock() {
    let e = env();
    let (id, lot) = e.opening_item(6.0, 10.0);
    e.buy(id, 10.0, 12.0);
    // A loss that drew all 6 opening units and 1 purchased one.
    let aid = e.inv.set_item_stock(set(id, 9.0, Some(SetStockMode::Adjustment))).unwrap().adjustment_id.unwrap();
    assert_eq!((e.stock(id), e.lot(lot).unwrap().remaining_quantity), (9.0, 0.0));

    // Without a mode, the only enabled path (the loss) is preselected and applied in the preview.
    let p = e.inv.preview_set_item_stock(set(id, 5.0, None)).unwrap();
    assert!(!p.choice_required);
    assert_eq!(p.mode, Some(SetStockMode::Adjustment));
    let o = &p.options[0];
    assert_eq!((o.mode, o.enabled), (SetStockMode::OpeningCorrection, false));
    let why = o.disabled_reason.clone().unwrap();
    assert!(why.contains(&format!("write-off #{aid} ({ADJ_DAY}, 7 PCS)")), "{why}");
    assert!(why.contains("undo those first"), "{why}");
    assert_eq!(p.undoable_writeoffs.iter().map(|w| w.adjustment_id).collect::<Vec<_>>(), vec![aid]);
    // Asking for it anyway is refused with that reason; nothing is written.
    assert_eq!(field(e.inv.set_item_stock(set(id, 5.0, Some(SetStockMode::OpeningCorrection)))).1, why);
    assert_eq!((e.stock(id), e.adjustments(id)), (9.0, 1));

    // The opening edit names the write-off inside "already used" and points to Undo.
    match e.inv.update_opening_lot_quantity(UpdateOpeningQuantityPayload { purchase_item_id: lot, quantity: 2.0 }) {
        Err(AppError::BillLocked { code, message }) => {
            assert_eq!(code, "LOT_BELOW_CONSUMED");
            assert!(message.contains(&format!("write-off #{aid}")) && message.contains("undo"), "{message}");
        }
        other => panic!("{other:?}"),
    }
    // After the undo the opening correction is open again.
    e.inv.undo_stock_adjustment(aid).unwrap();
    let p = e.inv.preview_set_item_stock(set(id, 13.0, None)).unwrap();
    assert!(p.choice_required && p.options.iter().all(|o| o.enabled));
    e.check();
}

#[test]
fn no_opening_lot_keeps_the_single_adjustment_path() {
    let e = env();
    let mut tx = e.db.begin_transaction().unwrap();
    seed_empty_item(&mut *tx, 70);
    tx.commit().unwrap();
    e.buy(70, 5.0, 4.0);
    let r = e.inv.set_item_stock(set(70, 3.0, None)).unwrap();
    assert_eq!((r.mode, r.reason, r.options.len(), r.value_change), (Some(SetStockMode::Adjustment), Some(AdjustmentReason::CountCorrection), 1, -8.0));
    assert_eq!(field(e.inv.preview_set_item_stock(set(70, 4.0, Some(SetStockMode::OpeningCorrection)))).0, "mode");
    let none = e.inv.preview_set_item_stock(set(70, 3.0, None)).unwrap();
    assert!(none.options.is_empty() && none.mode.is_none() && !none.choice_required);
    e.check();
}

/// DD-95 (H7): a unit restocked by a linked customer return is back in the lot's remaining but still
/// tied to its sale allocation, so it counts as used: the opening correction takes only the units
/// never sold, and the return stays deletable.
#[test]
fn opening_correction_floor_with_a_restocked_customer_return() {
    let e = env();
    let (id, lot) = e.opening_item(6.0, 10.0);
    let (sale, line) = e.sell(id, 4.0);
    let item = CustomerReturnItemPayload {
        item_id: id, sale_item_id: Some(line), quantity: 2.0, unit_price: 15.0,
        condition: StockCondition::Restockable, unit_cost_override: None,
    };
    let ret = e.rets.create_customer_return(CustomerReturnPayload {
        original_sale_id: Some(sale), party_id: None, return_date: DAY.into(), settlement_type: SettlementType::Refund,
        reason: None, notes: None, items: vec![item], replacement_items: vec![],
    }).unwrap();
    assert_eq!((e.stock(id), e.lot(lot).unwrap().remaining_quantity), (4.0, 4.0));

    // Only the 2 never-sold units are unused; the 2 restocked ones count as sold.
    let p = e.inv.preview_set_item_stock(set(id, 0.0, None)).unwrap();
    let opening = p.options.iter().find(|o| o.mode == SetStockMode::OpeningCorrection).unwrap();
    assert!(!opening.enabled);
    let why = opening.disabled_reason.clone().unwrap();
    assert!(why.contains("Only 2 PCS") && why.contains("customer return"), "{why}");
    assert!(e.inv.set_item_stock(set(id, 0.0, Some(SetStockMode::OpeningCorrection))).is_err());
    e.inv.set_item_stock(set(id, 2.0, Some(SetStockMode::OpeningCorrection))).unwrap();
    let l = e.lot(lot).unwrap();
    assert_eq!((l.quantity, l.remaining_quantity, e.stock(id)), (4.0, 2.0, 2.0));
    e.check();

    // The return still deletes: its 2 restocked units are taken back out of the lot.
    e.rets.delete_return(ret.return_id).unwrap();
    assert_eq!((e.stock(id), e.lot(lot).unwrap().remaining_quantity), (0.0, 0.0));
    e.check();
}

#[test]
fn opening_edit_refuses_a_deleted_item() {
    let e = env();
    let (id, lot) = e.opening_item(6.0, 10.0);
    e.sell(id, 1.0); // history, so the delete is a soft one (DD-79)
    e.items.delete_item(id, false).unwrap();
    match e.inv.preview_opening_lot_quantity(UpdateOpeningQuantityPayload { purchase_item_id: lot, quantity: 2.0 }) {
        Err(AppError::DeletedReference { entity, id: rid, name, .. }) => {
            assert_eq!((entity.as_str(), rid, name.as_str()), ("ITEM", id, "Pen"));
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        e.inv.update_opening_lot_quantity(UpdateOpeningQuantityPayload { purchase_item_id: lot, quantity: 2.0 }),
        Err(AppError::DeletedReference { .. })
    ));
    // Set stock on a deleted item stays refused before any path is offered.
    assert!(e.inv.preview_set_item_stock(set(id, 2.0, Some(SetStockMode::OpeningCorrection))).is_err());
    assert_eq!((e.stock(id), e.lot(lot).unwrap().quantity), (5.0, 6.0));
    e.check();
}
