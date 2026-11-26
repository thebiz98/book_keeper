use std::collections::HashSet;
use std::sync::Arc;
use crate::core::dtos::{
    BillEditGuard, CreatePurchasePayload, DeleteBillResult, Paginated, PurchaseDetailView, PurchaseHistoryRow,
    PurchaseHistoryFilter, PurchaseItemDetail, PurchaseSummaryResponse, RecordPaymentPayload, LotPriceBillEffect,
    LotPriceUpdateResult, UpdateLotPricePayload, BillPaymentPreview, BillPreview, PreviewPurchasePayload,
};
use crate::core::entities::{Item, PurchaseItem, PurchaseTransaction};
use crate::core::error::AppError;
use crate::core::money::{check_entry_date_now, round2, MONEY_EPS};
use crate::services::r#impl::bill_guard::{
    evaluate_purchase_guard, locked, locked_with, PARTY_CHANGE_WITH_CREDITS, PARTY_REQUIRED_WITH_PAYMENTS,
    TOTAL_BELOW_APPLIED,
};
use crate::services::r#impl::bill_math::{
    bill_preview, compute_bill_totals, delete_bill_charges, lot_price_upfront, insert_bill_charges, purchase_upfront, resolve_bill_charges,
    validate_charge_inputs, BillFigures, BillTotals, ResolvedCharge,
};
use crate::core::value_objects::{LotSource, PartyType, PaymentKind, PaymentStatus, TransactionType};
use crate::data::contracts::unit_of_work::{TransactionContext, UnitOfWork};
use crate::services::contracts::inventory_service::{InventoryService, LotEdit};
use crate::services::contracts::payment_service::PaymentService;
use crate::services::contracts::purchase_service::PurchaseService;

fn validation(field: &str, message: impl Into<String>) -> AppError {
    AppError::ValidationError { field: field.to_string(), message: message.into() }
}

#[derive(Clone)]
pub struct PurchaseServiceImpl<
    I: InventoryService = Arc<dyn InventoryService>,
    P: PaymentService = Arc<dyn PaymentService>,
    U: UnitOfWork = Arc<dyn UnitOfWork>,
> {
    pub inventory_service: I,
    pub payment_service: P,
    pub uow: U,
}

impl<I: InventoryService, P: PaymentService, U: UnitOfWork> PurchaseServiceImpl<I, P, U> {
    pub fn new(inventory_service: I, payment_service: P, uow: U) -> Self {
        Self {
            inventory_service,
            payment_service,
            uow,
        }
    }
}


struct ResolvedPurchaseLine {
    item: Item,
    pack_label: Option<String>,
    pack_quantity: f64,
    base_qty: f64,
    line_total: f64,
    /// Edit only: the existing lot this line keeps.
    purchase_item_id: Option<i64>,
}

struct ResolvedPurchase {
    party_id: Option<i64>,
    party_name: Option<String>,
    lines: Vec<ResolvedPurchaseLine>,
}

/// Payload-only validation (no database).
fn validate_purchase_shape(payload: &CreatePurchasePayload) -> Result<(), AppError> {
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
                format!("Inward quantity must be positive, got {}", line.bundle_quantity),
            ));
        }
        if !line.unit_cost.is_finite() || line.unit_cost < 0.0 {
            return Err(validation("unit_cost", "unit_cost must be a non-negative number"));
        }
    }
    Ok(())
}

fn clean_ref(r: Option<&str>) -> Option<String> {
    r.map(|r| r.trim().to_string()).filter(|r| !r.is_empty())
}

