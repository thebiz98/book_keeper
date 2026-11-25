use std::sync::Arc;
use crate::core::dtos::{
    BundleSuggestion, CatalogFilter, CreateBundlePayload, CreateItemPayload, DeletedItemRow, ItemBundleSearchResult,
    ItemDeletePreview, DeleteItemResult, DeleteMode,
    ItemSearchResult, PageRequest, Paginated, SavedBundle, ServiceComponentInput, ServiceComponentView,
    UpdateBundlePayload, UpdateItemPayload,
};
use crate::core::entities::{Item, ItemBundle};
use crate::core::error::AppError;
use crate::core::money::round2;
use crate::core::stock_rules::{
    is_low_stock, validate_low_stock_threshold, DEFAULT_LOW_STOCK_THRESHOLD,
};
use crate::core::value_objects::{clean_name, clean_optional, ItemType, LotSource};
use crate::data::contracts::search_indexer::ItemSearchIndexer;
use crate::data::contracts::unit_of_work::{TransactionContext, UnitOfWork};
use crate::services::contracts::inventory_service::InventoryService;
use crate::services::contracts::item_service::ItemService;

/// Item + bundle master data. Opening stock is NOT written here: `create_item` delegates it to
/// `InventoryService::create_source_lot(Opening, ..)` inside its own transaction (NFR-05 F-04).
/// `InventoryService` reads items via `uow.items()` and does not depend on `ItemService`, so
/// this dependency is acyclic (the LLD 6.1 edge `InventoryService -> ItemService` is dropped).
#[derive(Clone)]
pub struct ItemServiceImpl<U: UnitOfWork = Arc<dyn UnitOfWork>, I: InventoryService = Arc<dyn InventoryService>> {
    pub uow: U,
    pub search_indexer: Arc<dyn ItemSearchIndexer>,
    pub inventory_service: I,
}

impl<U: UnitOfWork, I: InventoryService> ItemServiceImpl<U, I> {
    pub fn new(uow: U, search_indexer: Arc<dyn ItemSearchIndexer>, inventory_service: I) -> Self {
        Self { uow, search_indexer, inventory_service }
    }
}

fn invalid(field: &str, message: impl Into<String>) -> AppError {
    AppError::ValidationError { field: field.to_string(), message: message.into() }
}

fn clean(s: Option<String>) -> Option<String> {
    s.map(|c| c.trim().to_string()).filter(|c| !c.is_empty())
}

/// Which table owns a barcode (used to exclude the record being edited from the collision check).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarcodeOwner {
    Item,
    Bundle,
}

/// Pure field validation for an item (create, update, import). `name` is checked trimmed.
/// `mrp` must be null or > 0 (0 is rejected: the UI sends null for "no MRP").
/// FR-36: a Service cost is optional, finite and >= 0 (rounded to 2 places).
pub fn validate_service_cost(cost: Option<f64>) -> Result<Option<f64>, AppError> {
    match cost {
        Some(c) if !c.is_finite() || c < 0.0 => {
            Err(invalid("service_cost", "Service cost must be a number that is 0 or more"))
        }
        Some(c) => Ok(Some(round2(c))),
        None => Ok(None),
    }
}

/// FR-36 rule for ONE component line, pure so the import dry run can call it for items that are
/// not saved yet: quantity per unit finite and > 0, not the service itself, a physical and active
/// item, listed once. `None` = fine.
pub fn component_rule_violation(
    service_id: Option<i64>,
    item: &Item,
    quantity_per_unit: f64,
    already_listed: bool,
) -> Option<String> {
    if !quantity_per_unit.is_finite() || quantity_per_unit <= 0.0 {
        return Some("Quantity per unit must be a number greater than 0".to_string());
    }
    if Some(item.id) == service_id {
        return Some("A service cannot be its own component".to_string());
    }
    if item.item_type != ItemType::Physical {
        return Some(format!("Component '{}' is a service; only physical items can be components", item.name));
    }
    if !item.is_active {
        return Some(format!("Component '{}' is deleted. Restore it first.", item.name));
    }
    if already_listed {
        return Some(format!("Component '{}' is listed more than once", item.name));
    }
    None
}

