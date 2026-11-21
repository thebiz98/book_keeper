use std::sync::Arc;
use crate::core::dtos::{
    CreatePurchasePayload, PaginatedList, PurchaseDetailView, PurchaseFilter,
    PurchaseHistoryFilter, PurchaseItemDetail, RecordPaymentPayload,
};
use crate::core::entities::{PurchaseItem, PurchaseTransaction};
use crate::core::error::AppError;
use crate::core::value_objects::PaymentStatus;
use crate::data::contracts::unit_of_work::UnitOfWork;
use crate::services::contracts::inventory_service::InventoryService;
use crate::services::contracts::payment_service::PaymentService;
use crate::services::contracts::purchase_service::PurchaseService;

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
        let mut tx = self.uow.begin_transaction()?;
        tx.purchases().get_next_transaction_code()
    }

    fn create_purchase(&self, payload: CreatePurchasePayload) -> Result<i64, AppError> {
        if payload.items.is_empty() {
            return Err(AppError::ValidationError {
                field: "items".to_string(),
                message: "items cannot be empty".to_string(),
            });
        }
        let mut tx = self.uow.begin_transaction()?;
        let pur_code = payload
            .transaction_code
            .clone()
            .map(Ok)
            // Use the open transaction; a nested begin_transaction() would deadlock on BEGIN IMMEDIATE.
            .unwrap_or_else(|| tx.purchases().get_next_transaction_code())?;
        let mut total_amount = 0.0;
        let mut resolved_lines = Vec::new();
        for item_payload in &payload.items {
            let (multiplier, unit_cost) = if let Some(bundle_id) = item_payload.bundle_id {
                let bundle = tx.bundles().find_by_id(bundle_id)?.ok_or_else(|| {
                    AppError::NotFound {
                        entity: "ItemBundle".to_string(),
                        id: bundle_id.to_string(),
                    }
                })?;
                let effective_cost = item_payload.unit_cost / bundle.units_per_bundle;
                (bundle.units_per_bundle, effective_cost)
            } else {
                (1.0, item_payload.unit_cost)
            };
            let base_qty = item_payload.bundle_quantity * multiplier;
            let line_total = item_payload.bundle_quantity * item_payload.unit_cost;
            total_amount += line_total;
            resolved_lines.push((item_payload.clone(), base_qty, unit_cost, line_total));
        }
        let purchase_tx = PurchaseTransaction {
            id: 0,
            transaction_code: pur_code,
            transaction_date: payload.transaction_date.clone(),
            party_id: payload.party_id,
            payment_mode: payload.payment_mode.clone(),
            payment_status: PaymentStatus::Unpaid,
            total_amount,
            paid_amount: 0.0,
            balance_due: total_amount,
            notes: payload.notes,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let purchase_id = tx.purchases().create_transaction(&purchase_tx)?;
        for (line_payload, base_qty, unit_cost, line_total) in resolved_lines {
            let purchase_item = PurchaseItem {
                id: 0,
                purchase_transaction_id: purchase_id,
                item_id: line_payload.item_id,
                bundle_id: line_payload.bundle_id,
                bundle_quantity: line_payload.bundle_quantity,
                quantity: base_qty,
                unit_cost,
                total_amount: line_total,
                remaining_quantity: base_qty,
            };
            self.inventory_service
                .add_stock_for_purchase(&mut *tx, &purchase_item)?;
        }
        let upfront_payment = payload.paid_amount.min(total_amount);
        if upfront_payment > 0.0 {
            let payment_payload = RecordPaymentPayload {
                party_id: payload.party_id,
                transaction_type: "PURCHASE".to_string(),
                transaction_id: purchase_id,
                settlement_date: payload.transaction_date,
                amount_paid: upfront_payment,
                payment_mode: payload.payment_mode,
                notes: Some("Supplier Inward Payment".to_string()),
            };
            self.payment_service
                .record_payment(&mut *tx, payment_payload)?;
        }
        tx.commit()?;
        Ok(purchase_id)
    }

    fn get_purchases_history(
        &self,
        filter: PurchaseHistoryFilter,
    ) -> Result<PaginatedList<PurchaseTransaction>, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        tx.purchases().list_transactions(&PurchaseFilter {
            party_id: filter.party_id,
            payment_status: filter.payment_status,
            // Accept either naming (frontend sends start/end; legacy callers from/to).
            start_date: filter.start_date.clone().or_else(|| filter.from_date.clone()),
            end_date: filter.end_date.clone().or_else(|| filter.to_date.clone()),
            from_date: filter.start_date.or(filter.from_date),
            to_date: filter.end_date.or(filter.to_date),
            page: filter.page,
            limit: filter.limit,
        })
    }

    fn get_purchase_details(&self, purchase_id: i64) -> Result<PurchaseDetailView, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let pur = tx
            .purchases()
            .find_by_id(purchase_id)?
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
            let item_name = tx
                .items()
                .find_by_id(item.item_id)?
                .map(|i| i.name)
                .unwrap_or_else(|| "Unknown Item".to_string());
            let bundle_name = if let Some(bid) = item.bundle_id {
                tx.bundles().find_by_id(bid)?.map(|b| b.bundle_name)
            } else {
                None
            };
            details.push(PurchaseItemDetail {
                purchase_item: item,
                item_name,
                bundle_name,
            });
        }
        let settlements = tx.settlements().list_by_transaction("PURCHASE", purchase_id)?;
        Ok(PurchaseDetailView {
            purchase: pur,
            party,
            items: details,
            settlements,
        })
    }
}
