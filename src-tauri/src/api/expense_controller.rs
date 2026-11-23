use crate::app_state::AppState;
use crate::core::dtos::{CreateExpenseTypePayload, ExpenseFilter, ExpenseListResponse, ExpensePayload};
use crate::core::entities::{Expense, ExpenseType};
use crate::core::error::AppError;
use tauri::State;

#[tauri::command]
pub async fn search_expense_types(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<ExpenseType>, AppError> {
    let svc = state.expense_service.clone();
    super::run_blocking(move || svc.search_expense_types(&query, limit)).await
}
#[tauri::command]
pub async fn list_expense_types(state: State<'_, AppState>) -> Result<Vec<ExpenseType>, AppError> {
    let svc = state.expense_service.clone();
    super::run_blocking(move || svc.list_expense_types()).await
}
#[tauri::command]
pub async fn create_expense_type(
    state: State<'_, AppState>,
    payload: CreateExpenseTypePayload,
) -> Result<ExpenseType, AppError> {
    let svc = state.expense_service.clone();
    super::run_blocking(move || svc.create_expense_type(payload)).await
}
#[tauri::command]
pub async fn update_expense_type(
    state: State<'_, AppState>,
    expense_type_id: i64,
    payload: CreateExpenseTypePayload,
) -> Result<ExpenseType, AppError> {
    let svc = state.expense_service.clone();
    super::run_blocking(move || svc.update_expense_type(expense_type_id, payload)).await
}
#[tauri::command]
pub async fn get_next_expense_code(state: State<'_, AppState>) -> Result<String, AppError> {
    let svc = state.expense_service.clone();
    super::run_blocking(move || svc.get_next_expense_code()).await
}
#[tauri::command]
pub async fn create_expense(state: State<'_, AppState>, payload: ExpensePayload) -> Result<Expense, AppError> {
    let svc = state.expense_service.clone();
    super::run_blocking(move || svc.create_expense(payload)).await
}
#[tauri::command]
pub async fn update_expense(
    state: State<'_, AppState>,
    expense_id: i64,
    payload: ExpensePayload,
) -> Result<Expense, AppError> {
    let svc = state.expense_service.clone();
    super::run_blocking(move || svc.update_expense(expense_id, payload)).await
}
#[tauri::command]
pub async fn delete_expense(state: State<'_, AppState>, expense_id: i64) -> Result<(), AppError> {
    let svc = state.expense_service.clone();
    super::run_blocking(move || svc.delete_expense(expense_id)).await
}
#[tauri::command]
pub async fn list_expenses(state: State<'_, AppState>, filter: ExpenseFilter) -> Result<ExpenseListResponse, AppError> {
    let svc = state.expense_service.clone();
    super::run_blocking(move || svc.list_expenses(filter)).await
}
