use crate::core::dtos::{ExpenseFilter, ExpenseTypeTotal};
use crate::core::entities::{Expense, ExpenseType};
use crate::core::error::AppError;

pub trait ExpenseRepository {
    fn create_type(&mut self, t: &ExpenseType) -> Result<i64, AppError>;
    /// NotFound when no row matches.
    fn update_type_name(&mut self, id: i64, name: &str) -> Result<(), AppError>;
    fn find_type_by_id(&self, id: i64) -> Result<Option<ExpenseType>, AppError>;
    /// Case-insensitive exact match.
    fn find_type_by_name(&self, name: &str) -> Result<Option<ExpenseType>, AppError>;
    /// Ordered by name (case-insensitive).
    fn list_types(&self, active_only: bool) -> Result<Vec<ExpenseType>, AppError>;
    /// Active types for the given ids, no particular order.
    fn find_types_by_ids(&self, ids: &[i64]) -> Result<Vec<ExpenseType>, AppError>;
    fn count_types(&self, active_only: bool) -> Result<usize, AppError>;

    fn get_next_expense_code(&self) -> Result<String, AppError>;
    fn create(&mut self, e: &Expense) -> Result<i64, AppError>;
    /// Rewrites every mutable column; NotFound when no row matches.
    fn update(&mut self, e: &Expense) -> Result<(), AppError>;
    /// NotFound when no row matches.
    fn delete(&mut self, id: i64) -> Result<(), AppError>;
    fn find_by_id(&self, id: i64) -> Result<Option<Expense>, AppError>;
    /// `(page rows, total matching, sum of all matching amounts)`; newest first.
    fn list(&self, f: &ExpenseFilter, offset: usize, limit: usize) -> Result<(Vec<Expense>, usize, f64), AppError>;
    fn sum_between(&self, from: &str, to: &str) -> Result<f64, AppError>;
    /// Grouped by type id, labelled with the latest snapshot name; `amount DESC, name`.
    fn totals_by_type_between(&self, from: &str, to: &str) -> Result<Vec<ExpenseTypeTotal>, AppError>;
}
