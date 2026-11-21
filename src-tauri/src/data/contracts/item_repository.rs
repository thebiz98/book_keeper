use crate::core::entities::Item;
use crate::core::error::AppError;

pub trait ItemRepository {
    fn find_by_id(&self, id: i64) -> Result<Option<Item>, AppError>;
    fn find_by_name(&self, name: &str) -> Result<Option<Item>, AppError>;
    fn find_by_barcode(&self, barcode: &str) -> Result<Option<Item>, AppError>;
    fn list_catalog(
        &self,
        category: Option<&str>,
        low_stock_only: bool,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<Item>, AppError>;
    fn create(&mut self, item: &Item) -> Result<i64, AppError>;
    fn update(&mut self, item: &Item) -> Result<(), AppError>;
    fn update_stock_cache(
        &mut self,
        item_id: i64,
        new_stock: f64,
        is_low_stock: bool,
    ) -> Result<(), AppError>;
}
