use serde::{Deserialize, Serialize};
use crate::core::value_objects::ItemType;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub item_type: ItemType,
    pub category: Option<String>,
    pub barcode: Option<String>,
    pub base_unit: String,
    pub mrp: Option<f64>,
    pub default_sale_price: f64,
    pub current_stock: f64,
    pub is_low_stock: bool,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default = "crate::core::stock_rules::default_low_stock_threshold")]
    pub low_stock_threshold: f64,
    /// SERVICE only (v12, FR-36): intended cost per service unit, a floor over the components' FIFO cost.
    #[serde(default)]
    pub service_cost: Option<f64>,
}

/// One row of a service's bill of materials (v12, FR-36): base units of `component_item_id`
/// consumed by one unit of `service_item_id`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceComponent {
    pub id: i64,
    pub service_item_id: i64,
    pub component_item_id: i64,
    pub quantity_per_unit: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemBundle {
    pub id: i64,
    pub item_id: i64,
    pub bundle_name: String,
    pub units_per_bundle: f64,
    pub mrp: Option<f64>,
    pub default_bundle_price: Option<f64>,
    pub barcode: Option<String>,
}
