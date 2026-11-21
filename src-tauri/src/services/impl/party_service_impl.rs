use std::sync::Arc;
use crate::core::entities::{CreatePartyPayload, Party, UpdatePartyPayload};
use crate::core::error::AppError;
use crate::core::value_objects::PartyType;
use crate::data::contracts::unit_of_work::UnitOfWork;
use crate::services::contracts::party_service::PartyService;

#[derive(Clone)]
pub struct PartyServiceImpl<U: UnitOfWork = Arc<dyn UnitOfWork>> {
    pub uow: U,
}

impl<U: UnitOfWork> PartyServiceImpl<U> {
    pub fn new(uow: U) -> Self {
        Self { uow }
    }
}

impl<U: UnitOfWork> PartyService for PartyServiceImpl<U> {
    fn create_party(&self, payload: CreatePartyPayload) -> Result<i64, AppError> {
        if payload.name.trim().is_empty() {
            return Err(AppError::ValidationError {
                field: "name".to_string(),
                message: "Party name cannot be empty".to_string(),
            });
        }

        let mut tx = self.uow.begin_transaction()?;
        let party = Party {
            id: 0,
            name: payload.name.trim().to_string(),
            phone: payload.phone.map(|p| p.trim().to_string()).filter(|p| !p.is_empty()),
            party_type: payload.party_type,
            notes: payload.notes,
            is_active: true,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };

        let party_id = tx.parties().create(&party)?;
        tx.commit()?;
        Ok(party_id)
    }

    fn update_party(&self, id: i64, payload: UpdatePartyPayload) -> Result<(), AppError> {
        if payload.name.trim().is_empty() {
            return Err(AppError::ValidationError {
                field: "name".to_string(),
                message: "Party name cannot be empty".to_string(),
            });
        }

        let mut tx = self.uow.begin_transaction()?;
        let mut party = tx
            .parties()
            .find_by_id(id)?
            .ok_or_else(|| AppError::NotFound {
                entity: "Party".to_string(),
                id: id.to_string(),
            })?;

        party.name = payload.name.trim().to_string();
        party.phone = payload.phone.map(|p| p.trim().to_string()).filter(|p| !p.is_empty());
        party.party_type = payload.party_type;
        party.notes = payload.notes;
        party.updated_at = chrono::Utc::now().to_rfc3339();

        tx.parties().update(&party)?;
        tx.commit()?;
        Ok(())
    }

    fn get_party(&self, id: i64) -> Result<Party, AppError> {
        let mut tx = self.uow.begin_read()?;
        tx.parties()
            .find_by_id(id)?
            .ok_or_else(|| AppError::NotFound {
                entity: "Party".to_string(),
                id: id.to_string(),
            })
    }

    fn list_parties(
        &self,
        party_type: Option<PartyType>,
        active_only: bool,
    ) -> Result<Vec<Party>, AppError> {
        let mut tx = self.uow.begin_read()?;
        tx.parties().list(party_type, active_only)
    }

    fn search_parties(
        &self,
        query: &str,
        party_type: Option<PartyType>,
    ) -> Result<Vec<Party>, AppError> {
        let mut tx = self.uow.begin_read()?;
        let parties = tx.parties().list(party_type, true)?;
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Ok(parties);
        }
        let matched = parties
            .into_iter()
            .filter(|p| {
                p.name.to_lowercase().contains(&q)
                    || p.phone.as_ref().is_some_and(|ph| ph.contains(&q))
            })
            .collect();
        Ok(matched)
    }
}
