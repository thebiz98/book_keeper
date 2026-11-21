use std::sync::Arc;
use crate::core::dtos::{
    CustomerReturnPayload, RecordRefundPayload, ReturnDetailView, ReturnItemDetail,
    VendorReturnPayload,
};
use crate::core::entities::{ReturnItem, ReturnTransaction};
use crate::core::error::AppError;
use crate::core::value_objects::{
    ReturnLineType, ReturnType, SettlementType, StockCondition, TransactionType,
};
use crate::data::contracts::unit_of_work::UnitOfWork;
use crate::services::contracts::inventory_service::InventoryService;
use crate::services::contracts::payment_service::PaymentService;
use crate::services::contracts::return_service::ReturnService;

#[derive(Clone)]
pub struct ReturnServiceImpl<
    I: InventoryService = Arc<dyn InventoryService>,
    P: PaymentService = Arc<dyn PaymentService>,
    U: UnitOfWork = Arc<dyn UnitOfWork>,
> {
    pub inventory_service: I,
    pub payment_service: P,
    pub uow: U,
}

impl<I: InventoryService, P: PaymentService, U: UnitOfWork> ReturnServiceImpl<I, P, U> {
    pub fn new(inventory_service: I, payment_service: P, uow: U) -> Self {
        Self {
            inventory_service,
            payment_service,
            uow,
        }
    }
}

impl<I: InventoryService, P: PaymentService, U: UnitOfWork> ReturnService for ReturnServiceImpl<I, P, U> {
    fn get_next_return_code(&self) -> Result<String, AppError> {
        // Display-only preview: the code is assigned for real inside the create_* write transaction.
        let mut tx = self.uow.begin_read()?;
        tx.returns().get_next_return_code()
    }

    fn create_customer_return(&self, payload: CustomerReturnPayload) -> Result<String, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let mut total_refund = 0.0;
        // Assigned inside the write transaction; customer and vendor returns share one RET- sequence.
        let return_code = tx.returns().get_next_return_code()?;
        let party_name = match payload.party_id {
            Some(pid) => tx.parties().find_by_id(pid)?.map(|p| p.name),
            None => None,
        };
        let ret_tx = ReturnTransaction {
            id: 0,
            return_code: return_code.clone(),
            return_date: payload.return_date.clone(),
            return_type: ReturnType::Customer,
            party_id: payload.party_id,
            party_name,
            original_transaction_id: payload.original_sale_id,
            settlement_type: payload.settlement_type,
            total_amount: 0.0,
            reason: payload.reason,
            notes: payload.notes,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let return_id = tx.returns().create_return_transaction(&ret_tx)?;
        for item_payload in &payload.items {
            let line_total = item_payload.quantity * item_payload.unit_price;
            total_refund += line_total;
            let item_name = tx
                .items()
                .find_by_id(item_payload.item_id)?
                .ok_or_else(|| AppError::not_found("Item", item_payload.item_id.to_string()))?
                .name;
            // DD-05: every linked line reverses its sale allocations; only RESTOCKABLE units go
            // back into lots/stock. The reversed FIFO cost is the line's cost_amount.
            let cost_amount = match item_payload.sale_item_id {
                Some(sale_item_id) => self.inventory_service.reverse_sale_line(
                    &mut *tx,
                    sale_item_id,
                    item_payload.item_id,
                    item_payload.quantity,
                    item_payload.condition == StockCondition::Restockable,
                )?,
                None => 0.0, // TODO(F4): unlinked returns have no allocation to reverse; cost policy
            };
            let ret_item = ReturnItem {
                id: 0,
                return_transaction_id: return_id,
                line_type: ReturnLineType::Returned,
                item_id: item_payload.item_id,
                item_name,
                sale_item_id: item_payload.sale_item_id,
                purchase_item_id: None,
                quantity: item_payload.quantity,
                unit_price: item_payload.unit_price,
                total_amount: line_total,
                cost_amount,
                condition: Some(item_payload.condition),
            };
            tx.returns().create_return_item(&ret_item)?;
        }
        // TODO(F4, DD-02/DD-04/DD-05): Refund must first apply RETURN_CREDIT to the linked bill /
        // oldest open bills, and Replacement must issue REPLACEMENT_ISSUED lines. For now every
        // Refund is a REFUND_OUT cash row.
        if payload.settlement_type == SettlementType::Refund && total_refund > 0.0 {
            let refund_payload = RecordRefundPayload {
                party_id: payload.party_id,
                transaction_type: TransactionType::Sale,
                transaction_id: payload.original_sale_id,
                return_id: Some(return_id),
                refund_date: payload.return_date,
                amount: total_refund,
                payment_mode: Some("CASH".to_string()),
                notes: Some("Customer return refund".to_string()),
            };
            self.payment_service
                .record_refund(&mut *tx, refund_payload)?;
        }
        tx.commit()?;
        Ok(return_code)
    }

