use serde::{Deserialize, Serialize};

use crate::core::dtos::{validate_date_range, PageRequest};
use crate::core::entities::Expense;
use crate::core::error::AppError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateExpenseTypePayload {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpensePayload {
    pub expense_date: String,
    pub expense_type_id: i64,
    pub amount: f64,
    pub notes: Option<String>,
}

/// Expenses list filter: inclusive `YYYY-MM-DD` range, type, and a case-insensitive search over
/// code / type name / notes. Newest first. Page defaults: page 1, limit 50 (max 200).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExpenseFilter {
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(default)]
    pub expense_type_id: Option<i64>,
    #[serde(default)]
    pub search_query: Option<String>,
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub limit: Option<usize>,
}

impl ExpenseFilter {
    pub fn validate(&self) -> Result<PageRequest, AppError> {
        validate_date_range(self.start_date.as_deref(), self.end_date.as_deref())?;
        PageRequest::new(self.page, self.limit)
    }
}

/// `period_total` is the sum over ALL rows matching the filter, not just this page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpenseListResponse {
    pub items: Vec<Expense>,
    pub total: usize,
    pub page: usize,
    pub limit: usize,
    pub period_total: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpenseTypeTotal {
    pub expense_type_id: i64,
    pub expense_type_name: String,
    pub amount: f64,
}
