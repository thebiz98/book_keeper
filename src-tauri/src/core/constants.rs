//! Domain constants defined once for the whole backend.

/// An item is "low stock" when its cached `current_stock` is strictly below this many base units.
/// The single definition used by the stock-cache SQL helper and every service (F3/T4 routes all
/// stock writes through it).
pub const LOW_STOCK_THRESHOLD: f64 = 5.0;

/// Quantity tolerance for float comparisons on stock (1e-9 base units), used by every
/// lot/allocation algorithm and the stock-cache invariant check.
pub const EPS: f64 = 1e-9;
