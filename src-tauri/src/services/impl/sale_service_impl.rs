use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use crate::core::dtos::{
    BillEditGuard, BillPaymentPreview, BillPreview, CreateSalePayload, DeleteBillResult, Paginated,
    PreviewSaleLinePayload, PreviewSalePayload, RecordPaymentPayload, SaleLinePreview, SaleDetailView, SaleItemDetail, SaleHistoryRow,
    SaleSummaryResponse, SalesHistoryFilter, ProfitSummary,
};
use crate::core::entities::{Item, SaleItem, SaleTransaction};
use crate::core::error::AppError;
use crate::core::money::{check_entry_date_now, round2, MONEY_EPS};
use crate::services::r#impl::bill_guard::{
    evaluate_sale_guard, locked, locked_with, PARTY_CHANGE_WITH_CREDITS, PARTY_REQUIRED_WITH_PAYMENTS,
    TOTAL_BELOW_APPLIED,
};
use crate::services::r#impl::bill_math::{
    bill_preview, compute_bill_totals, delete_bill_charges, insert_bill_charges, line_adjustment,
    line_adjustment_charges, line_total, net_unit_price, resolve_bill_charges, sale_payment_split,
    validate_charge_inputs, BillFigures, BillTotals, ResolvedCharge,
};
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
    /// FR-47: signed difference between the typed total and `line_total` (0 when none).
    adjustment: f64,
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
        line_adjustment(0.0, line.entered_total)?;
    }
    Ok(())
}

/// Party must be an active customer (an edit may keep its current, since-deactivated customer);
/// every line: active item, bundle belongs to the item, snapshots, round2 totals.
fn resolve_sale(
    tx: &mut dyn TransactionContext,
    payload: &CreateSalePayload,
    keep_party: Option<i64>,
    keep_items: &HashSet<i64>,
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
                return Err(validation("party_id", format!("'{}' is deleted. Restore the party first.", party.name)));
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
        if !item.is_active && !keep_items.contains(&item.id) {
            return Err(validation("item_id", format!("'{}' is deleted. Restore the item first.", item.name)));
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
        let line_total = line_total(line.bundle_quantity, line.unit_price);
        let adjustment = line_adjustment(line_total, line.entered_total)?;
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
            adjustment,
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
    let mut charges = resolve_bill_charges(tx, TransactionType::Sale, &payload.charges)?;
    // FR-47 / DD-81: edited item totals become bill-level extra charge / discount lines.
    let adjustments: Vec<f64> = r.lines.iter().map(|l| l.adjustment).collect();
    charges.extend(line_adjustment_charges(tx, &adjustments)?);
    fig.line_adjustments = adjustments;
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
                    notes: Some("Upfront Counter Payment".to_string()),
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

        // Party and lines; charges and totals (FR-19), then the aggregated stock check across all
        // lines of one item (DD-10) and the payment cross-check (DD-02).
        let r = resolve_sale(tx, payload, None, &HashSet::new())?;
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

        // FR-45: lines of an item deleted since the sale was made may stay on the bill.
        let keep_items: HashSet<i64> =
            tx.sales().list_items_by_transaction_id(sale_id)?.into_iter().map(|l| l.item_id).collect();

        // Reverse the old version: stock, lines, charges, upfront cash. Later settlements stay.
        self.inventory_service.reverse_sale_bill_stock(tx, sale_id)?;
        tx.sales().delete_sale_items(sale_id)?;
        delete_bill_charges(tx, TransactionType::Sale, sale_id)?;
        self.payment_service
            .delete_bill_ledger(tx, TransactionType::Sale, sale_id, &[PaymentKind::Upfront])?;

        // Repost through the create rules.
        let r = resolve_sale(tx, payload, old.party_id, &keep_items)?;
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
        let (charges, totals) = price_sale(tx, payload, &r, g.applied_amount, fig)?;
        self.check_sale_stock(tx, &r)?;
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
        tx.sales().update_header_for_edit(&header)?;
        insert_bill_charges(tx, TransactionType::Sale, sale_id, &charges)?;
        self.post_sale_lines(tx, sale_id, &r.lines)?;
        if party_changed {
            self.payment_service.repoint_bill_party(tx, TransactionType::Sale, sale_id, r.party_id)?;
        }
        self.payment_service.recompute(tx, TransactionType::Sale, sale_id)?;
        self.record_upfront(tx, sale_id, r.party_id, payload, upfront)?;

        let header = tx.sales().find_transaction_by_id(sale_id)?.ok_or_else(|| AppError::NotFound {
            entity: "SaleTransaction".to_string(),
            id: sale_id.to_string(),
        })?;
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
        if !payload.bundle_quantity.is_finite() || payload.bundle_quantity < 0.0 {
            return Err(validation("bundle_quantity", "Quantity must be 0 or more"));
        }
        if !payload.unit_price.is_finite() || payload.unit_price < 0.0 {
            return Err(validation("unit_price", "unit_price must be a non-negative number"));
        }
        let total = line_total(payload.bundle_quantity, payload.unit_price);
        let adjustment = line_adjustment(total, payload.entered_total)?;
        Ok(SaleLinePreview { line_total: total, adjustment })
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
        let returned: std::collections::HashMap<i64, f64> =
            tx.sales().get_returned_quantities_for_sale(sale_id)?.into_iter().collect();
        // FR-37 / X3: misc. cost per line comes from the one shared SQL fragment.
        let line_misc: std::collections::HashMap<i64, f64> =
            tx.sales().list_line_misc_costs(sale_id)?.into_iter().collect();
        let mut details: Vec<SaleItemDetail> = Vec::new();
        for item in items {
            // DD-30 / DD-28: bills read the write-time snapshots, not the live catalog.
            let item_name = item.item_name.clone();
            let bundle_name = item.pack_label.clone();
            let allocations = tx.allocations().find_by_sale_item_id(item.id)?;
            let returned_quantity = returned.get(&item.id).copied().unwrap_or(0.0);
            let net_unit_price = net_unit_price(item.unit_price, sale.items_subtotal, sale.discounts_total);
            // FR-34: cost net of returned units; revenue net of the bill discount share (FR-19 rule).
            let misc_cost = round2(line_misc.get(&item.id).copied().unwrap_or(0.0));
            let master = tx.items().find_by_id(item.item_id)?;
            let is_service = master.as_ref().is_some_and(|i| i.item_type == ItemType::Service);
            let item_deleted = master.as_ref().is_some_and(|i| !i.is_active);
            let cost_amount = round2(
                allocations.iter().map(|a| (a.quantity_depleted - a.returned_quantity) * a.unit_cost).sum::<f64>()
                    + misc_cost,
            );
            let share = net_unit_price_factor(sale.items_subtotal, sale.discounts_total);
            let profit = round2(item.total_amount * share - cost_amount);
            details.push(SaleItemDetail {
                sale_item: item,
                item_name,
                bundle_name,
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
