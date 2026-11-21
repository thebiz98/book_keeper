//! Domain constants defined once for the whole backend.

/// An item is "low stock" when its cached `current_stock` is strictly below this many base units.
/// The single definition used by the stock-cache SQL helper and every service (F3/T4 routes all
/// stock writes through it).
pub const LOW_STOCK_THRESHOLD: f64 = 5.0;
