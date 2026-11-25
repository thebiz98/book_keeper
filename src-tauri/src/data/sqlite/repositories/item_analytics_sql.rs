//! Item profit (FR-38) and inventory speed (FR-39, DD-68): one aggregate statement each over the
//! existing tables, with sorting, searching and paging in SQL. Read-only (X6). Misc. cost and net
//! allocation cost come from the shared fragments in `service_sql` (X3), never re-derived here.
use rusqlite::params;

use crate::core::dtos::{
    InventorySpeedReport, InventorySpeedRow, ItemProfitReconciliation, ItemProfitReport, ItemProfitRow,
    ItemProfitTotals, SpeedClass, SpeedCounts,
};
use crate::core::error::AppError;
use crate::core::money::{round2, MONEY_EPS};
use crate::data::contracts::report_repository::ReportListQuery;
use crate::data::sqlite::repositories::sale_repo::{profit_expr, PROFIT_REPLACEMENT};
use crate::data::sqlite::service_sql::{line_net_cost_sql, misc_cost_sql};
use crate::data::sqlite::SqliteConn;

fn dir(desc: bool) -> &'static str {
    if desc { "DESC" } else { "ASC" }
}

fn margin(profit: f64, revenue: f64) -> Option<f64> {
    (revenue.abs() > MONEY_EPS).then(|| round2(profit / revenue * 100.0))
}

/// Per sale line of non-opening bills dated `?1..?2`: quantity net of linked returns, revenue (line
/// total x the FR-19 discount share, less linked refund value) and cost (net allocations + scrapped
/// returned units + misc. cost).
fn line_cte() -> String {
    let linked = |extra: &str, col: &str| {
        format!(
            "COALESCE((SELECT SUM(ri.{col}) FROM return_items ri \
             JOIN return_transactions r ON r.id = ri.return_transaction_id \
             WHERE ri.sale_item_id = si.id AND ri.line_type = 'RETURNED' AND r.return_type = 'CUSTOMER'{extra}), 0.0)"
        )
    };
    format!(
        "line AS (SELECT si.item_id AS item_id, si.item_name AS snap, \
           si.quantity - {ret_qty} AS qty, \
           si.total_amount * (CASE WHEN s.items_subtotal <= {eps} THEN 1.0 \
              ELSE MAX(0.0, MIN(1.0, 1.0 - s.discounts_total / s.items_subtotal)) END) \
             - {refund} AS rev, \
           {net} + {scrap} + {misc} AS cost \
         FROM sale_transactions s JOIN sale_items si ON si.sale_transaction_id = s.id \
         WHERE s.is_opening_balance = 0 AND s.transaction_date BETWEEN ?1 AND ?2)",
        ret_qty = linked("", "quantity"),
        refund = linked(" AND r.settlement_type = 'REFUND'", "total_amount"),
        scrap = linked(" AND ri.condition = 'DAMAGED_SCRAP'", "cost_amount"),
        net = line_net_cost_sql("si"),
        misc = misc_cost_sql("si"),
        eps = MONEY_EPS,
    )
}

fn profit_order(key: &str) -> &'static str {
    match key {
        "name" => "name COLLATE NOCASE",
        "qty" => "qty",
        "revenue" => "revenue",
        "cost" => "cost",
        "margin" => "CASE WHEN revenue > 0.005 THEN profit / revenue END",
        _ => "profit",
    }
}

/// Statement prefix: `?1`/`?2` = range, `?3` = search; ends with the filtered CTE `flt`.
pub fn profit_ctes() -> String {
    format!(
        "WITH {line}, agg AS (SELECT l.item_id AS item_id, COALESCE(i.name, MAX(l.snap)) AS name, \
           COALESCE(i.base_unit, '') AS base_unit, SUM(l.qty) AS qty, SUM(l.rev) AS revenue, \
           SUM(l.cost) AS cost, SUM(l.rev) - SUM(l.cost) AS profit \
         FROM line l LEFT JOIN items i ON i.id = l.item_id GROUP BY l.item_id), \
         flt AS (SELECT * FROM agg WHERE (?3 IS NULL OR instr(lower(name), lower(?3)) > 0))",
        line = line_cte()
    )
}

