use crate::core::dtos::{
    BillEditGuard, BillPreview, CreatePurchasePayload, PreviewPurchasePayload, LotPriceUpdateResult, UpdateLotPricePayload, DeleteBillResult, Paginated, PurchaseDetailView,
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

    /// FR-25: changes one lot's Purchase Price in a single transaction (bill + payment rule for
    /// PURCHASE lots, re-cost only for OPENING / ADJUSTMENT, RETURN rejected).
    fn update_lot_purchase_price(&self, payload: UpdateLotPricePayload) -> Result<LotPriceUpdateResult, AppError>;

    /// DD-56: dry run of `create_purchase` (`purchase_id = None`) / `update_purchase` in a transaction
    /// that is dropped. Business errors come back in `BillPreview.errors`; the payment is optional.
    fn preview_purchase(
        &self,
        purchase_id: Option<i64>,
        payload: PreviewPurchasePayload,
    ) -> Result<BillPreview, AppError>;

    /// DD-56: dry run of `update_lot_purchase_price`; fails with the same error the command would.
    fn preview_lot_purchase_price(&self, payload: UpdateLotPricePayload) -> Result<LotPriceUpdateResult, AppError>;
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

    fn update_lot_purchase_price(&self, payload: UpdateLotPricePayload) -> Result<LotPriceUpdateResult, AppError> {
        (**self).update_lot_purchase_price(payload)
    }

    fn preview_purchase(
        &self,
        purchase_id: Option<i64>,
        payload: PreviewPurchasePayload,
    ) -> Result<BillPreview, AppError> {
        (**self).preview_purchase(purchase_id, payload)
    }

    fn preview_lot_purchase_price(&self, payload: UpdateLotPricePayload) -> Result<LotPriceUpdateResult, AppError> {
        (**self).preview_lot_purchase_price(payload)
    }
}
