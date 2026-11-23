//! SQLite implementation of `ReportRepository`: SQL-level aggregates for dashboard & reports.
use rusqlite::params;

use crate::core::dtos::{
    CategoryValuation, LowStockItem, PnLReport, RecentTransaction, RecentTransactionKind,
    StockValuationReport,
};
use crate::core::error::AppError;
use crate::core::money::round2;
use crate::data::contracts::report_repository::ReportRepository;
use crate::data::sqlite::{opt_enum_col, SqliteConn};

pub struct SqliteReportRepository {
    conn: SqliteConn,
}

impl SqliteReportRepository {
    pub fn new(conn: SqliteConn) -> Self {
        Self { conn }
    }
}

/// All P&L components in one statement (?1 = from, ?2 = to, inclusive lexical date range).
const PNL_SQL: &str = "SELECT
  (SELECT COALESCE(SUM(total_amount), 0.0) FROM sale_transactions
     WHERE transaction_date BETWEEN ?1 AND ?2),
  (SELECT COALESCE(SUM(total_amount), 0.0) FROM return_transactions
     WHERE return_type = 'CUSTOMER' AND settlement_type = 'REFUND' AND return_date BETWEEN ?1 AND ?2),
  (SELECT COALESCE(SUM((a.quantity_depleted - a.returned_quantity) * a.unit_cost), 0.0)
     FROM sale_stock_allocations a
     JOIN sale_items si ON si.id = a.sale_item_id
     JOIN sale_transactions s ON s.id = si.sale_transaction_id
     WHERE s.transaction_date BETWEEN ?1 AND ?2),
  (SELECT COALESCE(SUM(o.quantity_depleted * o.unit_cost), 0.0)
     FROM stock_outflow_allocations o
     JOIN return_transactions r ON r.id = o.source_id
     WHERE o.source_type = 'REPLACEMENT' AND r.return_date BETWEEN ?1 AND ?2),
  (SELECT COALESCE(SUM(o.quantity_depleted * o.unit_cost), 0.0)
     FROM stock_outflow_allocations o
     JOIN stock_adjustments j ON j.id = o.source_id
     WHERE o.source_type = 'ADJUSTMENT' AND j.adjustment_date BETWEEN ?1 AND ?2),
  (SELECT COALESCE(SUM(ri.cost_amount), 0.0)
     FROM return_items ri
     JOIN return_transactions r ON r.id = ri.return_transaction_id
     WHERE r.return_type = 'CUSTOMER' AND ri.line_type = 'RETURNED'
       AND ri.condition = 'DAMAGED_SCRAP' AND r.return_date BETWEEN ?1 AND ?2),
  (SELECT COALESCE(SUM(total_amount), 0.0) FROM purchase_transactions
     WHERE source = 'PURCHASE' AND transaction_date BETWEEN ?1 AND ?2),
  (SELECT COALESCE(SUM(total_amount), 0.0) FROM return_transactions
     WHERE return_type = 'VENDOR' AND return_date BETWEEN ?1 AND ?2)";

impl ReportRepository for SqliteReportRepository {
    fn pnl(&self, from: &str, to: &str) -> Result<PnLReport, AppError> {
        let v: [f64; 8] = self.conn.query_row(PNL_SQL, params![from, to], |r| {
            let mut a = [0.0; 8];
            for (i, slot) in a.iter_mut().enumerate() {
                *slot = r.get(i)?;
            }
            Ok(a)
        })?;
        let [gross, returns, cogs_sales, cogs_repl, loss_adj, loss_scrap, purchases, vendor_returns] = v;
        let gross_sales = round2(gross);
        let sales_returns = round2(returns);
        let net_sales = round2(gross_sales - sales_returns);
        let cogs = round2(cogs_sales + cogs_repl);
        let gross_profit = round2(net_sales - cogs);
        let inventory_losses = round2(loss_adj + loss_scrap);
        Ok(PnLReport {
            gross_sales,
            sales_returns,
            net_sales,
            cogs,
            gross_profit,
            inventory_losses,
            net_profit: round2(gross_profit - inventory_losses),
            total_purchases: round2(purchases - vendor_returns),
        })
    }

