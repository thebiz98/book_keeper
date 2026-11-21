use crate::app_state::AppState;
use tauri::State;
use crate::core::entities::{CreatePartyPayload, Party, UpdatePartyPayload};
use crate::core::value_objects::PartyType;

#[tauri::command]
pub fn create_party(state: State<'_, AppState>, payload: CreatePartyPayload) -> Result<i64, String> {
    state.party_service.create_party(payload).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn update_party(state: State<'_, AppState>, party_id: i64, payload: UpdatePartyPayload) -> Result<(), String> {
    state.party_service.update_party(party_id, payload).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn get_party(state: State<'_, AppState>, party_id: i64) -> Result<Party, String> {
    state.party_service.get_party(party_id).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn list_parties(state: State<'_, AppState>, party_type: Option<PartyType>, active_only: Option<bool>) -> Result<Vec<Party>, String> {
    state.party_service.list_parties(party_type, active_only.unwrap_or(true)).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn search_parties(state: State<'_, AppState>, query: String, party_type: Option<PartyType>) -> Result<Vec<Party>, String> {
    state.party_service.search_parties(&query, party_type).map_err(|e| e.to_string())
}