/// FR-36: every component is an active physical item other than the service itself, listed once,
/// with a finite quantity per unit > 0. Returns the rows to store, in the given order.
pub fn validate_components(
    tx: &mut dyn TransactionContext,
    service_id: Option<i64>,
    components: &[ServiceComponentInput],
) -> Result<Vec<(i64, f64)>, AppError> {
    let mut out: Vec<(i64, f64)> = Vec::with_capacity(components.len());
    for c in components {
        let item = tx
            .items()
            .find_by_id(c.component_item_id)?
            .ok_or_else(|| not_found("Item", c.component_item_id))?;
        let listed = out.iter().any(|(id, _)| *id == c.component_item_id);
        if let Some(message) = component_rule_violation(service_id, &item, c.quantity_per_unit, listed) {
            return Err(invalid("components", message));
        }
        out.push((c.component_item_id, c.quantity_per_unit));
    }
    Ok(out)
}

pub fn validate_item_fields(name: &str, default_sale_price: f64, mrp: Option<f64>) -> Result<(), AppError> {
    if clean_name(name).is_empty() {
        return Err(invalid("name", "Item name cannot be empty"));
    }
    if !default_sale_price.is_finite() || default_sale_price < 0.0 {
        return Err(invalid("default_sale_price", "Sale price must be a number that is 0 or more"));
    }
    validate_mrp(mrp)
}

fn validate_mrp(mrp: Option<f64>) -> Result<(), AppError> {
    match mrp {
        Some(m) if !m.is_finite() || m <= 0.0 => {
            Err(invalid("mrp", "MRP must be greater than 0 (leave it empty for no MRP)"))
        }
        _ => Ok(()),
    }
}

/// Pure field validation for a bundle (create, update, nested in create_item).
pub fn validate_bundle_fields(
    bundle_name: &str,
    units_per_bundle: f64,
    mrp: Option<f64>,
    default_bundle_price: Option<f64>,
) -> Result<(), AppError> {
    if bundle_name.trim().is_empty() {
        return Err(invalid("bundle_name", "Bundle name cannot be empty"));
    }
    if !units_per_bundle.is_finite() || units_per_bundle <= 1.0 {
        return Err(invalid("units_per_bundle", "Units per bundle must be greater than 1"));
    }
    validate_mrp(mrp).map_err(|_| invalid("mrp", "Bundle MRP must be greater than 0 (leave it empty for no MRP)"))?;
    if let Some(p) = default_bundle_price {
        if !p.is_finite() || p < 0.0 {
            return Err(invalid("default_bundle_price", "Bundle price must be a number that is 0 or more"));
        }
    }
    Ok(())
}

/// Rejects `barcode` when any item or bundle other than `exclude` already carries it.
pub fn ensure_barcode_unique(
    tx: &mut dyn TransactionContext,
    barcode: &str,
    exclude: Option<(BarcodeOwner, i64)>,
) -> Result<(), AppError> {
    let barcode = barcode.trim();
    if barcode.is_empty() {
        return Ok(());
    }
    let clash = |what: &str| invalid("barcode", format!("Barcode '{barcode}' is already used by another {what}"));
    if let Some(it) = tx.items().find_by_barcode(barcode)? {
        if exclude != Some((BarcodeOwner::Item, it.id)) {
            return Err(clash("item"));
        }
    }
    if let Some(b) = tx.bundles().find_by_barcode(barcode)? {
        if exclude != Some((BarcodeOwner::Bundle, b.id)) {
            return Err(clash("bundle"));
        }
    }
    Ok(())
}

/// Case-insensitive item-name uniqueness with a friendly error (the schema also enforces it).
pub fn ensure_item_name_unique(
    tx: &mut dyn TransactionContext,
    name: &str,
    exclude_item_id: Option<i64>,
) -> Result<(), AppError> {
    if let Some(it) = tx.items().find_by_name(&clean_name(name))? {
        if Some(it.id) != exclude_item_id {
            if !it.is_active {
                return Err(deleted_name_clash(&it));
            }
            return Err(invalid("name", format!("An item named '{}' already exists", it.name)));
        }
    }
    Ok(())
}

/// FR-45: the name belongs to a deleted item; the UI offers Restore for `existing_id`.
pub fn deleted_name_clash(it: &Item) -> AppError {
    AppError::DeletedNameClash {
        field: "name".to_string(),
        code: "ITEM_NAME_DELETED".to_string(),
        existing_id: it.id,
        message: format!("'{}' was deleted. Restore it from Deleted items instead of creating it again.", it.name),
    }
}

