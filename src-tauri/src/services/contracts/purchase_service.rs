use crate::core::dtos::{
    CreatePurchasePayload, PaginatedList, PurchaseDetailView, PurchaseHistoryFilter,
};
use crate::core::entities::PurchaseTransaction;
use crate::core::error::AppError;

pub trait PurchaseService: Send + Sync {
    fn get_next_purchase_code(&self) -> Result<String, AppError>;
    fn create_purchase(&self, payload: CreatePurchasePayload) -> Result<i64, AppError>;
    fn get_purchases_history(
        &self,
        filter: PurchaseHistoryFilter,
    ) -> Result<PaginatedList<PurchaseTransaction>, AppError>;
    fn get_purchase_details(&self, purchase_id: i64) -> Result<PurchaseDetailView, AppError>;
}
