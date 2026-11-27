use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use crate::core::dtos::{
    BillEditGuard, BillPaymentPreview, BillPreview, CreateSalePayload, DeleteBillResult, Paginated,
    PreviewSaleLinePayload, PreviewSalePayload, RecordPaymentPayload, SaleLinePreview, SaleDetailView, SaleItemDetail, SaleHistoryRow,
    SaleSummaryResponse, SalesHistoryFilter, ProfitSummary,
};
use crate::core::entities::{Item, SaleItem, SaleTransaction};
use crate::core::error::AppError;
use crate::core::money::{check_entry_date_now, check_not_after_follow_up, round2, MONEY_EPS};
use crate::services::r#impl::bill_guard::{
    earliest_follow_up, evaluate_sale_guard, locked, locked_with, settlements_move_check, PARTY_CHANGE_WITH_CREDITS,
    PARTY_REQUIRED_WITH_PAYMENTS, TOTAL_BELOW_APPLIED,
};
use crate::services::r#impl::party_guard::{check_due_not_raised, deleted_party};
use crate::services::r#impl::item_guard::{ensure_active_item, require_active_item};
use crate::services::r#impl::bill_math::{
    apply_bill_charges_edit, bill_preview, compute_bill_totals, delete_bill_charges, insert_bill_charges,
    net_unit_price, plan_bill_charges_edit, resolve_bill_charges, resolve_line_bundle, sale_payment_split,
    units_per_pack, validate_charge_inputs, BillFigures, BillTotals, ResolvedCharge,
};
use crate::core::constants::EPS;
use crate::services::contracts::payment_service::BillUpfront;
use crate::core::value_objects::{ItemType, PartyType, PaymentKind, PaymentStatus, TransactionType};
use crate::data::contracts::unit_of_work::{TransactionContext, UnitOfWork};
use crate::services::contracts::inventory_service::InventoryService;
use crate::services::contracts::payment_service::PaymentService;
use crate::services::contracts::sale_service::SaleService;

fn validation(field: &str, message: impl Into<String>) -> AppError {
    AppError::ValidationError { field: field.to_string(), message: message.into() }
}

#[derive(Clone)]
pub struct SaleServiceImpl<
    I: InventoryService = Arc<dyn InventoryService>,
    P: PaymentService = Arc<dyn PaymentService>,
    U: UnitOfWork = Arc<dyn UnitOfWork>,
> {
    pub inventory_service: I,
    pub payment_service: P,
    pub uow: U,
}

impl<I: InventoryService, P: PaymentService, U: UnitOfWork> SaleServiceImpl<I, P, U> {
    pub fn new(inventory_service: I, payment_service: P, uow: U) -> Self {
        Self {
            inventory_service,
            payment_service,
            uow,
        }
    }
}


/// One line after item/bundle resolution.
struct ResolvedSaleLine {
    item: Item,
    pack_label: Option<String>,
    pack_quantity: f64,
    base_qty: f64,
    line_total: f64,
    /// SERVICE lines (FR-37): component item ids with the base quantity this line consumes.
    components: Vec<(i64, f64)>,
    /// SERVICE lines: `round2(service_cost x quantity)`; `None` without a Service cost.
    service_cost_total: Option<f64>,
}

/// Party and lines of a sale payload, resolved against the database (no charges, no stock check).
struct ResolvedSale {
    party_id: Option<i64>,
    party_name: Option<String>,
    lines: Vec<ResolvedSaleLine>,
    /// Required base quantity per item, summed across lines.
    requirements: Vec<(i64, f64)>,
}

/// Payload-only validation (no database).
fn validate_sale_shape(payload: &CreateSalePayload) -> Result<(), AppError> {
    if payload.items.is_empty() {
        return Err(validation("items", "items cannot be empty"));
    }
    check_entry_date_now("transaction_date", &payload.transaction_date)?;
    validate_charge_inputs(&payload.charges)?;
    if !payload.paid_amount.is_finite() || payload.paid_amount < 0.0 {
        return Err(validation("paid_amount", "paid_amount must be a non-negative number"));
    }
    for line in &payload.items {
        if !line.bundle_quantity.is_finite() || line.bundle_quantity <= 0.0 {
            return Err(validation(
                "bundle_quantity",
                format!("Sale quantity must be positive, got {}", line.bundle_quantity),
            ));
        }
        if !line.unit_price.is_finite() || line.unit_price < 0.0 {
            return Err(validation("unit_price", "unit_price must be a non-negative number"));
        }
    }
    Ok(())
}

