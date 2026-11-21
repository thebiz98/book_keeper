//! SQLite implementation of `SaleRepository`.
use rusqlite::{params, types::Value, OptionalExtension, Row};

use crate::core::dtos::{PaginatedList, SalesFilter};
use crate::core::entities::{SaleItem, SaleTransaction};
use crate::core::error::AppError;
use crate::core::value_objects::PaymentStatus;
use crate::data::contracts::sale_repository::SaleRepository;
use crate::data::sqlite::code_gen::{next_code, SALE_CODE};
use crate::data::sqlite::{enum_col, enum_to_sql, SqliteConn};

const TX_COLS: &str = "id, transaction_code, transaction_date, party_id, party_name, payment_mode, payment_status, total_amount, paid_amount, balance_due, notes, created_at";
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

impl SaleRepository for SqliteSaleRepository {
    fn create_transaction(&mut self, tx: &SaleTransaction) -> Result<i64, AppError> {
        self.conn.execute(
            &format!("INSERT INTO sale_transactions ({TX_COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)"),
            params![
                opt_id(tx.id), tx.transaction_code, tx.transaction_date, tx.party_id, tx.party_name,
                tx.payment_mode, enum_to_sql(&tx.payment_status)?, tx.total_amount, tx.paid_amount, tx.balance_due,
                tx.notes, tx.created_at
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn create_sale_item(&mut self, l: &SaleItem) -> Result<i64, AppError> {
        self.conn.execute(
            &format!("INSERT INTO sale_items ({ITEM_COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)"),
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

    fn list_transactions(&self, filter: &SalesFilter) -> Result<PaginatedList<SaleTransaction>, AppError> {
        let from = filter.from_date.as_deref().or(filter.start_date.as_deref());
        let to = filter.to_date.as_deref().or(filter.end_date.as_deref());
        let page = filter.page.unwrap_or(1);
        let limit = filter.limit.unwrap_or(50);
        let offset = page.saturating_sub(1) * limit;

        let (w, vals) = Self::where_clause(from, to, filter.payment_status, filter.party_id)?;
        let total: i64 = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM sale_transactions{w}"),
            rusqlite::params_from_iter(vals.iter()),
            |r| r.get(0),
        )?;
        let items = self.list_sales(from, to, filter.payment_status, filter.party_id, offset, limit)?;
        Ok(PaginatedList::new(items, total as usize, page, limit))
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