/// FR-45 / Y3: every write to a deleted item is refused except restore.
pub fn ensure_item_active(item: &Item) -> Result<(), AppError> {
    if item.is_active {
        Ok(())
    } else {
        Err(invalid("item_id", format!("'{}' is deleted. Restore it first.", item.name)))
    }
}

/// Bundle name must be unique per item (case-insensitive).
fn ensure_bundle_name_unique(
    tx: &mut dyn TransactionContext,
    item_id: i64,
    name: &str,
    exclude_bundle_id: Option<i64>,
) -> Result<(), AppError> {
    let name = name.trim();
    let dup = tx
        .bundles()
        .find_by_item_id(item_id)?
        .into_iter()
        .any(|b| Some(b.id) != exclude_bundle_id && b.bundle_name.trim().eq_ignore_ascii_case(name));
    if dup {
        return Err(invalid("bundle_name", format!("This item already has a bundle named '{name}'")));
    }
    Ok(())
}

/// Catalog/typeahead row for an item with its already-converted bundles.
pub fn to_search_result(i: Item, bundles: Vec<ItemBundleSearchResult>) -> ItemSearchResult {
    ItemSearchResult {
        bundles,
        item_id: i.id,
        name: i.name,
        category: i.category,
        barcode: i.barcode,
        base_unit: i.base_unit,
        item_type: i.item_type,
        mrp: i.mrp,
        default_sale_price: i.default_sale_price,
        current_stock: i.current_stock,
        is_low_stock: i.is_low_stock,
        low_stock_threshold: i.low_stock_threshold,
        matched_bundle_id: None,
        service_cost: i.service_cost,
        can_make: None,
        is_active: i.is_active,
    }
}

fn component_block_message(name: &str, services: &[String]) -> String {
    format!("'{name}' is a component of: {}. Remove it from those services first.", services.join(", "))
}

fn not_found(entity: &str, id: i64) -> AppError {
    AppError::NotFound { entity: entity.to_string(), id: id.to_string() }
}

impl<U: UnitOfWork, I: InventoryService> ItemServiceImpl<U, I> {
    /// Reindex after commit. A failure is logged and surfaced as `SearchIndexError`; the master
    /// data is already committed and the next startup rebuilds on mismatch.
    fn reindex(&self, item: &Item, bundles: &[ItemBundle]) -> Result<(), AppError> {
        self.search_indexer.index_item_with_bundles(item, bundles).map_err(|e| {
            eprintln!("[search] reindex of item #{} failed: {e}", item.id);
            match e {
                AppError::SearchIndexError(_) => e,
                other => AppError::SearchIndexError(other.to_string()),
            }
        })
    }

    fn validate_and_build_bundle(
        tx: &mut dyn TransactionContext,
        item_id: i64,
        bundle_id: Option<i64>,
        p: &UpdateBundlePayload,
    ) -> Result<ItemBundle, AppError> {
        validate_bundle_fields(&p.bundle_name, p.units_per_bundle, p.mrp, p.default_bundle_price)?;
        ensure_bundle_name_unique(tx, item_id, &clean_name(&p.bundle_name), bundle_id)?;
        let barcode = clean(p.barcode.clone());
        if let Some(bc) = barcode.as_deref() {
            ensure_barcode_unique(tx, bc, bundle_id.map(|id| (BarcodeOwner::Bundle, id)))?;
        }
        Ok(ItemBundle {
            id: bundle_id.unwrap_or(0),
            item_id,
            bundle_name: clean_name(&p.bundle_name),
            units_per_bundle: p.units_per_bundle,
            mrp: p.mrp,
            default_bundle_price: p.default_bundle_price,
            barcode,
        })
    }
}

