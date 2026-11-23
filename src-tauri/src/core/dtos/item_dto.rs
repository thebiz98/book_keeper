use serde::{Deserialize, Serialize};
use crate::core::value_objects::{ItemType, LotSource};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateItemPayload {
    pub name: String,
    pub category: Option<String>,
    pub barcode: Option<String>,
    pub base_unit: String,
    pub mrp: Option<f64>,
    pub default_sale_price: f64,
    pub initial_stock: Option<f64>,
    pub initial_cost: Option<f64>,
    pub bundles: Option<Vec<CreateBundlePayload>>,
    /// `None` => `PHYSICAL`. `SERVICE` is rejected in v1.1 (FR-21).
    #[serde(default)]
    pub item_type: Option<ItemType>,
    /// `None` => 5 (DEFAULT_LOW_STOCK_THRESHOLD). 0 = never low.
    #[serde(default)]
    pub low_stock_threshold: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateItemPayload {
    pub name: String,
    pub category: Option<String>,
    pub barcode: Option<String>,
    pub base_unit: String,
    pub mrp: Option<f64>,
    pub default_sale_price: f64,
    /// `None` => keep the current type. `SERVICE` is rejected in v1.1 (FR-21).
    #[serde(default)]
    pub item_type: Option<ItemType>,
    /// `None` => keep the current level.
    #[serde(default)]
    pub low_stock_threshold: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateBundlePayload {
    pub item_id: Option<i64>,
    pub bundle_name: String,
    pub units_per_bundle: f64,
    pub mrp: Option<f64>,
    pub default_bundle_price: Option<f64>,
    pub barcode: Option<String>,
}

/// Full replacement of a bundle's editable fields (DD-28: any field, no reference guard).
/// A bundle cannot be moved to another item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateBundlePayload {
    pub bundle_name: String,
    pub units_per_bundle: f64,
    pub mrp: Option<f64>,
    pub default_bundle_price: Option<f64>,
    pub barcode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleSuggestion {
    pub bundle_name: String,
    pub units_per_bundle: f64,
    pub usage_count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemSearchResult {
    #[serde(alias = "id")]
    pub item_id: i64,
    pub name: String,
    pub category: Option<String>,
    pub barcode: Option<String>,
    pub base_unit: String,
    #[serde(default)]
    pub item_type: ItemType,
    pub mrp: Option<f64>,
    pub default_sale_price: f64,
    pub current_stock: f64,
    pub is_low_stock: bool,
    #[serde(default = "crate::core::stock_rules::default_low_stock_threshold")]
    pub low_stock_threshold: f64,
    pub bundles: Vec<ItemBundleSearchResult>,
    /// Set when the hit came from a bundle barcode row (scanner on a pack barcode).
    #[serde(default)]
    pub matched_bundle_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemBundleSearchResult {
    #[serde(alias = "id")]
    pub bundle_id: i64,
    pub bundle_name: String,
    pub units_per_bundle: f64,
    #[serde(alias = "default_bundle_price")]
    pub bundle_price: Option<f64>,
    pub barcode: Option<String>,
}

/// Lot inspector row (any source): the lot joined with its header (code, source, date, vendor).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemLot {
    pub purchase_item_id: i64,
    pub purchase_transaction_id: i64,
    pub transaction_code: String,
    pub source: LotSource,
    pub transaction_date: String,
    pub vendor_name: Option<String>,
    pub quantity: f64,
    pub remaining_quantity: f64,
    pub unit_cost: f64,
}

/// Inventory catalog query. `search` goes through the FTS indexer; `category` and
/// `low_stock_only` still apply while searching. Defaults: page 1, limit 50 (max 200),
/// `low_stock_only = false`. Blank `category` / `search` mean no filter.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CatalogFilter {
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub low_stock_only: Option<bool>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub limit: Option<usize>,
}
