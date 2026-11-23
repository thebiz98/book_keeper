//! Bulk deletes used only by the testing-only data reset. Every method runs inside the caller's
//! write transaction; deletes run child-first so foreign keys are never violated.
use crate::core::error::AppError;

pub trait MaintenanceRepository {
    /// Deletes every sale, purchase (except OPENING lots), payment, return, adjustment and expense, and
    /// restores OPENING lots to their full quantity. Items, bundles, parties and expense types stay. Code high-water marks of the
    /// deleted prefixes are cleared so codes restart. Does NOT
    /// touch `items.current_stock`; the caller repairs the stock cache afterwards.
    /// Returns `(table, rows deleted)` in delete order.
    fn clear_transactions(&mut self) -> Result<Vec<(String, i64)>, AppError>;

    /// Deletes the remaining OPENING lots, bundles, items, parties, expense types and all code high-water marks. Call after
    /// `clear_transactions` in the same transaction. Returns `(table, rows deleted)`.
    fn clear_catalog(&mut self) -> Result<Vec<(String, i64)>, AppError>;
}
