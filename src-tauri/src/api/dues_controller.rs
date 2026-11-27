use crate::app_state::AppState;
use tauri::State;
use crate::core::error::AppError;
use crate::core::dtos::*;
use crate::core::value_objects::PartyType;

#[tauri::command]
pub async fn get_party_dues_summary(state: State<'_, AppState>, party_type: PartyType) -> Result<Vec<PartyDueSummary>, AppError> {
    let svc = state.accounting_service.clone();
    super::run_blocking(move || svc.get_party_dues_summary(party_type)).await
}
#[tauri::command]
pub async fn settle_bill_payment(state: State<'_, AppState>, payload: SettlePaymentPayload) -> Result<SettlementResult, AppError> {
    let svc = state.accounting_service.clone();
    super::run_blocking(move || svc.settle_bill_payment(payload)).await
}
#[tauri::command]
pub async fn get_party_ledger(state: State<'_, AppState>, party_id: i64) -> Result<PartyLedgerView, AppError> {
    let svc = state.accounting_service.clone();
    super::run_blocking(move || svc.get_party_ledger(party_id)).await
}
#[tauri::command]
pub async fn settle_bills_batch(state: State<'_, AppState>, payload: SettleBatchPayload) -> Result<SettlementResult, AppError> {
    let svc = state.accounting_service.clone();
    super::run_blocking(move || svc.settle_bills_batch(payload)).await
}
#[tauri::command]
pub async fn update_settlement(state: State<'_, AppState>, settlement_id: i64, payload: UpdateSettlementPayload) -> Result<crate::core::entities::PaymentSettlement, AppError> {
    let svc = state.accounting_service.clone();
    super::run_blocking(move || svc.update_settlement(settlement_id, payload)).await
}
#[tauri::command]
pub async fn get_party_open_bills(state: State<'_, AppState>, party_id: i64, transaction_type: crate::core::value_objects::TransactionType) -> Result<Vec<OpenBillRow>, AppError> {
    let svc = state.accounting_service.clone();
    super::run_blocking(move || svc.get_party_open_bills(party_id, transaction_type)).await
}
#[tauri::command]
pub async fn get_settlement_history(state: State<'_, AppState>, filter: SettlementHistoryFilter) -> Result<SettlementHistoryPage, AppError> {
    let svc = state.accounting_service.clone();
    super::run_blocking(move || svc.get_settlement_history(filter)).await
}
#[tauri::command]
pub async fn delete_settlement(state: State<'_, AppState>, settlement_id: i64) -> Result<DeleteSettlementResult, AppError> {
    let svc = state.accounting_service.clone();
    super::run_blocking(move || svc.delete_settlement(settlement_id)).await
}
#[tauri::command]
pub async fn preview_settlement_allocation(state: State<'_, AppState>, party_id: i64, transaction_type: crate::core::value_objects::TransactionType, amount: f64) -> Result<Vec<SettleAllocation>, AppError> {
    let svc = state.accounting_service.clone();
    super::run_blocking(move || svc.preview_settlement_allocation(party_id, transaction_type, amount)).await
}