    fn create_vendor_return(&self, payload: VendorReturnPayload) -> Result<String, AppError> {
        if payload.settlement_type == SettlementType::Replacement {
            return Err(AppError::ValidationError {
                field: "settlement_type".to_string(),
                message: "Replacement is customer-only; vendor returns are always Refund".to_string(),
            });
        }
        let mut tx = self.uow.begin_transaction()?;
        let return_code = tx.returns().get_next_return_code()?;
        let mut total_amount = 0.0;
        for item in &payload.items {
            total_amount += item.quantity * item.unit_cost;
        }

        let party_name = match payload.party_id {
            Some(pid) => tx.parties().find_by_id(pid)?.map(|p| p.name),
            None => None,
        };
        let ret_tx = ReturnTransaction {
            id: 0,
            return_code: return_code.clone(),
            return_date: payload.return_date.clone(),
            return_type: ReturnType::Vendor,
            party_id: payload.party_id,
            party_name,
            original_transaction_id: payload.original_purchase_id,
            settlement_type: payload.settlement_type,
            total_amount,
            reason: payload.reason,
            notes: payload.notes,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let return_id = tx.returns().create_return_transaction(&ret_tx)?;

        for item in &payload.items {
            let line_total = item.quantity * item.unit_cost;
            let item_name = tx
                .items()
                .find_by_id(item.item_id)?
                .ok_or_else(|| AppError::not_found("Item", item.item_id.to_string()))?
                .name;
            // DD-11: the lot cost removed (named lot or FIFO) is the line's cost_amount.
            let cost_amount = self.inventory_service.remove_stock_for_vendor_return(
                &mut *tx,
                return_id,
                item.item_id,
                item.purchase_item_id,
                item.quantity,
            )?;
            let ret_item = ReturnItem {
                id: 0,
                return_transaction_id: return_id,
                line_type: ReturnLineType::Returned,
                item_id: item.item_id,
                item_name,
                sale_item_id: None,
                purchase_item_id: item.purchase_item_id,
                quantity: item.quantity,
                unit_price: item.unit_cost,
                total_amount: line_total,
                cost_amount,
                condition: None, // condition applies to customer lines only
            };
            tx.returns().create_return_item(&ret_item)?;
        }

        // TODO(F4, DD-02/DD-04): RETURN_CREDIT against payables first, remainder as REFUND_IN.
        if payload.settlement_type == SettlementType::Refund && total_amount > 0.0 {
            let refund_payload = RecordRefundPayload {
                party_id: payload.party_id,
                transaction_type: TransactionType::Purchase,
                transaction_id: payload.original_purchase_id,
                return_id: Some(return_id),
                refund_date: payload.return_date,
                amount: total_amount,
                payment_mode: Some("CASH".to_string()),
                notes: Some("Vendor return refund".to_string()),
            };
            self.payment_service
                .record_refund(&mut *tx, refund_payload)?;
        }

        tx.commit()?;
        Ok(return_code)
    }

    fn get_return_details(&self, return_id: i64) -> Result<ReturnDetailView, AppError> {
        let mut tx = self.uow.begin_read()?;
        let ret_tx = tx
            .returns()
            .find_return_by_id(return_id)?
            .ok_or_else(|| AppError::NotFound {
                entity: "ReturnTransaction".to_string(),
                id: return_id.to_string(),
            })?;
        let party = if let Some(pid) = ret_tx.party_id {
            tx.parties().find_by_id(pid)?
        } else {
            None
        };
        let items = tx.returns().find_items_by_return_id(return_id)?;
        let mut item_details = Vec::new();
        for it in items {
            // DD-30: read the write-time snapshot.
            let name = it.item_name.clone();
            item_details.push(ReturnItemDetail {
                return_item: it,
                item_name: name,
            });
        }
        Ok(ReturnDetailView {
            return_tx: ret_tx,
            party,
            items: item_details,
        })
    }
}
