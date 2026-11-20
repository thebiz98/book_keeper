use crate::core::entities::ShopSetting;
use crate::core::error::AppError;

pub trait ShopSettingService: Send + Sync {
    fn get_settings(&self) -> Result<ShopSetting, AppError>;
    fn update_settings(&self, settings: ShopSetting) -> Result<(), AppError>;
}
