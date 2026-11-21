//! SQLite implementation of `PurchaseRepository`.
use rusqlite::{params, types::Value, OptionalExtension, Row};

use crate::core::dtos::{ItemLot, PaginatedList, PurchaseFilter};
use crate::core::entities::{PurchaseItem, PurchaseTransaction};
use crate::core::error::AppError;
use crate::core::value_objects::{LotSource, PaymentStatus};
use crate::data::contracts::purchase_repository::PurchaseRepository;
use crate::data::sqlite::code_gen::{next_code, seq_for_source};
use crate::data::sqlite::{enum_col, enum_to_sql, SqliteConn};

const TX_COLS: &str = "id, transaction_code, source, supplier_invoice_ref, transaction_date, party_id, party_name, payment_mode, payment_status, total_amount, paid_amount, balance_due, notes, created_at";
const ITEM_COLS: &str = "id, purchase_transaction_id, item_id, item_name, pack_label, pack_quantity, quantity, unit_cost, total_amount, remaining_quantity";

fn map_tx(r: &Row<'_>) -> rusqlite::Result<PurchaseTransaction> {
    Ok(PurchaseTransaction {
        id: r.get(0)?,
        transaction_code: r.get(1)?,
        source: enum_col(r, 2)?,
        supplier_invoice_ref: r.get(3)?,
        transaction_date: r.get(4)?,
        party_id: r.get(5)?,
        party_name: r.get(6)?,
        payment_mode: r.get(7)?,
        payment_status: enum_col(r, 8)?,
        total_amount: r.get(9)?,
        paid_amount: r.get(10)?,
        balance_due: r.get(11)?,
        notes: r.get(12)?,
        created_at: r.get(13)?,
    })
}

