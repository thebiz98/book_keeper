use crate::app_state::AppState;
use tauri::State;
use crate::core::error::AppError;
use crate::core::entities::{CreatePartyPayload, Party, UpdatePartyPayload};
use crate::core::value_objects::PartyType;

#[tauri::command]
pub async fn create_party(state: State<'_, AppState>, payload: CreatePartyPayload) -> Result<i64, AppError> {
    let svc = state.party_service.clone();
    super::run_blocking(move || svc.create_party(payload)).await
}
#[tauri::command]
pub async fn update_party(state: State<'_, AppState>, party_id: i64, payload: UpdatePartyPayload) -> Result<(), AppError> {
    let svc = state.party_service.clone();
    super::run_blocking(move || svc.update_party(party_id, payload)).await
}
#[tauri::command]
pub async fn get_party(state: State<'_, AppState>, party_id: i64) -> Result<Party, AppError> {
    let svc = state.party_service.clone();
    super::run_blocking(move || svc.get_party(party_id)).await
}
#[tauri::command]
pub async fn list_parties(state: State<'_, AppState>, party_type: Option<PartyType>, active_only: Option<bool>) -> Result<Vec<Party>, AppError> {
    let svc = state.party_service.clone();
    super::run_blocking(move || svc.list_parties(party_type, active_only.unwrap_or(true))).await
}
#[tauri::command]
pub async fn search_parties(state: State<'_, AppState>, query: String, party_type: Option<PartyType>) -> Result<Vec<Party>, AppError> {
    let svc = state.party_service.clone();
    super::run_blocking(move || svc.search_parties(&query, party_type)).await
}
