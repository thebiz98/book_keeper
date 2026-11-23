use crate::core::dtos::{
    BillEditGuard, CustomerReturnPayload, DeleteReturnResult, Paginated, ReturnDetailView, ReturnResult,
    ReturnsFilter, VendorReturnPayload,
};
use crate::core::entities::ReturnTransaction;
use crate::core::error::AppError;

pub trait ReturnService: Send + Sync {
    /// Display-only preview of the next `RET-` code; the real code is assigned at creation.
    fn get_next_return_code(&self) -> Result<String, AppError>;
    fn create_customer_return(&self, payload: CustomerReturnPayload) -> Result<ReturnResult, AppError>;
    fn create_vendor_return(&self, payload: VendorReturnPayload) -> Result<ReturnResult, AppError>;
    /// DD-56: dry run of `create_customer_return` (transaction dropped); same result, same errors.
    fn preview_customer_return(&self, payload: CustomerReturnPayload) -> Result<ReturnResult, AppError>;
    /// DD-56: dry run of `create_vendor_return` (transaction dropped); same result, same errors.
    fn preview_vendor_return(&self, payload: VendorReturnPayload) -> Result<ReturnResult, AppError>;
    /// Newest first, paginated in SQL (`page` 1-based, default 1; `limit` default 50).
    fn list_returns(&self, filter: ReturnsFilter) -> Result<Paginated<ReturnTransaction>, AppError>;
    fn get_return_details(&self, return_id: i64) -> Result<ReturnDetailView, AppError>;

    /// FR-20: deletes a return and reverses its stock and ledger effects (newest first per bill).
    fn delete_return(&self, return_id: i64) -> Result<DeleteReturnResult, AppError>;

    /// FR-20 pre-check for the UI (same evaluation as the command).
    fn get_return_delete_guard(&self, return_id: i64) -> Result<BillEditGuard, AppError>;
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

    fn preview_customer_return(&self, payload: CustomerReturnPayload) -> Result<ReturnResult, AppError> {
        (**self).preview_customer_return(payload)
    }

    fn preview_vendor_return(&self, payload: VendorReturnPayload) -> Result<ReturnResult, AppError> {
        (**self).preview_vendor_return(payload)
    }

    fn list_returns(&self, filter: ReturnsFilter) -> Result<Paginated<ReturnTransaction>, AppError> {
        (**self).list_returns(filter)
    }

    fn get_return_details(&self, return_id: i64) -> Result<ReturnDetailView, AppError> {
        (**self).get_return_details(return_id)
    }

    fn delete_return(&self, return_id: i64) -> Result<DeleteReturnResult, AppError> {
        (**self).delete_return(return_id)
    }

    fn get_return_delete_guard(&self, return_id: i64) -> Result<BillEditGuard, AppError> {
        (**self).get_return_delete_guard(return_id)
    }
}
