use std::sync::Arc;
use crate::core::dtos::{
    BundleSuggestion, CreateBundlePayload, CreateItemPayload, PaginatedList, UpdateItemPayload,
};
use crate::core::entities::{Item, ItemBundle, PurchaseItem, PurchaseTransaction};
use crate::core::constants::LOW_STOCK_THRESHOLD;
use crate::core::error::AppError;
use crate::core::value_objects::{LotSource, PaymentStatus};
use crate::data::contracts::search_indexer::ItemSearchIndexer;
use crate::data::contracts::unit_of_work::UnitOfWork;
use crate::services::contracts::item_service::ItemService;

#[derive(Clone)]
pub struct ItemServiceImpl<U: UnitOfWork = Arc<dyn UnitOfWork>> {
    pub uow: U,
    pub search_indexer: Arc<dyn ItemSearchIndexer>,
}

impl<U: UnitOfWork> ItemServiceImpl<U> {
    pub fn new(uow: U, search_indexer: Arc<dyn ItemSearchIndexer>) -> Self {
        Self { uow, search_indexer }
    }
}

impl<U: UnitOfWork> ItemService for ItemServiceImpl<U> {
    fn get_catalog(
        &self,
        category: Option<String>,
        low_stock_only: bool,
        page: usize,
        limit: usize,
    ) -> Result<PaginatedList<Item>, AppError> {
        let mut tx = self.uow.begin_read()?;
        let offset = page.saturating_sub(1) * limit;
        let items = tx
            .items()
            .list_catalog(category.as_deref(), low_stock_only, offset, limit)?;
        let total = tx.items().count_catalog(category.as_deref(), low_stock_only)?;
        Ok(PaginatedList::new(items, total, page, limit))
    }

    fn create_item(&self, payload: CreateItemPayload) -> Result<i64, AppError> {
        if payload.name.trim().is_empty() {
            return Err(AppError::ValidationError {
                field: "name".to_string(),
                message: "Item name cannot be empty".to_string(),
            });
        }
        if payload.default_sale_price < 0.0 {
            return Err(AppError::ValidationError {
                field: "default_sale_price".to_string(),
                message: "default_sale_price cannot be negative".to_string(),
            });
        }

        let mut tx = self.uow.begin_transaction()?;
        if let Some(ref bc) = payload.barcode {
            let trimmed = bc.trim();
            if !trimmed.is_empty() && tx.items().find_by_barcode(trimmed)?.is_some() {
                return Err(AppError::ValidationError {
                    field: "barcode".to_string(),
                    message: format!("Barcode '{}' already exists", trimmed),
                });
            }
        }

        let initial_stock = payload.initial_stock.unwrap_or(0.0);
        let item = Item {
            id: 0,
            name: payload.name.trim().to_string(),
            category: payload.category.map(|c| c.trim().to_string()).filter(|c| !c.is_empty()),
            barcode: payload.barcode.map(|b| b.trim().to_string()).filter(|b| !b.is_empty()),
            base_unit: payload.base_unit.trim().to_string(),
            mrp: payload.mrp,
            default_sale_price: payload.default_sale_price,
            current_stock: initial_stock,
            is_low_stock: initial_stock < LOW_STOCK_THRESHOLD,
            is_active: true,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };

        let item_id = tx.items().create(&item)?;
        let mut created_bundles = Vec::new();

        if initial_stock > 0.0 {
            let unit_cost = payload.initial_cost.unwrap_or(payload.default_sale_price * 0.7);
            let lot_code = tx.purchases().get_next_transaction_code(LotSource::Opening)?;
            let synthetic_tx = PurchaseTransaction {
                id: 0,
                transaction_code: lot_code,
                source: LotSource::Opening,
                supplier_invoice_ref: None,
                transaction_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                party_id: None,
                party_name: None,
                payment_mode: None,
                payment_status: PaymentStatus::Paid,
                // DD-07: opening lots are not supplier bills.
                total_amount: 0.0,
                paid_amount: 0.0,
                balance_due: 0.0,
                notes: Some("Initial Stock Batch".to_string()),
                created_at: chrono::Utc::now().to_rfc3339(),
            };
            let synth_tx_id = tx.purchases().create_transaction(&synthetic_tx)?;
            let synth_lot = PurchaseItem {
                id: 0,
                purchase_transaction_id: synth_tx_id,
                item_id,
                item_name: item.name.clone(),
                pack_label: None,
                pack_quantity: initial_stock,
                quantity: initial_stock,
                unit_cost,
                total_amount: initial_stock * unit_cost,
                remaining_quantity: initial_stock,
            };
            tx.purchases().create_purchase_item(&synth_lot)?;
        }

        if let Some(bundles) = payload.bundles {
            for b_payload in bundles {
                let bundle = ItemBundle {
                    id: 0,
                    item_id,
                    bundle_name: b_payload.bundle_name,
                    units_per_bundle: b_payload.units_per_bundle,
                    mrp: b_payload.mrp,
                    default_bundle_price: b_payload.default_bundle_price,
                    barcode: b_payload.barcode,
                };
                let bid = tx.bundles().create(&bundle)?;
                let mut saved_b = bundle;
                saved_b.id = bid;
                created_bundles.push(saved_b);
            }
        }

        tx.commit()?;

        let mut saved_item = item;
        saved_item.id = item_id;
        let _ = self
            .search_indexer
            .index_item_with_bundles(&saved_item, &created_bundles);

        Ok(item_id)
    }

