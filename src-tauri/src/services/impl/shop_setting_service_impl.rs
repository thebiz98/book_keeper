use std::sync::Arc;
use crate::core::entities::ShopSetting;
use crate::core::error::AppError;
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
        let mut tx = self.uow.begin_transaction()?;
        tx.settings().get_shop_setting()
    }

    fn update_settings(&self, settings: ShopSetting) -> Result<(), AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let mut s = settings;
        s.id = 1; // Enforce single-row singleton profile id = 1
        tx.settings().upsert_shop_setting(&s)?;
        tx.commit()
    }
}