/// Vendor must be an active vendor (an edit may keep its current, since-deactivated vendor); every
/// line: active item, bundle belongs to the item, cost normalised to base units.
fn resolve_purchase(
    tx: &mut dyn TransactionContext,
    payload: &CreatePurchasePayload,
    keep_party: Option<i64>,
    keep_items: &HashSet<i64>,
) -> Result<ResolvedPurchase, AppError> {
    let party_name = match payload.party_id {
        Some(pid) => {
            let party = tx.parties().find_by_id(pid)?.ok_or_else(|| AppError::NotFound {
                entity: "Party".to_string(),
                id: pid.to_string(),
            })?;
            if party.party_type != PartyType::Vendor {
                return Err(validation("party_id", "Purchases need a vendor party"));
            }
            if !party.is_active && keep_party != Some(pid) {
                return Err(validation("party_id", format!("'{}' is deleted. Restore the party first.", party.name)));
            }
            Some(party.name)
        }
        None => None,
    };
    let mut lines = Vec::with_capacity(payload.items.len());
    for line in &payload.items {
        let item = tx.items().find_by_id(line.item_id)?.ok_or_else(|| AppError::NotFound {
            entity: "Item".to_string(),
            id: line.item_id.to_string(),
        })?;
        if !item.is_active && !keep_items.contains(&item.id) {
            return Err(validation("item_id", format!("'{}' is deleted. Restore the item first.", item.name)));
        }
        if item.item_type == crate::core::value_objects::ItemType::Service {
            return Err(validation("item_id", format!("'{}' is a service item and cannot be purchased", item.name)));
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
        lines.push(ResolvedPurchaseLine {
            item,
            pack_label,
            pack_quantity: line.bundle_quantity,
            base_qty: line.bundle_quantity * multiplier,
            line_total: round2(line.bundle_quantity * line.unit_cost),
            purchase_item_id: line.purchase_item_id,
        });
    }
    Ok(ResolvedPurchase { party_id: payload.party_id, party_name, lines })
}

fn price_purchase(
    tx: &mut dyn TransactionContext,
    payload: &CreatePurchasePayload,
    r: &ResolvedPurchase,
    applied: f64,
    fig: &mut BillFigures,
) -> Result<(Vec<ResolvedCharge>, BillTotals), AppError> {
    let charges = resolve_bill_charges(tx, TransactionType::Purchase, &payload.charges)?;
    let line_totals: Vec<f64> = r.lines.iter().map(|l| l.line_total).collect();
    let totals = compute_bill_totals(&line_totals, &charges)?;
    fig.set_totals(&line_totals, totals, applied);
    Ok((charges, totals))
}

/// Payment cross-check (DD-02) against `open` (the whole total on create; `total - applied` on an
/// edit). Returns the upfront amount.
fn check_purchase_payment(
    status: PaymentStatus,
    paid: f64,
    open: f64,
    party_id: Option<i64>,
) -> Result<f64, AppError> {
    let upfront = purchase_upfront(status, open, paid)?;
    if status != PaymentStatus::Paid && party_id.is_none() {
        return Err(validation("party_id", "Pick a vendor to record an unpaid or part-paid purchase"));
    }
    Ok(upfront)
}

fn new_lot(purchase_id: i64, l: &ResolvedPurchaseLine) -> PurchaseItem {
    PurchaseItem {
        id: 0,
        purchase_transaction_id: purchase_id,
        item_id: l.item.id,
        item_name: l.item.name.clone(),
        pack_label: l.pack_label.clone(),
        pack_quantity: l.pack_quantity,
        quantity: l.base_qty,
        unit_cost: l.line_total / l.base_qty,
        total_amount: l.line_total,
        remaining_quantity: l.base_qty,
    }
}

impl<I: InventoryService, P: PaymentService, U: UnitOfWork> PurchaseServiceImpl<I, P, U> {
    /// Upfront payment through the one ledger path.
    fn record_upfront(
        &self,
        tx: &mut dyn TransactionContext,
        purchase_id: i64,
        party_id: Option<i64>,
        payload: &CreatePurchasePayload,
        upfront: f64,
    ) -> Result<(), AppError> {
        if upfront > 0.0 {
            self.payment_service.record_payment(
                tx,
                RecordPaymentPayload {
                    party_id,
                    transaction_type: TransactionType::Purchase,
                    kind: PaymentKind::Upfront,
                    return_id: None,
                    transaction_id: purchase_id,
                    settlement_date: payload.transaction_date.clone(),
                    amount_paid: upfront,
                    payment_mode: payload.payment_mode.clone(),
                    notes: Some("Supplier Inward Payment".to_string()),
                },
            )?;
        }
        Ok(())
    }
}

/// The create / edit / lot-price bodies of the purchase commands, run inside the caller's
/// transaction. The real commands commit afterwards; the previews drop the transaction (DD-56).
/// `with_payment = false` (preview only) skips the payment check and books no upfront cash. `fig`
/// records the totals reached, even when a later check fails.
impl<I: InventoryService, P: PaymentService, U: UnitOfWork> PurchaseServiceImpl<I, P, U> {
    fn create_purchase_in_tx(
        &self,
        tx: &mut dyn TransactionContext,
        payload: &CreatePurchasePayload,
        with_payment: bool,
        fig: &mut BillFigures,
    ) -> Result<(PurchaseSummaryResponse, PurchaseTransaction), AppError> {
        validate_purchase_shape(payload)?;
        if payload.items.iter().any(|i| i.purchase_item_id.is_some()) {
            return Err(validation("purchase_item_id", "purchase_item_id is only valid when editing a purchase"));
        }

        let r = resolve_purchase(tx, payload, None, &HashSet::new())?;
        // Charges (FR-19), totals and the payment cross-check (DD-02).
        let (charges, totals) = price_purchase(tx, payload, &r, 0.0, fig)?;
        let total_amount = totals.total_amount;
        let upfront = if with_payment {
            check_purchase_payment(payload.payment_status, round2(payload.paid_amount), total_amount, r.party_id)?
        } else {
            0.0
        };

        // Header: paid 0 / balance = total; the ledger fixes both below.
        let pur_code = tx.purchases().get_next_transaction_code(LotSource::Purchase)?;
        let purchase_tx = PurchaseTransaction {
            id: 0,
            transaction_code: pur_code.clone(),
            source: LotSource::Purchase,
            supplier_invoice_ref: clean_ref(payload.supplier_invoice_ref.as_deref()),
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
        let purchase_id = tx.purchases().create_transaction(&purchase_tx)?;
        insert_bill_charges(tx, TransactionType::Purchase, purchase_id, &charges)?;

        // Lots and stock cache. Lot unit_cost = line_total / base_qty, unaffected by bill-level
        // charges and discounts (DD-38).
        for l in &r.lines {
            self.inventory_service.add_stock_for_purchase(tx, &new_lot(purchase_id, l))?;
        }
        self.record_upfront(tx, purchase_id, r.party_id, payload, upfront)?;
        let header = tx
            .purchases()
            .find_transaction_by_id(purchase_id)?
            .ok_or_else(|| AppError::not_found("PurchaseTransaction", purchase_id.to_string()))?;
        Ok((PurchaseSummaryResponse { purchase_id, transaction_code: pur_code }, header))
    }

    fn update_purchase_in_tx(
        &self,
        tx: &mut dyn TransactionContext,
        purchase_id: i64,
        payload: &CreatePurchasePayload,
        with_payment: bool,
        fig: &mut BillFigures,
    ) -> Result<(PurchaseSummaryResponse, PurchaseTransaction), AppError> {
        validate_purchase_shape(payload)?;
        let old = tx
            .purchases()
            .find_transaction_by_id(purchase_id)?
            .ok_or_else(|| AppError::not_found("PurchaseTransaction", purchase_id.to_string()))?;
        let g = evaluate_purchase_guard(tx, purchase_id)?;
        if let Some(reason) = g.reasons.iter().find(|r| r.blocks_edit) {
            return Err(locked(reason));
        }

        // Reverse what is not kept in place: charges and the upfront cash. Later settlements stay.
        delete_bill_charges(tx, TransactionType::Purchase, purchase_id)?;
        self.payment_service
            .delete_bill_ledger(tx, TransactionType::Purchase, purchase_id, &[PaymentKind::Upfront])?;

        // FR-45: lines of an item deleted since the purchase was made may stay on the bill.
        let keep_items: HashSet<i64> =
            tx.purchases().list_items_by_transaction_id(purchase_id)?.into_iter().map(|l| l.item_id).collect();
        let r = resolve_purchase(tx, payload, old.party_id, &keep_items)?;
        let party_changed = r.party_id != old.party_id;
        if party_changed && !g.can_change_party {
            return Err(locked_with(
                PARTY_CHANGE_WITH_CREDITS,
                "Return credit was applied to this bill, so its vendor cannot change",
            ));
        }
        if r.party_id.is_none() && g.applied_amount > MONEY_EPS {
            return Err(locked_with(
                PARTY_REQUIRED_WITH_PAYMENTS,
                "Payments were recorded later for this bill, so it needs a vendor",
            ));
        }
        let (charges, totals) = price_purchase(tx, payload, &r, g.applied_amount, fig)?;
        let total_amount = totals.total_amount;
        if total_amount + MONEY_EPS < g.applied_amount {
            return Err(locked_with(
                TOTAL_BELOW_APPLIED,
                format!("The new total \u{20b9}{total_amount:.2} is below the \u{20b9}{:.2} already applied to this bill", g.applied_amount),
            ));
        }
        let open = round2((total_amount - g.applied_amount).max(0.0));
        let upfront = if with_payment {
            check_purchase_payment(payload.payment_status, round2(payload.paid_amount), open, r.party_id)?
        } else {
            0.0
        };

        // Lots: kept lines update in place (ids kept: allocations point at them), the rest are
        // removed (if unreferenced) or created.
        let old_lots = tx.purchases().list_items_by_transaction_id(purchase_id)?;
        let mut kept: Vec<i64> = Vec::new();
        for l in &r.lines {
            if let Some(lid) = l.purchase_item_id {
                if !old_lots.iter().any(|o| o.id == lid) {
                    return Err(validation("purchase_item_id", format!("Lot {lid} does not belong to this purchase")));
                }
                if kept.contains(&lid) {
                    return Err(validation("purchase_item_id", format!("Lot {lid} is used by more than one line")));
                }
                kept.push(lid);
            }
        }
        for lot in old_lots.iter().filter(|o| !kept.contains(&o.id)) {
            self.inventory_service.remove_purchase_lot(tx, lot.id)?;
        }
        for l in &r.lines {
            match l.purchase_item_id {
                Some(lid) => self.inventory_service.edit_purchase_lot(
                    tx,
                    lid,
                    &LotEdit {
                        item_id: l.item.id,
                        item_name: l.item.name.clone(),
                        pack_label: l.pack_label.clone(),
                        pack_quantity: l.pack_quantity,
                        quantity: l.base_qty,
                        unit_cost: l.line_total / l.base_qty,
                        total_amount: l.line_total,
                    },
                )?,
                None => self.inventory_service.add_stock_for_purchase(tx, &new_lot(purchase_id, l))?,
            }
        }

        let header = PurchaseTransaction {
            supplier_invoice_ref: clean_ref(payload.supplier_invoice_ref.as_deref()),
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
        tx.purchases().update_header_for_edit(&header)?;
        insert_bill_charges(tx, TransactionType::Purchase, purchase_id, &charges)?;
        if party_changed {
            self.payment_service.repoint_bill_party(tx, TransactionType::Purchase, purchase_id, r.party_id)?;
        }
        self.payment_service.recompute(tx, TransactionType::Purchase, purchase_id)?;
        self.record_upfront(tx, purchase_id, r.party_id, payload, upfront)?;
        let after = tx
            .purchases()
            .find_transaction_by_id(purchase_id)?
            .ok_or_else(|| AppError::not_found("PurchaseTransaction", purchase_id.to_string()))?;
        Ok((PurchaseSummaryResponse { purchase_id, transaction_code: old.transaction_code }, after))
    }

    fn update_lot_price_in_tx(&self, tx: &mut dyn TransactionContext, payload: &UpdateLotPricePayload) -> Result<LotPriceUpdateResult, AppError> {
        if !payload.unit_price.is_finite() || payload.unit_price < 0.0 {
            return Err(validation("unit_price", "unit_price must be a non-negative number"));
        }
        let lot = tx
            .purchases()
            .find_purchase_item_by_id(payload.purchase_item_id)?
            .ok_or_else(|| AppError::not_found("PurchaseItem", payload.purchase_item_id.to_string()))?;
        let header = tx
            .purchases()
            .find_transaction_by_id(lot.purchase_transaction_id)?
            .ok_or_else(|| AppError::not_found("PurchaseTransaction", lot.purchase_transaction_id.to_string()))?;
        if header.source == LotSource::Return {
            return Err(validation("purchase_item_id", "Lots created by customer returns cannot be re-priced"));
        }
        let new_total_line = round2(lot.pack_quantity * payload.unit_price);
        let new_cost = new_total_line / lot.quantity;
        if (new_cost - lot.unit_cost).abs() <= 1e-9 {
            let bill = (header.source == LotSource::Purchase).then(|| LotPriceBillEffect {
                purchase_id: header.id,
                transaction_code: header.transaction_code.clone(),
                old_total: header.total_amount,
                new_total: header.total_amount,
                old_paid: header.paid_amount,
                new_paid: header.paid_amount,
                balance_due: header.balance_due,
                payment_status: header.payment_status,
            });
            return Ok(LotPriceUpdateResult {
                purchase_item_id: lot.id,
                old_unit_cost: lot.unit_cost,
                new_unit_cost: lot.unit_cost,
                bill,
            });
        }
        let edit = LotEdit {
            item_id: lot.item_id,
            item_name: lot.item_name.clone(),
            pack_label: lot.pack_label.clone(),
            pack_quantity: lot.pack_quantity,
            quantity: lot.quantity,
            unit_cost: new_cost,
            total_amount: new_total_line,
        };
        if header.source != LotSource::Purchase {
            // OPENING / ADJUSTMENT: re-cost only (the vendor-return lock lives in edit_purchase_lot).
            self.inventory_service.edit_purchase_lot(tx, lot.id, &edit)?;
            if header.source == LotSource::Adjustment {
                let adj_id = header
                    .notes
                    .as_deref()
                    .and_then(|n| n.strip_prefix("Stock adjustment #"))
                    .and_then(|n| n.split(':').next())
                    .and_then(|n| n.trim().parse::<i64>().ok());
                if let Some(id) = adj_id {
                    tx.adjustments().update_unit_cost(id, new_cost)?;
                }
            }
            return Ok(LotPriceUpdateResult {
                purchase_item_id: lot.id,
                old_unit_cost: lot.unit_cost,
                new_unit_cost: new_cost,
                bill: None,
            });
        }
        let g = evaluate_purchase_guard(tx, header.id)?;
        if let Some(reason) = g.reasons.iter().find(|r| r.blocks_edit) {
            return Err(locked(reason));
        }
        // Stored line totals with this lot's new total, plus the existing charges (never rebuilt from rates).
        let line_totals: Vec<f64> = tx
            .purchases()
            .list_items_by_transaction_id(header.id)?
            .iter()
            .map(|l| if l.id == lot.id { new_total_line } else { l.total_amount })
            .collect();
        let charges: Vec<ResolvedCharge> = tx
            .bill_charges()
            .list_by_transaction(TransactionType::Purchase, header.id)?
            .into_iter()
            .map(|c| ResolvedCharge {
                charge_type_id: c.charge_type_id,
                charge_name: c.charge_name,
                effect: c.effect,
                amount: c.amount,
            })
            .collect();
        let totals = compute_bill_totals(&line_totals, &charges)?;
        let upfront_new =
            lot_price_upfront(totals.total_amount, g.applied_amount, g.upfront_amount, header.balance_due)?;
        self.inventory_service.edit_purchase_lot(tx, lot.id, &edit)?;
        let updated = PurchaseTransaction {
            total_amount: totals.total_amount,
            items_subtotal: totals.items_subtotal,
            charges_total: totals.charges_total,
            discounts_total: totals.discounts_total,
            updated_at: Some(chrono::Utc::now().to_rfc3339()),
            is_opening_balance: false,
            ..header.clone()
        };
        tx.purchases().update_header_for_edit(&updated)?;
        self.payment_service
            .delete_bill_ledger(tx, TransactionType::Purchase, header.id, &[PaymentKind::Upfront])?;
        if upfront_new > MONEY_EPS {
            self.payment_service.record_payment(
                tx,
                RecordPaymentPayload {
                    party_id: header.party_id,
                    transaction_type: TransactionType::Purchase,
                    kind: PaymentKind::Upfront,
                    return_id: None,
                    transaction_id: header.id,
                    settlement_date: header.transaction_date.clone(),
                    amount_paid: upfront_new,
                    payment_mode: header.payment_mode.clone(),
                    notes: Some("Supplier Inward Payment".to_string()),
                },
            )?;
        }
        self.payment_service.recompute(tx, TransactionType::Purchase, header.id)?;
        let after = tx
            .purchases()
            .find_transaction_by_id(header.id)?
            .ok_or_else(|| AppError::not_found("PurchaseTransaction", header.id.to_string()))?;
        Ok(LotPriceUpdateResult {
            purchase_item_id: lot.id,
            old_unit_cost: lot.unit_cost,
            new_unit_cost: new_cost,
            bill: Some(LotPriceBillEffect {
                purchase_id: header.id,
                transaction_code: header.transaction_code,
                old_total: header.total_amount,
                new_total: after.total_amount,
                old_paid: header.paid_amount,
                new_paid: after.paid_amount,
                balance_due: after.balance_due,
                payment_status: after.payment_status,
            }),
        })
    }

}

impl<I: InventoryService, P: PaymentService, U: UnitOfWork> PurchaseService for PurchaseServiceImpl<I, P, U> {
    fn get_next_purchase_code(&self) -> Result<String, AppError> {
        // Display-only preview: the code is assigned for real inside create_purchase's write transaction.
        let mut tx = self.uow.begin_read()?;
        tx.purchases().get_next_transaction_code(LotSource::Purchase)
    }

    fn create_purchase(&self, payload: CreatePurchasePayload) -> Result<PurchaseSummaryResponse, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let (summary, _) = self.create_purchase_in_tx(&mut *tx, &payload, true, &mut BillFigures::default())?;
        tx.commit()?;
        Ok(summary)
    }

    fn update_purchase(
        &self,
        purchase_id: i64,
        payload: CreatePurchasePayload,
    ) -> Result<PurchaseSummaryResponse, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let (summary, _) =
            self.update_purchase_in_tx(&mut *tx, purchase_id, &payload, true, &mut BillFigures::default())?;
        tx.commit()?;
        Ok(summary)
    }

    fn preview_purchase(
        &self,
        purchase_id: Option<i64>,
        payload: PreviewPurchasePayload,
    ) -> Result<BillPreview, AppError> {
        let (payload, with_payment) = payload.into_command();
        let mut tx = self.uow.begin_transaction()?;
        let mut fig = BillFigures::default();
        let outcome = match purchase_id {
            Some(id) => self.update_purchase_in_tx(&mut *tx, id, &payload, with_payment, &mut fig),
            None => self.create_purchase_in_tx(&mut *tx, &payload, with_payment, &mut fig),
        };
        tx.rollback()?; // never committed
        bill_preview(
            fig,
            outcome.map(|(_, h)| {
                with_payment.then_some(BillPaymentPreview {
                    paid_amount: h.paid_amount,
                    balance_due: h.balance_due,
                    change_amount: 0.0,
                    payment_status: h.payment_status,
                })
            }),
        )
    }

    fn update_lot_purchase_price(&self, payload: UpdateLotPricePayload) -> Result<LotPriceUpdateResult, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let result = self.update_lot_price_in_tx(&mut *tx, &payload)?;
        tx.commit()?;
        Ok(result)
    }

    fn preview_lot_purchase_price(&self, payload: UpdateLotPricePayload) -> Result<LotPriceUpdateResult, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let result = self.update_lot_price_in_tx(&mut *tx, &payload);
        tx.rollback()?; // never committed
        result
    }

    fn delete_purchase(&self, purchase_id: i64) -> Result<DeleteBillResult, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let g = evaluate_purchase_guard(&mut *tx, purchase_id)?;
        if let Some(reason) = g.reasons.iter().find(|r| r.blocks_delete) {
            return Err(locked(reason));
        }
        for lot in tx.purchases().list_items_by_transaction_id(purchase_id)? {
            self.inventory_service.remove_purchase_lot(&mut *tx, lot.id)?;
        }
        delete_bill_charges(&mut *tx, TransactionType::Purchase, purchase_id)?;
        let removed = self.payment_service.delete_bill_ledger(
            &mut *tx,
            TransactionType::Purchase,
            purchase_id,
            &[PaymentKind::Upfront, PaymentKind::Settlement, PaymentKind::ReturnCredit],
        )?;
        tx.purchases().delete_transaction(purchase_id)?;
        tx.commit()?;
        Ok(DeleteBillResult { removed_cash_amount: removed })
    }

    fn get_purchase_edit_guard(&self, purchase_id: i64) -> Result<BillEditGuard, AppError> {
        let mut tx = self.uow.begin_read()?;
        evaluate_purchase_guard(&mut *tx, purchase_id)
    }

    fn get_purchases_history(
        &self,
        filter: PurchaseHistoryFilter,
    ) -> Result<Paginated<PurchaseHistoryRow>, AppError> {
        let page = filter.validate()?;
        let mut tx = self.uow.begin_read()?;
        let (rows, total) = tx.purchases().list_history(&filter, page.offset(), page.limit)?;
        Ok(Paginated::new(rows, total, page.page, page.limit))
    }

    fn get_purchase_details(&self, purchase_id: i64) -> Result<PurchaseDetailView, AppError> {
        let mut tx = self.uow.begin_read()?;
        let pur = tx
            .purchases()
            .find_transaction_by_id(purchase_id)?
            .ok_or_else(|| AppError::NotFound {
                entity: "PurchaseTransaction".to_string(),
                id: purchase_id.to_string(),
            })?;
        let party = if let Some(pid) = pur.party_id {
            tx.parties().find_by_id(pid)?
        } else {
            None
        };
        let items = tx.purchases().list_items_by_transaction_id(purchase_id)?;
        let mut details: Vec<PurchaseItemDetail> = Vec::new();
        for item in items {
            // DD-30 / DD-28: bills read the write-time snapshots, not the live catalog.
            let item_name = item.item_name.clone();
            let bundle_name = item.pack_label.clone();
            details.push(PurchaseItemDetail {
                purchase_item: item,
                item_name,
                bundle_name,
            });
        }
        let settlements = tx.settlements().list_by_transaction(TransactionType::Purchase, purchase_id)?;
        let charges = tx.bill_charges().list_by_transaction(TransactionType::Purchase, purchase_id)?;
        Ok(PurchaseDetailView {
            purchase: pur,
            party,
            items: details,
            settlements,
            charges,
        })
    }
}
