//! SQLite implementation of `PurchaseRepository`.
use rusqlite::{params, types::Value, OptionalExtension, Row};

use crate::core::dtos::{PaginatedList, PurchaseFilter};
use crate::core::entities::{PurchaseItem, PurchaseTransaction};
use crate::core::error::AppError;
use crate::core::value_objects::PaymentStatus;
use crate::data::contracts::purchase_repository::PurchaseRepository;
use crate::data::sqlite::{enum_col, enum_to_sql, SqliteConn};

const TX_COLS: &str = "id, transaction_code, transaction_date, party_id, payment_mode, payment_status, total_amount, paid_amount, balance_due, notes, created_at";
const ITEM_COLS: &str = "id, purchase_transaction_id, item_id, bundle_id, bundle_quantity, quantity, unit_cost, total_amount, remaining_quantity";

fn map_tx(r: &Row<'_>) -> rusqlite::Result<PurchaseTransaction> {
    Ok(PurchaseTransaction {
        id: r.get(0)?,
        transaction_code: r.get(1)?,
        transaction_date: r.get(2)?,
        party_id: r.get(3)?,
        payment_mode: r.get(4)?,
        payment_status: enum_col(r, 5)?,
        total_amount: r.get(6)?,
        paid_amount: r.get(7)?,
        balance_due: r.get(8)?,
        notes: r.get(9)?,
        created_at: r.get(10)?,
    })
}

fn map_item(r: &Row<'_>) -> rusqlite::Result<PurchaseItem> {
    Ok(PurchaseItem {
        id: r.get(0)?,
        purchase_transaction_id: r.get(1)?,
        item_id: r.get(2)?,
        bundle_id: r.get(3)?,
        bundle_quantity: r.get(4)?,
        quantity: r.get(5)?,
        unit_cost: r.get(6)?,
        total_amount: r.get(7)?,
        remaining_quantity: r.get(8)?,
    })
}

fn opt_id(id: i64) -> Option<i64> {
    if id > 0 { Some(id) } else { None }
}

fn lim(n: usize) -> i64 {
    n.min(i64::MAX as usize) as i64
}

pub struct SqlitePurchaseRepository {
    conn: SqliteConn,
}

impl SqlitePurchaseRepository {
    pub fn new(conn: SqliteConn) -> Self {
        Self { conn }
    }