impl<U: UnitOfWork, I: InventoryService> ItemService for ItemServiceImpl<U, I> {
    fn get_catalog(&self, filter: CatalogFilter) -> Result<Paginated<ItemSearchResult>, AppError> {
        let page = PageRequest::new(filter.page, filter.limit)?;
        let category = clean(filter.category);
        let low_stock_only = filter.low_stock_only.unwrap_or(false);
        let search = clean(filter.search);

        // Search goes through the indexer; category / low-stock are applied in the same SQL.
        if let Some(q) = search {
            let (rows, total) = self.search_indexer.search_page(
                &q,
                category.as_deref(),
                low_stock_only,
                page.offset(),
                page.limit,
            )?;
            return Ok(Paginated::new(rows, total, page.page, page.limit));
        }

        let mut tx = self.uow.begin_read()?;
        let items = tx.items().list_catalog(category.as_deref(), low_stock_only, page.offset(), page.limit)?;
        let total = tx.items().count_catalog(category.as_deref(), low_stock_only)?;
        // One batched query for the bundles of the whole page.
        let ids: Vec<i64> = items.iter().map(|i| i.id).collect();
        let mut by_item: std::collections::HashMap<i64, Vec<ItemBundleSearchResult>> = std::collections::HashMap::new();
        for b in tx.bundles().find_by_item_ids(&ids)? {
            let price = items.iter().find(|i| i.id == b.item_id).map_or(0.0, |i| i.default_sale_price);
            by_item.entry(b.item_id).or_default().push(ItemBundleSearchResult::from_bundle(b, price));
        }
        let service_ids: Vec<i64> = items.iter().filter(|i| i.item_type == ItemType::Service).map(|i| i.id).collect();
        let can_make = tx.service_components().can_make(&service_ids)?;
        let rows = items
            .into_iter()
            .map(|i| {
                let b = by_item.remove(&i.id).unwrap_or_default();
                let id = i.id;
                let mut row = to_search_result(i, b);
                row.can_make = can_make.get(&id).copied();
                row
            })
            .collect();
        Ok(Paginated::new(rows, total, page.page, page.limit))
    }

    fn search_items_fts(&self, query: &str, limit: Option<usize>) -> Result<Vec<ItemSearchResult>, AppError> {
        match limit {
            Some(l) => self.search_indexer.search_with_limit(query, l),
            None => self.search_indexer.search(query),
        }
    }

    fn get_item(&self, item_id: i64) -> Result<ItemSearchResult, AppError> {
        let mut tx = self.uow.begin_read()?;
        let item = tx.items().find_by_id(item_id)?.ok_or_else(|| not_found("Item", item_id))?;
        let bundles = tx
            .bundles()
            .find_by_item_id(item_id)?
            .into_iter()
            .map(|b| ItemBundleSearchResult::from_bundle(b, item.default_sale_price))
            .collect();
        let is_service = item.item_type == ItemType::Service;
        let mut row = to_search_result(item, bundles);
        if is_service {
            row.can_make = tx.service_components().can_make(&[item_id])?.get(&item_id).copied();
        }
        Ok(row)
    }

    fn list_categories(&self) -> Result<Vec<String>, AppError> {
        let mut tx = self.uow.begin_read()?;
        tx.items().list_categories()
    }