    fn update_item(&self, item_id: i64, payload: UpdateItemPayload) -> Result<(), AppError> {
        if payload.name.trim().is_empty() {
            return Err(AppError::ValidationError {
                field: "name".to_string(),
                message: "Item name cannot be empty".to_string(),
            });
        }
        if payload.default_sale_price < 0.0 {
            return Err(AppError::ValidationError {
                field: "default_sale_price".to_string(),
                message: "default_sale_price cannot be negative".to_string(),
            });
        }

        let mut tx = self.uow.begin_transaction()?;
        let mut existing = tx
            .items()
            .find_by_id(item_id)?
            .ok_or_else(|| AppError::NotFound {
                entity: "Item".to_string(),
                id: item_id.to_string(),
            })?;

        if let Some(ref bc) = payload.barcode {
            let trimmed = bc.trim();
            if !trimmed.is_empty() && existing.barcode.as_deref() != Some(trimmed) {
                if let Some(found) = tx.items().find_by_barcode(trimmed)? {
                    if found.id != item_id {
                        return Err(AppError::ValidationError {
                            field: "barcode".to_string(),
                            message: format!("Barcode '{}' already exists", trimmed),
                        });
                    }
                }
            }
        }

        existing.name = payload.name.trim().to_string();
        existing.category = payload.category.map(|c| c.trim().to_string()).filter(|c| !c.is_empty());
        existing.barcode = payload.barcode.map(|b| b.trim().to_string()).filter(|b| !b.is_empty());
        existing.base_unit = payload.base_unit.trim().to_string();
        existing.mrp = payload.mrp;
        existing.default_sale_price = payload.default_sale_price;
        existing.updated_at = chrono::Utc::now().to_rfc3339();

        tx.items().update(&existing)?;
        let bundles = tx.bundles().find_by_item_id(item_id)?;
        tx.commit()?;

        let _ = self.search_indexer.index_item_with_bundles(&existing, &bundles);
        Ok(())
    }

    fn create_bundle(&self, payload: CreateBundlePayload) -> Result<i64, AppError> {
        if payload.bundle_name.trim().is_empty() {
            return Err(AppError::ValidationError {
                field: "bundle_name".to_string(),
                message: "Bundle name cannot be empty".to_string(),
            });
        }
        if payload.units_per_bundle <= 0.0 {
            return Err(AppError::ValidationError {
                field: "units_per_bundle".to_string(),
                message: "Units per bundle must be positive".to_string(),
            });
        }

        let item_id = payload.item_id.ok_or_else(|| AppError::ValidationError {
            field: "item_id".to_string(),
            message: "item_id is required for creating a bundle".to_string(),
        })?;

        let mut tx = self.uow.begin_transaction()?;
        let item = tx
            .items()
            .find_by_id(item_id)?
            .ok_or_else(|| AppError::NotFound {
                entity: "Item".to_string(),
                id: item_id.to_string(),
            })?;

        let bundle = ItemBundle {
            id: 0,
            item_id,
            bundle_name: payload.bundle_name.trim().to_string(),
            units_per_bundle: payload.units_per_bundle,
            mrp: payload.mrp,
            default_bundle_price: payload.default_bundle_price,
            barcode: payload.barcode.map(|b| b.trim().to_string()).filter(|b| !b.is_empty()),
        };

        let bundle_id = tx.bundles().create(&bundle)?;
        let bundles = tx.bundles().find_by_item_id(item_id)?;
        tx.commit()?;

        let _ = self.search_indexer.index_item_with_bundles(&item, &bundles);
        Ok(bundle_id)
    }

    fn get_bundle_suggestions(&self) -> Result<Vec<BundleSuggestion>, AppError> {
        let mut tx = self.uow.begin_read()?;
        tx.bundles().get_bundle_suggestions()
    }
}