    fn where_clause(
        start: Option<&str>,
        end: Option<&str>,
        status: Option<PaymentStatus>,
        party_id: Option<i64>,
    ) -> (String, Vec<Value>) {
        let mut conds: Vec<&str> = Vec::new();
        let mut vals: Vec<Value> = Vec::new();
        if let Some(p) = party_id {
            conds.push("party_id = ?");
            vals.push(Value::Integer(p));
        }
        if let Some(s) = status {
            conds.push("payment_status = ?");
            vals.push(Value::Text(enum_to_sql(&s)));
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
        (w, vals)
    }

    fn query_items(&self, sql: &str, p: impl rusqlite::Params) -> Result<Vec<PurchaseItem>, AppError> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(p, map_item)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

impl PurchaseRepository for SqlitePurchaseRepository {
    fn create_transaction(&mut self, tx: &PurchaseTransaction) -> Result<i64, AppError> {
        self.conn.execute(
            &format!("INSERT INTO purchase_transactions ({TX_COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)"),
            params![
                opt_id(tx.id), tx.transaction_code, tx.transaction_date, tx.party_id, tx.payment_mode,
                enum_to_sql(&tx.payment_status), tx.total_amount, tx.paid_amount, tx.balance_due,
                tx.notes, tx.created_at
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn create_purchase_item(&mut self, l: &PurchaseItem) -> Result<i64, AppError> {
        self.conn.execute(
            &format!("INSERT INTO purchase_items ({ITEM_COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)"),
            params![
                opt_id(l.id), l.purchase_transaction_id, l.item_id, l.bundle_id, l.bundle_quantity,
                l.quantity, l.unit_cost, l.total_amount, l.remaining_quantity
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn find_transaction_by_id(&self, id: i64) -> Result<Option<PurchaseTransaction>, AppError> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {TX_COLS} FROM purchase_transactions WHERE id = ?1"), params![id], map_tx)
            .optional()?)
    }

    fn find_by_id(&self, id: i64) -> Result<Option<PurchaseTransaction>, AppError> {
        self.find_transaction_by_id(id)
    }

    fn find_by_code(&self, code: &str) -> Result<Option<PurchaseTransaction>, AppError> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {TX_COLS} FROM purchase_transactions WHERE transaction_code = ?1"),
                params![code],
                map_tx,
            )
            .optional()?)
    }

    fn get_next_transaction_code(&self) -> Result<String, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT transaction_code FROM purchase_transactions WHERE transaction_code LIKE 'PUR-%'")?;
        let codes = stmt.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
        let max = codes.iter().filter_map(|c| c[4..].parse::<u64>().ok()).max().unwrap_or(1000);
        Ok(format!("PUR-{}", max + 1))
    }

    fn get_latest_code(&self) -> Result<Option<String>, AppError> {
        Ok(self
            .conn
            .query_row("SELECT MAX(transaction_code) FROM purchase_transactions", [], |r| r.get::<_, Option<String>>(0))?)
    }

    fn get_active_lots_for_item(&self, item_id: i64) -> Result<Vec<PurchaseItem>, AppError> {
        self.query_items(
            &format!("SELECT {ITEM_COLS} FROM purchase_items WHERE item_id = ?1 AND remaining_quantity > 0 ORDER BY id"),
            params![item_id],
        )
    }

    fn list_active_lots_for_item(&self, item_id: i64) -> Result<Vec<PurchaseItem>, AppError> {
        self.get_active_lots_for_item(item_id)
    }

    fn update_remaining_quantity(&mut self, lot_id: i64, new_remaining_qty: f64) -> Result<(), AppError> {
        let n = self.conn.execute(
            "UPDATE purchase_items SET remaining_quantity = ?1 WHERE id = ?2",
            params![new_remaining_qty, lot_id],
        )?;
        if n == 0 {
            return Err(AppError::NotFound { entity: "PurchaseItem".to_string(), id: lot_id.to_string() });
        }
        Ok(())
    }

    fn update_balance_due(&mut self, tx_id: i64, new_balance: f64, status: PaymentStatus) -> Result<(), AppError> {
        let n = self.conn.execute(
            "UPDATE purchase_transactions SET paid_amount = paid_amount + MAX(balance_due - ?1, 0.0), balance_due = ?1, payment_status = ?2 WHERE id = ?3",
            params![new_balance, enum_to_sql(&status), tx_id],
        )?;
        if n == 0 {
            return Err(AppError::NotFound { entity: "PurchaseTransaction".to_string(), id: tx_id.to_string() });
        }
        Ok(())
    }

    fn list_purchases(
        &self,
        start_date: Option<&str>,
        end_date: Option<&str>,
        status: Option<PaymentStatus>,
        party_id: Option<i64>,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<PurchaseTransaction>, AppError> {
        let (w, mut vals) = Self::where_clause(start_date, end_date, status, party_id);
        vals.push(Value::Integer(lim(limit)));
        vals.push(Value::Integer(lim(offset)));
        let sql = format!("SELECT {TX_COLS} FROM purchase_transactions{w} ORDER BY id LIMIT ? OFFSET ?");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(vals.iter()), map_tx)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn list_items_by_transaction_id(&self, tx_id: i64) -> Result<Vec<PurchaseItem>, AppError> {
        self.query_items(
            &format!("SELECT {ITEM_COLS} FROM purchase_items WHERE purchase_transaction_id = ?1 ORDER BY id"),
            params![tx_id],
        )
    }

    fn get_latest_purchase_item_for_item(&self, item_id: i64) -> Result<Option<PurchaseItem>, AppError> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {ITEM_COLS} FROM purchase_items WHERE item_id = ?1 ORDER BY id DESC LIMIT 1"),
                params![item_id],
                map_item,
            )
            .optional()?)
    }

    fn list_all_lots(&self) -> Result<Vec<PurchaseItem>, AppError> {
        self.query_items(&format!("SELECT {ITEM_COLS} FROM purchase_items ORDER BY id"), [])
    }

    fn list_transactions(&self, filter: &PurchaseFilter) -> Result<PaginatedList<PurchaseTransaction>, AppError> {
        let from = filter.from_date.as_deref().or(filter.start_date.as_deref());
        let to = filter.to_date.as_deref().or(filter.end_date.as_deref());
        let page = filter.page.unwrap_or(1);
        let limit = filter.limit.unwrap_or(50);
        let offset = page.saturating_sub(1) * limit;

        let (w, vals) = Self::where_clause(from, to, filter.payment_status, filter.party_id);
        let total: i64 = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM purchase_transactions{w}"),
            rusqlite::params_from_iter(vals.iter()),
            |r| r.get(0),
        )?;
        let items = self.list_purchases(from, to, filter.payment_status, filter.party_id, offset, limit)?;
        Ok(PaginatedList::new(items, total as usize, page, limit))
    }
}

impl SqlitePurchaseRepository {
    pub fn find_purchase_item_by_id(&self, id: i64) -> Result<Option<PurchaseItem>, AppError> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {ITEM_COLS} FROM purchase_items WHERE id = ?1"), params![id], map_item)
            .optional()?)
    }
}
