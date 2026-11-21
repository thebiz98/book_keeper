use crate::core::dtos::{
    CreateSalePayload, Paginated, SaleDetailView, SaleHistoryRow, SaleSummaryResponse, SalesHistoryFilter,
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
}
