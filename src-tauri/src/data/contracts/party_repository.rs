use crate::core::dtos::{DeletedPartyRow, PartyDueSummary};
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
    /// FR-46: `is_active` + `updated_at` in one statement.
    fn set_active(&mut self, id: i64, active: bool, updated_at: &str) -> Result<(), AppError>;
    /// FR-46 / DD-80: physically removes the row (the caller checked `reference_count = 0`).
    fn hard_delete(&mut self, id: i64) -> Result<(), AppError>;
    /// Rows of sales, purchases (any source), returns and settlements carrying `party_id`.
    fn reference_count(&self, id: i64) -> Result<i64, AppError>;
    /// `(sales, purchases, returns, settlements)` row counts behind `reference_count`.
    fn reference_breakdown(&self, id: i64) -> Result<(i64, i64, i64, i64), AppError>;
    /// `(count, sum)` of the party's open bills (`balance_due > 0`), opening bills included:
    /// sales for a customer, `source = PURCHASE` purchases for a vendor.
    fn open_dues(&self, id: i64, party_type: PartyType) -> Result<(i64, f64), AppError>;
    /// Deleted parties of both types, name or phone `LIKE` search, newest delete first, plus the total.
    fn list_inactive(&self, search: &str, offset: usize, limit: usize) -> Result<(Vec<DeletedPartyRow>, usize), AppError>;
    /// Parties (deleted ones too, FR-46) of `party_type` with open bills, in one grouped query (count, earliest
    /// `transaction_date`, Σ total / paid / balance). Customers read sales; vendors read open
    /// `source = PURCHASE` purchases.
    fn get_party_due_summary(&self, party_type: PartyType) -> Result<Vec<PartyDueSummary>, AppError>;
}
