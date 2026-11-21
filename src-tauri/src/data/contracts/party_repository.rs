use crate::core::dtos::PartyDueSummary;
use crate::core::entities::Party;
use crate::core::error::AppError;
use crate::core::value_objects::PartyType;

pub trait PartyRepository {
    fn find_by_id(&self, id: i64) -> Result<Option<Party>, AppError>;
    fn list(&self, party_type: Option<PartyType>, active_only: bool) -> Result<Vec<Party>, AppError> {
        match party_type {
            Some(pt) => self.list_by_type(pt, active_only),
            None => Ok(Vec::new()),
        }
    }
    fn list_by_type(&self, party_type: PartyType, active_only: bool) -> Result<Vec<Party>, AppError> {
        self.list(Some(party_type), active_only)
    }
    fn create(&mut self, party: &Party) -> Result<i64, AppError>;
    fn update(&mut self, party: &Party) -> Result<(), AppError>;
    fn get_party_due_summary(&self, _party_type: PartyType) -> Result<Vec<PartyDueSummary>, AppError> {
        Ok(Vec::new())
    }
}
