use crate::core::error::AppError;

/// `company_manifest.json` fields we care about.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VedbillManifest {
    pub company_name: Option<String>,
    pub schema_version: Option<i64>,
}

/// One row of VedBill's `items` table (raw values, not yet cleaned).
#[derive(Debug, Clone, PartialEq)]
pub struct VedbillItem {
    pub legacy_id: String,
    pub name: String,
    /// `PRODUCT` / `SERVICE`
    pub item_type: Option<String>,
    /// Short name of the primary unit (e.g. `PCS`).
    pub unit: Option<String>,
    pub sale_price: f64,
    pub purchase_price: Option<f64>,
    pub mrp: Option<f64>,
    /// The stock VedBill shows on screen.
    pub primary_quantity: Option<f64>,
    pub is_active: bool,
    pub opening_stock_date: Option<String>,
}

/// One row of VedBill's `stock_lots` table.
#[derive(Debug, Clone, PartialEq)]
pub struct VedbillLot {
    pub item_id: String,
    /// `YYYY-MM-DD` or a full ISO timestamp, as stored.
    pub lot_date: String,
    pub purchase_price: f64,
    pub remaining_qty: f64,
}

/// What the inventory migration needs from a VedBill backup.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VedbillSnapshot {
    pub manifest: VedbillManifest,
    /// In backup order.
    pub items: Vec<VedbillItem>,
    /// Ordered by item, then lot date, then creation time (FIFO order).
    pub lots: Vec<VedbillLot>,
}

/// Reads a foreign VedBill backup (`.vb`). Read-only: never writes to the backup.
pub trait VedbillBackupReader: Send + Sync {
    fn read(&self, path: &str) -> Result<VedbillSnapshot, AppError>;
}

impl<T: VedbillBackupReader + ?Sized> VedbillBackupReader for std::sync::Arc<T> {
    fn read(&self, path: &str) -> Result<VedbillSnapshot, AppError> {
        (**self).read(path)
    }
}
