use crate::core::entities::ShopSetting;
use crate::core::error::AppError;

pub trait SettingRepository {
    fn get_shop_setting(&self) -> Result<ShopSetting, AppError>;
    fn upsert_shop_setting(&mut self, setting: &ShopSetting) -> Result<(), AppError>;
}
