use crate::core::dtos::{CreateExpenseTypePayload, ExpenseFilter, ExpenseListResponse, ExpensePayload};
use crate::core::entities::{Expense, ExpenseType};
use crate::core::error::AppError;

pub trait ExpenseService: Send + Sync {
    /// Live typeahead: blank query returns the active types (by name), otherwise FTS prefix hits
    /// in rank order. `limit` defaults to 8 and is capped at 50.
    fn search_expense_types(&self, query: &str, limit: Option<usize>) -> Result<Vec<ExpenseType>, AppError>;
    /// Every active type sorted by name.
    fn list_expense_types(&self) -> Result<Vec<ExpenseType>, AppError>;
    /// Idempotent: an existing name (case-insensitive) returns the existing type (R5, DD-47).
    fn create_expense_type(&self, payload: CreateExpenseTypePayload) -> Result<ExpenseType, AppError>;
    /// Renames the master only; a clash with another type is a `ValidationError` on `name`.
    fn update_expense_type(&self, id: i64, payload: CreateExpenseTypePayload) -> Result<ExpenseType, AppError>;
    /// Display-only preview of the next `EXP-` code.
    fn get_next_expense_code(&self) -> Result<String, AppError>;
    fn create_expense(&self, payload: ExpensePayload) -> Result<Expense, AppError>;
    fn update_expense(&self, id: i64, payload: ExpensePayload) -> Result<Expense, AppError>;
    fn delete_expense(&self, id: i64) -> Result<(), AppError>;
    fn list_expenses(&self, filter: ExpenseFilter) -> Result<ExpenseListResponse, AppError>;
}

impl<T: ExpenseService + ?Sized> ExpenseService for std::sync::Arc<T> {
    fn search_expense_types(&self, query: &str, limit: Option<usize>) -> Result<Vec<ExpenseType>, AppError> {
        (**self).search_expense_types(query, limit)
    }
    fn list_expense_types(&self) -> Result<Vec<ExpenseType>, AppError> {
        (**self).list_expense_types()
    }
    fn create_expense_type(&self, payload: CreateExpenseTypePayload) -> Result<ExpenseType, AppError> {
        (**self).create_expense_type(payload)
    }
    fn update_expense_type(&self, id: i64, payload: CreateExpenseTypePayload) -> Result<ExpenseType, AppError> {
        (**self).update_expense_type(id, payload)
    }
    fn get_next_expense_code(&self) -> Result<String, AppError> {
        (**self).get_next_expense_code()
    }
    fn create_expense(&self, payload: ExpensePayload) -> Result<Expense, AppError> {
        (**self).create_expense(payload)
    }
    fn update_expense(&self, id: i64, payload: ExpensePayload) -> Result<Expense, AppError> {
        (**self).update_expense(id, payload)
    }
    fn delete_expense(&self, id: i64) -> Result<(), AppError> {
        (**self).delete_expense(id)
    }
    fn list_expenses(&self, filter: ExpenseFilter) -> Result<ExpenseListResponse, AppError> {
        (**self).list_expenses(filter)
    }
}
