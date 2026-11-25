//! SQLite implementation of `ServiceComponentRepository`.
use std::collections::HashMap;

use rusqlite::params;

use crate::core::entities::ServiceComponent;
use crate::core::error::AppError;
use crate::data::contracts::ServiceComponentRepository;
use crate::data::sqlite::service_sql::can_make_sql;
use crate::data::sqlite::SqliteConn;

pub struct SqliteServiceComponentRepository {
    conn: SqliteConn,
}

impl SqliteServiceComponentRepository {
    pub fn new(conn: SqliteConn) -> Self {
        Self { conn }
    }
}

impl ServiceComponentRepository for SqliteServiceComponentRepository {
    fn list_for_service(&self, service_id: i64) -> Result<Vec<ServiceComponent>, AppError> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, service_item_id, component_item_id, quantity_per_unit \
             FROM service_components WHERE service_item_id = ?1 ORDER BY id",
        )?;
        let rows = stmt
            .query_map(params![service_id], |r| {
                Ok(ServiceComponent {
                    id: r.get(0)?,
                    service_item_id: r.get(1)?,
                    component_item_id: r.get(2)?,
                    quantity_per_unit: r.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn replace_for_service(&mut self, service_id: i64, components: &[(i64, f64)]) -> Result<(), AppError> {
        self.conn.execute("DELETE FROM service_components WHERE service_item_id = ?1", params![service_id])?;
        for (component_id, qty) in components {
            self.conn.execute(
                "INSERT INTO service_components (service_item_id, component_item_id, quantity_per_unit) VALUES (?1, ?2, ?3)",
                params![service_id, component_id, qty],
            )?;
        }
        Ok(())
    }

    fn services_using(&self, component_id: i64) -> Result<Vec<i64>, AppError> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT service_item_id FROM service_components WHERE component_item_id = ?1 ORDER BY service_item_id",
        )?;
        let rows = stmt.query_map(params![component_id], |r| r.get(0))?.collect::<Result<Vec<i64>, _>>()?;
        Ok(rows)
    }

    fn can_make(&self, service_ids: &[i64]) -> Result<HashMap<i64, i64>, AppError> {
        if service_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let ph = vec!["?"; service_ids.len()].join(",");
        let sql = format!("SELECT i.id, {} FROM items i WHERE i.id IN ({ph})", can_make_sql("i"));
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(service_ids.iter()), |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows.into_iter().filter_map(|(id, n)| n.map(|n| (id, n))).collect())
    }
}