/// Party must be an active customer (an edit may keep its current, since-deactivated customer);
/// every line: bundle belongs to the item, snapshots, round2 totals. A deleted item is
/// `DeletedReference(ITEM)` unless `edit` (the line diff then decides, DD-90).
fn resolve_sale(
    tx: &mut dyn TransactionContext,
    payload: &CreateSalePayload,
    keep_party: Option<i64>,
    edit: bool,
) -> Result<ResolvedSale, AppError> {
    let party_name = match payload.party_id {
        Some(pid) => {
            let party = tx.parties().find_by_id(pid)?.ok_or_else(|| AppError::NotFound {
                entity: "Party".to_string(),
                id: pid.to_string(),
            })?;
            if party.party_type != PartyType::Customer {
                return Err(validation("party_id", "Sales need a customer party"));
            }
            if !party.is_active && keep_party != Some(pid) {
                return Err(deleted_party(&party));
            }
            Some(party.name)
        }
        None => None,
    };
    let mut lines = Vec::with_capacity(payload.items.len());
    let mut required: BTreeMap<i64, f64> = BTreeMap::new();
    for line in &payload.items {
        let item = tx.items().find_by_id(line.item_id)?.ok_or_else(|| AppError::NotFound {
            entity: "Item".to_string(),
            id: line.item_id.to_string(),
        })?;
        if !edit {
            ensure_active_item(&item)?;
        }
        let is_service = item.item_type == ItemType::Service;
        if is_service && line.bundle_id.is_some() {
            return Err(validation("bundle_id", format!("'{}' is a service; services are sold in single units", item.name)));
        }
        let (multiplier, pack_label) = match line.bundle_id {
            Some(bundle_id) => {
                let bundle = tx.bundles().find_by_id(bundle_id)?.ok_or_else(|| AppError::NotFound {
                    entity: "ItemBundle".to_string(),
                    id: bundle_id.to_string(),
                })?;
                if bundle.item_id != item.id {
                    return Err(validation(
                        "bundle_id",
                        format!("Bundle {bundle_id} does not belong to item {}", item.id),
                    ));
                }
                if bundle.units_per_bundle <= 0.0 || bundle.units_per_bundle.is_nan() {
                    return Err(validation("bundle_id", "Bundle multiplier must be positive"));
                }
                (bundle.units_per_bundle, Some(bundle.bundle_name))
            }
            None => (1.0, None),
        };
        let base_qty = line.bundle_quantity * multiplier;
        let line_total = round2(line.bundle_quantity * line.unit_price);
        let mut components: Vec<(i64, f64)> = Vec::new();
        let mut service_cost_total = None;
        if is_service {
            // FR-37: the line draws its components; the service itself holds no stock.
            for c in tx.service_components().list_for_service(item.id)? {
                let qty = base_qty * c.quantity_per_unit;
                components.push((c.component_item_id, qty));
                *required.entry(c.component_item_id).or_insert(0.0) += qty;
            }
            service_cost_total = item.service_cost.map(|c| round2(c * base_qty));
        } else {
            *required.entry(item.id).or_insert(0.0) += base_qty;
        }
        lines.push(ResolvedSaleLine {
            item,
            pack_label,
            pack_quantity: line.bundle_quantity,
            base_qty,
            line_total,
            components,
            service_cost_total,
        });
    }
    Ok(ResolvedSale {
        party_id: payload.party_id,
        party_name,
        lines,
        requirements: required.into_iter().collect(),
    })
}

/// Charge lines (FR-19) and the bill totals of a resolved sale.
fn price_sale(
    tx: &mut dyn TransactionContext,
    payload: &CreateSalePayload,
    r: &ResolvedSale,
    applied: f64,
    fig: &mut BillFigures,
) -> Result<(Vec<ResolvedCharge>, BillTotals), AppError> {
    let charges = resolve_bill_charges(tx, TransactionType::Sale, &payload.charges)?;
    let line_totals: Vec<f64> = r.lines.iter().map(|l| l.line_total).collect();
    let totals = compute_bill_totals(&line_totals, &charges)?;
    fig.set_totals(&line_totals, totals, applied);
    Ok((charges, totals))
}

/// Payment cross-check (DD-02) against `open` (the whole total on create; `total - applied` on an
/// edit). Returns `(upfront, change)`.
fn check_sale_payment(
    status: PaymentStatus,
    paid: f64,
    open: f64,
    party_id: Option<i64>,
) -> Result<(f64, f64), AppError> {
    let split = sale_payment_split(status, open, paid)?;
    if status != PaymentStatus::Paid && party_id.is_none() {
        return Err(validation("party_id", "Pick a customer to sell on credit (unpaid or part-paid)"));
    }
    Ok(split)
}

/// Notes of a sale's UPFRONT ledger row.
const UPFRONT_NOTE: &str = "Upfront Counter Payment";

/// Quantities equal within this are "unchanged" (DD-89).
const QTY_EPS: f64 = 1e-9;

/// What a change-only edit does to one payload line (DD-89).
enum LineOp {
    /// Unchanged: no write at all.
    Keep,
    /// Same item: price, pack or quantity changed. Only the quantity difference moves stock:
    /// `release` gives back (newest allocations first), `draw` takes the extra FIFO.
    Update { row: SaleItem, release: Vec<(i64, f64)>, draw: Vec<(i64, f64)> },
    /// Item changed: everything the line drew goes back, the new item is drawn fresh, the line id stays.
    Replace { row: SaleItem, draw: Vec<(i64, f64)> },
    /// A new line: inserted with the live snapshot, drawn fresh.
    Insert,
}

/// The line diff of a sale edit, computed before any write.
struct EditPlan {
    /// One per payload line, in payload order.
    ops: Vec<LineOp>,
    /// Line totals in payload order (a kept line uses its stored total).
    line_totals: Vec<f64>,
    /// Stored lines the payload no longer has.
    removed: Vec<SaleItem>,
    /// Stored base quantity drawn per lot item, per stored line (a service line: per component).
    stored: BTreeMap<i64, Vec<(i64, f64)>>,
}