    fn sales_totals(&self, from: &str, to: &str) -> Result<(f64, usize), AppError> {
        let (sum, n): (f64, i64) = self.conn.query_row(
            "SELECT COALESCE(SUM(total_amount), 0.0), COUNT(*) FROM sale_transactions \
             WHERE transaction_date BETWEEN ?1 AND ?2",
            params![from, to],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok((round2(sum), n as usize))
    }

    fn purchase_totals(&self, from: &str, to: &str) -> Result<(f64, usize), AppError> {
        let (sum, n): (f64, i64) = self.conn.query_row(
            "SELECT COALESCE(SUM(total_amount), 0.0), COUNT(*) FROM purchase_transactions \
             WHERE source = 'PURCHASE' AND transaction_date BETWEEN ?1 AND ?2",
            params![from, to],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok((round2(sum), n as usize))
    }

    fn dues_totals(&self) -> Result<(f64, f64), AppError> {
        let (rec, pay): (f64, f64) = self.conn.query_row(
            "SELECT \
               (SELECT COALESCE(SUM(balance_due), 0.0) FROM sale_transactions WHERE balance_due > 0), \
               (SELECT COALESCE(SUM(balance_due), 0.0) FROM purchase_transactions \
                  WHERE source = 'PURCHASE' AND balance_due > 0)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok((round2(rec), round2(pay)))
    }

    fn stock_valuation(&self) -> Result<StockValuationReport, AppError> {
        let (total, lots): (f64, i64) = self.conn.query_row(
            "SELECT COALESCE(SUM(remaining_quantity * unit_cost), 0.0), COUNT(*) \
             FROM purchase_items WHERE remaining_quantity > 0",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let mut stmt = self.conn.prepare(
            "SELECT COALESCE(i.category, 'Uncategorized') AS cat, COUNT(DISTINCT i.id), \
                    COALESCE(SUM(p.remaining_quantity * p.unit_cost), 0.0) \
             FROM purchase_items p JOIN items i ON i.id = p.item_id \
             WHERE p.remaining_quantity > 0 GROUP BY cat ORDER BY cat",
        )?;
        let categories_breakdown = stmt
            .query_map([], |r| {
                Ok(CategoryValuation {
                    category: r.get(0)?,
                    items_count: r.get::<_, i64>(1)? as usize,
                    valuation: round2(r.get(2)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(StockValuationReport {
            total_valuation: round2(total),
            active_lots_count: lots as usize,
            categories_breakdown,
        })
    }

    fn low_stock_items(&self) -> Result<Vec<LowStockItem>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, category, base_unit, current_stock, default_sale_price, mrp, low_stock_threshold \
             FROM items WHERE is_active = 1 AND is_low_stock = 1 \
             ORDER BY current_stock ASC, name COLLATE NOCASE, id",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(LowStockItem {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    category: r.get(2)?,
                    base_unit: r.get(3)?,
                    current_stock: r.get(4)?,
                    default_sale_price: r.get(5)?,
                    mrp: r.get(6)?,
                    low_stock_threshold: r.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn trend_buckets(&self, from: &str, to: &str, key_len: usize) -> Result<Vec<(String, f64, f64)>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT k, SUM(s), SUM(p) FROM ( \
               SELECT substr(transaction_date, 1, ?3) AS k, total_amount AS s, 0.0 AS p \
                 FROM sale_transactions WHERE transaction_date BETWEEN ?1 AND ?2 \
               UNION ALL \
               SELECT substr(transaction_date, 1, ?3), 0.0, total_amount \
                 FROM purchase_transactions WHERE source = 'PURCHASE' AND transaction_date BETWEEN ?1 AND ?2 \
             ) GROUP BY k ORDER BY k",
        )?;
        let rows = stmt
            .query_map(params![from, to, key_len as i64], |r| {
                Ok((r.get::<_, String>(0)?, round2(r.get(1)?), round2(r.get(2)?)))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn recent_transactions(&self, limit: usize) -> Result<Vec<RecentTransaction>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT kind, id, code, date, created_at, party_name, amount, status FROM ( \
               SELECT 'SALE' AS kind, id, transaction_code AS code, transaction_date AS date, created_at, \
                      party_name, total_amount AS amount, payment_status AS status \
                 FROM sale_transactions ORDER BY created_at DESC, id DESC LIMIT ?1) \
             UNION ALL \
             SELECT kind, id, code, date, created_at, party_name, amount, status FROM ( \
               SELECT 'PURCHASE' AS kind, id, transaction_code AS code, transaction_date AS date, created_at, \
                      party_name, total_amount AS amount, payment_status AS status \
                 FROM purchase_transactions WHERE source = 'PURCHASE' \
                 ORDER BY created_at DESC, id DESC LIMIT ?1) \
             UNION ALL \
             SELECT kind, id, code, date, created_at, party_name, amount, status FROM ( \
               SELECT 'RETURN' AS kind, id, return_code AS code, return_date AS date, created_at, \
                      party_name, total_amount AS amount, NULL AS status \
                 FROM return_transactions ORDER BY created_at DESC, id DESC LIMIT ?1) \
             ORDER BY created_at DESC, id DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit as i64], |r| {
                let kind: String = r.get(0)?;
                let kind = match kind.as_str() {
                    "SALE" => RecentTransactionKind::Sale,
                    "PURCHASE" => RecentTransactionKind::Purchase,
                    _ => RecentTransactionKind::Return,
                };
                Ok(RecentTransaction {
                    kind,
                    id: r.get(1)?,
                    code: r.get(2)?,
                    date: r.get(3)?,
                    created_at: r.get(4)?,
                    party_name: r.get(5)?,
                    amount: round2(r.get(6)?),
                    payment_status: opt_enum_col(r, 7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}
