use std::sync::Arc;
use crate::core::entities::ShopSetting;
use crate::core::error::AppError;
use crate::core::money::round2;
use crate::data::contracts::unit_of_work::UnitOfWork;
use crate::services::contracts::shop_setting_service::ShopSettingService;

#[derive(Clone)]
pub struct ShopSettingServiceImpl<U: UnitOfWork = Arc<dyn UnitOfWork>> {
    pub uow: U,
}

impl<U: UnitOfWork> ShopSettingServiceImpl<U> {
    pub fn new(uow: U) -> Self {
        Self { uow }
    }
}

impl<U: UnitOfWork> ShopSettingService for ShopSettingServiceImpl<U> {
    fn get_settings(&self) -> Result<ShopSetting, AppError> {
        let mut tx = self.uow.begin_read()?;
        tx.settings().get_shop_setting()
    }

    fn update_settings(&self, settings: ShopSetting) -> Result<(), AppError> {
        let mut s = settings;
        if !s.opening_cash.is_finite() || s.opening_cash < 0.0 {
            return Err(AppError::ValidationError {
                field: "opening_cash".to_string(),
                message: "Opening cash must be a number, zero or more".to_string(),
            });
        }
        s.opening_cash = round2(s.opening_cash);
        let mut tx = self.uow.begin_transaction()?;
        s.id = 1; // Enforce single-row singleton profile id = 1
        tx.settings().upsert_shop_setting(&s)?;
        tx.commit()
    }
}
