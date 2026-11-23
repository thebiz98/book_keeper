//! SQLite implementation of `SaleRepository`.
use rusqlite::{params, types::Value, OptionalExtension, Row};

use crate::core::dtos::{ItemBillRef, SaleHistoryRow, SalesHistoryFilter};
use crate::core::entities::{SaleItem, SaleTransaction};
use crate::core::error::AppError;
use crate::core::value_objects::PaymentStatus;
use crate::data::contracts::sale_repository::SaleRepository;
use crate::data::sqlite::code_gen::{next_code, SALE_CODE};
use crate::data::sqlite::{enum_col, enum_to_sql, history_where, SqliteConn};

const TX_COLS: &str = "id, transaction_code, transaction_date, party_id, party_name, payment_mode, payment_status, total_amount, paid_amount, balance_due, notes, created_at, items_subtotal, charges_total, discounts_total, updated_at";
const ITEM_COLS: &str = "id, sale_transaction_id, item_id, item_name, pack_label, pack_quantity, quantity, unit_price, total_amount";

fn map_tx(r: &Row<'_>) -> rusqlite::Result<SaleTransaction> {
    Ok(SaleTransaction {
        id: r.get(0)?,
        transaction_code: r.get(1)?,
        transaction_date: r.get(2)?,
        party_id: r.get(3)?,
        party_name: r.get(4)?,
        payment_mode: r.get(5)?,
        payment_status: enum_col(r, 6)?,
        total_amount: r.get(7)?,
        paid_amount: r.get(8)?,
        balance_due: r.get(9)?,
        notes: r.get(10)?,
        created_at: r.get(11)?,
        items_subtotal: r.get(12)?,
        charges_total: r.get(13)?,
        discounts_total: r.get(14)?,
        updated_at: r.get(15)?,
    })
}

fn map_item(r: &Row<'_>) -> rusqlite::Result<SaleItem> {
    Ok(SaleItem {
        id: r.get(0)?,
        sale_transaction_id: r.get(1)?,
        item_id: r.get(2)?,
        item_name: r.get(3)?,
        pack_label: r.get(4)?,
        pack_quantity: r.get(5)?,
        quantity: r.get(6)?,
        unit_price: r.get(7)?,
        total_amount: r.get(8)?,
    })
}

fn opt_id(id: i64) -> Option<i64> {
    if id > 0 { Some(id) } else { None }
}

fn lim(n: usize) -> i64 {
    n.min(i64::MAX as usize) as i64
}

pub struct SqliteSaleRepository {
    conn: SqliteConn,
}

impl SqliteSaleRepository {
    pub fn new(conn: SqliteConn) -> Self {
        Self { conn }
    }

    fn where_clause(
        start: Option<&str>,
        end: Option<&str>,
        status: Option<PaymentStatus>,
        party_id: Option<i64>,
    ) -> Result<(String, Vec<Value>), AppError> {
        let mut conds: Vec<&str> = Vec::new();
        let mut vals: Vec<Value> = Vec::new();
        if let Some(p) = party_id {
            conds.push("party_id = ?");
            vals.push(Value::Integer(p));
        }
        if let Some(s) = status {
            conds.push("payment_status = ?");
            vals.push(Value::Text(enum_to_sql(&s)?));
        }
        if let Some(s) = start {
            conds.push("transaction_date >= ?");
            vals.push(Value::Text(s.to_string()));
        }
        if let Some(e) = end {
            conds.push("transaction_date <= ?");
            vals.push(Value::Text(e.to_string()));
        }
        let w = if conds.is_empty() { String::new() } else { format!(" WHERE {}", conds.join(" AND ")) };
        Ok((w, vals))
    }

