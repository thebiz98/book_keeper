use std::sync::Arc;
use calamine::{open_workbook_auto, Data, Reader};
use crate::core::dtos::ImportSummary;
use crate::core::entities::{Item, PurchaseItem, PurchaseTransaction};
use crate::core::error::AppError;
use crate::core::value_objects::PaymentStatus;
use crate::data::contracts::unit_of_work::UnitOfWork;
use crate::services::contracts::item_import_service::ItemImportService;

#[derive(Clone)]
pub struct ItemImportServiceImpl<U: UnitOfWork = Arc<dyn UnitOfWork>> {
    pub uow: U,
}

impl<U: UnitOfWork> ItemImportServiceImpl<U> {
    pub fn new(uow: U) -> Self {
        Self { uow }
    }
}

impl<U: UnitOfWork> ItemImportService for ItemImportServiceImpl<U> {
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
                let item = Item {
                    id: 0,
                    name,
                    category,
                    barcode,
                    base_unit,
                    mrp,
                    default_sale_price,
                    current_stock,
                    is_low_stock: current_stock < 5.0,
                    is_active: true,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    updated_at: chrono::Utc::now().to_rfc3339(),
                };
                match tx.items().create(&item) {
                    Ok(item_id) => {
                        if current_stock > 0.0 {
                            let unit_cost = default_sale_price * 0.7;
                            let synthetic_tx = PurchaseTransaction {
                                id: 0,
                                transaction_code: format!("IMP-{}", item_id),
                                transaction_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                                party_id: None,
                                payment_mode: None,
                                payment_status: PaymentStatus::Paid,
                                total_amount: current_stock * unit_cost,
                                paid_amount: current_stock * unit_cost,
                                balance_due: 0.0,
                                notes: Some("Excel Inward Import".to_string()),
                                created_at: chrono::Utc::now().to_rfc3339(),
                            };
                            if let Ok(p_id) = tx.purchases().create_transaction(&synthetic_tx) {
                                let lot = PurchaseItem {
                                    id: 0,
                                    purchase_transaction_id: p_id,
                                    item_id,
                                    bundle_id: None,
                                    bundle_quantity: current_stock,
                                    quantity: current_stock,
                                    unit_cost,
                                    total_amount: current_stock * unit_cost,
                                    remaining_quantity: current_stock,
                                };
                                let _ = tx.purchases().create_purchase_item(&lot);
                            }
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
