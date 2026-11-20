use crate::core::entities::{ReturnItem, ReturnTransaction};
use crate::core::error::AppError;
use crate::core::value_objects::ReturnType;

pub trait ReturnRepository {
    fn create_return_transaction(&mut self, tx: &ReturnTransaction) -> Result<i64, AppError> {
        self.create_transaction(tx)
    }
    fn create_transaction(&mut self, tx: &ReturnTransaction) -> Result<i64, AppError> {
        self.create_return_transaction(tx)
    }
    fn create_return_item(&mut self, item: &ReturnItem) -> Result<i64, AppError>;
    fn find_return_by_id(&self, id: i64) -> Result<Option<ReturnTransaction>, AppError> {
        self.find_by_id(id)
    }
    fn find_by_id(&self, id: i64) -> Result<Option<ReturnTransaction>, AppError> {
        self.find_return_by_id(id)
    }
    fn get_next_return_code(&self) -> Result<String, AppError> {
        let code = self.get_latest_code()?;
        Ok(code.unwrap_or_else(|| "RET-1001".to_string()))
    }
    fn get_latest_code(&self) -> Result<Option<String>, AppError> {
        Ok(None)
    }
    fn list_returns(
        &self,
        _return_type: Option<ReturnType>,
        _offset: usize,
        _limit: usize,
    ) -> Result<Vec<ReturnTransaction>, AppError> {
        Ok(Vec::new())
    }
    fn list_items_by_transaction_id(&self, _tx_id: i64) -> Result<Vec<ReturnItem>, AppError> {
        Ok(Vec::new())
    }
}