    fn query_items(&self, sql: &str, p: impl rusqlite::Params) -> Result<Vec<SaleItem>, AppError> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(p, map_item)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

/// FR-23 page query (a const so the query-plan test runs the exact production SQL).
pub const ITEM_SALES_PAGE_SQL: &str = "SELECT st.id, st.transaction_code, st.transaction_date, st.party_name \
     FROM (SELECT DISTINCT sale_transaction_id AS tid, transaction_date AS d FROM sale_items \
           WHERE item_id = ?1 ORDER BY transaction_date DESC, sale_transaction_id DESC LIMIT ?2 OFFSET ?3) x \
     JOIN sale_transactions st ON st.id = x.tid";
pub const ITEM_SALES_COUNT_SQL: &str = "SELECT COUNT(*) FROM (SELECT 1 FROM sale_items WHERE item_id = ?1 \n     GROUP BY transaction_date, sale_transaction_id)";

impl SaleRepository for SqliteSaleRepository {
    fn list_bills_for_item(&self, item_id: i64, offset: usize, limit: usize) -> Result<(Vec<ItemBillRef>, usize), AppError> {
        let total: i64 = self.conn.query_row(ITEM_SALES_COUNT_SQL, params![item_id], |r| r.get(0))?;
        let mut stmt = self.conn.prepare(ITEM_SALES_PAGE_SQL)?;
        let mut rows = stmt
            .query_map(params![item_id, lim(limit), lim(offset)], |r| {
                Ok(ItemBillRef { bill_id: r.get(0)?, transaction_code: r.get(1)?, transaction_date: r.get(2)?, party_name: r.get(3)? })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        // The sub-select page is already in index order; the join may not keep it, so fix the order here
        // (at most 50 rows) instead of paying a TEMP B-TREE in SQL.
        rows.sort_by(|a, b| b.transaction_date.cmp(&a.transaction_date).then(b.bill_id.cmp(&a.bill_id)));
        Ok((rows, total.max(0) as usize))
    }

    fn update_header_for_edit(&mut self, h: &SaleTransaction) -> Result<(), AppError> {
        let n = self.conn.execute(
            "UPDATE sale_transactions SET transaction_date = ?2, party_id = ?3, party_name = ?4, payment_mode = ?5, \
             notes = ?6, items_subtotal = ?7, charges_total = ?8, discounts_total = ?9, total_amount = ?10, \
             updated_at = ?11 WHERE id = ?1",
            params![
                h.id, h.transaction_date, h.party_id, h.party_name, h.payment_mode, h.notes, h.items_subtotal,
                h.charges_total, h.discounts_total, h.total_amount, h.updated_at
            ],
        )?;
        if n == 0 {
            return Err(AppError::not_found("SaleTransaction", h.id.to_string()));
        }
        // Keep the denormalized line date equal to the header date (schema v9 invariant).
        self.conn.execute(
            "UPDATE sale_items SET transaction_date = ?2 WHERE sale_transaction_id = ?1",
            params![h.id, h.transaction_date],
        )?;
        Ok(())
    }

    fn delete_sale_items(&mut self, sale_id: i64) -> Result<usize, AppError> {
        Ok(self.conn.execute("DELETE FROM sale_items WHERE sale_transaction_id = ?1", params![sale_id])?)
    }

    fn delete_transaction(&mut self, id: i64) -> Result<(), AppError> {
        self.conn.execute("DELETE FROM sale_transactions WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn create_transaction(&mut self, tx: &SaleTransaction) -> Result<i64, AppError> {
        self.conn.execute(
            &format!("INSERT INTO sale_transactions ({TX_COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)"),
            params![
                opt_id(tx.id), tx.transaction_code, tx.transaction_date, tx.party_id, tx.party_name,
                tx.payment_mode, enum_to_sql(&tx.payment_status)?, tx.total_amount, tx.paid_amount, tx.balance_due,
                tx.notes, tx.created_at, tx.items_subtotal, tx.charges_total, tx.discounts_total, tx.updated_at
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn create_sale_item(&mut self, l: &SaleItem) -> Result<i64, AppError> {
        self.conn.execute(
            &format!(
                "INSERT INTO sale_items ({ITEM_COLS}, transaction_date) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,                  (SELECT transaction_date FROM sale_transactions WHERE id = ?2))"
            ),
            params![
                opt_id(l.id), l.sale_transaction_id, l.item_id, l.item_name, l.pack_label, l.pack_quantity,
                l.quantity, l.unit_price, l.total_amount
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn find_transaction_by_id(&self, id: i64) -> Result<Option<SaleTransaction>, AppError> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {TX_COLS} FROM sale_transactions WHERE id = ?1"), params![id], map_tx)
            .optional()?)
    }

    fn find_sale_item_by_id(&self, id: i64) -> Result<Option<SaleItem>, AppError> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {ITEM_COLS} FROM sale_items WHERE id = ?1"), params![id], map_item)
            .optional()?)
    }

    fn get_next_transaction_code(&self) -> Result<String, AppError> {
        next_code(&self.conn, "sale_transactions", "transaction_code", &SALE_CODE)
    }

    fn update_money_fields(
        &mut self,
        tx_id: i64,
        paid_amount: f64,
        balance_due: f64,
        status: PaymentStatus,
    ) -> Result<(), AppError> {
        let n = self.conn.execute(
            "UPDATE sale_transactions SET paid_amount = ?1, balance_due = ?2, payment_status = ?3 WHERE id = ?4",
            params![paid_amount, balance_due, enum_to_sql(&status)?, tx_id],
        )?;
        if n == 0 {
            return Err(AppError::NotFound { entity: "SaleTransaction".to_string(), id: tx_id.to_string() });
        }
        Ok(())
    }

    fn list_open_for_party(&self, party_id: i64) -> Result<Vec<SaleTransaction>, AppError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {TX_COLS} FROM sale_transactions \
             WHERE party_id = ?1 AND balance_due > 0 ORDER BY transaction_date, id"
        ))?;
        let rows = stmt.query_map(params![party_id], map_tx)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn list_sales(
        &self,
        start_date: Option<&str>,
        end_date: Option<&str>,
        status: Option<PaymentStatus>,
        party_id: Option<i64>,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SaleTransaction>, AppError> {
        let (w, mut vals) = Self::where_clause(start_date, end_date, status, party_id)?;
        vals.push(Value::Integer(lim(limit)));
        vals.push(Value::Integer(lim(offset)));
        let sql = format!("SELECT {TX_COLS} FROM sale_transactions{w} ORDER BY id LIMIT ? OFFSET ?");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(vals.iter()), map_tx)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn list_items_by_transaction_id(&self, tx_id: i64) -> Result<Vec<SaleItem>, AppError> {
        self.query_items(
            &format!("SELECT {ITEM_COLS} FROM sale_items WHERE sale_transaction_id = ?1 ORDER BY id"),
            params![tx_id],
        )
    }

    fn list_history(
        &self,
        filter: &SalesHistoryFilter,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<SaleHistoryRow>, usize), AppError> {
        let (w, vals) = history_where(filter, None, &["t.transaction_code", "t.party_name"])?;
        let total: i64 = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM sale_transactions t{w}"),
            rusqlite::params_from_iter(vals.iter()),
            |r| r.get(0),
        )?;
        let mut page_vals = vals;
        page_vals.push(Value::Integer(lim(limit)));
        page_vals.push(Value::Integer(lim(offset)));
        let sql = format!(
            "SELECT t.id, t.transaction_code, t.transaction_date, t.created_at, t.party_id, t.party_name, \
             (SELECT COUNT(*) FROM sale_items si WHERE si.sale_transaction_id = t.id), \
             t.total_amount, t.paid_amount, t.balance_due, t.payment_status, t.items_subtotal, t.charges_total, t.discounts_total \
             FROM sale_transactions t{w} ORDER BY t.transaction_date DESC, t.id DESC LIMIT ? OFFSET ?"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(page_vals.iter()), |r| {
                Ok(SaleHistoryRow {
                    id: r.get(0)?,
                    transaction_code: r.get(1)?,
                    transaction_date: r.get(2)?,
                    created_at: r.get(3)?,
                    party_id: r.get(4)?,
                    party_name: r.get(5)?,
                    line_count: r.get(6)?,
                    total_amount: r.get(7)?,
                    paid_amount: r.get(8)?,
                    balance_due: r.get(9)?,
                    payment_status: enum_col(r, 10)?,
                    items_subtotal: r.get(11)?,
                    charges_total: r.get(12)?,
                    discounts_total: r.get(13)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok((rows, total.max(0) as usize))
    }

    fn get_returned_quantities_for_sale(&self, sale_id: i64) -> Result<Vec<(i64, f64)>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT ri.sale_item_id, SUM(ri.quantity) FROM return_items ri \
             JOIN return_transactions rt ON rt.id = ri.return_transaction_id \
             JOIN sale_items si ON si.id = ri.sale_item_id \
             WHERE si.sale_transaction_id = ?1 AND ri.line_type = 'RETURNED' AND rt.return_type = 'CUSTOMER' \
             GROUP BY ri.sale_item_id",
        )?;
        let rows = stmt
            .query_map(params![sale_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn get_returned_quantity_for_line(&self, sale_item_id: i64) -> Result<f64, AppError> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(SUM(ri.quantity), 0.0) FROM return_items ri \
             JOIN return_transactions rt ON rt.id = ri.return_transaction_id \
             WHERE ri.sale_item_id = ?1 AND ri.line_type = 'RETURNED' AND rt.return_type = 'CUSTOMER'",
            params![sale_item_id],
            |r| r.get::<_, f64>(0),
        )?)
    }
}
