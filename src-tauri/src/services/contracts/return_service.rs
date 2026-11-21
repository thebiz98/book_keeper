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

impl<T: ReturnService + ?Sized> ReturnService for std::sync::Arc<T> {
    fn get_next_return_code(&self) -> Result<String, AppError> {
        (**self).get_next_return_code()
    }

    fn create_customer_return(&self, payload: CustomerReturnPayload) -> Result<String, AppError> {
        (**self).create_customer_return(payload)
    }

    fn create_vendor_return(&self, payload: VendorReturnPayload) -> Result<String, AppError> {
        (**self).create_vendor_return(payload)
    }

    fn get_return_details(&self, return_id: i64) -> Result<ReturnDetailView, AppError> {
        (**self).get_return_details(return_id)
    }
}
