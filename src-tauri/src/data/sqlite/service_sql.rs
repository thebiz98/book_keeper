//! SQL fragments shared by the service-item features (FR-36, FR-37). One definition each, so every
//! caller (bill profit, line profit, P&L, item profit, item lists) agrees (v1.4 reconciliation X3).

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

/// Net FIFO cost of the allocations of the sale line aliased `si` (depleted minus returned).
pub fn line_net_cost_sql(si: &str) -> String {
    format!(
        "COALESCE((SELECT SUM((a.quantity_depleted - a.returned_quantity) * a.unit_cost) \
         FROM sale_stock_allocations a WHERE a.sale_item_id = {si}.id), 0.0)"
    )
}

/// Miscellaneous cost of the sale line aliased `si` (DD-66, X3): the part of the stored target
/// `service_cost_total` that the components' real FIFO cost does not cover, never below 0. Physical
/// lines have a NULL target, so their misc. cost is 0. Derived, never stored.
pub fn misc_cost_sql(si: &str) -> String {
    format!("MAX(0.0, COALESCE({si}.service_cost_total, 0.0) - {})", line_net_cost_sql(si))
}
