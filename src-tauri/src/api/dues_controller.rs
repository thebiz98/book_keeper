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
