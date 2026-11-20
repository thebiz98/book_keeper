use crate::core::dtos::{
    CreateSalePayload, PaginatedList, SaleDetailView, SaleSummaryResponse, SalesHistoryFilter,
};
use crate::core::entities::SaleTransaction;
use crate::core::error::AppError;

pub trait SaleService: Send + Sync {
    /// Generates next sequential sale transaction code (e.g., "SAL-1043").
    fn get_next_sale_code(&self) -> Result<String, AppError>;

    /// Executes complete atomic checkout workflow.
    fn create_sale(&self, payload: CreateSalePayload) -> Result<SaleSummaryResponse, AppError>;

    /// Retrieves paginated sale history.
    fn get_sales_history(
        &self,
        filter: SalesHistoryFilter,
    ) -> Result<PaginatedList<SaleTransaction>, AppError>;

    /// Retrieves detailed breakdown of a sale including lines, bundles, and FIFO cost breakdown.
    fn get_sale_details(&self, sale_id: i64) -> Result<SaleDetailView, AppError>;
}
