use crate::core::entities::ShopSetting;
use crate::core::error::AppError;

pub trait ShopSettingService: Send + Sync {
    fn get_settings(&self) -> Result<ShopSetting, AppError>;
    fn update_settings(&self, settings: ShopSetting) -> Result<(), AppError>;
}

impl<T: ShopSettingService + ?Sized> ShopSettingService for std::sync::Arc<T> {
    fn get_settings(&self) -> Result<ShopSetting, AppError> {
        (**self).get_settings()
    }

    fn update_settings(&self, settings: ShopSetting) -> Result<(), AppError> {
        (**self).update_settings(settings)
    }
}
