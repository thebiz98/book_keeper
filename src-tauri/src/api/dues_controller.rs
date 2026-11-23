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
pub async fn delete_settlement(state: State<'_, AppState>, settlement_id: i64) -> Result<DeleteSettlementResult, AppError> {
    let svc = state.accounting_service.clone();
    super::run_blocking(move || svc.delete_settlement(settlement_id)).await
}
