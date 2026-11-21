use std::sync::Arc;
use crate::core::dtos::{
    CreateSalePayload, PaginatedList, RecordPaymentPayload, SaleDetailView, SaleItemDetail,
    SaleSummaryResponse, SalesFilter, SalesHistoryFilter,
};
use crate::core::entities::{SaleItem, SaleTransaction};
use crate::core::error::AppError;
use crate::core::value_objects::{PaymentKind, PaymentStatus, TransactionType};
use crate::data::contracts::unit_of_work::UnitOfWork;
use crate::services::contracts::inventory_service::InventoryService;
use crate::services::contracts::payment_service::PaymentService;
use crate::services::contracts::sale_service::SaleService;

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

impl<I: InventoryService, P: PaymentService, U: UnitOfWork> SaleService for SaleServiceImpl<I, P, U> {
    fn get_next_sale_code(&self) -> Result<String, AppError> {
        // Display-only preview: the code is assigned for real inside create_sale's write transaction.
        let mut tx = self.uow.begin_read()?;
        tx.sales().get_next_transaction_code()
    }

    fn create_sale(&self, payload: CreateSalePayload) -> Result<SaleSummaryResponse, AppError> {
        if payload.items.is_empty() {
            return Err(AppError::ValidationError {
                field: "items".to_string(),
                message: "items cannot be empty".to_string(),
            });
        }
        for item_payload in &payload.items {
            if item_payload.bundle_quantity <= 0.0 {
                return Err(AppError::ValidationError {
                    field: "bundle_quantity".to_string(),
                    message: format!(
                        "Sale quantity must be positive, got {}",
                        item_payload.bundle_quantity
                    ),
                });
            }
            if let Some(rate) = item_payload.unit_price {
                if rate < 0.0 {
                    return Err(AppError::ValidationError {
                        field: "unit_price".to_string(),
                        message: "unit_price cannot be negative".to_string(),
                    });
                }
            }
        }
        if payload.paid_amount < 0.0 {
            return Err(AppError::ValidationError {
                field: "paid_amount".to_string(),
                message: "paid_amount cannot be negative".to_string(),
            });
        }

        let mut tx = self.uow.begin_transaction()?;
        // Assigned inside the write transaction (never client-supplied): the IMMEDIATE lock makes
        // MAX + 1 race-free.
        let sale_code = tx.sales().get_next_transaction_code()?;
        let mut total_amount = 0.0;
        let mut resolved_lines = Vec::new();
        for item_payload in &payload.items {
            // LLD §6.7: `unit_price` is the rate per selected bundle (or per base unit
            // when no bundle). line_total = bundle_quantity * rate.
            let item = tx.items().find_by_id(item_payload.item_id)?.ok_or_else(|| {
                AppError::NotFound {
                    entity: "Item".to_string(),
                    id: item_payload.item_id.to_string(),
                }
            })?;
            let (multiplier, rate, pack_label) = if let Some(bundle_id) = item_payload.bundle_id {
                let bundle = tx.bundles().find_by_id(bundle_id)?.ok_or_else(|| {
                    AppError::NotFound {
                        entity: "ItemBundle".to_string(),
                        id: bundle_id.to_string(),
                    }
                })?;
                if bundle.item_id != item.id {
                    return Err(AppError::ValidationError {
                        field: "bundle_id".to_string(),
                        message: format!("Bundle {bundle_id} does not belong to item {}", item.id),
                    });
                }
                let rate = item_payload.unit_price.unwrap_or_else(|| {
                    bundle
                        .default_bundle_price
                        .unwrap_or(item.default_sale_price * bundle.units_per_bundle)
                });
                (bundle.units_per_bundle, rate, Some(bundle.bundle_name.clone()))
            } else {
                (
                    1.0,
                    item_payload.unit_price.unwrap_or(item.default_sale_price),
                    None,
                )
            };
            let base_qty = item_payload.bundle_quantity * multiplier;
            let line_total = round2(item_payload.bundle_quantity * rate);
            let unit_price = line_total / base_qty;
            total_amount += line_total;
            resolved_lines.push((item_payload.clone(), item.name.clone(), pack_label, base_qty, unit_price, line_total));
        }

        let party_name = match payload.party_id {
            Some(pid) => tx.parties().find_by_id(pid)?.map(|p| p.name),
            None => None,
        };
        let sale_tx = SaleTransaction {
            id: 0,
            transaction_code: sale_code.clone(),
            transaction_date: payload.transaction_date,
            party_id: payload.party_id,
            party_name,
            payment_mode: payload.payment_mode.clone(),
            payment_status: PaymentStatus::Unpaid,
            total_amount,
            paid_amount: 0.0,
            balance_due: total_amount,
            notes: payload.notes,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let sale_id = tx.sales().create_transaction(&sale_tx)?;
        for (line_payload, item_name, pack_label, base_qty, unit_price, line_total) in resolved_lines {
            let sale_item = SaleItem {
                id: 0,
                sale_transaction_id: sale_id,
                item_id: line_payload.item_id,
                item_name,
                pack_label,
                pack_quantity: line_payload.bundle_quantity,
                quantity: base_qty,
                unit_price,
                total_amount: line_total,
            };
            let sale_item_id = tx.sales().create_sale_item(&sale_item)?;
            self.inventory_service
                .deplete_stock_for_sale(&mut *tx, sale_item_id, line_payload.item_id, base_qty)?;
        }

        let upfront_payment = payload.paid_amount.min(total_amount);
        if upfront_payment > 0.0 {
            let payment_payload = RecordPaymentPayload {
                party_id: payload.party_id,
                transaction_type: TransactionType::Sale,
                kind: PaymentKind::Upfront,
                transaction_id: sale_id,
                settlement_date: sale_tx.transaction_date.clone(),
                amount_paid: upfront_payment,
                payment_mode: payload.payment_mode,
                notes: Some("Upfront Counter Payment".to_string()),
            };
            self.payment_service
                .record_payment(&mut *tx, payment_payload)?;
        }
        tx.commit()?;
        let balance_due = (total_amount - payload.paid_amount).max(0.0);
        let change = (payload.paid_amount - total_amount).max(0.0);
        let payment_status = if balance_due == 0.0 {
            PaymentStatus::Paid
        } else if payload.paid_amount > 0.0 {
            PaymentStatus::Partial
        } else {
            PaymentStatus::Unpaid
        };
        Ok(SaleSummaryResponse {
            sale_id,
            transaction_code: sale_code,
            total_amount,
            paid_amount: payload.paid_amount,
            balance_due,
            change_amount: change,
            change_returned: change,
            payment_status,
        })
    }

    fn get_sales_history(
        &self,
        filter: SalesHistoryFilter,
    ) -> Result<PaginatedList<SaleTransaction>, AppError> {
        let mut tx = self.uow.begin_read()?;
        tx.sales().list_transactions(&SalesFilter {
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
        let mut details: Vec<SaleItemDetail> = Vec::new();
        for item in items {
            // DD-30 / DD-28: bills read the write-time snapshots, not the live catalog.
            let item_name = item.item_name.clone();
            let bundle_name = item.pack_label.clone();
            let allocations = tx.allocations().find_by_sale_item_id(item.id)?;
            details.push(SaleItemDetail {
                sale_item: item,
                item_name,
                bundle_name,
                allocations,
            });
        }
        let settlements = tx.settlements().list_by_transaction(TransactionType::Sale, sale_id)?;
        Ok(SaleDetailView {
            sale,
            party,
            items: details,
            settlements,
        })
    }
}

/// Rounds a currency amount to 2 decimal places (paise precision).
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
