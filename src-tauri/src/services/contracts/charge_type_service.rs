use crate::core::dtos::CreateChargeTypePayload;
use crate::core::entities::ChargeType;
use crate::core::error::AppError;
use crate::core::value_objects::TransactionType;

pub trait ChargeTypeService: Send + Sync {
    /// Live typeahead for one bill side: blank query lists the applicable types by name, otherwise
    /// FTS prefix hits in rank order. `limit` defaults to 8, valid range 1..=50.
    fn search_charge_types(
        &self,
        query: &str,
        side: TransactionType,
        limit: Option<usize>,
    ) -> Result<Vec<ChargeType>, AppError>;
    /// An existing name (case-insensitive) is `ValidationError(name)` (R5): effect and applies_to
    /// of the existing row would otherwise be ambiguous.
    fn create_charge_type(&self, payload: CreateChargeTypePayload) -> Result<ChargeType, AppError>;
}

impl<T: ChargeTypeService + ?Sized> ChargeTypeService for std::sync::Arc<T> {
    fn search_charge_types(
        &self,
        query: &str,
        side: TransactionType,
        limit: Option<usize>,
    ) -> Result<Vec<ChargeType>, AppError> {
        (**self).search_charge_types(query, side, limit)
    }
    fn create_charge_type(&self, payload: CreateChargeTypePayload) -> Result<ChargeType, AppError> {
        (**self).create_charge_type(payload)
    }
}
