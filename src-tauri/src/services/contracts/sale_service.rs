use crate::core::dtos::{
    BillEditGuard, BillPreview, CreateSalePayload, DeleteBillResult, Paginated, PreviewSaleLinePayload,
    PreviewSalePayload, SaleDetailView, SaleHistoryRow, SaleLinePreview, SaleSummaryResponse, SalesHistoryFilter,
};
use crate::core::error::AppError;

pub trait SaleService: Send + Sync {
    /// Generates next sequential sale transaction code (e.g., "SAL-1043").
    fn get_next_sale_code(&self) -> Result<String, AppError>;

    /// Executes complete atomic checkout workflow.
    fn create_sale(&self, payload: CreateSalePayload) -> Result<SaleSummaryResponse, AppError>;

    /// Paginated history, newest first: filtered, sorted and paged in SQL (see `SalesHistoryFilter`).
    fn get_sales_history(
        &self,
        filter: SalesHistoryFilter,
    ) -> Result<Paginated<SaleHistoryRow>, AppError>;

    /// Retrieves detailed breakdown of a sale including lines, bundles, and FIFO cost breakdown.
    fn get_sale_details(&self, sale_id: i64) -> Result<SaleDetailView, AppError>;

    /// FR-20: replaces a sale (same id and code): reverse the old stock, ledger and charges, then
    /// repost the payload through the create rules, in one transaction. `BillLocked` when guarded.
    fn update_sale(&self, sale_id: i64, payload: CreateSalePayload) -> Result<SaleSummaryResponse, AppError>;

    /// DD-56: dry run of `create_sale` (`sale_id = None`) or `update_sale` in a transaction that is dropped.
    /// Business errors come back in `BillPreview.errors`; the payment is optional.
    fn preview_sale(&self, sale_id: Option<i64>, payload: PreviewSalePayload) -> Result<BillPreview, AppError>;

    /// FR-47 / DD-82: the entry row's line total, and the rate a typed total implies (no database).
    fn preview_sale_line(&self, payload: PreviewSaleLinePayload) -> Result<SaleLinePreview, AppError>;

    /// FR-20: deletes a sale and every effect it had. The code is never reissued.
    fn delete_sale(&self, sale_id: i64) -> Result<DeleteBillResult, AppError>;

    /// FR-20 pre-check for the UI (same evaluation as the commands).
    fn get_sale_edit_guard(&self, sale_id: i64) -> Result<BillEditGuard, AppError>;
}

impl<T: SaleService + ?Sized> SaleService for std::sync::Arc<T> {
    fn get_next_sale_code(&self) -> Result<String, AppError> {
        (**self).get_next_sale_code()
    }

    fn create_sale(&self, payload: CreateSalePayload) -> Result<SaleSummaryResponse, AppError> {
        (**self).create_sale(payload)
    }

    fn get_sales_history(
        &self,
        filter: SalesHistoryFilter,
    ) -> Result<Paginated<SaleHistoryRow>, AppError> {
        (**self).get_sales_history(filter)
    }

    fn get_sale_details(&self, sale_id: i64) -> Result<SaleDetailView, AppError> {
        (**self).get_sale_details(sale_id)
    }

    fn update_sale(&self, sale_id: i64, payload: CreateSalePayload) -> Result<SaleSummaryResponse, AppError> {
        (**self).update_sale(sale_id, payload)
    }

    fn preview_sale(&self, sale_id: Option<i64>, payload: PreviewSalePayload) -> Result<BillPreview, AppError> {
        (**self).preview_sale(sale_id, payload)
    }

    fn preview_sale_line(&self, payload: PreviewSaleLinePayload) -> Result<SaleLinePreview, AppError> {
        (**self).preview_sale_line(payload)
    }

    fn delete_sale(&self, sale_id: i64) -> Result<DeleteBillResult, AppError> {
        (**self).delete_sale(sale_id)
    }

    fn get_sale_edit_guard(&self, sale_id: i64) -> Result<BillEditGuard, AppError> {
        (**self).get_sale_edit_guard(sale_id)
    }
}
