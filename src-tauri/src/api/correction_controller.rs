//! FR-20: the UI pre-check for bill edit / delete. Dispatches to the owning service so the answer
//! is exactly what the command itself would enforce.
use crate::app_state::AppState;
use crate::core::dtos::{BillEditGuard, CorrectionTarget};
use crate::core::error::AppError;
use tauri::State;

#[tauri::command]
pub async fn get_bill_edit_guard(
    state: State<'_, AppState>,
    target: CorrectionTarget,
    id: i64,
) -> Result<BillEditGuard, AppError> {
    let (sales, purchases, returns) =
        (state.sale_service.clone(), state.purchase_service.clone(), state.return_service.clone());
    super::run_blocking(move || match target {
        CorrectionTarget::Sale => sales.get_sale_edit_guard(id),
        CorrectionTarget::Purchase => purchases.get_purchase_edit_guard(id),
        CorrectionTarget::Return => returns.get_return_delete_guard(id),
    })
    .await
}
