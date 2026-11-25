use std::collections::HashMap;

use crate::core::entities::ServiceComponent;
use crate::core::error::AppError;

/// Bill of materials of service items (FR-36, schema v12).
pub trait ServiceComponentRepository {
    /// Component rows of one service, ordered by `id`.
    fn list_for_service(&self, service_id: i64) -> Result<Vec<ServiceComponent>, AppError>;
    /// Replaces the whole component list of a service (delete + insert).
    fn replace_for_service(&mut self, service_id: i64, components: &[(i64, f64)]) -> Result<(), AppError>;
    /// Ids of the services that consume `component_id`.
    fn services_using(&self, component_id: i64) -> Result<Vec<i64>, AppError>;
    /// "Can make N" per service (HLD section 7): `MIN(FLOOR(component stock / quantity_per_unit))`.
    /// Services without components are absent from the map.
    fn can_make(&self, service_ids: &[i64]) -> Result<HashMap<i64, i64>, AppError>;
}