/// A line's stored draw: Σ `quantity_depleted` per lot item, in first-allocation order.
fn stored_draws(tx: &mut dyn TransactionContext, sale_item_id: i64) -> Result<Vec<(i64, f64)>, AppError> {
    let mut out: Vec<(i64, f64)> = Vec::new();
    for a in tx.allocations().find_by_sale_item_id(sale_item_id)? {
        let lot = tx
            .purchases()
            .find_purchase_item_by_id(a.purchase_item_id)?
            .ok_or_else(|| AppError::not_found("PurchaseItem", a.purchase_item_id.to_string()))?;
        match out.iter_mut().find(|(i, _)| *i == lot.item_id) {
            Some(e) => e.1 += a.quantity_depleted,
            None => out.push((lot.item_id, a.quantity_depleted)),
        }
    }
    Ok(out)
}

/// The live draw of a resolved line: its components for a service, else the item itself.
fn live_draw(l: &ResolvedSaleLine) -> Vec<(i64, f64)> {
    if l.item.item_type == ItemType::Service {
        l.components.iter().copied().filter(|&(_, q)| q > 0.0).collect()
    } else {
        vec![(l.item.id, l.base_qty)]
    }
}

/// DD-89: matches payload lines to stored lines by `sale_item_id` and decides, per line, the
/// smallest write. A same-item line moves stock only by its quantity difference, scaled for a
/// service line by its **stored** component ratios (never the live definition); its stored
/// `service_cost_total` scales the same way. Reads only.
fn plan_sale_edit(
    tx: &mut dyn TransactionContext,
    payload: &CreateSalePayload,
    r: &ResolvedSale,
    old_lines: &[SaleItem],
) -> Result<EditPlan, AppError> {
    let mut seen: HashSet<i64> = HashSet::new();
    for line in &payload.items {
        if let Some(id) = line.sale_item_id {
            if !old_lines.iter().any(|o| o.id == id) {
                return Err(validation("sale_item_id", format!("Line {id} does not belong to this sale")));
            }
            if !seen.insert(id) {
                return Err(validation("sale_item_id", format!("Line {id} is used by more than one line")));
            }
        }
    }
    let mut stored: BTreeMap<i64, Vec<(i64, f64)>> = BTreeMap::new();
    for o in old_lines {
        stored.insert(o.id, stored_draws(tx, o.id)?);
    }
    let mut ops = Vec::with_capacity(payload.items.len());
    let mut line_totals = Vec::with_capacity(payload.items.len());
    for (line, n) in payload.items.iter().zip(&r.lines) {
        let Some(o) = line.sale_item_id.and_then(|id| old_lines.iter().find(|o| o.id == id)) else {
            ops.push(LineOp::Insert);
            line_totals.push(n.line_total);
            continue;
        };
        let unit_price = n.line_total / n.base_qty;
        if n.item.id != o.item_id {
            ops.push(LineOp::Replace {
                row: SaleItem {
                    item_id: n.item.id,
                    item_name: n.item.name.clone(),
                    pack_label: n.pack_label.clone(),
                    pack_quantity: n.pack_quantity,
                    quantity: n.base_qty,
                    unit_price,
                    total_amount: n.line_total,
                    service_cost_total: n.service_cost_total,
                    ..o.clone()
                },
                draw: live_draw(n),
            });
            line_totals.push(n.line_total);
            continue;
        }
        let delta = n.base_qty - o.quantity;
        let qty_changed = delta.abs() > QTY_EPS;
        let total_changed = (n.line_total - o.total_amount).abs() > QTY_EPS;
        // Same units per pack: the stored label stays (a renamed bundle is not a change).
        let same_pack = (units_per_pack(n.base_qty, n.pack_quantity) - units_per_pack(o.quantity, o.pack_quantity)).abs()
            <= QTY_EPS;
        let pack_changed = !same_pack || (n.pack_quantity - o.pack_quantity).abs() > QTY_EPS;
        if !qty_changed && !total_changed && !pack_changed {
            ops.push(LineOp::Keep);
            line_totals.push(o.total_amount);
            continue;
        }
        let mut row = o.clone();
        if total_changed || qty_changed {
            row.unit_price = unit_price;
            row.total_amount = n.line_total;
        }
        if !same_pack {
            row.pack_label = n.pack_label.clone();
        }
        row.pack_quantity = n.pack_quantity;
        let (mut release, mut draw) = (Vec::new(), Vec::new());
        if qty_changed {
            row.quantity = n.base_qty;
            row.service_cost_total = o.service_cost_total.map(|t| round2(t * n.base_qty / o.quantity));
            // Per stored draw item (the item itself, or each component of a service line).
            let per_unit: Vec<(i64, f64)> = if n.item.item_type == ItemType::Service {
                stored[&o.id].iter().map(|&(c, q)| (c, q / o.quantity)).collect()
            } else {
                vec![(o.item_id, 1.0)]
            };
            for (item_id, ratio) in per_unit {
                let qty = ratio * delta.abs();
                if qty <= QTY_EPS {
                    continue;
                }
                if delta < 0.0 { release.push((item_id, qty)) } else { draw.push((item_id, qty)) }
            }
        }
        ops.push(LineOp::Update { row, release, draw });
        line_totals.push(n.line_total);
    }
    let removed = old_lines.iter().filter(|o| !seen.contains(&o.id)).cloned().collect();
    Ok(EditPlan { ops, line_totals, removed, stored })
}

