use crate::core::dtos::{PaginatedList, PurchaseFilter};
use crate::core::entities::{PurchaseItem, PurchaseTransaction};
use crate::core::error::AppError;
use crate::core::value_objects::PaymentStatus;

pub trait PurchaseRepository {
    fn create_transaction(&mut self, tx: &PurchaseTransaction) -> Result<i64, AppError>;
    fn create_purchase_item(&mut self, line: &PurchaseItem) -> Result<i64, AppError>;
    fn find_transaction_by_id(&self, id: i64) -> Result<Option<PurchaseTransaction>, AppError> {
        self.find_by_id(id)
    }
    fn find_by_id(&self, id: i64) -> Result<Option<PurchaseTransaction>, AppError> {
        self.find_transaction_by_id(id)
    }
    fn find_by_code(&self, _code: &str) -> Result<Option<PurchaseTransaction>, AppError> {
        Ok(None)
    }
    fn get_next_transaction_code(&self) -> Result<String, AppError> {
        let latest = self.get_latest_code()?;
        let next_num = match latest {
            Some(code) if code.starts_with("PUR-") => code[4..].parse::<u64>().unwrap_or(1000) + 1,
            _ => 1001,
        };
        Ok(format!("PUR-{}", next_num))
    }
    fn get_latest_code(&self) -> Result<Option<String>, AppError> {
        Ok(None)
    }

    /// Core FIFO query: returns available active purchase lots ordered chronologically by lot ID
    fn get_active_lots_for_item(&self, item_id: i64) -> Result<Vec<PurchaseItem>, AppError> {
        self.list_active_lots_for_item(item_id)
    }
    fn list_active_lots_for_item(&self, item_id: i64) -> Result<Vec<PurchaseItem>, AppError> {
        self.get_active_lots_for_item(item_id)
    }

    /// Decrements or increments remaining quantity on a specific inward batch
    fn update_remaining_quantity(&mut self, lot_id: i64, new_remaining_qty: f64) -> Result<(), AppError>;
    fn update_balance_due(
        &mut self,
        tx_id: i64,
        new_balance: f64,
        status: PaymentStatus,
    ) -> Result<(), AppError>;
    fn list_purchases(
        &self,
        _start_date: Option<&str>,
        _end_date: Option<&str>,
        _status: Option<PaymentStatus>,
        _party_id: Option<i64>,
        _offset: usize,
        _limit: usize,
    ) -> Result<Vec<PurchaseTransaction>, AppError> {
        Ok(Vec::new())
    }
    fn list_items_by_transaction_id(&self, _tx_id: i64) -> Result<Vec<PurchaseItem>, AppError> {
        Ok(Vec::new())
    }
    fn get_latest_purchase_item_for_item(&self, _item_id: i64) -> Result<Option<PurchaseItem>, AppError> {
        Ok(None)
    }
    fn list_all_lots(&self) -> Result<Vec<PurchaseItem>, AppError> {
        Ok(Vec::new())
    }
    fn list_transactions(&self, filter: &PurchaseFilter) -> Result<PaginatedList<PurchaseTransaction>, AppError> {
        let items = self.list_purchases(
            filter.start_date.as_deref(),
            filter.end_date.as_deref(),
            filter.payment_status,
            filter.party_id,
            filter.page.unwrap_or(1).saturating_sub(1) * filter.limit.unwrap_or(50),
            filter.limit.unwrap_or(50),
        )?;
        let total = items.len();
        Ok(PaginatedList::new(
            items,
            total,
            filter.page.unwrap_or(1),
            filter.limit.unwrap_or(50),
        ))
    }
}
