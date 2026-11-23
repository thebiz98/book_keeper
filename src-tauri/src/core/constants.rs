//! Domain constants defined once for the whole backend.

/// Quantity tolerance for float comparisons on stock (1e-9 base units), used by every
/// lot/allocation algorithm and the stock-cache invariant check.
pub const EPS: f64 = 1e-9;