/// DD-90 item half (E5): a line whose item, or a stored component of its service, is deleted may
/// only stay unchanged. Changing, replacing or removing it, adding a deleted item, or drawing a
/// deleted component is `DeletedReference(ITEM)`.
fn check_edit_deleted_items(
    tx: &mut dyn TransactionContext,
    plan: &EditPlan,
    r: &ResolvedSale,
    old_lines: &[SaleItem],
) -> Result<(), AppError> {
    let mut touched_old: Vec<&SaleItem> = plan.removed.iter().collect();
    for (op, n) in plan.ops.iter().zip(&r.lines) {
        match op {
            LineOp::Keep => {}
            LineOp::Update { row, .. } => touched_old.extend(old_lines.iter().find(|o| o.id == row.id)),
            LineOp::Replace { row, draw } => {
                touched_old.extend(old_lines.iter().find(|o| o.id == row.id));
                ensure_active_item(&n.item)?;
                for &(c, _) in draw {
                    require_active_item(tx, c)?;
                }
            }
            LineOp::Insert => {
                ensure_active_item(&n.item)?;
                for (c, _) in live_draw(n) {
                    require_active_item(tx, c)?;
                }
            }
        }
    }
    for o in touched_old {
        require_active_item(tx, o.item_id)?;
        for &(c, _) in plan.stored.get(&o.id).map(Vec::as_slice).unwrap_or(&[]) {
            require_active_item(tx, c)?;
        }
    }
    Ok(())
}

fn summary(header: &SaleTransaction, change: f64) -> SaleSummaryResponse {
    SaleSummaryResponse {
        sale_id: header.id,
        transaction_code: header.transaction_code.clone(),
        total_amount: header.total_amount,
        paid_amount: header.paid_amount,
        balance_due: header.balance_due,
        change_amount: change,
        payment_status: header.payment_status,
    }
}

impl<I: InventoryService, P: PaymentService, U: UnitOfWork> SaleServiceImpl<I, P, U> {
    /// Inserts the lines and depletes stock FIFO for each.
    fn post_sale_lines(
        &self,
        tx: &mut dyn TransactionContext,
        sale_id: i64,
        lines: &[ResolvedSaleLine],
    ) -> Result<(), AppError> {
        for l in lines {
            let sale_item = SaleItem {
                id: 0,
                sale_transaction_id: sale_id,
                item_id: l.item.id,
                item_name: l.item.name.clone(),
                pack_label: l.pack_label.clone(),
                pack_quantity: l.pack_quantity,
                quantity: l.base_qty,
                unit_price: l.line_total / l.base_qty,
                total_amount: l.line_total,
                service_cost_total: l.service_cost_total,
            };
            let sale_item_id = tx.sales().create_sale_item(&sale_item)?;
            if l.item.item_type == ItemType::Service {
                // FR-37: allocations of a service line point at its components' lots.
                for &(component_id, qty) in &l.components {
                    self.inventory_service.deplete_stock_for_sale(tx, sale_item_id, component_id, qty)?;
                }
            } else {
                self.inventory_service.deplete_stock_for_sale(tx, sale_item_id, l.item.id, l.base_qty)?;
            }
        }
        Ok(())
    }

    /// Stock check for a sale (DD-10). Without service lines this is the aggregated per-item check.
    /// With service lines the lines are walked in order against a running total, so physical and
    /// service lines share the same lots and a shortfall on a service line names the component, the
    /// service and the line number (FR-37).
    fn check_sale_stock(&self, tx: &mut dyn TransactionContext, r: &ResolvedSale) -> Result<(), AppError> {
        if !r.lines.iter().any(|l| l.item.item_type == ItemType::Service) {
            return self.inventory_service.validate_stock_availability(tx, &r.requirements);
        }
        let mut running: BTreeMap<i64, f64> = BTreeMap::new();
        for (n, l) in r.lines.iter().enumerate() {
            let draws: Vec<(i64, f64)> = if l.item.item_type == ItemType::Service {
                l.components.clone()
            } else {
                vec![(l.item.id, l.base_qty)]
            };
            for (item_id, qty) in draws {
                let total = running.entry(item_id).or_insert(0.0);
                *total += qty;
                let need = *total;
                match self.inventory_service.validate_stock_availability(tx, &[(item_id, need)]) {
                    Err(AppError::InsufficientStockError { item_name, required, available, .. })
                        if l.item.item_type == ItemType::Service =>
                    {
                        return Err(validation(
                            "items",
                            format!(
                                "Not enough '{item_name}' for service '{}' (line {}): need {required}, have {available}",
                                l.item.name,
                                n + 1
                            ),
                        ));
                    }
                    other => other?,
                }
            }
        }
        Ok(())
    }

