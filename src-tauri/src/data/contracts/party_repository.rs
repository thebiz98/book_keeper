use crate::core::dtos::PartyDueSummary;
use crate::core::entities::{Party, PartyListRow};
use crate::core::error::AppError;
use crate::core::value_objects::PartyType;

pub trait PartyRepository {
    fn find_by_id(&self, id: i64) -> Result<Option<Party>, AppError>;
    fn list(&self, party_type: Option<PartyType>, active_only: bool) -> Result<Vec<Party>, AppError>;
    /// Directory rows with `outstanding_balance` and `transaction_count` computed in the same
    /// query, ordered by name. `search` goes through `party_search_fts` (token prefix over name, phone, phone digits; a digits-only
    /// query of 3+ digits also matches anywhere in the phone digits); blank = no filter; a non-blank
    /// query with no alphanumeric token matches nothing.
    fn list_rows(
        &self,
        party_type: Option<PartyType>,
        active_only: bool,
        search: Option<&str>,
    ) -> Result<Vec<PartyListRow>, AppError>;
    fn create(&mut self, party: &Party) -> Result<i64, AppError>;
    fn update(&mut self, party: &Party) -> Result<(), AppError>;
    /// Active parties of `party_type` with open bills, in one grouped query (count, earliest
    /// `transaction_date`, Σ total / paid / balance). Customers read sales; vendors read open
    /// `source = PURCHASE` purchases.
    fn get_party_due_summary(&self, party_type: PartyType) -> Result<Vec<PartyDueSummary>, AppError>;
}
