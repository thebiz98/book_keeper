use crate::core::dtos::PartyDueSummary;
use crate::core::entities::Party;
use crate::core::error::AppError;
use crate::core::value_objects::PartyType;

pub trait PartyRepository {
    fn find_by_id(&self, id: i64) -> Result<Option<Party>, AppError>;
    fn list(&self, party_type: Option<PartyType>, active_only: bool) -> Result<Vec<Party>, AppError>;
    fn create(&mut self, party: &Party) -> Result<i64, AppError>;
    fn update(&mut self, party: &Party) -> Result<(), AppError>;
    /// Active parties of `party_type` with an outstanding balance, in one query. Customers sum
    /// open sales; vendors sum open `source = PURCHASE` purchases.
    fn get_party_due_summary(&self, party_type: PartyType) -> Result<Vec<PartyDueSummary>, AppError>;
}
