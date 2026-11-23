use crate::app_state::AppState;
use crate::core::dtos::CreateChargeTypePayload;
use crate::core::entities::ChargeType;
use crate::core::error::AppError;
use crate::core::value_objects::TransactionType;
use tauri::State;

#[tauri::command]
pub async fn search_charge_types(
    state: State<'_, AppState>,
    query: String,
    applies_to: TransactionType,
    limit: Option<usize>,
) -> Result<Vec<ChargeType>, AppError> {
    let svc = state.charge_type_service.clone();
    super::run_blocking(move || svc.search_charge_types(&query, applies_to, limit)).await
}

#[tauri::command]
pub async fn create_charge_type(
    state: State<'_, AppState>,
    payload: CreateChargeTypePayload,
) -> Result<ChargeType, AppError> {
    let svc = state.charge_type_service.clone();
    super::run_blocking(move || svc.create_charge_type(payload)).await
}
