use serde::{Deserialize, Serialize};

/// Expense category master row (FR-18). Names are unique case-insensitively.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpenseType {
    pub id: i64,
    pub name: String,
    pub is_active: bool,
    pub created_at: String,
}

/// One cash expense paid on `expense_date`. `expense_type_name` is the snapshot of the type name
/// at the time of the last save (DD-30).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expense {
    pub id: i64,
    pub expense_code: String,
    pub expense_date: String,
    pub expense_type_id: i64,
    pub expense_type_name: String,
    pub amount: f64,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
}