    /// DD-89 stock check of an edit, before any write: only the **net extra demand** per item
    /// (Σ extra and fresh draws − Σ released units) is checked against current stock. The lines are
    /// walked in payload order against a running total that starts at minus what the edit gives
    /// back, so a shortfall names the line that needs more (a service line: the component too).
    fn check_edit_stock(&self, tx: &mut dyn TransactionContext, plan: &EditPlan, r: &ResolvedSale) -> Result<(), AppError> {
        let mut running: BTreeMap<i64, f64> = BTreeMap::new();
        let mut give_back = |item_id: i64, qty: f64| *running.entry(item_id).or_insert(0.0) -= qty;
        for o in &plan.removed {
            for &(item_id, qty) in &plan.stored[&o.id] {
                give_back(item_id, qty);
            }
        }
        for op in &plan.ops {
            match op {
                LineOp::Replace { row, .. } => {
                    for &(item_id, qty) in &plan.stored[&row.id] {
                        give_back(item_id, qty);
                    }
                }
                LineOp::Update { release, .. } => {
                    for &(item_id, qty) in release {
                        give_back(item_id, qty);
                    }
                }
                LineOp::Keep | LineOp::Insert => {}
            }
        }
        for (n, (op, l)) in plan.ops.iter().zip(&r.lines).enumerate() {
            let draws: Vec<(i64, f64)> = match op {
                LineOp::Keep => continue,
                LineOp::Update { draw, .. } | LineOp::Replace { draw, .. } => draw.clone(),
                LineOp::Insert => live_draw(l),
            };
            for (item_id, qty) in draws {
                let total = running.entry(item_id).or_insert(0.0);
                *total += qty;
                let need = *total;
                if need <= EPS {
                    continue;
                }
                match self.inventory_service.validate_stock_availability(tx, &[(item_id, need)]) {
                    Err(AppError::InsufficientStockError { item_name, required, available, .. })
                        if l.item.item_type == ItemType::Service =>
                    {
                        return Err(validation(
                            "items",
                            format!(
                                "Not enough '{item_name}' for service '{}' (line {}): need {required}, have {available}",
                                l.item.name,
                                n + 1
                            ),
                        ));
                    }
                    other => other?,
                }
            }
        }
        Ok(())
    }

    /// Upfront cash goes through the one ledger path.
    fn record_upfront(
        &self,
        tx: &mut dyn TransactionContext,
        sale_id: i64,
        party_id: Option<i64>,
        payload: &CreateSalePayload,
        upfront: f64,
    ) -> Result<(), AppError> {
        if upfront > 0.0 {
            self.payment_service.record_payment(
                tx,
                RecordPaymentPayload {
                    party_id,
                    transaction_type: TransactionType::Sale,
                    kind: PaymentKind::Upfront,
                    return_id: None,
                    transaction_id: sale_id,
                    settlement_date: payload.transaction_date.clone(),
                    amount_paid: upfront,
                    payment_mode: payload.payment_mode.clone(),
                    notes: Some(UPFRONT_NOTE.to_string()),
                },
            )?;
        }
        Ok(())
    }
}

/// The create and edit bodies of the sale commands, run inside the caller's transaction. The real
/// commands commit afterwards; the previews drop the transaction (DD-56). `with_payment = false`
/// (preview only) skips the payment check and books no upfront cash. `fig` records the totals
/// reached, even when a later check fails.
impl<I: InventoryService, P: PaymentService, U: UnitOfWork> SaleServiceImpl<I, P, U> {
    fn create_sale_in_tx(
        &self,
        tx: &mut dyn TransactionContext,
        payload: &CreateSalePayload,
        with_payment: bool,
        fig: &mut BillFigures,
    ) -> Result<SaleSummaryResponse, AppError> {
        validate_sale_shape(payload)?;
        if payload.items.iter().any(|i| i.sale_item_id.is_some()) {
            return Err(validation("sale_item_id", "New bills can't reference existing lines"));
        }

        // Party and lines; charges and totals (FR-19), then the aggregated stock check across all
        // lines of one item (DD-10) and the payment cross-check (DD-02).
        let r = resolve_sale(tx, payload, None, false)?;
        let (charges, totals) = price_sale(tx, payload, &r, 0.0, fig)?;
        self.check_sale_stock(tx, &r)?;
        let total_amount = totals.total_amount;
        let (upfront, change) = if with_payment {
            check_sale_payment(payload.payment_status, round2(payload.paid_amount), total_amount, r.party_id)?
        } else {
            (0.0, 0.0)
        };

        // Header: paid 0 / balance = total; the ledger fixes both below.
        let sale_code = tx.sales().get_next_transaction_code()?;
        let sale_tx = SaleTransaction {
            id: 0,
            transaction_code: sale_code.clone(),
            transaction_date: payload.transaction_date.clone(),
            party_id: r.party_id,
            party_name: r.party_name.clone(),
            payment_mode: payload.payment_mode.clone(),
            payment_status: if total_amount <= MONEY_EPS { PaymentStatus::Paid } else { PaymentStatus::Unpaid },
            total_amount,
            paid_amount: 0.0,
            balance_due: total_amount,
            notes: payload.notes.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            items_subtotal: totals.items_subtotal,
            charges_total: totals.charges_total,
            discounts_total: totals.discounts_total,
            updated_at: None,
            is_opening_balance: false,
        };
        let sale_id = tx.sales().create_transaction(&sale_tx)?;
        insert_bill_charges(tx, TransactionType::Sale, sale_id, &charges)?;
        self.post_sale_lines(tx, sale_id, &r.lines)?;
        self.record_upfront(tx, sale_id, r.party_id, payload, upfront)?;

        // Respond from the header as the ledger left it.
        let header = tx.sales().find_transaction_by_id(sale_id)?.ok_or_else(|| AppError::NotFound {
            entity: "SaleTransaction".to_string(),
            id: sale_id.to_string(),
        })?;
        Ok(summary(&header, change))
    }