/// The page statement (`?4` = limit, `?5` = offset), exposed so tests can EXPLAIN it.
pub fn profit_page_sql(order: &str, desc: bool) -> String {
    format!(
        "{} SELECT item_id, name, base_unit, qty, revenue, cost, profit FROM flt \n         ORDER BY ({ord}) IS NULL, ({ord}) {d}, name COLLATE NOCASE, item_id LIMIT ?4 OFFSET ?5",
        profit_ctes(),
        ord = profit_order(order),
        d = dir(desc)
    )
}

pub(super) fn item_profit(conn: &SqliteConn, q: &ReportListQuery) -> Result<ItemProfitReport, AppError> {
    let ctes = profit_ctes();
    let (count, revenue, cost): (i64, f64, f64) = conn.query_row(
        &format!("{ctes} SELECT COUNT(*), COALESCE(SUM(revenue), 0.0), COALESCE(SUM(cost), 0.0) FROM flt"),
        params![q.from, q.to, q.search],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let page_sql = format!(
        "{ctes} SELECT item_id, name, base_unit, qty, revenue, cost, profit FROM flt \
         ORDER BY ({ord}) IS NULL, ({ord}) {d}, name COLLATE NOCASE, item_id LIMIT ?4 OFFSET ?5",
        ord = profit_order(q.order),
        d = dir(q.desc)
    );
    let mut stmt = conn.prepare(&page_sql)?;
    let rows = stmt
        .query_map(params![q.from, q.to, q.search, q.limit as i64, q.offset as i64], |r| {
            let (revenue, cost, profit): (f64, f64, f64) = (r.get(4)?, r.get(5)?, r.get(6)?);
            let (revenue, cost, profit) = (round2(revenue), round2(cost), round2(profit));
            Ok(ItemProfitRow {
                item_id: r.get(0)?,
                name: r.get(1)?,
                base_unit: r.get(2)?,
                qty_sold: r.get(3)?,
                revenue,
                cost,
                profit,
                margin_pct: margin(profit, revenue),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // Reconciliation (FR-38): every non-opening bill in range, ignoring the search.
    let items_profit: f64 = conn.query_row(
        &format!("WITH {} SELECT COALESCE(SUM(rev - cost), 0.0) FROM line", line_cte()),
        params![q.from, q.to],
        |r| r.get(0),
    )?;
    let (charges, replacement, bills): (f64, f64, f64) = conn.query_row(
        &format!(
            "SELECT COALESCE(SUM(t.charges_total), 0.0), COALESCE(SUM({PROFIT_REPLACEMENT}), 0.0), \
             COALESCE(SUM({}), 0.0) FROM sale_transactions t \
             WHERE t.is_opening_balance = 0 AND t.transaction_date BETWEEN ?1 AND ?2",
            profit_expr()
        ),
        params![q.from, q.to],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let (revenue, cost) = (round2(revenue), round2(cost));
    let profit = round2(revenue - cost);
    Ok(ItemProfitReport {
        rows,
        totals: ItemProfitTotals { revenue, cost, profit, margin_pct: margin(profit, revenue) },
        reconciliation: ItemProfitReconciliation {
            items_profit: round2(items_profit),
            charges: round2(charges),
            replacement_cost: round2(replacement),
            bills_profit: round2(bills),
        },
        total_count: count as usize,
        page: 0,
        page_size: q.limit,
    })
}

fn speed_order(key: &str) -> &'static str {
    match key {
        "name" => "name COLLATE NOCASE",
        "stock" => "stock",
        "cover" => "CASE WHEN u > 1e-9 THEN stock / (u / ?4) END",
        "class" => "CASE cls WHEN 'FAST' THEN 0 WHEN 'NORMAL' THEN 1 WHEN 'SLOW' THEN 2 ELSE 3 END",
        _ => "u", // units_out and per_day order the same way
    }
}

fn class_of(s: &str) -> SpeedClass {
    match s {
        "FAST" => SpeedClass::Fast,
        "SLOW" => SpeedClass::Slow,
        "DEAD" => SpeedClass::Dead,
        _ => SpeedClass::Normal,
    }
}

/// `?1`/`?2` = range, `?3` = search, `?4` = days, `?5` = class filter; ends with the CTE `flt`.
///
/// `units`: depleted minus returned units per physical item, through the allocation's lot, so
/// direct sales and service consumption share one path (X4). Driven by the bill date index.
/// `classed`: rank over items with units out > 0 only, before any search / class filter;
/// k = ceil(0.2 n) >= 1, top wins on overlap; units out 0 with stock on hand = DEAD.
const SPEED_CTES: &str = "WITH units AS (SELECT pi.item_id AS item_id, SUM(a.quantity_depleted - a.returned_quantity) AS u \
           FROM sale_transactions s \
           JOIN sale_items si ON si.sale_transaction_id = s.id \
           JOIN sale_stock_allocations a ON a.sale_item_id = si.id \
           JOIN purchase_items pi ON pi.id = a.purchase_item_id \
           WHERE s.is_opening_balance = 0 AND s.transaction_date BETWEEN ?1 AND ?2 GROUP BY pi.item_id), \
         cand AS (SELECT i.id AS item_id, i.name AS name, i.base_unit AS base_unit, i.current_stock AS stock, \
             COALESCE(un.u, 0.0) AS u \
           FROM items i LEFT JOIN units un ON un.item_id = i.id \
           WHERE i.item_type <> 'SERVICE' \
             AND (COALESCE(un.u, 0.0) > 1e-9 OR (i.current_stock > 1e-9 AND i.is_active = 1))), \
         ranked AS (SELECT c.*, \
             ROW_NUMBER() OVER (PARTITION BY (c.u > 1e-9) ORDER BY c.u DESC, c.name COLLATE NOCASE, c.item_id) AS rn, \
             SUM(CASE WHEN c.u > 1e-9 THEN 1 ELSE 0 END) OVER () AS n FROM cand c), \
         classed AS (SELECT item_id, name, base_unit, stock, u, \
             CASE WHEN u <= 1e-9 THEN 'DEAD' \
                  WHEN rn <= MAX(1, (2 * n + 9) / 10) THEN 'FAST' \
                  WHEN rn > n - MAX(1, (2 * n + 9) / 10) THEN 'SLOW' \
                  ELSE 'NORMAL' END AS cls FROM ranked), \
         srch AS (SELECT * FROM classed WHERE (?3 IS NULL OR instr(lower(name), lower(?3)) > 0)), \
         flt AS (SELECT * FROM srch WHERE (?5 IS NULL OR cls = ?5))";

/// The page statement (`?6` = limit, `?7` = offset), exposed so tests can EXPLAIN it.
pub fn speed_page_sql(order: &str, desc: bool) -> String {
    let ord = speed_order(order);
    format!(
        "{SPEED_CTES} SELECT item_id, name, base_unit, u, stock, cls FROM flt \
         ORDER BY ({ord}) IS NULL, ({ord}) {d}, name COLLATE NOCASE, item_id LIMIT ?6 OFFSET ?7",
        d = dir(desc)
    )
}

pub(super) fn inventory_speed(
    conn: &SqliteConn,
    q: &ReportListQuery,
    class: Option<&str>,
    days: u32,
) -> Result<InventorySpeedReport, AppError> {
    let days_f = f64::from(days.max(1));
    let ctes = SPEED_CTES;

    let mut counts = SpeedCounts::default();
    {
        let mut stmt = conn.prepare(&format!("{ctes} SELECT cls, COUNT(*) FROM srch GROUP BY cls"))?;
        let mut rows = stmt.query(params![q.from, q.to, q.search, days_f, class])?;
        while let Some(r) = rows.next()? {
            let (c, n): (String, i64) = (r.get(0)?, r.get(1)?);
            let n = n as usize;
            match class_of(&c) {
                SpeedClass::Fast => counts.fast = n,
                SpeedClass::Normal => counts.normal = n,
                SpeedClass::Slow => counts.slow = n,
                SpeedClass::Dead => counts.dead = n,
            }
        }
    }
    let total: i64 = conn.query_row(
        &format!("{ctes} SELECT COUNT(*) FROM flt"),
        params![q.from, q.to, q.search, days_f, class],
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare(&speed_page_sql(q.order, q.desc))?;
    let rows = stmt
        .query_map(params![q.from, q.to, q.search, days_f, class, q.limit as i64, q.offset as i64], |r| {
            let (u, stock): (f64, f64) = (r.get(3)?, r.get(4)?);
            let per_day = u / days_f;
            let cls: String = r.get(5)?;
            Ok(InventorySpeedRow {
                item_id: r.get(0)?,
                name: r.get(1)?,
                base_unit: r.get(2)?,
                units_out: u,
                units_per_day: per_day,
                stock,
                days_of_cover: (u > 1e-9).then(|| stock / per_day),
                class: class_of(&cls),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(InventorySpeedReport { rows, counts, days, total_count: total as usize, page: 0, page_size: q.limit })
}
