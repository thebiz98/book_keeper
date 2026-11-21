//! The single human-readable code generator (RC-7, DD-07, DD-22).
//!
//! Every creator (sale, purchase, adjustment lot, opening lot, return lot, customer and vendor
//! return) obtains its code from [`next_code`] **inside its own write transaction**, so the
//! `BEGIN IMMEDIATE` lock makes `MAX + 1` race-free. The `get_next_*_code` previews call it from a
//! read context: the result is display-only and may be taken by a concurrent writer first.
//!
//! The numeric part is computed in SQL (`MAX(CAST(SUBSTR(code, len(prefix) + 1) AS INTEGER))`)
//! over codes that are exactly `prefix` + digits. Anything else (legacy or hand-made codes such as
//! `PUR-C1`) is ignored, gaps are never refilled, and the sequence keeps growing past the zero
//! padding (`SAL-9999` then `SAL-10000`).
use rusqlite::{params, Connection};

use crate::core::error::AppError;
use crate::core::value_objects::LotSource;

/// One independent code sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeSeq {
    pub prefix: &'static str,
    /// Number issued when the table holds no code of this sequence yet.
    pub start: i64,
    /// Minimum zero-padded width of the numeric part (never truncates).
    pub width: usize,
}

pub const SALE_CODE: CodeSeq = CodeSeq { prefix: "SAL-", start: 1001, width: 4 };
pub const PURCHASE_CODE: CodeSeq = CodeSeq { prefix: "PUR-", start: 1001, width: 4 };
/// Customer and vendor returns share one `RET-` sequence (`return_transactions.return_code`).
pub const RETURN_CODE: CodeSeq = CodeSeq { prefix: "RET-", start: 1001, width: 4 };
pub const ADJUSTMENT_LOT_CODE: CodeSeq = CodeSeq { prefix: "ADJ-", start: 1, width: 4 };
pub const OPENING_LOT_CODE: CodeSeq = CodeSeq { prefix: "OPN-", start: 1, width: 4 };
pub const RETURN_LOT_CODE: CodeSeq = CodeSeq { prefix: "RTL-", start: 1, width: 4 };

/// The `purchase_transactions` sequence for a lot header of the given source (DD-07).
pub fn seq_for_source(source: LotSource) -> &'static CodeSeq {
    match source {
        LotSource::Purchase => &PURCHASE_CODE,
        LotSource::Adjustment => &ADJUSTMENT_LOT_CODE,
        LotSource::Opening => &OPENING_LOT_CODE,
        LotSource::Return => &RETURN_LOT_CODE,
    }
}

/// Next free code of `seq` in `table.column`. `table` and `column` are compile-time identifiers
/// chosen by the repositories, never user input.
pub fn next_code(
    conn: &Connection,
    table: &'static str,
    column: &'static str,
    seq: &CodeSeq,
) -> Result<String, AppError> {
    let sql = format!(
        "SELECT MAX(CAST(SUBSTR({column}, ?1) AS INTEGER)) FROM {table} \
         WHERE {column} GLOB ?2 AND {column} NOT GLOB ?3"
    );
    let digits_start = seq.prefix.len() as i64 + 1;
    let starts_with_digit = format!("{}[0-9]*", seq.prefix);
    let has_non_digit = format!("{}*[^0-9]*", seq.prefix);
    let max: Option<i64> = conn.query_row(&sql, params![digits_start, starts_with_digit, has_non_digit], |r| r.get(0))?;
    let next = match max {
        Some(m) => m
            .checked_add(1)
            .ok_or_else(|| AppError::DatabaseError(format!("{} code sequence exhausted", seq.prefix)))?
            .max(seq.start),
        None => seq.start,
    };
    Ok(format!("{}{:0width$}", seq.prefix, next, width = seq.width))
}
