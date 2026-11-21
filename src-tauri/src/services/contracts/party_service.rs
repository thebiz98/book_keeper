use crate::core::entities::{CreatePartyPayload, Party, UpdatePartyPayload};
use crate::core::error::AppError;
use crate::core::value_objects::PartyType;

pub trait PartyService: Send + Sync {
    fn create_party(&self, payload: CreatePartyPayload) -> Result<i64, AppError>;
    fn update_party(&self, id: i64, payload: UpdatePartyPayload) -> Result<(), AppError>;
    fn archive_party(&self, id: i64) -> Result<(), AppError>;
    fn get_party(&self, id: i64) -> Result<Party, AppError>;
    fn list_parties(
        &self,
        party_type: Option<PartyType>,
        active_only: bool,
    ) -> Result<Vec<Party>, AppError>;
    fn search_parties(
        &self,
        query: &str,
        party_type: Option<PartyType>,
    ) -> Result<Vec<Party>, AppError>;
}

impl<T: PartyService + ?Sized> PartyService for std::sync::Arc<T> {
    fn create_party(&self, payload: CreatePartyPayload) -> Result<i64, AppError> {
        (**self).create_party(payload)
    }

    fn update_party(&self, id: i64, payload: UpdatePartyPayload) -> Result<(), AppError> {
        (**self).update_party(id, payload)
    }

    fn archive_party(&self, id: i64) -> Result<(), AppError> {
        (**self).archive_party(id)
    }

    fn get_party(&self, id: i64) -> Result<Party, AppError> {
        (**self).get_party(id)
    }

    fn list_parties(
        &self,
        party_type: Option<PartyType>,
        active_only: bool,
    ) -> Result<Vec<Party>, AppError> {
        (**self).list_parties(party_type, active_only)
    }

    fn search_parties(
        &self,
        query: &str,
        party_type: Option<PartyType>,
    ) -> Result<Vec<Party>, AppError> {
        (**self).search_parties(query, party_type)
    }
}