fn map_item(r: &Row<'_>) -> rusqlite::Result<PurchaseItem> {
    Ok(PurchaseItem {
        id: r.get(0)?,
        purchase_transaction_id: r.get(1)?,
        item_id: r.get(2)?,
        item_name: r.get(3)?,
        pack_label: r.get(4)?,
        pack_quantity: r.get(5)?,
        quantity: r.get(6)?,
        unit_cost: r.get(7)?,
        total_amount: r.get(8)?,
        remaining_quantity: r.get(9)?,
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
    ) -> Result<(String, Vec<Value>), AppError> {
        // History is PURCHASE-source only: ADJUSTMENT / OPENING / RETURN headers are not supplier bills.
        let mut conds: Vec<&str> = vec!["source = 'PURCHASE'"];
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
        let w = format!(" WHERE {}", conds.join(" AND "));
        Ok((w, vals))
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
            &format!("INSERT INTO purchase_transactions ({TX_COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)"),
            params![
                opt_id(tx.id), tx.transaction_code, enum_to_sql(&tx.source)?, tx.supplier_invoice_ref,
                tx.transaction_date, tx.party_id, tx.party_name, tx.payment_mode,
                enum_to_sql(&tx.payment_status)?, tx.total_amount, tx.paid_amount, tx.balance_due,
                tx.notes, tx.created_at
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn create_purchase_item(&mut self, l: &PurchaseItem) -> Result<i64, AppError> {
        self.conn.execute(
            &format!("INSERT INTO purchase_items ({ITEM_COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)"),
            params![
                opt_id(l.id), l.purchase_transaction_id, l.item_id, l.item_name, l.pack_label, l.pack_quantity,
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

    fn find_purchase_item_by_id(&self, id: i64) -> Result<Option<PurchaseItem>, AppError> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {ITEM_COLS} FROM purchase_items WHERE id = ?1"), params![id], map_item)
            .optional()?)
    }

    fn get_next_transaction_code(&self, source: LotSource) -> Result<String, AppError> {
        next_code(&self.conn, "purchase_transactions", "transaction_code", seq_for_source(source))
    }

    fn get_active_lots_for_item(&self, item_id: i64) -> Result<Vec<PurchaseItem>, AppError> {
        self.query_items(
            &format!("SELECT {ITEM_COLS} FROM purchase_items WHERE item_id = ?1 AND remaining_quantity > 0 ORDER BY id"),
            params![item_id],
        )
    }

    fn get_latest_lot_cost(&self, item_id: i64) -> Result<Option<f64>, AppError> {
        Ok(self
            .conn
            .query_row(
                "SELECT unit_cost FROM purchase_items WHERE item_id = ?1 ORDER BY id DESC LIMIT 1",
                params![item_id],
                |r| r.get::<_, f64>(0),
            )
            .optional()?)
    }

    fn list_lots_for_item(&self, item_id: i64) -> Result<Vec<ItemLot>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT pi.id, pi.purchase_transaction_id, pt.transaction_code, pt.source, pt.transaction_date,                     pt.party_name, pi.quantity, pi.remaining_quantity, pi.unit_cost              FROM purchase_items pi JOIN purchase_transactions pt ON pt.id = pi.purchase_transaction_id              WHERE pi.item_id = ?1 ORDER BY pi.id",
        )?;
        let rows = stmt
            .query_map(params![item_id], |r| {
                Ok(ItemLot {
                    purchase_item_id: r.get(0)?,
                    purchase_transaction_id: r.get(1)?,
                    transaction_code: r.get(2)?,
                    source: enum_col(r, 3)?,
                    transaction_date: r.get(4)?,
                    vendor_name: r.get(5)?,
                    quantity: r.get(6)?,
                    remaining_quantity: r.get(7)?,
                    unit_cost: r.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn update_remaining_quantity(&mut self, lot_id: i64, new_remaining_qty: f64) -> Result<(), AppError> {
        const EPS: f64 = 1e-9;
        let quantity: f64 = self
            .conn
            .query_row("SELECT quantity FROM purchase_items WHERE id = ?1", params![lot_id], |r| r.get(0))
            .optional()?
            .ok_or_else(|| AppError::NotFound { entity: "PurchaseItem".to_string(), id: lot_id.to_string() })?;
        if !new_remaining_qty.is_finite() || new_remaining_qty < -EPS || new_remaining_qty > quantity + EPS {
            return Err(AppError::ValidationError {
                field: "remaining_quantity".to_string(),
                message: format!(
                    "Remaining quantity {new_remaining_qty} is outside 0..={quantity} for lot #{lot_id}"
                ),
            });
        }
        // Absorb float residue so a lot never holds a sliver below 0 or above its quantity.
        let clamped = new_remaining_qty.clamp(0.0, quantity);
        self.conn.execute(
            "UPDATE purchase_items SET remaining_quantity = ?1 WHERE id = ?2",
            params![clamped, lot_id],
        )?;
        Ok(())
    }

    fn update_balance_due(&mut self, tx_id: i64, new_balance: f64, status: PaymentStatus) -> Result<(), AppError> {
        let n = self.conn.execute(
            "UPDATE purchase_transactions SET paid_amount = paid_amount + MAX(balance_due - ?1, 0.0), balance_due = ?1, payment_status = ?2 WHERE id = ?3",
            params![new_balance, enum_to_sql(&status)?, tx_id],
        )?;
        if n == 0 {
            return Err(AppError::NotFound { entity: "PurchaseTransaction".to_string(), id: tx_id.to_string() });
        }
        Ok(())
    }

    fn update_money_fields(
        &mut self,
        tx_id: i64,
        paid_amount: f64,
        balance_due: f64,
        status: PaymentStatus,
    ) -> Result<(), AppError> {
        let n = self.conn.execute(
            "UPDATE purchase_transactions SET paid_amount = ?1, balance_due = ?2, payment_status = ?3 WHERE id = ?4",
            params![paid_amount, balance_due, enum_to_sql(&status)?, tx_id],
        )?;
        if n == 0 {
            return Err(AppError::NotFound { entity: "PurchaseTransaction".to_string(), id: tx_id.to_string() });
        }
        Ok(())
    }

    fn list_open_for_party(&self, party_id: i64) -> Result<Vec<PurchaseTransaction>, AppError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {TX_COLS} FROM purchase_transactions \
             WHERE party_id = ?1 AND source = 'PURCHASE' AND balance_due > 0 ORDER BY transaction_date, id"
        ))?;
        let rows = stmt.query_map(params![party_id], map_tx)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
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
        let (w, mut vals) = Self::where_clause(start_date, end_date, status, party_id)?;
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

    fn list_all_lots(&self) -> Result<Vec<PurchaseItem>, AppError> {
        self.query_items(&format!("SELECT {ITEM_COLS} FROM purchase_items ORDER BY id"), [])
    }

    fn list_transactions(&self, filter: &PurchaseFilter) -> Result<PaginatedList<PurchaseTransaction>, AppError> {
        let from = filter.from_date.as_deref().or(filter.start_date.as_deref());
        let to = filter.to_date.as_deref().or(filter.end_date.as_deref());
        let page = filter.page.unwrap_or(1);
        let limit = filter.limit.unwrap_or(50);
        let offset = page.saturating_sub(1) * limit;

        let (w, vals) = Self::where_clause(from, to, filter.payment_status, filter.party_id)?;
        let total: i64 = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM purchase_transactions{w}"),
            rusqlite::params_from_iter(vals.iter()),
            |r| r.get(0),
        )?;
        let items = self.list_purchases(from, to, filter.payment_status, filter.party_id, offset, limit)?;
        Ok(PaginatedList::new(items, total as usize, page, limit))
    }

    fn get_returned_quantity_for_lot(&self, purchase_item_id: i64) -> Result<f64, AppError> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(SUM(ri.quantity), 0.0) FROM return_items ri \
             JOIN return_transactions rt ON rt.id = ri.return_transaction_id \
             WHERE ri.purchase_item_id = ?1 AND ri.line_type = 'RETURNED' AND rt.return_type = 'VENDOR'",
            params![purchase_item_id],
            |r| r.get::<_, f64>(0),
        )?)
    }
}
