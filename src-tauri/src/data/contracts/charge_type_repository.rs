use crate::core::entities::ChargeType;
use crate::core::error::AppError;
use crate::core::value_objects::TransactionType;

/// Charge/discount type master (FR-19).
pub trait ChargeTypeRepository {
    fn find_by_id(&self, id: i64) -> Result<Option<ChargeType>, AppError>;
    /// Case-insensitive (NOCASE) name lookup.
    fn find_by_name(&self, name: &str) -> Result<Option<ChargeType>, AppError>;
    /// System type by `system_key` (FR-47: `LINE_EXTRA`, `LINE_DISCOUNT`).
    fn find_by_system_key(&self, key: &str) -> Result<Option<ChargeType>, AppError>;
    fn create(&mut self, ct: &ChargeType) -> Result<i64, AppError>;
    /// Active, non-system types among `ids` that cover `side`, in the order of `ids` (the picker; FR-47 system types are hidden).
    fn list_by_ids_for_side(&self, ids: &[i64], side: TransactionType) -> Result<Vec<ChargeType>, AppError>;
    /// Active, non-system types covering `side`, `ORDER BY name COLLATE NOCASE, id LIMIT n`.
    fn list_for_side(&self, side: TransactionType, limit: usize) -> Result<Vec<ChargeType>, AppError>;
}
