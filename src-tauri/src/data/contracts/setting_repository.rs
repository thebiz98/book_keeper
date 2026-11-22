use crate::core::entities::ShopSetting;
use crate::core::error::AppError;

pub trait SettingRepository {
    fn get_shop_setting(&self) -> Result<ShopSetting, AppError>;
    fn upsert_shop_setting(&mut self, setting: &ShopSetting) -> Result<(), AppError>;
    /// User-chosen backup folder; `None` means "use the default".
    fn get_backup_directory(&self) -> Result<Option<String>, AppError>;
    fn set_backup_directory(&mut self, dir: Option<&str>) -> Result<(), AppError>;
}
