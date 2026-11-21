use std::sync::Arc;
use calamine::{open_workbook_auto, Data, Reader};
use crate::core::dtos::ImportSummary;
use crate::core::entities::{Item, PurchaseItem, PurchaseTransaction};
use crate::core::error::AppError;
use crate::core::value_objects::PaymentStatus;
use crate::data::contracts::unit_of_work::UnitOfWork;
use crate::services::contracts::import_export_service::DataExportImportService;

#[derive(Clone)]
pub struct ImportExportServiceImpl<U: UnitOfWork = Arc<dyn UnitOfWork>> {
    pub uow: U,
}

pub type DataExportImportServiceImpl = ImportExportServiceImpl;

impl<U: UnitOfWork> ImportExportServiceImpl<U> {
    pub fn new(uow: U) -> Self {
        Self { uow }
    }
}

impl<U: UnitOfWork> DataExportImportService for ImportExportServiceImpl<U> {
    fn export_items_to_excel(&self, output_path: &str) -> Result<(), AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let items = tx.items().list_catalog(None, false, 0, usize::MAX)?;

        let mut workbook = rust_xlsxwriter::Workbook::new();
        let worksheet = workbook.add_worksheet();
        worksheet
            .set_name("Items")
            .map_err(|e| AppError::IoError(e.to_string()))?;

        let headers = [
            "ID",
            "Name",
            "Category",
            "Barcode",
            "Base Unit",
            "MRP",
            "Sale Price",
            "Current Stock",
            "Is Low Stock",
            "Is Active",
        ];
        for (col, header) in headers.iter().enumerate() {
            worksheet
                .write(0, col as u16, *header)
                .map_err(|e| AppError::IoError(e.to_string()))?;
        }

        for (row_idx, item) in items.iter().enumerate() {
            let row = (row_idx + 1) as u32;
            worksheet
                .write(row, 0, item.id as f64)
                .map_err(|e| AppError::IoError(e.to_string()))?;
            worksheet
                .write(row, 1, item.name.as_str())
                .map_err(|e| AppError::IoError(e.to_string()))?;
            worksheet
                .write(row, 2, item.category.as_deref().unwrap_or(""))
                .map_err(|e| AppError::IoError(e.to_string()))?;
            worksheet
                .write(row, 3, item.barcode.as_deref().unwrap_or(""))
                .map_err(|e| AppError::IoError(e.to_string()))?;
            worksheet
                .write(row, 4, item.base_unit.as_str())
                .map_err(|e| AppError::IoError(e.to_string()))?;
            if let Some(mrp) = item.mrp {
                worksheet
                    .write(row, 5, mrp)
                    .map_err(|e| AppError::IoError(e.to_string()))?;
            }
            worksheet
                .write(row, 6, item.default_sale_price)
                .map_err(|e| AppError::IoError(e.to_string()))?;
            worksheet
                .write(row, 7, item.current_stock)
                .map_err(|e| AppError::IoError(e.to_string()))?;
            worksheet
                .write(row, 8, if item.is_low_stock { "YES" } else { "NO" })
                .map_err(|e| AppError::IoError(e.to_string()))?;
            worksheet
                .write(
                    row,
                    9,
                    if item.is_active {
                        "ACTIVE"
                    } else {
                        "ARCHIVED"
                    },
                )
                .map_err(|e| AppError::IoError(e.to_string()))?;
        }

