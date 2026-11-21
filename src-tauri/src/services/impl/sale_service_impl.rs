use std::sync::Arc;
use crate::core::dtos::{
    CreateSalePayload, PaginatedList, RecordPaymentPayload, SaleDetailView, SaleItemDetail,
    SaleSummaryResponse, SalesFilter, SalesHistoryFilter,
};
use crate::core::entities::{SaleItem, SaleTransaction};
use crate::core::error::AppError;
use crate::core::value_objects::PaymentStatus;
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
        let mut tx = self.uow.begin_transaction()?;
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
        let sale_code = payload
            .transaction_code
            .clone()
            .unwrap_or_else(|| self.get_next_sale_code().unwrap_or_else(|_| "SAL-1001".to_string()));
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
            let (multiplier, rate) = if let Some(bundle_id) = item_payload.bundle_id {
                let bundle = tx.bundles().find_by_id(bundle_id)?.ok_or_else(|| {
                    AppError::NotFound {
                        entity: "ItemBundle".to_string(),
                        id: bundle_id.to_string(),
                    }
                })?;
                let rate = item_payload.unit_price.unwrap_or_else(|| {
                    bundle
                        .default_bundle_price
                        .unwrap_or(item.default_sale_price * bundle.units_per_bundle)
                });
                (bundle.units_per_bundle, rate)
            } else {
                (
                    1.0,
                    item_payload.unit_price.unwrap_or(item.default_sale_price),
                )
            };
            let base_qty = item_payload.bundle_quantity * multiplier;
            let line_total = round2(item_payload.bundle_quantity * rate);
            let unit_price = line_total / base_qty;
            total_amount += line_total;
            resolved_lines.push((item_payload.clone(), base_qty, unit_price, line_total));
        }

        let sale_tx = SaleTransaction {
            id: 0,
            transaction_code: sale_code.clone(),
            transaction_date: payload.transaction_date,
            party_id: payload.party_id,
            payment_mode: payload.payment_mode.clone(),
            payment_status: PaymentStatus::Unpaid,
            total_amount,
            paid_amount: 0.0,
            balance_due: total_amount,
            notes: payload.notes,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let sale_id = tx.sales().create_transaction(&sale_tx)?;
        for (line_payload, base_qty, unit_price, line_total) in resolved_lines {
            let sale_item = SaleItem {
                id: 0,
                sale_transaction_id: sale_id,
                item_id: line_payload.item_id,
                bundle_id: line_payload.bundle_id,
                bundle_quantity: line_payload.bundle_quantity,
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
                transaction_type: "SALE".to_string(),
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
        let mut tx = self.uow.begin_transaction()?;
        tx.sales().list_transactions(&SalesFilter {
            party_id: filter.party_id,
            payment_status: filter.payment_status,
            start_date: filter.from_date.clone(),
            end_date: filter.to_date.clone(),
            from_date: filter.from_date,
            to_date: filter.to_date,
            page: filter.page,
            limit: filter.limit,
        })
    }

    fn get_sale_details(&self, sale_id: i64) -> Result<SaleDetailView, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let sale = tx
            .sales()
            .find_by_id(sale_id)?
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
            let allocations = tx.allocations().find_by_sale_item_id(item.id)?;
            details.push(SaleItemDetail {
                sale_item: item,
                item_name,
                bundle_name,
                allocations,
            });
        }
        let settlements = tx.settlements().list_by_transaction("SALE", sale_id)?;
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
