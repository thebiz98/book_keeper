//! SQLite implementation of `SettingRepository` (single row, id = 1).
use rusqlite::{params, OptionalExtension};

use crate::core::entities::ShopSetting;
use crate::core::error::AppError;
use crate::data::contracts::SettingRepository;
use crate::data::sqlite::SqliteConn;

pub struct SqliteSettingRepository {
    conn: SqliteConn,
}

impl SqliteSettingRepository {
    pub fn new(conn: SqliteConn) -> Self {
        Self { conn }
    }
}

impl SettingRepository for SqliteSettingRepository {
    fn get(&self) -> Result<Option<ShopSetting>, AppError> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, shop_name, phone, address, tax_id FROM shop_settings ORDER BY id LIMIT 1",
                [],
                |r| {
                    Ok(ShopSetting {
                        id: r.get(0)?,
                        shop_name: r.get(1)?,
                        phone: r.get(2)?,
                        address: r.get(3)?,
                        tax_id: r.get(4)?,
                    })
                },
            )
            .optional()?)
    }

    fn update(&mut self, s: &ShopSetting) -> Result<(), AppError> {
        let id = if s.id > 0 { s.id } else { 1 };
        self.conn.execute(
            "INSERT INTO shop_settings (id, shop_name, phone, address, tax_id) VALUES (?1,?2,?3,?4,?5) \
             ON CONFLICT(id) DO UPDATE SET shop_name=excluded.shop_name, phone=excluded.phone, \
             address=excluded.address, tax_id=excluded.tax_id",
            params![id, s.shop_name, s.phone, s.address, s.tax_id],
        )?;
        Ok(())
    }
}
