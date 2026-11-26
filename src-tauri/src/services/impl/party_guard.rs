//! DD-90 party half (v1.7 FR-54, LLD 32): writes that would use a soft-deleted party fail with
//! `AppError::DeletedReference { entity: PARTY }`. History that only references it stays as is.
use crate::core::entities::Party;
use crate::core::error::AppError;
use crate::core::money::MONEY_EPS;
use crate::data::contracts::unit_of_work::TransactionContext;

pub const PARTY_ENTITY: &str = "PARTY";

/// The `DeletedReference` error for a deleted party.
pub fn deleted_party(party: &Party) -> AppError {
    AppError::deleted_reference(PARTY_ENTITY, party.id, &party.name)
}

/// `Ok(())` for an active party, else `DeletedReference(PARTY)`.
pub fn ensure_active_party(party: &Party) -> Result<(), AppError> {
    if party.is_active {
        Ok(())
    } else {
        Err(deleted_party(party))
    }
}

/// Loads the party and refuses a deleted one.
pub fn require_active_party(tx: &mut dyn TransactionContext, party_id: i64) -> Result<Party, AppError> {
    let party = tx
        .parties()
        .find_by_id(party_id)?
        .ok_or_else(|| AppError::not_found("Party", party_id.to_string()))?;
    ensure_active_party(&party)?;
    Ok(party)
}

/// A bill write may lower or keep a deleted party's due, never raise it (E6).
pub fn check_due_not_raised(
    tx: &mut dyn TransactionContext,
    party_id: Option<i64>,
    old_due: f64,
    new_due: f64,
) -> Result<(), AppError> {
    let Some(pid) = party_id else { return Ok(()) };
    if new_due <= old_due + MONEY_EPS {
        return Ok(());
    }
    require_active_party(tx, pid).map(|_| ())
}
