use std::sync::Arc;
use crate::core::dtos::{
    CustomerReturnPayload, RecordRefundPayload, ReturnDetailView, ReturnItemDetail,
    VendorReturnPayload,
};
use crate::core::entities::{ReturnItem, ReturnTransaction};
use crate::core::error::AppError;
use crate::core::value_objects::{ReturnType, SettlementType, StockCondition};
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
        let mut tx = self.uow.begin_transaction()?;
        tx.returns().get_next_return_code()
    }

    fn create_customer_return(&self, payload: CustomerReturnPayload) -> Result<String, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let mut total_refund = 0.0;
        let return_code = payload
            .return_code
            .clone()
            .unwrap_or_else(|| "RET-1001".to_string());
        let ret_tx = ReturnTransaction {
            id: 0,
            return_code: return_code.clone(),
            return_date: payload.return_date.clone(),
            return_type: ReturnType::Customer,
            party_id: payload.party_id,
            original_transaction_id: payload.original_sale_id,
            settlement_type: payload.settlement_type,
            total_amount: 0.0,
            reason: payload.reason,
            notes: payload.notes,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let return_id = tx.returns().create_transaction(&ret_tx)?;
        for item_payload in &payload.items {
            let line_total = item_payload.quantity * item_payload.unit_price;
            total_refund += line_total;
            let ret_item = ReturnItem {
                id: 0,
                return_transaction_id: return_id,
                item_id: item_payload.item_id,
                sale_item_id: item_payload.sale_item_id,
                purchase_item_id: None,
                quantity: item_payload.quantity,
                unit_price: item_payload.unit_price,
                total_amount: line_total,
                condition: item_payload.condition,
            };
            tx.returns().create_return_item(&ret_item)?;
            if item_payload.condition == StockCondition::Restockable {
                if let Some(sale_item_id) = item_payload.sale_item_id {
                    self.inventory_service.restore_stock_for_return(
                        &mut *tx,
                        sale_item_id,
                        item_payload.item_id,
                        item_payload.quantity,
                    )?;
                }
            }
        }
        if payload.settlement_type == SettlementType::CashRefund {
            let refund_payload = RecordRefundPayload {
                party_id: payload.party_id,
                transaction_type: "SALE".to_string(),
                transaction_id: payload.original_sale_id,
                refund_date: payload.return_date,
                amount: total_refund,
                payment_mode: Some("CASH".to_string()),
                notes: Some("Customer return refund".to_string()),
            };
            self.payment_service
                .record_refund(&mut *tx, refund_payload)?;
        } else if payload.settlement_type == SettlementType::ReduceDue {
            if let Some(sale_id) = payload.original_sale_id {
                self.payment_service.reduce_transaction_balance(
                    &mut *tx,
                    "SALE",
                    sale_id,
                    total_refund,
                )?;
            }
        }
        tx.commit()?;
        Ok(return_code)
    }

    fn create_vendor_return(&self, payload: VendorReturnPayload) -> Result<String, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let return_code = format!(
            "VRET-{}",
            (tx.returns().list_returns(Some(ReturnType::Vendor), 0, 1000)?.len() as i64) + 1001
        );
        let mut total_amount = 0.0;
        for item in &payload.items {
            total_amount += item.quantity * item.unit_cost;
        }

        let ret_tx = ReturnTransaction {
            id: 0,
            return_code: return_code.clone(),
            return_date: payload.return_date.clone(),
            return_type: ReturnType::Vendor,
            party_id: payload.party_id,
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
            let ret_item = ReturnItem {
                id: 0,
                return_transaction_id: return_id,
                item_id: item.item_id,
                sale_item_id: None,
                purchase_item_id: item.purchase_item_id,
                quantity: item.quantity,
                unit_price: item.unit_cost,
                total_amount: line_total,
                condition: StockCondition::Restockable,
            };
            tx.returns().create_return_item(&ret_item)?;
            self.inventory_service.remove_stock_for_vendor_return(
                &mut *tx,
                item.item_id,
                item.purchase_item_id,
                item.quantity,
            )?;
        }

        if payload.settlement_type == SettlementType::CashRefund {
            let refund_payload = RecordRefundPayload {
                party_id: payload.party_id,
                transaction_type: "PURCHASE".to_string(),
                transaction_id: payload.original_purchase_id,
                refund_date: payload.return_date,
                amount: total_amount,
                payment_mode: Some("CASH".to_string()),
                notes: Some("Vendor return refund".to_string()),
            };
            self.payment_service
                .record_refund(&mut *tx, refund_payload)?;
        } else if payload.settlement_type == SettlementType::ReduceDue {
            if let Some(pur_id) = payload.original_purchase_id {
                self.payment_service.reduce_transaction_balance(
                    &mut *tx,
                    "PURCHASE",
                    pur_id,
                    total_amount,
                )?;
            }
        }

        tx.commit()?;
        Ok(return_code)
    }

    fn get_return_details(&self, return_id: i64) -> Result<ReturnDetailView, AppError> {
        let mut tx = self.uow.begin_transaction()?;
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
        let items = tx.returns().list_items_by_transaction_id(return_id)?;
        let mut item_details = Vec::new();
        for it in items {
            let name = tx
                .items()
                .find_by_id(it.item_id)?
                .map(|i| i.name)
                .unwrap_or_else(|| "Unknown Item".to_string());
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
