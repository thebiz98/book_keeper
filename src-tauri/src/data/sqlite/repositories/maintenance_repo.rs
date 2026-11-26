//! SQLite implementation of `MaintenanceRepository` (testing-only data reset).
use crate::core::error::AppError;
use crate::data::contracts::MaintenanceRepository;
use crate::data::sqlite::SqliteConn;

pub struct SqliteMaintenanceRepository {
    conn: SqliteConn,
}

impl SqliteMaintenanceRepository {
    pub fn new(conn: SqliteConn) -> Self {
        Self { conn }
    }

    /// Runs each `(label, sql)` and records the affected row count.
    fn run(&self, steps: &[(&str, &str)]) -> Result<Vec<(String, i64)>, AppError> {
        let mut out = Vec::with_capacity(steps.len());
        for (label, sql) in steps {
            let n = self.conn.execute(sql, [])?;
            out.push(((*label).to_string(), n as i64));
        }
        Ok(out)
    }

    /// Lets AUTOINCREMENT ids restart (from MAX(id)+1, or 1 for an empty table).
    fn reset_sequences(&self, tables: &[&str]) -> Result<(), AppError> {
        for t in tables {
            self.conn.execute("DELETE FROM sqlite_sequence WHERE name = ?1", [t])?;
        }
        Ok(())
    }
}

/// Child tables first. OPENING lots (and their purchase headers) are kept.
const TRANSACTION_STEPS: &[(&str, &str)] = &[
    ("bill_charges", "DELETE FROM bill_charges"),
    ("expenses", "DELETE FROM expenses"),
    ("stock_outflow_allocations", "DELETE FROM stock_outflow_allocations"),
    ("sale_stock_allocations", "DELETE FROM sale_stock_allocations"),
    ("payment_settlements", "DELETE FROM payment_settlements"),
    ("return_items", "DELETE FROM return_items"),
    ("return_transactions", "DELETE FROM return_transactions"),
    ("sale_items", "DELETE FROM sale_items"),
    ("sale_transactions", "DELETE FROM sale_transactions"),
    ("stock_adjustments", "DELETE FROM stock_adjustments"),
    (
        "purchase_items",
        "DELETE FROM purchase_items WHERE purchase_transaction_id IN \
         (SELECT id FROM purchase_transactions WHERE source <> 'OPENING')",
    ),
    ("purchase_transactions", "DELETE FROM purchase_transactions WHERE source <> 'OPENING'"),
    // Codes restart (DD-34). OPN- stays: OPENING lots are kept.
    (
        "code_sequences",
        "DELETE FROM code_sequences WHERE prefix IN ('SAL-', 'PUR-', 'ADJ-', 'RTL-', 'RET-', 'EXP-', 'OB-')",
    ),
];

const TRANSACTION_TABLES: &[&str] = &[
    "stock_outflow_allocations", "sale_stock_allocations", "payment_settlements", "return_items",
    "return_transactions", "sale_items", "sale_transactions", "stock_adjustments",
    "purchase_items", "purchase_transactions", "expenses", "bill_charges",
];

const CATALOG_STEPS: &[(&str, &str)] = &[
    ("purchase_items", "DELETE FROM purchase_items"),
    ("purchase_transactions", "DELETE FROM purchase_transactions"),
    ("service_components", "DELETE FROM service_components"),
    ("item_bundles", "DELETE FROM item_bundles"),
    ("items", "DELETE FROM items"),
    ("parties", "DELETE FROM parties"),
    ("expense_types", "DELETE FROM expense_types"),
    ("charge_types", "DELETE FROM charge_types"),
    ("code_sequences", "DELETE FROM code_sequences"),
];

const CATALOG_TABLES: &[&str] = &["purchase_items", "purchase_transactions", "service_components", "item_bundles", "items", "parties", "expense_types", "charge_types"];

impl MaintenanceRepository for SqliteMaintenanceRepository {
    fn clear_transactions(&mut self) -> Result<Vec<(String, i64)>, AppError> {
        let counts = self.run(TRANSACTION_STEPS)?;
        // Opening stock back to what was entered: nothing has been sold or written off.
        self.conn.execute("UPDATE purchase_items SET remaining_quantity = quantity", [])?;
        self.reset_sequences(TRANSACTION_TABLES)?;
        Ok(counts)
    }

    fn clear_catalog(&mut self) -> Result<Vec<(String, i64)>, AppError> {
        let counts = self.run(CATALOG_STEPS)?;
        self.reset_sequences(CATALOG_TABLES)?;
        Ok(counts)
    }
}