    fn create_item(&self, payload: CreateItemPayload) -> Result<i64, AppError> {
        let item_type = payload.item_type.unwrap_or_default();
        validate_item_fields(&payload.name, payload.default_sale_price, payload.mrp)?;
        let threshold = payload.low_stock_threshold.unwrap_or(DEFAULT_LOW_STOCK_THRESHOLD);
        validate_low_stock_threshold(threshold)?;
        if let Some(bundles) = &payload.bundles {
            for b in bundles {
                validate_bundle_fields(&b.bundle_name, b.units_per_bundle, b.mrp, b.default_bundle_price)?;
            }
        }
        let is_service = item_type == ItemType::Service;
        let service_cost = if is_service {
            // FR-36: a service has no stock and no bundles.
            if payload.initial_stock.unwrap_or(0.0) != 0.0 || payload.initial_cost.is_some() {
                return Err(invalid("initial_stock", "A service has no stock, so it cannot have opening stock"));
            }
            if payload.bundles.as_ref().is_some_and(|b| !b.is_empty()) {
                return Err(invalid("bundles", "A service cannot have bundles"));
            }
            validate_service_cost(payload.service_cost)?
        } else {
            if payload.service_cost.is_some() {
                return Err(invalid("service_cost", "Only a service can have a Service cost"));
            }
            if payload.components.as_ref().is_some_and(|c| !c.is_empty()) {
                return Err(invalid("components", "Only a service can have components"));
            }
            None
        };

        let mut tx = self.uow.begin_transaction()?;
        ensure_item_name_unique(&mut *tx, &payload.name, None)?;
        let components = match (&payload.components, is_service) {
            (Some(c), true) => validate_components(&mut *tx, None, c)?,
            _ => Vec::new(),
        };
        let barcode = clean(payload.barcode);
        if let Some(bc) = barcode.as_deref() {
            ensure_barcode_unique(&mut *tx, bc, None)?;
        }

        let initial_stock = payload.initial_stock.unwrap_or(0.0);
        let now = chrono::Utc::now().to_rfc3339();
        let item = Item {
            id: 0,
            name: clean_name(&payload.name),
            item_type,
            category: clean_optional(payload.category.as_deref()),
            barcode,
            base_unit: payload.base_unit.trim().to_string(),
            mrp: payload.mrp,
            default_sale_price: payload.default_sale_price,
            // Stock cache starts at 0; the opening lot below raises it (InventoryService is the sole writer).
            current_stock: 0.0,
            is_low_stock: is_low_stock(item_type, 0.0, threshold),
            is_active: true,
            created_at: now.clone(),
            updated_at: now,
            low_stock_threshold: threshold,
            service_cost,
        };

        let item_id = tx.items().create(&item)?;
        if is_service && !components.is_empty() {
            tx.service_components().replace_for_service(item_id, &components)?;
        }

        if initial_stock != 0.0 {
            // initial_cost is required (or a prior lot): never derived from the sale price (DD-12).
            self.inventory_service.create_source_lot(
                &mut *tx,
                LotSource::Opening,
                &chrono::Utc::now().format("%Y-%m-%d").to_string(),
                item_id,
                initial_stock,
                payload.initial_cost,
                Some("Initial Stock Batch".to_string()),
            )?;
        }

        for b in payload.bundles.unwrap_or_default() {
            let p = UpdateBundlePayload {
                bundle_name: b.bundle_name,
                units_per_bundle: b.units_per_bundle,
                mrp: b.mrp,
                default_bundle_price: b.default_bundle_price,
                barcode: b.barcode,
            };
            let bundle = Self::validate_and_build_bundle(&mut *tx, item_id, None, &p)?;
            tx.bundles().create(&bundle)?;
        }

        let saved = tx.items().find_by_id(item_id)?.ok_or_else(|| not_found("Item", item_id))?;
        let bundles = tx.bundles().find_by_item_id(item_id)?;
        tx.commit()?;
        self.reindex(&saved, &bundles)?;
        Ok(item_id)
    }

    fn update_item(&self, item_id: i64, payload: UpdateItemPayload) -> Result<(), AppError> {
        validate_item_fields(&payload.name, payload.default_sale_price, payload.mrp)?;
        if let Some(t) = payload.low_stock_threshold {
            validate_low_stock_threshold(t)?;
        }

        let mut tx = self.uow.begin_transaction()?;
        let mut existing = tx.items().find_by_id(item_id)?.ok_or_else(|| not_found("Item", item_id))?;
        ensure_item_active(&existing)?;
        // FR-36: the type is fixed once the item exists.
        if payload.item_type.is_some_and(|t| t != existing.item_type) {
            return Err(invalid("item_type", "Item type cannot be changed"));
        }
        let is_service = existing.item_type == ItemType::Service;
        let service_cost = if is_service {
            validate_service_cost(payload.service_cost)?
        } else {
            if payload.service_cost.is_some() {
                return Err(invalid("service_cost", "Only a service can have a Service cost"));
            }
            if payload.components.as_ref().is_some_and(|c| !c.is_empty()) {
                return Err(invalid("components", "Only a service can have components"));
            }
            None
        };
        let components = match (&payload.components, is_service) {
            (Some(c), true) => Some(validate_components(&mut *tx, Some(item_id), c)?),
            _ => None,
        };
        ensure_item_name_unique(&mut *tx, &payload.name, Some(item_id))?;
        let barcode = clean(payload.barcode);
        if let Some(bc) = barcode.as_deref() {
            ensure_barcode_unique(&mut *tx, bc, Some((BarcodeOwner::Item, item_id)))?;
        }

        // Master fields only: the repository update never touches stock.
        existing.name = clean_name(&payload.name);
        existing.category = clean_optional(payload.category.as_deref());
        existing.barcode = barcode;
        existing.base_unit = payload.base_unit.trim().to_string();
        existing.mrp = payload.mrp; // None clears the MRP
        existing.default_sale_price = payload.default_sale_price;
        if let Some(t) = payload.low_stock_threshold {
            existing.low_stock_threshold = t;
        }
        existing.service_cost = service_cost;
        existing.updated_at = chrono::Utc::now().to_rfc3339();

        tx.items().update(&existing)?;
        if let Some(c) = components {
            tx.service_components().replace_for_service(item_id, &c)?;
        }
        let bundles = tx.bundles().find_by_item_id(item_id)?;
        tx.commit()?;

        self.reindex(&existing, &bundles)
    }

