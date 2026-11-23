use crate::core::dtos::{
    BillEditGuard, CreatePurchasePayload, DeleteBillResult, Paginated, PurchaseDetailView,
    PurchaseHistoryFilter, PurchaseHistoryRow, PurchaseSummaryResponse,
};
use crate::core::error::AppError;

pub trait PurchaseService: Send + Sync {
    fn get_next_purchase_code(&self) -> Result<String, AppError>;
    fn create_purchase(&self, payload: CreatePurchasePayload) -> Result<PurchaseSummaryResponse, AppError>;
    fn get_purchases_history(
        &self,
        filter: PurchaseHistoryFilter,
    ) -> Result<Paginated<PurchaseHistoryRow>, AppError>;
    fn get_purchase_details(&self, purchase_id: i64) -> Result<PurchaseDetailView, AppError>;

    /// FR-20: edits a purchase (same id and code). Lines carrying `purchase_item_id` update that
    /// lot in place, other lines create new lots, omitted lots are removed. `BillLocked` when guarded.
    fn update_purchase(
        &self,
        purchase_id: i64,
        payload: CreatePurchasePayload,
    ) -> Result<PurchaseSummaryResponse, AppError>;

    /// FR-20: deletes a purchase whose lots are all unreferenced. The code is never reissued.
    fn delete_purchase(&self, purchase_id: i64) -> Result<DeleteBillResult, AppError>;

    /// FR-20 pre-check for the UI (same evaluation as the commands).
    fn get_purchase_edit_guard(&self, purchase_id: i64) -> Result<BillEditGuard, AppError>;
}

impl<T: PurchaseService + ?Sized> PurchaseService for std::sync::Arc<T> {
    fn get_next_purchase_code(&self) -> Result<String, AppError> {
        (**self).get_next_purchase_code()
    }

    fn create_purchase(&self, payload: CreatePurchasePayload) -> Result<PurchaseSummaryResponse, AppError> {
        (**self).create_purchase(payload)
    }

    fn get_purchases_history(
        &self,
        filter: PurchaseHistoryFilter,
    ) -> Result<Paginated<PurchaseHistoryRow>, AppError> {
        (**self).get_purchases_history(filter)
    }

    fn get_purchase_details(&self, purchase_id: i64) -> Result<PurchaseDetailView, AppError> {
        (**self).get_purchase_details(purchase_id)
    }

    fn update_purchase(
        &self,
        purchase_id: i64,
        payload: CreatePurchasePayload,
    ) -> Result<PurchaseSummaryResponse, AppError> {
        (**self).update_purchase(purchase_id, payload)
    }

    fn delete_purchase(&self, purchase_id: i64) -> Result<DeleteBillResult, AppError> {
        (**self).delete_purchase(purchase_id)
    }

    fn get_purchase_edit_guard(&self, purchase_id: i64) -> Result<BillEditGuard, AppError> {
        (**self).get_purchase_edit_guard(purchase_id)
    }
}