    fn update_sale_in_tx(
        &self,
        tx: &mut dyn TransactionContext,
        sale_id: i64,
        payload: &CreateSalePayload,
        with_payment: bool,
        fig: &mut BillFigures,
    ) -> Result<SaleSummaryResponse, AppError> {
        validate_sale_shape(payload)?;
        let old = tx
            .sales()
            .find_transaction_by_id(sale_id)?
            .ok_or_else(|| AppError::not_found("SaleTransaction", sale_id.to_string()))?;
        let g = evaluate_sale_guard(tx, sale_id)?;
        if let Some(reason) = g.reasons.iter().find(|r| r.blocks_edit) {
            return Err(locked(reason));
        }
        // DD-94: a later date must stay on or before the bill's earliest payment or return.
        if payload.transaction_date > old.transaction_date {
            let earliest = earliest_follow_up(tx, TransactionType::Sale, sale_id)?;
            check_not_after_follow_up(&payload.transaction_date, earliest, "bill")?;
        }

        // DD-89: diff the payload against the stored lines; nothing is written until every check
        // has passed, and only what changed is written.
        let r = resolve_sale(tx, payload, old.party_id, true)?;
        let old_lines = tx.sales().list_items_by_transaction_id(sale_id)?;
        let party_changed = r.party_id != old.party_id;
        if party_changed && !g.can_change_party {
            return Err(locked_with(
                PARTY_CHANGE_WITH_CREDITS,
                "Return credit was applied to this bill, so its customer cannot change",
            ));
        }
        if r.party_id.is_none() && g.applied_amount > MONEY_EPS {
            return Err(locked_with(
                PARTY_REQUIRED_WITH_PAYMENTS,
                "Payments were recorded later for this bill, so it needs a customer",
            ));
        }
        if party_changed {
            settlements_move_check(&g, r.party_name.as_deref(), payload.confirm_move_settlements)?;
        }
        let plan = plan_sale_edit(tx, payload, &r, &old_lines)?;
        // E9: stored charge rows stay unless the user changed them.
        let charge_plan = plan_bill_charges_edit(tx, TransactionType::Sale, sale_id, &payload.charges)?;
        let totals = compute_bill_totals(&plan.line_totals, &charge_plan.charges)?;
        fig.set_totals(&plan.line_totals, totals, g.applied_amount);
        check_edit_deleted_items(tx, &plan, &r, &old_lines)?;
        self.check_edit_stock(tx, &plan, &r)?;
        let total_amount = totals.total_amount;
        if total_amount + MONEY_EPS < g.applied_amount {
            return Err(locked_with(
                TOTAL_BELOW_APPLIED,
                format!("The new total \u{20b9}{total_amount:.2} is below the \u{20b9}{:.2} already applied to this bill", g.applied_amount),
            ));
        }
        let open = round2((total_amount - g.applied_amount).max(0.0));
        let (upfront, change) = if with_payment {
            check_sale_payment(payload.payment_status, round2(payload.paid_amount), open, r.party_id)?
        } else {
            (0.0, 0.0)
        };

        let header = SaleTransaction {
            transaction_date: payload.transaction_date.clone(),
            party_id: r.party_id,
            party_name: r.party_name.clone(),
            payment_mode: payload.payment_mode.clone(),
            notes: payload.notes.clone(),
            total_amount,
            items_subtotal: totals.items_subtotal,
            charges_total: totals.charges_total,
            discounts_total: totals.discounts_total,
            updated_at: Some(chrono::Utc::now().to_rfc3339()),
            is_opening_balance: false,
            ..old.clone()
        };
        // Writes. Stock first goes back (removed lines, replaced lines, quantity cuts), so other
        // lines may draw it; then the header (new lines take its date), changed rows, extra draws.
        for o in &plan.removed {
            self.inventory_service.release_whole_sale_line(tx, o.id)?;
            tx.sales().delete_sale_item(o.id)?;
        }
        for op in &plan.ops {
            match op {
                LineOp::Replace { row, .. } => {
                    self.inventory_service.release_whole_sale_line(tx, row.id)?;
                }
                LineOp::Update { row, release, .. } => {
                    for &(item_id, qty) in release {
                        self.inventory_service.release_sale_line(tx, row.id, item_id, qty)?;
                    }
                }
                LineOp::Keep | LineOp::Insert => {}
            }
        }
        tx.sales().update_header_for_edit(&header)?;
        apply_bill_charges_edit(tx, TransactionType::Sale, sale_id, &charge_plan)?;
        for (op, n) in plan.ops.iter().zip(&r.lines) {
            match op {
                LineOp::Keep => {}
                LineOp::Update { row, draw, .. } | LineOp::Replace { row, draw } => {
                    tx.sales().update_sale_item(row)?;
                    for &(item_id, qty) in draw {
                        self.inventory_service.deplete_stock_for_sale(tx, row.id, item_id, qty)?;
                    }
                }
                LineOp::Insert => self.post_sale_lines(tx, sale_id, std::slice::from_ref(n))?,
            }
        }
        if party_changed {
            self.payment_service.repoint_bill_party(tx, TransactionType::Sale, sale_id, r.party_id)?;
        }
        if with_payment {
            // DD-89: the UPFRONT row is updated in place (created only when none, deleted at 0).
            self.payment_service.set_bill_upfront(
                tx,
                BillUpfront {
                    tx_type: TransactionType::Sale,
                    tx_id: sale_id,
                    party_id: r.party_id,
                    date: payload.transaction_date.clone(),
                    amount: upfront,
                    payment_mode: payload.payment_mode.clone(),
                    notes: UPFRONT_NOTE.to_string(),
                },
            )?;
        } else {
            // Preview without a payment (rolled back): the cash is unknown, so it is left out.
            self.payment_service
                .delete_bill_ledger(tx, TransactionType::Sale, sale_id, &[PaymentKind::Upfront])?;
        }

        let header = tx.sales().find_transaction_by_id(sale_id)?.ok_or_else(|| AppError::NotFound {
            entity: "SaleTransaction".to_string(),
            id: sale_id.to_string(),
        })?;
        // E6 / DD-90: a kept deleted customer may see its due fall, never rise. A preview without
        // a payment books no upfront, so it compares totals instead.
        if !party_changed {
            let (before, after) = if with_payment {
                (old.balance_due, header.balance_due)
            } else {
                (old.total_amount, header.total_amount)
            };
            check_due_not_raised(tx, r.party_id, before, after)?;
        }
        Ok(summary(&header, change))
    }
}