    fn update_low_stock_threshold(&self, item_id: i64, threshold: f64) -> Result<ItemSearchResult, AppError> {
        validate_low_stock_threshold(threshold)?;
        let mut tx = self.uow.begin_transaction()?;
        let current = tx.items().find_by_id(item_id)?.ok_or_else(|| not_found("Item", item_id))?;
        ensure_item_active(&current)?;
        tx.items().update_low_stock_threshold(item_id, threshold, &chrono::Utc::now().to_rfc3339())?;
        let item = tx.items().find_by_id(item_id)?.ok_or_else(|| not_found("Item", item_id))?;
        let bundles = tx
            .bundles()
            .find_by_item_id(item_id)?
            .into_iter()
            .map(|b| ItemBundleSearchResult::from_bundle(b, item.default_sale_price))
            .collect();
        tx.commit()?;
        // The threshold is not indexed: no reindex needed.
        Ok(to_search_result(item, bundles))
    }

    fn create_bundle(&self, payload: CreateBundlePayload) -> Result<SavedBundle, AppError> {
        let item_id = payload.item_id.ok_or_else(|| invalid("item_id", "item_id is required for creating a bundle"))?;
        let p = UpdateBundlePayload {
            bundle_name: payload.bundle_name,
            units_per_bundle: payload.units_per_bundle,
            mrp: payload.mrp,
            default_bundle_price: payload.default_bundle_price,
            barcode: payload.barcode,
        };

        let mut tx = self.uow.begin_transaction()?;
        let item = tx.items().find_by_id(item_id)?.ok_or_else(|| not_found("Item", item_id))?;
        ensure_item_active(&item)?;
        if item.item_type == ItemType::Service {
            return Err(invalid("item_id", format!("'{}' is a service item; services cannot have bundles", item.name)));
        }
        let mut bundle = Self::validate_and_build_bundle(&mut *tx, item_id, None, &p)?;
        bundle.id = tx.bundles().create(&bundle)?;
        let bundles = tx.bundles().find_by_item_id(item_id)?;
        tx.commit()?;

        self.reindex(&item, &bundles)?;
        Ok(SavedBundle::new(bundle, item.default_sale_price))
    }

    fn update_bundle(&self, bundle_id: i64, payload: UpdateBundlePayload) -> Result<SavedBundle, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let existing = tx.bundles().find_by_id(bundle_id)?.ok_or_else(|| not_found("ItemBundle", bundle_id))?;
        let item = tx.items().find_by_id(existing.item_id)?.ok_or_else(|| not_found("Item", existing.item_id))?;
        ensure_item_active(&item)?;
        // A bundle stays on its item; the payload has no item_id.
        let bundle = Self::validate_and_build_bundle(&mut *tx, existing.item_id, Some(bundle_id), &payload)?;
        tx.bundles().update(&bundle)?;
        let bundles = tx.bundles().find_by_item_id(existing.item_id)?;
        tx.commit()?;

