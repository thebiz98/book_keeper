use crate::core::dtos::{DeletePartyResult, DeletedPartyRow, PartyDeletePreview, Paginated};
use crate::core::entities::{CreatePartyPayload, Party, PartyListRow, UpdatePartyPayload};
use crate::core::error::AppError;
use crate::core::value_objects::PartyType;

pub trait PartyService: Send + Sync {
    fn create_party(&self, payload: CreatePartyPayload) -> Result<i64, AppError>;
    fn update_party(&self, id: i64, payload: UpdatePartyPayload) -> Result<(), AppError>;
    fn get_party(&self, id: i64) -> Result<Party, AppError>;
    /// Directory rows (party + `outstanding_balance` + `transaction_count`), by name. NOT
    /// paginated: party pickers need the whole directory. `active_only` defaults to true.
    fn list_parties(
        &self,
        party_type: Option<PartyType>,
        active_only: Option<bool>,
    ) -> Result<Vec<PartyListRow>, AppError>;
    /// Same rows filtered by name or phone (case-insensitive for ASCII), active parties only.
    fn search_parties(
        &self,
        query: &str,
        party_type: Option<PartyType>,
    ) -> Result<Vec<PartyListRow>, AppError>;
    /// FR-46: what deleting would do (HARD / SOFT, open dues). Writes nothing.
    fn preview_party_delete(&self, party_id: i64) -> Result<PartyDeletePreview, AppError>;
    /// FR-46 / DD-80: blocked by open dues; removes the row when nothing references it, else
    /// sets `is_active = 0`.
    fn delete_party(&self, party_id: i64) -> Result<DeletePartyResult, AppError>;
    fn restore_party(&self, party_id: i64) -> Result<(), AppError>;
    /// Deleted parties, customers and vendors together; `search` on name or phone digits.
    fn list_deleted_parties(
        &self,
        search: Option<String>,
        page: Option<usize>,
        page_size: Option<usize>,
    ) -> Result<Paginated<DeletedPartyRow>, AppError>;
}

impl<T: PartyService + ?Sized> PartyService for std::sync::Arc<T> {
    fn create_party(&self, payload: CreatePartyPayload) -> Result<i64, AppError> {
        (**self).create_party(payload)
    }

    fn update_party(&self, id: i64, payload: UpdatePartyPayload) -> Result<(), AppError> {
        (**self).update_party(id, payload)
    }

    fn get_party(&self, id: i64) -> Result<Party, AppError> {
        (**self).get_party(id)
    }

    fn list_parties(
        &self,
        party_type: Option<PartyType>,
        active_only: Option<bool>,
    ) -> Result<Vec<PartyListRow>, AppError> {
        (**self).list_parties(party_type, active_only)
    }

    fn search_parties(
        &self,
        query: &str,
        party_type: Option<PartyType>,
    ) -> Result<Vec<PartyListRow>, AppError> {
        (**self).search_parties(query, party_type)
    }

    fn preview_party_delete(&self, party_id: i64) -> Result<PartyDeletePreview, AppError> {
        (**self).preview_party_delete(party_id)
    }

    fn delete_party(&self, party_id: i64) -> Result<DeletePartyResult, AppError> {
        (**self).delete_party(party_id)
    }

    fn restore_party(&self, party_id: i64) -> Result<(), AppError> {
        (**self).restore_party(party_id)
    }

    fn list_deleted_parties(
        &self,
        search: Option<String>,
        page: Option<usize>,
        page_size: Option<usize>,
    ) -> Result<Paginated<DeletedPartyRow>, AppError> {
        (**self).list_deleted_parties(search, page, page_size)
    }
}