impl<I: InventoryService, P: PaymentService, U: UnitOfWork> SaleService for SaleServiceImpl<I, P, U> {
    fn get_next_sale_code(&self) -> Result<String, AppError> {
        // Display-only preview: the code is assigned for real inside create_sale's write transaction.
        let mut tx = self.uow.begin_read()?;
        tx.sales().get_next_transaction_code()
    }

    fn create_sale(&self, payload: CreateSalePayload) -> Result<SaleSummaryResponse, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let result = self.create_sale_in_tx(&mut *tx, &payload, true, &mut BillFigures::default())?;
        tx.commit()?;
        Ok(result)
    }

    fn update_sale(&self, sale_id: i64, payload: CreateSalePayload) -> Result<SaleSummaryResponse, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let result = self.update_sale_in_tx(&mut *tx, sale_id, &payload, true, &mut BillFigures::default())?;
        tx.commit()?;
        Ok(result)
    }

    fn preview_sale(&self, sale_id: Option<i64>, payload: PreviewSalePayload) -> Result<BillPreview, AppError> {
        let (payload, with_payment) = payload.into_command();
        let mut tx = self.uow.begin_transaction()?;
        let mut fig = BillFigures::default();
        let outcome = match sale_id {
            Some(id) => self.update_sale_in_tx(&mut *tx, id, &payload, with_payment, &mut fig),
            None => self.create_sale_in_tx(&mut *tx, &payload, with_payment, &mut fig),
        };
        tx.rollback()?; // never committed
        bill_preview(
            fig,
            outcome.map(|s| {
                with_payment.then_some(BillPaymentPreview {
                    paid_amount: s.paid_amount,
                    balance_due: s.balance_due,
                    change_amount: s.change_amount,
                    payment_status: s.payment_status,
                })
            }),
        )
    }

    fn preview_sale_line(&self, payload: PreviewSaleLinePayload) -> Result<SaleLinePreview, AppError> {
        let qty = payload.bundle_quantity;
        if !qty.is_finite() || qty < 0.0 {
            return Err(validation("bundle_quantity", "Quantity must be 0 or more"));
        }
        if !payload.unit_price.is_finite() || payload.unit_price < 0.0 {
            return Err(validation("unit_price", "unit_price must be a non-negative number"));
        }
        let unit_price = match payload.entered_total {
            None => payload.unit_price,
            Some(t) if !t.is_finite() || t < 0.0 => {
                return Err(validation("entered_total", "Total must be 0 or more"));
            }
            Some(_) if qty <= 0.0 => {
                return Err(validation("entered_total", "Enter a quantity before the total"));
            }
            // DD-82: full precision, so round2(qty × rate) gives the typed total back.
            Some(t) => t / qty,
        };
        Ok(SaleLinePreview { line_total: round2(qty * unit_price), unit_price })
    }

    fn delete_sale(&self, sale_id: i64) -> Result<DeleteBillResult, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let g = evaluate_sale_guard(&mut *tx, sale_id)?;
        if let Some(reason) = g.reasons.iter().find(|r| r.blocks_delete) {
            return Err(locked(reason));
        }
        self.inventory_service.reverse_sale_bill_stock(&mut *tx, sale_id)?;
        tx.sales().delete_sale_items(sale_id)?;
        delete_bill_charges(&mut *tx, TransactionType::Sale, sale_id)?;
        let removed = self.payment_service.delete_bill_ledger(
            &mut *tx,
            TransactionType::Sale,
            sale_id,
            &[PaymentKind::Upfront, PaymentKind::Settlement, PaymentKind::ReturnCredit],
        )?;
        tx.sales().delete_transaction(sale_id)?;
        tx.commit()?;
        Ok(DeleteBillResult { removed_cash_amount: removed })
    }

    fn get_sale_edit_guard(&self, sale_id: i64) -> Result<BillEditGuard, AppError> {
        let mut tx = self.uow.begin_read()?;
        evaluate_sale_guard(&mut *tx, sale_id)
    }

    fn get_sales_history(
        &self,
        filter: SalesHistoryFilter,
    ) -> Result<Paginated<SaleHistoryRow>, AppError> {
        let page = filter.validate()?;
        let mut tx = self.uow.begin_read()?;
        let (rows, total) = tx.sales().list_history(&filter, page.offset(), page.limit)?;
        Ok(Paginated::new(rows, total, page.page, page.limit))
    }

    fn get_sale_details(&self, sale_id: i64) -> Result<SaleDetailView, AppError> {
        let mut tx = self.uow.begin_read()?;
        let sale = tx
            .sales()
            .find_transaction_by_id(sale_id)?
            .ok_or_else(|| AppError::NotFound {
                entity: "SaleTransaction".to_string(),
                id: sale_id.to_string(),
            })?;
        let party = if let Some(pid) = sale.party_id {
            tx.parties().find_by_id(pid)?
        } else {
            None
        };
        let items = tx.sales().list_items_by_transaction_id(sale_id)?;
        // (returned qty, returned cost_amount) per line; DD-91: the cost a line's returns credit back.
        let returned: std::collections::HashMap<i64, (f64, f64)> = tx
            .sales()
            .get_returned_quantities_for_sale(sale_id)?
            .into_iter()
            .map(|(id, q, c)| (id, (q, c)))
            .collect();
        // FR-37 / X3: misc. cost per line comes from the one shared SQL fragment.
        let line_misc: std::collections::HashMap<i64, f64> =
            tx.sales().list_line_misc_costs(sale_id)?.into_iter().collect();
        let mut details: Vec<SaleItemDetail> = Vec::new();
        for item in items {
            // DD-30 / DD-28: bills read the write-time snapshots, not the live catalog.
            let item_name = item.item_name.clone();
            let bundle_name = item.pack_label.clone();
            let allocations = tx.allocations().find_by_sale_item_id(item.id)?;
            let (returned_quantity, returned_cost) = returned.get(&item.id).copied().unwrap_or((0.0, 0.0));
            let net_unit_price = net_unit_price(item.unit_price, sale.items_subtotal, sale.discounts_total);
            // FR-34 / DD-91: gross cost less the cost its linked returns credited (the same terms as
            // `bill_profit_parts.cost_net`); revenue net of the bill discount share (FR-19 rule).
            let misc_cost = round2(line_misc.get(&item.id).copied().unwrap_or(0.0));
            let master = tx.items().find_by_id(item.item_id)?;
            let is_service = master.as_ref().is_some_and(|i| i.item_type == ItemType::Service);
            let item_deleted = master.as_ref().is_some_and(|i| !i.is_active);
            let cost_amount = round2(
                allocations.iter().map(|a| a.quantity_depleted * a.unit_cost).sum::<f64>() - returned_cost + misc_cost,
            );
            let share = net_unit_price_factor(sale.items_subtotal, sale.discounts_total);
            let profit = round2(item.total_amount * share - cost_amount);
            let bundle_id = resolve_line_bundle(
                &mut *tx,
                item.item_id,
                item.pack_label.as_deref(),
                units_per_pack(item.quantity, item.pack_quantity),
            )?;
            details.push(SaleItemDetail {
                sale_item: item,
                item_name,
                bundle_name,
                bundle_id,
                returned_quantity,
                allocations,
                net_unit_price,
                cost_amount,
                misc_cost,
                is_service,
                item_deleted,
                profit,
            });
        }
        let parts = tx.sales().bill_profit_parts(sale_id)?;
        let return_adjustment = -round2(parts.refunds + parts.scrap_cost + parts.replacement_cost);
        let profit_summary = ProfitSummary {
            lines_profit: round2(parts.total - sale.charges_total - return_adjustment),
            charges: sale.charges_total,
            return_adjustment,
            total: parts.total,
        };
        let settlements = tx.settlements().list_by_transaction(TransactionType::Sale, sale_id)?;
        let charges = tx.bill_charges().list_by_transaction(TransactionType::Sale, sale_id)?;
        Ok(SaleDetailView {
            sale,
            party,
            items: details,
            settlements,
            charges,
            profit_summary,
        })
    }
}

/// Share of a line's total kept after the bill discount (`net_unit_price(1.0, ..)`).
fn net_unit_price_factor(items_subtotal: f64, discounts_total: f64) -> f64 {
    net_unit_price(1.0, items_subtotal, discounts_total)
}
