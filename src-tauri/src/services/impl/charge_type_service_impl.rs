//! Charge/discount type master (FR-19, LLD 13). Names are searched through the shared FTS index
//! (`CHARGE_TYPE_FTS`); creating an existing name is a `ValidationError(name)` (R5).
use std::sync::Arc;

use crate::core::dtos::CreateChargeTypePayload;
use crate::core::entities::ChargeType;
use crate::core::error::AppError;
use crate::core::money::round2;
use crate::core::value_objects::{clean_name, TransactionType};
use crate::data::contracts::search_indexer::FtsIndex;
use crate::data::contracts::unit_of_work::UnitOfWork;
use crate::data::sqlite::fts::sync_after_commit;
use crate::services::contracts::charge_type_service::ChargeTypeService;

const MAX_NAME: usize = 80;
const DEFAULT_SEARCH_LIMIT: usize = 8;
const MAX_SEARCH_LIMIT: usize = 50;
/// FTS candidates fetched before the side/active filter.
const FTS_CANDIDATES: usize = 200;

#[derive(Clone)]
pub struct ChargeTypeServiceImpl<U: UnitOfWork = Arc<dyn UnitOfWork>> {
    pub uow: U,
    pub index: Arc<dyn FtsIndex>,
}

impl<U: UnitOfWork> ChargeTypeServiceImpl<U> {
    pub fn new(uow: U, index: Arc<dyn FtsIndex>) -> Self {
        Self { uow, index }
    }
}

fn validation(field: &str, message: impl Into<String>) -> AppError {
    AppError::ValidationError { field: field.to_string(), message: message.into() }
}

impl<U: UnitOfWork> ChargeTypeService for ChargeTypeServiceImpl<U> {
    fn search_charge_types(
        &self,
        query: &str,
        side: TransactionType,
        limit: Option<usize>,
    ) -> Result<Vec<ChargeType>, AppError> {
        let limit = limit.unwrap_or(DEFAULT_SEARCH_LIMIT);
        if !(1..=MAX_SEARCH_LIMIT).contains(&limit) {
            return Err(validation("limit", format!("limit must be between 1 and {MAX_SEARCH_LIMIT}")));
        }
        if query.trim().is_empty() {
            let mut tx = self.uow.begin_read()?;
            return tx.charge_types().list_for_side(side, limit);
        }
        let keys = self.index.search_keys(query, FTS_CANDIDATES)?;
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let mut tx = self.uow.begin_read()?;
        let mut found = tx.charge_types().list_by_ids_for_side(&keys, side)?;
        found.truncate(limit);
        Ok(found)
    }

    fn create_charge_type(&self, payload: CreateChargeTypePayload) -> Result<ChargeType, AppError> {
        let name = clean_name(&payload.name);
        if name.is_empty() {
            return Err(validation("name", "Charge type name cannot be empty"));
        }
        if name.chars().count() > MAX_NAME {
            return Err(validation("name", "Charge type name must be 80 characters or fewer"));
        }
        let default_amount = match payload.default_amount {
            None => None,
            Some(a) => {
                if !a.is_finite() || round2(a) <= 0.0 {
                    return Err(validation("default_amount", "Default amount must be greater than 0"));
                }
                Some(round2(a))
            }
        };
        let mut tx = self.uow.begin_transaction()?;
        if let Some(existing) = tx.charge_types().find_by_name(&name)? {
            return Err(validation("name", format!("A charge type named '{}' already exists", existing.name)));
        }
        let row = ChargeType {
            id: 0,
            name,
            effect: payload.effect,
            applies_to: payload.applies_to,
            default_amount,
            is_active: true,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let id = tx.charge_types().create(&row)?;
        let created = tx
            .charge_types()
            .find_by_id(id)?
            .ok_or_else(|| AppError::NotFound { entity: "ChargeType".into(), id: id.to_string() })?;
        tx.commit()?;
        sync_after_commit(&*self.index, id, "charge type");
        Ok(created)
    }
}
