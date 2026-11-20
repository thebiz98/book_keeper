use crate::core::dtos::{PaginatedList, SalesFilter};
use crate::core::entities::{SaleItem, SaleTransaction};
use crate::core::error::AppError;
use crate::core::value_objects::PaymentStatus;

pub trait SaleRepository {
    fn create_transaction(&mut self, tx: &SaleTransaction) -> Result<i64, AppError>;
    fn create_sale_item(&mut self, line: &SaleItem) -> Result<i64, AppError>;
    fn find_transaction_by_id(&self, id: i64) -> Result<Option<SaleTransaction>, AppError> {
        self.find_by_id(id)
    }
    fn find_by_id(&self, id: i64) -> Result<Option<SaleTransaction>, AppError> {
        self.find_transaction_by_id(id)
    }
    fn find_by_code(&self, _code: &str) -> Result<Option<SaleTransaction>, AppError> {
        Ok(None)
    }
    fn get_next_transaction_code(&self) -> Result<String, AppError> {
        let latest = self.get_latest_code()?;
        let next_num = match latest {
            Some(code) if code.starts_with("SAL-") => code[4..].parse::<u64>().unwrap_or(1000) + 1,
            _ => 1001,
        };
        Ok(format!("SAL-{}", next_num))
    }
    fn get_latest_code(&self) -> Result<Option<String>, AppError> {
        Ok(None)
    }
    fn update_balance_due(
        &mut self,
        tx_id: i64,
        new_balance: f64,
        status: PaymentStatus,
    ) -> Result<(), AppError>;
    fn list_sales(
        &self,
        _start_date: Option<&str>,
        _end_date: Option<&str>,
        _status: Option<PaymentStatus>,
        _party_id: Option<i64>,
        _offset: usize,
        _limit: usize,
    ) -> Result<Vec<SaleTransaction>, AppError> {
        Ok(Vec::new())
    }
    fn list_items_by_transaction_id(&self, _tx_id: i64) -> Result<Vec<SaleItem>, AppError> {
        Ok(Vec::new())
    }
    fn list_unpaid_by_party_id(&self, _party_id: i64) -> Result<Vec<SaleTransaction>, AppError> {
        Ok(Vec::new())
    }
    fn list_transactions(&self, filter: &SalesFilter) -> Result<PaginatedList<SaleTransaction>, AppError> {
        let items = self.list_sales(
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
