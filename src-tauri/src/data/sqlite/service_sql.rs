//! SQL fragments shared by the service-item features (FR-36, FR-37) and the return dating rule
//! (FR-56, DD-91). One definition each, so every caller (bill profit, line profit, P&L, item
//! profit, item lists) agrees (v1.4 reconciliation X3).

/// "Can make N" subquery for the `items` row aliased `alias`: the smallest whole number of service
/// units the components' current stock covers; NULL when the item has no components. The `1e-9`
/// absorbs float error in the division (stock is never negative).
pub fn can_make_sql(alias: &str) -> String {
    format!(
        "(SELECT MIN(CAST(c.current_stock / sc.quantity_per_unit + 1e-9 AS INTEGER)) \
         FROM service_components sc JOIN items c ON c.id = sc.component_item_id \
         WHERE sc.service_item_id = {alias}.id)"
    )
}

/// Gross FIFO cost of the allocations of the sale line aliased `si` (DD-91): the cost booked on
/// the bill date. Returns credit theirs on the return date through `returned_cost_sql`.
pub fn line_gross_cost_sql(si: &str) -> String {
    format!(
        "COALESCE((SELECT SUM(a.quantity_depleted * a.unit_cost) \
         FROM sale_stock_allocations a WHERE a.sale_item_id = {si}.id), 0.0)"
    )
}

/// Miscellaneous cost of the sale line aliased `si` (DD-66, X3): the part of the stored target
/// `service_cost_total` that the components' real FIFO cost does not cover, never below 0. Physical
/// lines have a NULL target, so their misc. cost is 0. Derived, never stored. Services are never
/// returned (DD-67), so their gross and net component cost are the same.
pub fn misc_cost_sql(si: &str) -> String {
    format!("MAX(0.0, COALESCE({si}.service_cost_total, 0.0) - {})", line_gross_cost_sql(si))
}

/// DD-91 cost credited on the return date for the customer `RETURNED` line aliased `ri`: its
/// stored `cost_amount`. An unlinked restockable line stored with 0 falls back to `quantity x` its
/// `RETURN` lot cost (R4, history). Restrict callers to `returned_cost_filter(ri)`.
pub fn returned_cost_sql(ri: &str) -> String {
    format!(
        "(CASE WHEN {ri}.sale_item_id IS NULL AND ABS({ri}.cost_amount) < 1e-9 \
           THEN COALESCE({ri}.quantity * (SELECT lp.unit_cost FROM purchase_items lp WHERE lp.id = {ri}.purchase_item_id), 0.0) \
           ELSE {ri}.cost_amount END)"
    )
}

/// The customer `RETURNED` lines whose cost is credited (DD-91): linked lines (restockable or
/// scrap; a scrap line also books its cost as an Inventory Loss) and unlinked restockable lines.
pub fn returned_cost_filter(ri: &str) -> String {
    format!("({ri}.sale_item_id IS NOT NULL OR {ri}.condition = 'RESTOCKABLE')")
}