        self.reindex(&item, &bundles)?;
        Ok(SavedBundle::new(bundle, item.default_sale_price))
    }

    fn delete_bundle(&self, bundle_id: i64) -> Result<(), AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let existing = tx.bundles().find_by_id(bundle_id)?.ok_or_else(|| not_found("ItemBundle", bundle_id))?;
        let item = tx.items().find_by_id(existing.item_id)?.ok_or_else(|| not_found("Item", existing.item_id))?;
        ensure_item_active(&item)?;
        // DD-28: no reference guard; bills keep their pack_label snapshot.
        tx.bundles().delete(bundle_id)?;
        let bundles = tx.bundles().find_by_item_id(existing.item_id)?;
        tx.commit()?;

        self.reindex(&item, &bundles)
    }

    fn get_bundle_suggestions(&self) -> Result<Vec<BundleSuggestion>, AppError> {
        let mut tx = self.uow.begin_read()?;
        tx.bundles().get_bundle_suggestions()
    }

    fn preview_item_delete(&self, item_id: i64) -> Result<ItemDeletePreview, AppError> {
        let mut tx = self.uow.begin_read()?;
        let item = tx.items().find_by_id(item_id)?.ok_or_else(|| not_found("Item", item_id))?;
        let blocking_services = tx.items().services_using_active(item_id)?;
        let stock_value = tx.items().stock_value(item_id)?;
        let reasons_used = tx.items().usage_reasons(item_id)?;
        let mode = if reasons_used.is_empty() { DeleteMode::Hard } else { DeleteMode::Soft };
        // A deleted item can still be removed permanently when it was never used (DD-79).
        let reason = if !item.is_active && mode == DeleteMode::Soft {
            Some(format!("'{}' is already deleted.", item.name))
        } else if item.is_active && !blocking_services.is_empty() {
            Some(component_block_message(&item.name, &blocking_services))
        } else {
            None
        };
        Ok(ItemDeletePreview {
            item_id,
            name: item.name,
            item_type: item.item_type,
            stock: item.current_stock,
            base_unit: item.base_unit,
            stock_value,
            blocking_services,
            can_delete: reason.is_none(),
            reason,
            mode,
            reasons_used,
        })
    }

    fn delete_item(&self, item_id: i64) -> Result<DeleteItemResult, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let item = tx.items().find_by_id(item_id)?.ok_or_else(|| not_found("Item", item_id))?;
        let used = !tx.items().usage_reasons(item_id)?.is_empty();
        if !item.is_active && used {
            return Err(invalid("item_id", format!("'{}' is already deleted", item.name)));
        }
        if item.is_active {
            let services = tx.items().services_using_active(item_id)?;
            if !services.is_empty() {
                return Err(AppError::BillLocked {
                    code: "ITEM_IS_COMPONENT".to_string(),
                    message: component_block_message(&item.name, &services),
                });
            }
        }
        let mode = if used {
            tx.items().set_active(item_id, false, &chrono::Utc::now().to_rfc3339())?;
            DeleteMode::Soft
        } else {
            tx.items().hard_delete(item_id)?;
            DeleteMode::Hard
        };
        tx.commit()?;
        // DD-75: the name row and every bundle barcode row leave the index.
        self.search_indexer.remove_item(item_id)?;
        Ok(DeleteItemResult { mode })
    }

    fn restore_item(&self, item_id: i64) -> Result<(), AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let item = tx.items().find_by_id(item_id)?.ok_or_else(|| not_found("Item", item_id))?;
        if item.is_active {
            return Err(invalid("item_id", format!("'{}' is not deleted", item.name)));
        }
        if item.item_type == ItemType::Service {
            for c in tx.service_components().list_for_service(item_id)? {
                let comp = tx
                    .items()
                    .find_by_id(c.component_item_id)?
                    .ok_or_else(|| not_found("Item", c.component_item_id))?;
                if !comp.is_active {
                    return Err(invalid(
                        "components",
                        format!("Component '{}' is deleted. Restore it first.", comp.name),
                    ));
                }
            }
        }
        let now = chrono::Utc::now().to_rfc3339();
        tx.items().set_active(item_id, true, &now)?;
        let restored = tx.items().find_by_id(item_id)?.ok_or_else(|| not_found("Item", item_id))?;
        let bundles = tx.bundles().find_by_item_id(item_id)?;
        tx.commit()?;
        self.reindex(&restored, &bundles)
    }

    fn list_deleted_items(
        &self,
        search: Option<String>,
        page: Option<usize>,
        page_size: Option<usize>,
    ) -> Result<Paginated<DeletedItemRow>, AppError> {
        let req = PageRequest::new(page, page_size.or(Some(10)))?;
        let mut tx = self.uow.begin_read()?;
        let (mut rows, total) = tx.items().list_inactive(search.as_deref().unwrap_or(""), req.offset(), req.limit)?;
        for r in &mut rows {
            r.can_purge = tx.items().usage_reasons(r.item_id)?.is_empty();
        }
        Ok(Paginated::new(rows, total, req.page, req.limit))
    }

    fn get_service_components(&self, item_id: i64) -> Result<Vec<ServiceComponentView>, AppError> {
        let mut tx = self.uow.begin_read()?;
        tx.items().find_by_id(item_id)?.ok_or_else(|| not_found("Item", item_id))?;
        let mut out = Vec::new();
        for c in tx.service_components().list_for_service(item_id)? {
            let it = tx
                .items()
                .find_by_id(c.component_item_id)?
                .ok_or_else(|| not_found("Item", c.component_item_id))?;
            out.push(ServiceComponentView {
                component_item_id: it.id,
                name: it.name,
                base_unit: it.base_unit,
                quantity_per_unit: c.quantity_per_unit,
                current_stock: it.current_stock,
            });
        }
        Ok(out)
    }
}
