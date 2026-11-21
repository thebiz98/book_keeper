use std::sync::Arc;
use calamine::{open_workbook_auto, Data, Reader};
use crate::core::dtos::ImportSummary;
use crate::core::entities::Item;
use crate::core::error::AppError;
use crate::core::value_objects::LotSource;
use crate::data::contracts::unit_of_work::UnitOfWork;
use crate::services::contracts::inventory_service::InventoryService;
use crate::services::contracts::item_import_service::ItemImportService;

/// Import of items. Opening stock goes through `InventoryService::create_source_lot(Opening, ..)`
/// (sole stock writer). The rest of the import is rewritten in T9.
#[derive(Clone)]
pub struct ItemImportServiceImpl<U: UnitOfWork = Arc<dyn UnitOfWork>, I: InventoryService = Arc<dyn InventoryService>> {
    pub uow: U,
    pub inventory_service: I,
}

impl<U: UnitOfWork, I: InventoryService> ItemImportServiceImpl<U, I> {
    pub fn new(uow: U, inventory_service: I) -> Self {
        Self { uow, inventory_service }
    }
}

impl<U: UnitOfWork, I: InventoryService> ItemImportService for ItemImportServiceImpl<U, I> {
    fn import_items_from_excel(&self, input_path: &str) -> Result<ImportSummary, AppError> {
        let mut workbook = open_workbook_auto(input_path)
            .map_err(|e| AppError::IoError(format!("Failed to open workbook: {}", e)))?;

        let range = workbook
            .worksheet_range_at(0)
            .ok_or_else(|| AppError::ValidationError {
                field: "excel".to_string(),
                message: "No sheets found in workbook".to_string(),
            })?
            .map_err(|e| AppError::IoError(format!("Corrupt worksheet: {}", e)))?;

        let mut rows = range.rows();
        let header_row = rows.next().ok_or_else(|| AppError::ValidationError {
            field: "excel".to_string(),
            message: "Excel sheet is empty".to_string(),
        })?;

        let mut col_name = None;
        let mut col_category = None;
        let mut col_barcode = None;
        let mut col_unit = None;
        let mut col_mrp = None;
        let mut col_price = None;
        let mut col_stock = None;
        let mut col_cost = None;

        for (idx, cell) in header_row.iter().enumerate() {
            let title = match cell {
                Data::String(s) => s.trim().to_lowercase(),
                _ => continue,
            };
            if title.contains("name") {
                col_name = Some(idx);
            } else if title.contains("category") {
                col_category = Some(idx);
            } else if title.contains("barcode") {
                col_barcode = Some(idx);
            } else if title.contains("unit") {
                col_unit = Some(idx);
            } else if title.contains("cost") {
                col_cost = Some(idx);
            } else if title == "mrp" {
                col_mrp = Some(idx);
            } else if title.contains("price") || title.contains("rate") {
                col_price = Some(idx);
            } else if title.contains("stock") || title.contains("qty") || title.contains("quantity") {
                col_stock = Some(idx);
            }
        }

        let name_idx = col_name.ok_or_else(|| AppError::ValidationError {
            field: "header".to_string(),
            message: "Missing 'Name' column header in Excel sheet".to_string(),
        })?;

        let mut total_rows = 0;
        let mut imported_count = 0;
        let mut skipped_count = 0;
        let mut errors = Vec::new();

        let mut tx = self.uow.begin_transaction()?;
        let import_date = chrono::Utc::now().format("%Y-%m-%d").to_string();

        let parse_f64 = |cell: Option<&Data>| -> Option<f64> {
            match cell {
                Some(Data::Float(f)) => Some(*f),
                Some(Data::Int(i)) => Some(*i as f64),
                Some(Data::String(s)) => s.trim().parse::<f64>().ok(),
                _ => None,
            }
        };

        let parse_string = |cell: Option<&Data>| -> Option<String> {
            match cell {
                Some(Data::String(s)) => {
                    let trimmed = s.trim().to_string();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed)
                    }
                }
                Some(Data::Int(i)) => Some(i.to_string()),
                Some(Data::Float(f)) => Some(f.to_string()),
                _ => None,
            }
        };

        for row in rows {
            total_rows += 1;
            let name = match parse_string(row.get(name_idx)) {
                Some(n) if !n.is_empty() => n,
                _ => {
                    skipped_count += 1;
                    continue;
                }
            };

            let category = col_category.and_then(|idx| parse_string(row.get(idx)));
            let barcode = col_barcode.and_then(|idx| parse_string(row.get(idx)));
            let base_unit = col_unit
                .and_then(|idx| parse_string(row.get(idx)))
                .unwrap_or_else(|| "PCS".to_string());
            let mrp = col_mrp.and_then(|idx| parse_f64(row.get(idx)));
            let default_sale_price = col_price
                .and_then(|idx| parse_f64(row.get(idx)))
                .unwrap_or(0.0);
            let current_stock = col_stock
                .and_then(|idx| parse_f64(row.get(idx)))
                .unwrap_or(0.0);
            let unit_cost = col_cost.and_then(|idx| parse_f64(row.get(idx)));

            // Check if item already exists by barcode or name
            let existing_item = if let Some(ref bc) = barcode {
                tx.items().find_by_barcode(bc)?
            } else {
                tx.items().find_by_name(&name)?
            };

            if let Some(mut existing) = existing_item {
                existing.name = name;
                existing.category = category;
                existing.barcode = barcode;
                existing.base_unit = base_unit;
                existing.mrp = mrp;
                existing.default_sale_price = default_sale_price;
                existing.updated_at = chrono::Utc::now().to_rfc3339();
                if let Err(e) = tx.items().update(&existing) {
                    errors.push(format!("Row {}: Failed to update item: {}", total_rows, e));
                    skipped_count += 1;
                    continue;
                }
            } else {
                // Opening stock needs a cost (no sale-price fallback, DD-12). Reject the row before
                // anything is written so the shared transaction stays consistent.
                if current_stock > 0.0 && unit_cost.map_or(true, |c| !c.is_finite() || c < 0.0) {
                    errors.push(format!(
                        "Row {}: opening stock {} needs a cost column value",
                        total_rows, current_stock
                    ));
                    skipped_count += 1;
                    continue;
                }
                let item = Item {
                    id: 0,
                    name,
                    category,
                    barcode,
                    base_unit,
                    mrp,
                    default_sale_price,
                    // The cache starts at 0; the opening lot raises it.
                    current_stock: 0.0,
                    is_low_stock: true,
                    is_active: true,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    updated_at: chrono::Utc::now().to_rfc3339(),
                };
                match tx.items().create(&item) {
                    Ok(item_id) => {
                        if current_stock > 0.0 {
                            self.inventory_service.create_source_lot(
                                &mut *tx,
                                LotSource::Opening,
                                &import_date,
                                item_id,
                                current_stock,
                                unit_cost,
                                Some("Excel Inward Import".to_string()),
                            )?;
                        }
                    }
                    Err(e) => {
                        errors.push(format!("Row {}: Failed to create item: {}", total_rows, e));
                        skipped_count += 1;
                        continue;
                    }
                }
            }

            imported_count += 1;
        }

        tx.commit()?;

        Ok(ImportSummary {
            total_rows,
            imported_count,
            skipped_count,
            errors,
        })
    }
}
