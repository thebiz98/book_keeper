use crate::core::dtos::{CustomerReturnPayload, ReturnDetailView, VendorReturnPayload};
use crate::core::error::AppError;

pub trait ReturnService: Send + Sync {
    fn get_next_return_code(&self) -> Result<String, AppError> {
        Ok("RET-1001".to_string())
    }
    fn create_customer_return(&self, payload: CustomerReturnPayload) -> Result<String, AppError>;
    fn create_vendor_return(&self, payload: VendorReturnPayload) -> Result<String, AppError>;
    fn get_return_details(&self, return_id: i64) -> Result<ReturnDetailView, AppError>;
}
