use std::sync::Arc;
use crate::core::dtos::{
    CreatePurchasePayload, Paginated, PurchaseDetailView, PurchaseHistoryRow,
    PurchaseHistoryFilter, PurchaseItemDetail, PurchaseSummaryResponse, RecordPaymentPayload,
};
use crate::core::entities::{PurchaseItem, PurchaseTransaction};
use crate::core::error::AppError;
use crate::core::money::{check_date, round2, MONEY_EPS};
use crate::core::value_objects::{LotSource, PartyType, PaymentKind, PaymentStatus, TransactionType};
use crate::data::contracts::unit_of_work::UnitOfWork;
use crate::services::contracts::inventory_service::InventoryService;
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

impl<I: InventoryService, P: PaymentService, U: UnitOfWork> PurchaseService for PurchaseServiceImpl<I, P, U> {
    fn get_next_purchase_code(&self) -> Result<String, AppError> {
        // Display-only preview: the code is assigned for real inside create_purchase's write transaction.
        let mut tx = self.uow.begin_read()?;
        tx.purchases().get_next_transaction_code(LotSource::Purchase)
    }

    fn create_purchase(&self, payload: CreatePurchasePayload) -> Result<PurchaseSummaryResponse, AppError> {
        // 1. Basic validation (no database needed).
        if payload.items.is_empty() {
            return Err(validation("items", "items cannot be empty"));
        }
        check_date("transaction_date", &payload.transaction_date)?;
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

        let mut tx = self.uow.begin_transaction()?;

        // 2. Vendor must be an active vendor.
        let party_name = match payload.party_id {
            Some(pid) => {
                let party = tx.parties().find_by_id(pid)?.ok_or_else(|| AppError::NotFound {
                    entity: "Party".to_string(),
                    id: pid.to_string(),
                })?;
                if party.party_type != PartyType::Vendor {
                    return Err(validation("party_id", "Purchases need a vendor party"));
                }
                if !party.is_active {
                    return Err(validation("party_id", "Vendor is inactive"));
                }
                Some(party.name)
            }
            None => None,
        };

        // 3. Lines: active item, bundle belongs to the item, cost normalised to base units.
        let mut resolved = Vec::with_capacity(payload.items.len());
        let mut total_amount = 0.0;
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
                    if !(bundle.units_per_bundle > 0.0) {
                        return Err(validation("bundle_id", "Bundle multiplier must be positive"));
                    }
                    (bundle.units_per_bundle, Some(bundle.bundle_name))
                }
                None => (1.0, None),
            };
            let base_qty = line.bundle_quantity * multiplier;
            let line_total = round2(line.bundle_quantity * line.unit_cost);
            total_amount += line_total;
            resolved.push((item, pack_label, line.bundle_quantity, base_qty, line_total));
        }
        let total_amount = round2(total_amount);

        // 4. Payment status cross-check (DD-02).
        let paid = round2(payload.paid_amount);
        if paid > total_amount + MONEY_EPS {
            return Err(validation("paid_amount", "paid_amount cannot exceed the purchase total"));
        }
        match payload.payment_status {
            PaymentStatus::Paid => {
                if (paid - total_amount).abs() > MONEY_EPS {
                    return Err(validation("paid_amount", "PAID needs paid_amount = total"));
                }
            }
            PaymentStatus::Partial => {
                if !(paid > MONEY_EPS && paid + MONEY_EPS < total_amount) {
                    return Err(validation("paid_amount", "PARTIAL needs 0 < paid_amount < total"));
                }
            }
            PaymentStatus::Unpaid => {
                if paid > MONEY_EPS {
                    return Err(validation("paid_amount", "UNPAID needs paid_amount = 0"));
                }
                if total_amount <= MONEY_EPS {
                    return Err(validation("payment_status", "A zero-value bill must be PAID"));
                }
            }
        }
        if payload.payment_status != PaymentStatus::Paid && payload.party_id.is_none() {
            return Err(validation("party_id", "vendor required for credit"));
        }
        let upfront = if payload.payment_status == PaymentStatus::Paid { total_amount } else { paid };

        // 5. Header: paid 0 / balance = total; the ledger fixes both in step 7.
        let pur_code = tx.purchases().get_next_transaction_code(LotSource::Purchase)?;
        let invoice_ref = payload
            .supplier_invoice_ref
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty());
        let purchase_tx = PurchaseTransaction {
            id: 0,
            transaction_code: pur_code.clone(),
            source: LotSource::Purchase,
            supplier_invoice_ref: invoice_ref,
            transaction_date: payload.transaction_date.clone(),
            party_id: payload.party_id,
            party_name,
            payment_mode: payload.payment_mode.clone(),
            payment_status: if total_amount <= MONEY_EPS { PaymentStatus::Paid } else { PaymentStatus::Unpaid },
            total_amount,
            paid_amount: 0.0,
            balance_due: total_amount,
            notes: payload.notes,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let purchase_id = tx.purchases().create_transaction(&purchase_tx)?;

        // 6. Lots and stock cache.
        for (item, pack_label, pack_quantity, base_qty, line_total) in resolved {
            let purchase_item = PurchaseItem {
                id: 0,
                purchase_transaction_id: purchase_id,
                item_id: item.id,
                item_name: item.name.clone(),
                pack_label,
                pack_quantity,
                quantity: base_qty,
                unit_cost: line_total / base_qty,
                total_amount: line_total,
                remaining_quantity: base_qty,
            };
            self.inventory_service.add_stock_for_purchase(&mut *tx, &purchase_item)?;
        }

        // 7. Upfront payment through the one ledger path.
        if upfront > 0.0 {
            self.payment_service.record_payment(
                &mut *tx,
                RecordPaymentPayload {
                    party_id: payload.party_id,
                    transaction_type: TransactionType::Purchase,
                    kind: PaymentKind::Upfront,
                    return_id: None,
                    transaction_id: purchase_id,
                    settlement_date: payload.transaction_date,
                    amount_paid: upfront,
                    payment_mode: payload.payment_mode,
                    notes: Some("Supplier Inward Payment".to_string()),
                },
            )?;
        }
        tx.commit()?;
        Ok(PurchaseSummaryResponse { purchase_id, transaction_code: pur_code })
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
        Ok(PurchaseDetailView {
            purchase: pur,
            party,
            items: details,
            settlements,
        })
    }
}
