use crate::app_state::AppState;
use tauri::State;
use crate::core::dtos::*;
use crate::core::value_objects::PartyType;

#[tauri::command]
pub fn get_party_dues_summary(state: State<'_, AppState>, party_type: PartyType) -> Result<Vec<PartyDueSummary>, String> {
    state.accounting_service.get_party_dues_summary(party_type).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn settle_bill_payment(state: State<'_, AppState>, payload: SettlePaymentPayload) -> Result<(), String> {
    state.accounting_service.settle_bill_payment(payload).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn get_party_ledger(state: State<'_, AppState>, party_id: i64) -> Result<PartyLedgerView, String> {
    state.accounting_service.get_party_ledger(party_id).map_err(|e| e.to_string())
}
