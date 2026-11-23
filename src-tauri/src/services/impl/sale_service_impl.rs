use std::collections::BTreeMap;
use std::sync::Arc;
use crate::core::dtos::{
    CreateSalePayload, Paginated, RecordPaymentPayload, SaleDetailView, SaleItemDetail,
    SaleHistoryRow, SaleSummaryResponse, SalesHistoryFilter,
};
use crate::core::entities::{SaleItem, SaleTransaction};
use crate::core::error::AppError;
use crate::core::money::{check_date, round2, MONEY_EPS};
use crate::services::r#impl::bill_math::{
    compute_bill_totals, insert_bill_charges, net_unit_price, resolve_bill_charges, sale_payment_split,
    validate_charge_inputs,
};
use crate::core::value_objects::{PartyType, PaymentKind, PaymentStatus, TransactionType};
use crate::data::contracts::unit_of_work::UnitOfWork;
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

impl<I: InventoryService, P: PaymentService, U: UnitOfWork> SaleService for SaleServiceImpl<I, P, U> {
    fn get_next_sale_code(&self) -> Result<String, AppError> {
        // Display-only preview: the code is assigned for real inside create_sale's write transaction.
        let mut tx = self.uow.begin_read()?;
        tx.sales().get_next_transaction_code()
    }

    fn create_sale(&self, payload: CreateSalePayload) -> Result<SaleSummaryResponse, AppError> {
        // 1. Basic validation (no database needed).
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
                    format!("Sale quantity must be positive, got {}", line.bundle_quantity),
                ));
            }
            if !line.unit_price.is_finite() || line.unit_price < 0.0 {
                return Err(validation("unit_price", "unit_price must be a non-negative number"));
            }
        }

        let mut tx = self.uow.begin_transaction()?;

        // 2. Party must be an active customer.
        let party_name = match payload.party_id {
            Some(pid) => {
                let party = tx.parties().find_by_id(pid)?.ok_or_else(|| AppError::NotFound {
                    entity: "Party".to_string(),
                    id: pid.to_string(),
                })?;
                if party.party_type != PartyType::Customer {
                    return Err(validation("party_id", "Sales need a customer party"));
                }
                if !party.is_active {
                    return Err(validation("party_id", "Customer is inactive"));
                }
                Some(party.name)
            }
            None => None,
        };

        // 3. Line resolution: active item, bundle belongs to the item, snapshots, round2 totals.
        let mut resolved = Vec::with_capacity(payload.items.len());
        let mut required: BTreeMap<i64, f64> = BTreeMap::new();
        let mut line_totals: Vec<f64> = Vec::with_capacity(payload.items.len());
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
            let base_qty = line.bundle_quantity * multiplier;
            let line_total = round2(line.bundle_quantity * line.unit_price);
            line_totals.push(line_total);
            *required.entry(item.id).or_insert(0.0) += base_qty;
            resolved.push((item, pack_label, line.bundle_quantity, base_qty, line_total));
        }

        // 4. Aggregated stock check across all lines of the same item (DD-10).
        let requirements: Vec<(i64, f64)> = required.into_iter().collect();
        self.inventory_service.validate_stock_availability(&mut *tx, &requirements)?;

        // 5. Charges (FR-19), totals and the payment cross-check (DD-02).
        let charges = resolve_bill_charges(&mut *tx, TransactionType::Sale, &payload.charges)?;
        let totals = compute_bill_totals(&line_totals, &charges)?;
        let total_amount = totals.total_amount;
        let paid = round2(payload.paid_amount);
        let (upfront, change) = sale_payment_split(payload.payment_status, total_amount, paid)?;
        if payload.payment_status != PaymentStatus::Paid && payload.party_id.is_none() {
            return Err(validation("party_id", "customer required for credit"));
        }

        // 6. Header: paid 0 / balance = total; the ledger fixes both in step 8.
        let sale_code = tx.sales().get_next_transaction_code()?;
        let sale_tx = SaleTransaction {
            id: 0,
            transaction_code: sale_code.clone(),
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
            items_subtotal: totals.items_subtotal,
            charges_total: totals.charges_total,
            discounts_total: totals.discounts_total,
        };
        let sale_id = tx.sales().create_transaction(&sale_tx)?;
        insert_bill_charges(&mut *tx, TransactionType::Sale, sale_id, &charges)?;

        // 7. Lines and stock depletion.
        for (item, pack_label, pack_quantity, base_qty, line_total) in resolved {
            let sale_item = SaleItem {
                id: 0,
                sale_transaction_id: sale_id,
                item_id: item.id,
                item_name: item.name.clone(),
                pack_label,
                pack_quantity,
                quantity: base_qty,
                unit_price: line_total / base_qty,
                total_amount: line_total,
            };
            let sale_item_id = tx.sales().create_sale_item(&sale_item)?;
            self.inventory_service
                .deplete_stock_for_sale(&mut *tx, sale_item_id, item.id, base_qty)?;
        }

        // 8. Upfront cash goes through the one ledger path.
        if upfront > 0.0 {
            self.payment_service.record_payment(
                &mut *tx,
                RecordPaymentPayload {
                    party_id: payload.party_id,
                    transaction_type: TransactionType::Sale,
                    kind: PaymentKind::Upfront,
                    return_id: None,
                    transaction_id: sale_id,
                    settlement_date: payload.transaction_date,
                    amount_paid: upfront,
                    payment_mode: payload.payment_mode,
                    notes: Some("Upfront Counter Payment".to_string()),
                },
            )?;
        }

        // 9. Respond from the header as the ledger left it.
        let header = tx.sales().find_transaction_by_id(sale_id)?.ok_or_else(|| AppError::NotFound {
            entity: "SaleTransaction".to_string(),
            id: sale_id.to_string(),
        })?;
        tx.commit()?;
        Ok(SaleSummaryResponse {
            sale_id,
            transaction_code: sale_code,
            total_amount: header.total_amount,
            paid_amount: header.paid_amount,
            balance_due: header.balance_due,
            change_amount: change,
            payment_status: header.payment_status,
        })
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
        let mut details: Vec<SaleItemDetail> = Vec::new();
        for item in items {
            // DD-30 / DD-28: bills read the write-time snapshots, not the live catalog.
            let item_name = item.item_name.clone();
            let bundle_name = item.pack_label.clone();
            let allocations = tx.allocations().find_by_sale_item_id(item.id)?;
            let returned_quantity = returned.get(&item.id).copied().unwrap_or(0.0);
            let net_unit_price = net_unit_price(item.unit_price, sale.items_subtotal, sale.discounts_total);
            details.push(SaleItemDetail {
                sale_item: item,
                item_name,
                bundle_name,
                returned_quantity,
                allocations,
                net_unit_price,
            });
        }
        let settlements = tx.settlements().list_by_transaction(TransactionType::Sale, sale_id)?;
        let charges = tx.bill_charges().list_by_transaction(TransactionType::Sale, sale_id)?;
        Ok(SaleDetailView {
            sale,
            party,
            items: details,
            settlements,
            charges,
        })
    }
}
