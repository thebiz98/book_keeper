//! DD-90 item half (v1.7 FR-55, LLD 33): writes that would use a soft-deleted item fail with
//! `AppError::DeletedReference { entity: ITEM }`. A bill line that only references it, unchanged,
//! stays as it is. The twin of `party_guard`.
use crate::core::entities::Item;
use crate::core::error::AppError;
use crate::data::contracts::unit_of_work::TransactionContext;

pub const ITEM_ENTITY: &str = "ITEM";

/// The `DeletedReference` error for a deleted item.
pub fn deleted_item(item: &Item) -> AppError {
    AppError::deleted_reference(ITEM_ENTITY, item.id, &item.name)
}

/// `Ok(())` for an active item, else `DeletedReference(ITEM)`.
pub fn ensure_active_item(item: &Item) -> Result<(), AppError> {
    if item.is_active {
        Ok(())
    } else {
        Err(deleted_item(item))
    }
}

/// Loads the item and refuses a deleted one.
pub fn require_active_item(tx: &mut dyn TransactionContext, item_id: i64) -> Result<Item, AppError> {
    let item = tx
        .items()
        .find_by_id(item_id)?
        .ok_or_else(|| AppError::not_found("Item", item_id.to_string()))?;
    ensure_active_item(&item)?;
    Ok(item)
}
