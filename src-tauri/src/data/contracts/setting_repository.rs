use crate::core::entities::ShopSetting;
use crate::core::error::AppError;

pub trait SettingRepository {
    fn get_shop_setting(&self) -> Result<ShopSetting, AppError> {
        self.get()?.ok_or_else(|| AppError::DatabaseError("Setting not found".to_string()))
    }
    fn get(&self) -> Result<Option<ShopSetting>, AppError> {
        self.get_shop_setting().map(Some)
    }
    fn upsert_shop_setting(&mut self, setting: &ShopSetting) -> Result<(), AppError> {
        self.update(setting)
    }
    fn update(&mut self, setting: &ShopSetting) -> Result<(), AppError> {
        self.upsert_shop_setting(setting)
    }
}