        workbook
            .save(output_path)
            .map_err(|e| AppError::IoError(e.to_string()))?;
        Ok(())
    }

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

    fn export_transactions_to_excel(
        &self,
        from_date: &str,
        to_date: &str,
        output_path: &str,
    ) -> Result<(), AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let sales = tx.sales().list_sales(
            Some(from_date),
            Some(to_date),
            None,
            None,
            0,
            usize::MAX,
        )?;
        let purchases = tx.purchases().list_purchases(
            Some(from_date),
            Some(to_date),
            None,
            None,
            0,
            usize::MAX,
        )?;

        let mut workbook = rust_xlsxwriter::Workbook::new();

        // Sheet 1: Sales
        let sales_sheet = workbook.add_worksheet();
        sales_sheet
            .set_name("Sales")
            .map_err(|e| AppError::IoError(e.to_string()))?;
        let s_headers = [
            "ID",
            "Code",
            "Date",
            "Party ID",
            "Payment Mode",
            "Payment Status",
            "Total Amount",
            "Paid Amount",
            "Balance Due",
            "Notes",
        ];
        for (col, header) in s_headers.iter().enumerate() {
            sales_sheet
                .write(0, col as u16, *header)
                .map_err(|e| AppError::IoError(e.to_string()))?;
        }
        for (row_idx, sale) in sales.iter().enumerate() {
            let row = (row_idx + 1) as u32;
            sales_sheet
                .write(row, 0, sale.id as f64)
                .map_err(|e| AppError::IoError(e.to_string()))?;
            sales_sheet
                .write(row, 1, sale.transaction_code.as_str())
                .map_err(|e| AppError::IoError(e.to_string()))?;
            sales_sheet
                .write(row, 2, sale.transaction_date.as_str())
                .map_err(|e| AppError::IoError(e.to_string()))?;
            if let Some(pid) = sale.party_id {
                sales_sheet
                    .write(row, 3, pid as f64)
                    .map_err(|e| AppError::IoError(e.to_string()))?;
            }
            sales_sheet
                .write(row, 4, sale.payment_mode.as_deref().unwrap_or(""))
                .map_err(|e| AppError::IoError(e.to_string()))?;
            sales_sheet
                .write(row, 5, format!("{:?}", sale.payment_status).as_str())
                .map_err(|e| AppError::IoError(e.to_string()))?;
            sales_sheet
                .write(row, 6, sale.total_amount)
                .map_err(|e| AppError::IoError(e.to_string()))?;
            sales_sheet
                .write(row, 7, sale.paid_amount)
                .map_err(|e| AppError::IoError(e.to_string()))?;
            sales_sheet
                .write(row, 8, sale.balance_due)
                .map_err(|e| AppError::IoError(e.to_string()))?;
            sales_sheet
                .write(row, 9, sale.notes.as_deref().unwrap_or(""))
                .map_err(|e| AppError::IoError(e.to_string()))?;
        }

        // Sheet 2: Purchases
        let pur_sheet = workbook.add_worksheet();
        pur_sheet
            .set_name("Purchases")
            .map_err(|e| AppError::IoError(e.to_string()))?;
        let p_headers = [
            "ID",
            "Code",
            "Date",
            "Party ID",
            "Payment Mode",
            "Payment Status",
            "Total Amount",
            "Paid Amount",
            "Balance Due",
            "Notes",
        ];
        for (col, header) in p_headers.iter().enumerate() {
            pur_sheet
                .write(0, col as u16, *header)
                .map_err(|e| AppError::IoError(e.to_string()))?;
        }
        for (row_idx, pur) in purchases.iter().enumerate() {
            let row = (row_idx + 1) as u32;
            pur_sheet
                .write(row, 0, pur.id as f64)
                .map_err(|e| AppError::IoError(e.to_string()))?;
            pur_sheet
                .write(row, 1, pur.transaction_code.as_str())
                .map_err(|e| AppError::IoError(e.to_string()))?;
            pur_sheet
                .write(row, 2, pur.transaction_date.as_str())
                .map_err(|e| AppError::IoError(e.to_string()))?;
            if let Some(pid) = pur.party_id {
                pur_sheet
                    .write(row, 3, pid as f64)
                    .map_err(|e| AppError::IoError(e.to_string()))?;
            }
            pur_sheet
                .write(row, 4, pur.payment_mode.as_deref().unwrap_or(""))
                .map_err(|e| AppError::IoError(e.to_string()))?;
            pur_sheet
                .write(row, 5, format!("{:?}", pur.payment_status).as_str())
                .map_err(|e| AppError::IoError(e.to_string()))?;
            pur_sheet
                .write(row, 6, pur.total_amount)
                .map_err(|e| AppError::IoError(e.to_string()))?;
            pur_sheet
                .write(row, 7, pur.paid_amount)
                .map_err(|e| AppError::IoError(e.to_string()))?;
            pur_sheet
                .write(row, 8, pur.balance_due)
                .map_err(|e| AppError::IoError(e.to_string()))?;
            pur_sheet
                .write(row, 9, pur.notes.as_deref().unwrap_or(""))
                .map_err(|e| AppError::IoError(e.to_string()))?;
        }

        workbook
            .save(output_path)
            .map_err(|e| AppError::IoError(e.to_string()))?;
        Ok(())
    }
}
