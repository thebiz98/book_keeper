// tests/lot_draw_test.rs
//! FR-31 / DD-60: `preview_lot_draw` on real SQLite: multi-lot draw after prior quantity, shortfall,
//! read-only, SERVICE item, unknown item, invalid quantities.

mod common;

use book_keeper::core::dtos::PriorLine;
use book_keeper::core::error::AppError;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::inventory_service::InventoryService;
use common::ledger::*;
use common::stock::*;
use common::*;

type Inv = InventoryServiceImpl<FifoStrategy>;

/// Earlier cart lines of item 1 totalling `q` base units (none when 0).
fn prior(q: f64) -> Vec<PriorLine> {
    if q == 0.0 { vec![] } else { vec![PriorLine { item_id: 1, quantity: q }] }
}

/// Item 1 with lot 1 (5 @ 40) and lot 2 (20 @ 42.5).
fn env() -> (TestDb, Inv) {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    seed_stocked_item(&db, &inv, 1, 5.0, 40.0, 60.0);
    let mut tx = db.begin_transaction().unwrap();
    seed_purchase_lot(&inv, &mut *tx, 2, 1, 20.0, 42.5);
    tx.commit().unwrap();
    (db, inv)
}

#[test]
fn draw_skips_prior_quantity_and_spans_lots() {
    let (_db, inv) = env();
    let d = inv.preview_lot_draw(1, 8.0, &prior(3.0)).unwrap();
    assert_eq!(d.lots.len(), 2);
    assert_eq!((d.lots[0].purchase_item_id, d.lots[1].purchase_item_id), (1, 2));
    assert_eq!((d.lots[0].remaining_quantity, d.lots[1].remaining_quantity), (5.0, 20.0));
    // prior 3 uses lot 1 first; the 8 units: 2 from lot 1, 6 from lot 2
    assert_eq!((d.lots[0].draw_quantity, d.lots[1].draw_quantity), (2.0, 6.0));
    assert_eq!(d.draw_cost, 2.0 * 40.0 + 6.0 * 42.5);
    assert_eq!((d.available, d.shortfall), (25.0, 0.0));
    assert!(!d.lots[0].transaction_code.is_empty());
}

#[test]
fn prior_beyond_first_lot_moves_the_draw_to_the_next() {
    let (_db, inv) = env();
    let d = inv.preview_lot_draw(1, 4.0, &prior(7.0)).unwrap();
    assert_eq!((d.lots[0].draw_quantity, d.lots[1].draw_quantity), (0.0, 4.0));
    assert_eq!(d.draw_cost, 4.0 * 42.5);
}

#[test]
fn shortfall_when_quantity_exceeds_stock() {
    let (_db, inv) = env();
    let d = inv.preview_lot_draw(1, 10.0, &prior(20.0)).unwrap();
    assert_eq!(d.shortfall, 5.0);
    assert_eq!(d.lots[1].draw_quantity, 5.0);
    assert_eq!(d.draw_cost, 5.0 * 42.5);
}

#[test]
fn zero_quantity_lists_lots_with_no_draw() {
    let (_db, inv) = env();
    let d = inv.preview_lot_draw(1, 0.0, &prior(0.0)).unwrap();
    assert_eq!(d.lots.len(), 2);
    assert!(d.lots.iter().all(|l| l.draw_quantity == 0.0));
    assert_eq!((d.draw_cost, d.shortfall), (0.0, 0.0));
}

#[test]
fn exhausted_lots_are_not_listed() {
    let (db, inv) = env();
    let mut tx = db.begin_transaction().unwrap();
    seed_sale(&mut *tx, 50, 1);
    inv.deplete_stock_for_sale(&mut *tx, 50, 1, 5.0).unwrap();
    tx.commit().unwrap();
    let d = inv.preview_lot_draw(1, 1.0, &prior(0.0)).unwrap();
    assert_eq!(d.lots.len(), 1);
    assert_eq!(d.lots[0].purchase_item_id, 2);
}

#[test]
fn preview_writes_nothing() {
    let (db, inv) = env();
    let snap = |db: &TestDb| {
        let c = db.pool.get().unwrap();
        let q = |sql: &str| c.query_row(sql, [], |r| r.get::<_, f64>(0)).unwrap();
        (
            q("SELECT SUM(remaining_quantity) FROM purchase_items"),
            q("SELECT current_stock FROM items WHERE id = 1"),
            q("SELECT COUNT(*) FROM sale_stock_allocations"),
            q("SELECT COUNT(*) FROM purchase_transactions"),
            q("SELECT COUNT(*) FROM code_sequences"),
        )
    };
    let before = snap(&db);
    inv.preview_lot_draw(1, 8.0, &prior(3.0)).unwrap();
    assert_eq!(before, snap(&db));
}

#[test]
fn service_item_without_components_has_no_lots() {
    let (db, inv) = env();
    db.pool.get().unwrap().execute("UPDATE items SET item_type = 'SERVICE' WHERE id = 1", []).unwrap();
    let d = inv.preview_lot_draw(1, 3.0, &prior(0.0)).unwrap();
    assert!(d.lots.is_empty());
    assert!(d.components.is_empty());
    assert_eq!((d.draw_cost, d.shortfall, d.line_cost), (0.0, 0.0, 0.0));
}

#[test]
fn unknown_item_is_not_found() {
    let (_db, inv) = env();
    assert!(matches!(inv.preview_lot_draw(999, 1.0, &prior(0.0)), Err(AppError::NotFound { .. })));
}

#[test]
fn invalid_quantities_are_rejected() {
    let (_db, inv) = env();
    for (q, p, field) in [(-1.0, 0.0, "quantity"), (f64::NAN, 0.0, "quantity"), (1.0, -2.0, "prior_lines")] {
        match inv.preview_lot_draw(1, q, &prior(p)) {
            Err(AppError::ValidationError { field: f, .. }) => assert_eq!(f, field),
            other => panic!("expected ValidationError, got {other:?}"),
        }
    }
}
