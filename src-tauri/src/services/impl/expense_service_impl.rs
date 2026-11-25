//! Store expenses (FR-18, LLD 12). Types are master data searched through the shared FTS index
//! (`EXPENSE_TYPE_FTS`); an expense is a standalone cash-out event on its date (HLD assumption 16).
use std::sync::Arc;

use crate::core::dtos::{CreateExpenseTypePayload, ExpenseFilter, ExpenseListResponse, ExpensePayload};
use crate::core::entities::{Expense, ExpenseType};
use crate::core::error::AppError;
use crate::core::money::{check_entry_date_now, round2};
use crate::core::value_objects::{clean_name, clean_optional};
use crate::data::contracts::search_indexer::FtsIndex;
use crate::data::contracts::unit_of_work::UnitOfWork;
use crate::data::sqlite::fts::sync_after_commit;
use crate::services::contracts::expense_service::ExpenseService;

const MAX_TYPE_NAME: usize = 80;
const MAX_NOTES: usize = 500;
const DEFAULT_SEARCH_LIMIT: usize = 8;
const MAX_SEARCH_LIMIT: usize = 50;

#[derive(Clone)]
pub struct ExpenseServiceImpl<U: UnitOfWork = Arc<dyn UnitOfWork>> {
    pub uow: U,
    pub type_index: Arc<dyn FtsIndex>,
}

impl<U: UnitOfWork> ExpenseServiceImpl<U> {
    pub fn new(uow: U, type_index: Arc<dyn FtsIndex>) -> Self {
        Self { uow, type_index }
    }
}

fn validation(field: &str, message: impl Into<String>) -> AppError {
    AppError::ValidationError { field: field.to_string(), message: message.into() }
}

pub(crate) fn valid_type_name(raw: &str) -> Result<String, AppError> {
    let name = clean_name(raw);
    if name.is_empty() {
        return Err(validation("name", "Expense type name cannot be empty"));
    }
    if name.chars().count() > MAX_TYPE_NAME {
        return Err(validation("name", "Expense type name must be 80 characters or fewer"));
    }
    Ok(name)
}

pub(crate) struct Checked {
    pub date: String,
    pub amount: f64,
    pub notes: Option<String>,
}

pub(crate) fn check_payload(p: &ExpensePayload) -> Result<Checked, AppError> {
    check_entry_date_now("expense_date", &p.expense_date)?;
    let positive = p.amount.is_finite() && p.amount > 0.0;
    let amount = round2(p.amount);
    if !positive || amount <= 0.0 {
        return Err(validation("amount", "Amount must be greater than 0"));
    }
    let notes = clean_optional(p.notes.as_deref());
    if notes.as_ref().is_some_and(|n| n.chars().count() > MAX_NOTES) {
        return Err(validation("notes", "Notes must be 500 characters or fewer"));
    }
    Ok(Checked { date: p.expense_date.clone(), amount, notes })
}

fn active_type(
    tx: &mut dyn crate::data::contracts::unit_of_work::TransactionContext,
    id: i64,
) -> Result<ExpenseType, AppError> {
    tx.expenses()
        .find_type_by_id(id)?
        .filter(|t| t.is_active)
        .ok_or_else(|| AppError::NotFound { entity: "ExpenseType".into(), id: id.to_string() })
}

/// Finds the type by name (case-insensitive) or creates it inside the caller's transaction.
/// `(type, created)`. The caller reindexes after commit when `created` (R5, DD-47).
pub(crate) fn get_or_create_type_in_tx(
    tx: &mut dyn crate::data::contracts::unit_of_work::TransactionContext,
    raw_name: &str,
) -> Result<(ExpenseType, bool), AppError> {
    let name = valid_type_name(raw_name)?;
    if let Some(existing) = tx.expenses().find_type_by_name(&name)? {
        return Ok((existing, false));
    }
    let row = ExpenseType { id: 0, name, is_active: true, created_at: chrono::Utc::now().to_rfc3339() };
    let id = tx.expenses().create_type(&row)?;
    let created = tx
        .expenses()
        .find_type_by_id(id)?
        .ok_or_else(|| AppError::NotFound { entity: "ExpenseType".into(), id: id.to_string() })?;
    Ok((created, true))
}

/// Validates and inserts one expense inside the caller's transaction (used by `create_expense`
/// and the expenses import).
pub(crate) fn create_expense_in_tx(
    tx: &mut dyn crate::data::contracts::unit_of_work::TransactionContext,
    payload: &ExpensePayload,
) -> Result<Expense, AppError> {
    let c = check_payload(payload)?;
    let t = active_type(tx, payload.expense_type_id)?;
    let code = tx.expenses().get_next_expense_code()?;
    let row = Expense {
        id: 0,
        expense_code: code,
        expense_date: c.date,
        expense_type_id: t.id,
        expense_type_name: t.name,
        amount: c.amount,
        notes: c.notes,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: None,
    };
    let id = tx.expenses().create(&row)?;
    tx.expenses()
        .find_by_id(id)?
        .ok_or_else(|| AppError::NotFound { entity: "Expense".into(), id: id.to_string() })
}

