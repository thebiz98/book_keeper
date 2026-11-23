use std::sync::Arc;
use crate::core::dtos::{
    BillEditGuard, CreatePurchasePayload, DeleteBillResult, Paginated, PurchaseDetailView, PurchaseHistoryRow,
    PurchaseHistoryFilter, PurchaseItemDetail, PurchaseSummaryResponse, RecordPaymentPayload,
};
use crate::core::entities::{Item, PurchaseItem, PurchaseTransaction};
use crate::core::error::AppError;
use crate::core::money::{check_date, round2, MONEY_EPS};
use crate::services::r#impl::bill_guard::{
    evaluate_purchase_guard, locked, locked_with, PARTY_CHANGE_WITH_CREDITS, PARTY_REQUIRED_WITH_PAYMENTS,
    TOTAL_BELOW_APPLIED,
};
use crate::services::r#impl::bill_math::{
    compute_bill_totals, delete_bill_charges, insert_bill_charges, purchase_upfront, resolve_bill_charges,
    validate_charge_inputs, BillTotals, ResolvedCharge,
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
    check_date("transaction_date", &payload.transaction_date)?;
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
                return Err(validation("party_id", "Vendor is inactive"));
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
        if !item.is_active {
            return Err(validation("item_id", format!("Item '{}' is inactive", item.name)));
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
) -> Result<(Vec<ResolvedCharge>, BillTotals), AppError> {
    let charges = resolve_bill_charges(tx, TransactionType::Purchase, &payload.charges)?;
    let line_totals: Vec<f64> = r.lines.iter().map(|l| l.line_total).collect();
    let totals = compute_bill_totals(&line_totals, &charges)?;
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
        return Err(validation("party_id", "vendor required for credit"));
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

impl<I: InventoryService, P: PaymentService, U: UnitOfWork> PurchaseService for PurchaseServiceImpl<I, P, U> {
    fn get_next_purchase_code(&self) -> Result<String, AppError> {
        // Display-only preview: the code is assigned for real inside create_purchase's write transaction.
        let mut tx = self.uow.begin_read()?;
        tx.purchases().get_next_transaction_code(LotSource::Purchase)
    }

    fn create_purchase(&self, payload: CreatePurchasePayload) -> Result<PurchaseSummaryResponse, AppError> {
        validate_purchase_shape(&payload)?;
        if payload.items.iter().any(|i| i.purchase_item_id.is_some()) {
            return Err(validation("purchase_item_id", "purchase_item_id is only valid when editing a purchase"));
        }
        let mut tx = self.uow.begin_transaction()?;

        let r = resolve_purchase(&mut *tx, &payload, None)?;
        // Charges (FR-19), totals and the payment cross-check (DD-02).
        let (charges, totals) = price_purchase(&mut *tx, &payload, &r)?;
        let total_amount = totals.total_amount;
        let upfront = check_purchase_payment(payload.payment_status, round2(payload.paid_amount), total_amount, r.party_id)?;

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
        };
        let purchase_id = tx.purchases().create_transaction(&purchase_tx)?;
        insert_bill_charges(&mut *tx, TransactionType::Purchase, purchase_id, &charges)?;

        // Lots and stock cache. Lot unit_cost = line_total / base_qty, unaffected by bill-level
        // charges and discounts (DD-38).
        for l in &r.lines {
            self.inventory_service.add_stock_for_purchase(&mut *tx, &new_lot(purchase_id, l))?;
        }
        self.record_upfront(&mut *tx, purchase_id, r.party_id, &payload, upfront)?;
        tx.commit()?;
        Ok(PurchaseSummaryResponse { purchase_id, transaction_code: pur_code })
    }

    fn update_purchase(
        &self,
        purchase_id: i64,
        payload: CreatePurchasePayload,
    ) -> Result<PurchaseSummaryResponse, AppError> {
        validate_purchase_shape(&payload)?;
        let mut tx = self.uow.begin_transaction()?;
        let old = tx
            .purchases()
            .find_transaction_by_id(purchase_id)?
            .ok_or_else(|| AppError::not_found("PurchaseTransaction", purchase_id.to_string()))?;
        let g = evaluate_purchase_guard(&mut *tx, purchase_id)?;
        if let Some(reason) = g.reasons.iter().find(|r| r.blocks_edit) {
            return Err(locked(reason));
        }

        // Reverse what is not kept in place: charges and the upfront cash. Later settlements stay.
        delete_bill_charges(&mut *tx, TransactionType::Purchase, purchase_id)?;
        self.payment_service
            .delete_bill_ledger(&mut *tx, TransactionType::Purchase, purchase_id, &[PaymentKind::Upfront])?;

        let r = resolve_purchase(&mut *tx, &payload, old.party_id)?;
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
        let (charges, totals) = price_purchase(&mut *tx, &payload, &r)?;
        let total_amount = totals.total_amount;
        if total_amount + MONEY_EPS < g.applied_amount {
            return Err(locked_with(
                TOTAL_BELOW_APPLIED,
                format!("The new total \u{20b9}{total_amount:.2} is below the \u{20b9}{:.2} already applied to this bill", g.applied_amount),
            ));
        }
        let open = round2((total_amount - g.applied_amount).max(0.0));
        let upfront = check_purchase_payment(payload.payment_status, round2(payload.paid_amount), open, r.party_id)?;

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
            self.inventory_service.remove_purchase_lot(&mut *tx, lot.id)?;
        }
        for l in &r.lines {
            match l.purchase_item_id {
                Some(lid) => self.inventory_service.edit_purchase_lot(
                    &mut *tx,
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
                None => self.inventory_service.add_stock_for_purchase(&mut *tx, &new_lot(purchase_id, l))?,
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
            ..old.clone()
        };
        tx.purchases().update_header_for_edit(&header)?;
        insert_bill_charges(&mut *tx, TransactionType::Purchase, purchase_id, &charges)?;
        if party_changed {
            self.payment_service.repoint_bill_party(&mut *tx, TransactionType::Purchase, purchase_id, r.party_id)?;
        }
        self.payment_service.recompute(&mut *tx, TransactionType::Purchase, purchase_id)?;
        self.record_upfront(&mut *tx, purchase_id, r.party_id, &payload, upfront)?;
        tx.commit()?;
        Ok(PurchaseSummaryResponse { purchase_id, transaction_code: old.transaction_code })
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
