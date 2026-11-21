use crate::core::dtos::{
    CustomerReturnPayload, PaginatedList, ReturnDetailView, ReturnResult, ReturnsFilter,
    VendorReturnPayload,
};
use crate::core::entities::ReturnTransaction;
use crate::core::error::AppError;

pub trait ReturnService: Send + Sync {
    /// Display-only preview of the next `RET-` code; the real code is assigned at creation.
    fn get_next_return_code(&self) -> Result<String, AppError>;
    fn create_customer_return(&self, payload: CustomerReturnPayload) -> Result<ReturnResult, AppError>;
    fn create_vendor_return(&self, payload: VendorReturnPayload) -> Result<ReturnResult, AppError>;
    /// Newest first, paginated in SQL (`page` 1-based, default 1; `limit` default 50).
    fn list_returns(&self, filter: ReturnsFilter) -> Result<PaginatedList<ReturnTransaction>, AppError>;
    fn get_return_details(&self, return_id: i64) -> Result<ReturnDetailView, AppError>;
}

impl<T: ReturnService + ?Sized> ReturnService for std::sync::Arc<T> {
    fn get_next_return_code(&self) -> Result<String, AppError> {
        (**self).get_next_return_code()
    }

    fn create_customer_return(&self, payload: CustomerReturnPayload) -> Result<ReturnResult, AppError> {
        (**self).create_customer_return(payload)
    }

    fn create_vendor_return(&self, payload: VendorReturnPayload) -> Result<ReturnResult, AppError> {
        (**self).create_vendor_return(payload)
    }

    fn list_returns(&self, filter: ReturnsFilter) -> Result<PaginatedList<ReturnTransaction>, AppError> {
        (**self).list_returns(filter)
    }

    fn get_return_details(&self, return_id: i64) -> Result<ReturnDetailView, AppError> {
        (**self).get_return_details(return_id)
    }
}