fn is_unique_violation(e: &AppError) -> bool {
    matches!(e, AppError::DatabaseError(m) if m.contains("UNIQUE"))
}

impl<U: UnitOfWork> ExpenseService for ExpenseServiceImpl<U> {
    fn search_expense_types(&self, query: &str, limit: Option<usize>) -> Result<Vec<ExpenseType>, AppError> {
        let limit = limit.unwrap_or(DEFAULT_SEARCH_LIMIT).clamp(1, MAX_SEARCH_LIMIT);
        if query.trim().is_empty() {
            let mut all = self.list_expense_types()?;
            all.truncate(limit);
            return Ok(all);
        }
        let keys = self.type_index.search_keys(query, limit)?;
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let mut tx = self.uow.begin_read()?;
        let mut found = tx.expenses().find_types_by_ids(&keys)?;
        found.sort_by_key(|t| keys.iter().position(|k| *k == t.id).unwrap_or(usize::MAX));
        Ok(found)
    }

    fn list_expense_types(&self) -> Result<Vec<ExpenseType>, AppError> {
        let mut tx = self.uow.begin_read()?;
        tx.expenses().list_types(true)
    }

    fn create_expense_type(&self, payload: CreateExpenseTypePayload) -> Result<ExpenseType, AppError> {
        let name = valid_type_name(&payload.name)?;
        let mut tx = self.uow.begin_transaction()?;
        match get_or_create_type_in_tx(&mut *tx, &name) {
            Ok((existing, false)) => {
                tx.rollback()?;
                Ok(existing)
            }
            Ok((created, true)) => {
                tx.commit()?;
                sync_after_commit(&*self.type_index, created.id, "expense type");
                Ok(created)
            }
            Err(e) if is_unique_violation(&e) => {
                // Lost a race against another creator: return the winner.
                drop(tx);
                let mut rd = self.uow.begin_read()?;
                rd.expenses().find_type_by_name(&name)?.ok_or(e)
            }
            Err(e) => Err(e),
        }
    }

    fn update_expense_type(&self, id: i64, payload: CreateExpenseTypePayload) -> Result<ExpenseType, AppError> {
        let name = valid_type_name(&payload.name)?;
        let mut tx = self.uow.begin_transaction()?;
        let current = active_type(&mut *tx, id)?;
        if let Some(other) = tx.expenses().find_type_by_name(&name)? {
            if other.id != current.id {
                return Err(validation("name", format!("An expense type named '{}' already exists", other.name)));
            }
        }
        tx.expenses().update_type_name(id, &name)?;
        let updated = active_type(&mut *tx, id)?;
        tx.commit()?;
        sync_after_commit(&*self.type_index, id, "expense type");
        Ok(updated)
    }

    fn get_next_expense_code(&self) -> Result<String, AppError> {
        let mut tx = self.uow.begin_read()?;
        tx.expenses().get_next_expense_code()
    }

    fn create_expense(&self, payload: ExpensePayload) -> Result<Expense, AppError> {
        check_payload(&payload)?;
        let mut tx = self.uow.begin_transaction()?;
        let stored = create_expense_in_tx(&mut *tx, &payload)?;
        tx.commit()?;
        Ok(stored)
    }

    fn update_expense(&self, id: i64, payload: ExpensePayload) -> Result<Expense, AppError> {
        let c = check_payload(&payload)?;
        let mut tx = self.uow.begin_transaction()?;
        let mut e = tx
            .expenses()
            .find_by_id(id)?
            .ok_or_else(|| AppError::NotFound { entity: "Expense".into(), id: id.to_string() })?;
        let t = active_type(&mut *tx, payload.expense_type_id)?;
        e.expense_date = c.date;
        e.expense_type_id = t.id;
        e.expense_type_name = t.name;
        e.amount = c.amount;
        e.notes = c.notes;
        e.updated_at = Some(chrono::Utc::now().to_rfc3339());
        tx.expenses().update(&e)?;
        tx.commit()?;
        Ok(e)
    }

    fn delete_expense(&self, id: i64) -> Result<(), AppError> {
        let mut tx = self.uow.begin_transaction()?;
        tx.expenses().delete(id)?;
        tx.commit()
    }

    fn list_expenses(&self, filter: ExpenseFilter) -> Result<ExpenseListResponse, AppError> {
        let page = filter.validate()?;
        let mut tx = self.uow.begin_read()?;
        let (items, total, period_total) = tx.expenses().list(&filter, page.offset(), page.limit)?;
        Ok(ExpenseListResponse { items, total, page: page.page, limit: page.limit, period_total: round2(period_total) })
    }
}
