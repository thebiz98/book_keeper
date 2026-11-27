use crate::core::entities::StockOutflowAllocation;
use crate::core::error::AppError;
use crate::core::value_objects::OutflowSourceType;

/// FIFO audit rows for non-sale stock outflows (adjustments, replacements, vendor returns).
pub trait OutflowAllocationRepository {
    fn create_allocation(&mut self, alloc: &StockOutflowAllocation) -> Result<i64, AppError>;
    fn find_by_source(
        &self,
        source_type: OutflowSourceType,
        source_id: i64,
    ) -> Result<Vec<StockOutflowAllocation>, AppError>;
    /// FR-20: removes the allocations of one source; returns them (to restore lots).
    fn delete_by_source(
        &mut self,
        source_type: OutflowSourceType,
        source_id: i64,
    ) -> Result<Vec<StockOutflowAllocation>, AppError>;
    fn exists_for_lot(&self, lot_id: i64, source_type: OutflowSourceType) -> Result<bool, AppError>;
    /// DD-95: Σ `quantity_depleted` of every outflow allocation on the lot (all source types).
    fn sum_for_lot(&self, lot_id: i64) -> Result<f64, AppError>;
    /// Distinct `source_id`s of the type that drew from the lot.
    fn source_ids_for_lot(&self, lot_id: i64, source_type: OutflowSourceType) -> Result<Vec<i64>, AppError>;
    /// FR-20 re-cost (`ADJUSTMENT` and `REPLACEMENT` rows only; vendor returns are locked).
    fn update_unit_cost_for_lot(&mut self, lot_id: i64, unit_cost: f64) -> Result<usize, AppError>;
}
