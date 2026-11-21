use crate::core::dtos::{LowStockItem, PnLReport, RecentTransaction, StockValuationReport};
use crate::core::error::AppError;

/// Read-only SQL aggregates behind the reports/dashboard (LLD §6.10 Metric Definitions).
/// Every method is one (or a few fixed) aggregate query; nothing loads whole tables.
/// Amounts come back already rounded to 2 decimals. Cash buckets are NOT here: they come from
/// `SettlementRepository::sum_by_type_kind_between` via the single `ledger_cash_summary`.
pub trait ReportRepository {
    /// Full P&L for inclusive `[from, to]` (`YYYY-MM-DD`).
    fn pnl(&self, from: &str, to: &str) -> Result<PnLReport, AppError>;
    /// `(amount, count)` of sale bills dated `[from, to]`.
    fn sales_totals(&self, from: &str, to: &str) -> Result<(f64, usize), AppError>;
    /// `(amount, count)` of PURCHASE-source bills dated `[from, to]`.
    fn purchase_totals(&self, from: &str, to: &str) -> Result<(f64, usize), AppError>;
    /// `(customer_receivables, vendor_payables)`: balance_due > 0, payables PURCHASE source only.
    fn dues_totals(&self) -> Result<(f64, f64), AppError>;
    fn stock_valuation(&self) -> Result<StockValuationReport, AppError>;
    /// Active, low-stock items ordered by `current_stock ASC, name`. Not capped.
    fn low_stock_items(&self) -> Result<Vec<LowStockItem>, AppError>;
    /// `(bucket, sales_amount, purchase_amount)` grouped by the first `key_len` chars of the
    /// date (10 = day, 7 = month), for dates in `[from, to]`. Only non-empty buckets.
    fn trend_buckets(&self, from: &str, to: &str, key_len: usize) -> Result<Vec<(String, f64, f64)>, AppError>;
    /// Newest `limit` sales / PURCHASE bills / returns merged, newest first.
    fn recent_transactions(&self, limit: usize) -> Result<Vec<RecentTransaction>, AppError>;
}
