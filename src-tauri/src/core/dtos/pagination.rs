//! The one paginated list shape and the shared list-input validation (F5/T8, DD-21).
//!
//! Decision: `Paginated<T> { items, total, page, limit }` exactly as HLD 9.3. `total_pages` is
//! dropped (the UI derives `ceil(total / limit)`).
//!
//! Page inputs are *rejected*, never silently clamped: `page >= 1` (default 1) and
//! `1 <= limit <= 200` (default 50); anything else is a `ValidationError` on `page` / `limit`.
//! Date filters must be zero-padded `YYYY-MM-DD` real dates and `start <= end`; both bounds are
//! inclusive.
use serde::{Deserialize, Serialize};

use crate::core::error::AppError;

pub const DEFAULT_PAGE_LIMIT: usize = 50;
pub const MAX_PAGE_LIMIT: usize = 200;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Paginated<T> {
    pub items: Vec<T>,
    /// Rows matching the filter across all pages.
    pub total: usize,
    pub page: usize,
    pub limit: usize,
}

impl<T> Paginated<T> {
    pub fn new(items: Vec<T>, total: usize, page: usize, limit: usize) -> Self {
        Self { items, total, page, limit }
    }
}

/// A validated page request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    pub page: usize,
    pub limit: usize,
}

impl PageRequest {
    /// Applies the defaults (page 1, limit 50) and rejects out-of-range values.
    pub fn new(page: Option<usize>, limit: Option<usize>) -> Result<Self, AppError> {
        let page = page.unwrap_or(1);
        let limit = limit.unwrap_or(DEFAULT_PAGE_LIMIT);
        if page < 1 {
            return Err(invalid("page", "page must be 1 or more"));
        }
        if limit < 1 || limit > MAX_PAGE_LIMIT {
            return Err(invalid("limit", format!("limit must be between 1 and {MAX_PAGE_LIMIT}")));
        }
        // Bounds the offset so `(page - 1) * limit` cannot overflow.
        if (page - 1).checked_mul(limit).map_or(true, |o| o > i64::MAX as usize) {
            return Err(invalid("page", "page is out of range"));
        }
        Ok(Self { page, limit })
    }

    pub fn offset(&self) -> usize {
        (self.page - 1) * self.limit
    }
}

fn invalid(field: &str, message: impl Into<String>) -> AppError {
    AppError::ValidationError { field: field.to_string(), message: message.into() }
}

fn check_ymd(field: &str, v: &str) -> Result<(), AppError> {
    let ok = v.len() == 10 && chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d").is_ok();
    if ok { Ok(()) } else { Err(invalid(field, format!("{field} must be a YYYY-MM-DD date, got '{v}'"))) }
}

/// Validates an inclusive date range filter (`start_date` / `end_date`).
pub fn validate_date_range(start: Option<&str>, end: Option<&str>) -> Result<(), AppError> {
    if let Some(s) = start {
        check_ymd("start_date", s)?;
    }
    if let Some(e) = end {
        check_ymd("end_date", e)?;
    }
    if let (Some(s), Some(e)) = (start, end) {
        if s > e {
            return Err(invalid("start_date", "start_date must not be after end_date"));
        }
    }
    Ok(())
}

/// Trims a free-text filter; blank means "no filter".
pub fn clean_text(s: Option<&str>) -> Option<&str> {
    s.map(str::trim).filter(|t| !t.is_empty())
}
