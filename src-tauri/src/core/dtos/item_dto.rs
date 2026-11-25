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
    /// `None` => `PHYSICAL`.
    #[serde(default)]
    pub item_type: Option<ItemType>,
    /// `None` => 5 (DEFAULT_LOW_STOCK_THRESHOLD). 0 = never low.
    #[serde(default)]
    pub low_stock_threshold: Option<f64>,
    /// SERVICE only (FR-36): intended cost per service unit, >= 0 or `None`.
    #[serde(default)]
    pub service_cost: Option<f64>,
    /// SERVICE only (FR-36): the bill of materials.
    #[serde(default)]
    pub components: Option<Vec<ServiceComponentInput>>,
}

/// One component of a service (FR-36): base units of a physical item used by one service unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceComponentInput {
    pub component_item_id: i64,
    pub quantity_per_unit: f64,
}

/// Component row of a service as the edit drawer shows it (FR-36).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceComponentView {
    pub component_item_id: i64,
    pub name: String,
    pub base_unit: String,
    pub quantity_per_unit: f64,
    pub current_stock: f64,
}

/// Input of `preview_service_cost` (FR-36, DD-56): the unsaved component list and Service cost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewServiceCostPayload {
    #[serde(default)]
    pub components: Vec<ServiceComponentInput>,
    #[serde(default)]
    pub service_cost: Option<f64>,
}

/// Cost of one service unit at today's FIFO lots (FR-36).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceCostPreview {
    /// FIFO cost of the components one unit would draw now.
    pub components_cost: f64,
    /// `max(0, service_cost - components_cost)`.
    pub misc_cost: f64,
    /// `components_cost + misc_cost`.
    pub unit_cost: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateItemPayload {
    pub name: String,
    pub category: Option<String>,
    pub barcode: Option<String>,
    pub base_unit: String,
    pub mrp: Option<f64>,
    pub default_sale_price: f64,
    /// `None` => keep the current type. A different type is rejected (FR-36).
    #[serde(default)]
    pub item_type: Option<ItemType>,
    /// `None` => keep the current level.
    #[serde(default)]
    pub low_stock_threshold: Option<f64>,
    /// SERVICE only: full replacement (`None` clears the Service cost).
    #[serde(default)]
    pub service_cost: Option<f64>,
    /// SERVICE only: `Some(list)` replaces the components, `None` keeps them.
    #[serde(default)]
    pub components: Option<Vec<ServiceComponentInput>>,
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
    /// SERVICE only (FR-36): intended cost per unit.
    #[serde(default)]
    pub service_cost: Option<f64>,
    /// SERVICE only (FR-36): "Can make N" from the components' stock; `None` without components.
    #[serde(default)]
    pub can_make: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemBundleSearchResult {
    #[serde(alias = "id")]
    pub bundle_id: i64,
    pub bundle_name: String,
    pub units_per_bundle: f64,
    #[serde(alias = "default_bundle_price")]
    pub bundle_price: Option<f64>,
    /// Suggested sale rate per pack: `bundle_price`, else `round2(default_sale_price x units_per_bundle)`.
    #[serde(default)]
    pub default_rate: f64,
    pub barcode: Option<String>,
}

/// Suggested sale rate per pack: the bundle's own price, else `round2(item price x units)`.
pub fn bundle_default_rate(b: &crate::core::entities::ItemBundle, item_default_sale_price: f64) -> f64 {
    b.default_bundle_price
        .unwrap_or_else(|| crate::core::money::round2(item_default_sale_price * b.units_per_bundle))
}

/// `create_bundle` / `update_bundle` result: the saved bundle plus its suggested rate (DD-56).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedBundle {
    #[serde(flatten)]
    pub bundle: crate::core::entities::ItemBundle,
    pub default_rate: f64,
}

impl SavedBundle {
    pub fn new(bundle: crate::core::entities::ItemBundle, item_default_sale_price: f64) -> Self {
        let default_rate = bundle_default_rate(&bundle, item_default_sale_price);
        Self { bundle, default_rate }
    }
}

impl std::ops::Deref for SavedBundle {
    type Target = crate::core::entities::ItemBundle;
    fn deref(&self) -> &Self::Target {
        &self.bundle
    }
}

impl ItemBundleSearchResult {
    /// Search/catalog row of a bundle (DD-56: the suggested rate is computed here, not in the UI).
    pub fn from_bundle(b: crate::core::entities::ItemBundle, item_default_sale_price: f64) -> Self {
        let default_rate = bundle_default_rate(&b, item_default_sale_price);
        Self {
            bundle_id: b.id,
            bundle_name: b.bundle_name,
            units_per_bundle: b.units_per_bundle,
            bundle_price: b.default_bundle_price,
            default_rate,
            barcode: b.barcode,
        }
    }
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
    pub pack_label: Option<String>,
    pub pack_quantity: f64,
    pub quantity: f64,
    pub remaining_quantity: f64,
    pub unit_cost: f64,
}

/// One open lot in a sale-row lot draw (FR-31).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LotDrawLot {
    pub purchase_item_id: i64,
    pub transaction_code: String,
    pub source: LotSource,
    pub unit_cost: f64,
    pub remaining_quantity: f64,
    /// Part of the requested quantity taken from this lot, after the prior lines' quantity is consumed first.
    pub draw_quantity: f64,
}

/// Base-unit quantity of an earlier cart line (FR-31, FR-37): the backend expands service lines
/// into their components, the UI only maps its rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriorLine {
    pub item_id: i64,
    pub quantity: f64,
}

/// One component's draw inside a service line's lot draw (FR-37).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LotDrawComponent {
    pub item_id: i64,
    pub name: String,
    pub base_unit: String,
    /// Base units of the component this line consumes.
    pub quantity: f64,
    pub lots: Vec<LotDrawLot>,
    pub draw_cost: f64,
    pub available: f64,
    pub shortfall: f64,
}

/// Read-only FIFO draw for the sale entry row (FR-31, DD-60, FR-37). For a physical item `lots` is
/// the draw and `line_cost == draw_cost`; for a service `lots` is empty and `components` carries
/// the per-component draws.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LotDraw {
    pub lots: Vec<LotDrawLot>,
    pub draw_cost: f64,
    pub available: f64,
    pub shortfall: f64,
    #[serde(default)]
    pub components: Vec<LotDrawComponent>,
    /// Service: components' FIFO cost (same as `draw_cost`).
    #[serde(default)]
    pub components_cost: f64,
    /// Service: `max(0, round2(service_cost x quantity) - components_cost)`; physical: 0.
    #[serde(default)]
    pub misc_cost: f64,
    /// `components_cost + misc_cost` for a service, `draw_cost` for a physical item.
    #[serde(default)]
    pub line_cost: f64,
}

/// One bill that contains an item (Item details drawer, FR-23). Each bill appears once per item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemBillRef {
    pub bill_id: i64,
    pub transaction_code: String,
    pub transaction_date: String,
    pub party_name: Option<String>,
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
