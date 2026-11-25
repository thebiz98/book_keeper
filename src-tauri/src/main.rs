// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use book_keeper::api::*;
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let env_override = std::env::var(book_keeper::app_state::DB_PATH_ENV).ok();
            let db_path = book_keeper::app_state::resolve_db_path(&dir, env_override.as_deref());
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let state = book_keeper::app_state::build_sqlite_app_state_with_config(
                &db_path,
                book_keeper::config::AppConfig::compiled(),
                dir.join(book_keeper::app_state::DEFAULT_BACKUP_DIR),
            )?;
            let backup_svc = state.backup_service.clone();
            let backup_cfg = state.config.backup.clone();
            app.manage(state);

            // Auto backups: one check now, then hourly. Reads the folder each time so a change
            // in Settings applies without a restart.
            let handle = app.handle().clone();
            book_keeper::services::r#impl::backup_scheduler::spawn(backup_svc, backup_cfg, move || {
                let st = handle.state::<book_keeper::app_state::AppState>();
                let guard = st.backup_dir.read().unwrap_or_else(|e| e.into_inner());
                guard.clone()
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            settings_controller::backup_database,
            settings_controller::get_app_config,
            maintenance_controller::reset_data,
            settings_controller::set_backup_directory,
            item_controller::create_bundle,
            item_controller::update_bundle,
            item_controller::delete_bundle,
            return_controller::create_customer_return,
            item_controller::create_item,
            party_controller::create_party,
            purchase_controller::create_purchase,
            sale_controller::create_sale,
            sale_controller::preview_sale,
            purchase_controller::preview_purchase,
            purchase_controller::preview_lot_purchase_price,
            return_controller::preview_customer_return,
            return_controller::preview_vendor_return,
            dues_controller::preview_settlement_allocation,
            adjustment_controller::preview_stock_adjustment,
            adjustment_controller::preview_set_item_stock,
            adjustment_controller::create_stock_adjustment,
            adjustment_controller::set_item_stock,
            adjustment_controller::get_item_lots,
            adjustment_controller::preview_lot_draw,
            adjustment_controller::get_item_sales,
            adjustment_controller::get_item_purchases,
            return_controller::create_vendor_return,
            item_controller::get_bundle_suggestions,
            report_controller::get_cash_flow_report,
            report_controller::get_dashboard_metrics,
            item_controller::get_items_catalog,
            item_controller::list_categories,
            report_controller::get_low_stock_alerts,
            purchase_controller::get_next_purchase_code,
            return_controller::get_next_return_code,
            sale_controller::get_next_sale_code,
            party_controller::get_party,
            dues_controller::get_party_dues_summary,
            dues_controller::get_party_ledger,
            report_controller::get_pnl_report,
            report_controller::get_recent_transactions,
            purchase_controller::get_purchase_details,
            purchase_controller::get_purchases_history,
            return_controller::get_return_details,
            return_controller::list_returns,
            sale_controller::get_sale_details,
            sale_controller::get_sales_history,
            sale_controller::update_sale,
            sale_controller::delete_sale,
            purchase_controller::update_purchase,
            purchase_controller::update_lot_purchase_price,
            purchase_controller::delete_purchase,
            return_controller::delete_return,
            correction_controller::get_bill_edit_guard,
            report_controller::get_sales_purchase_trend,
            settings_controller::get_shop_settings,
            report_controller::get_stock_valuation_report,
            import_controller::validate_items_import,
            import_controller::execute_items_import,
            import_controller::validate_vedbill_import,
            import_controller::execute_vedbill_import,
            import_controller::write_items_import_template,
            party_controller::list_parties,
            adjustment_controller::list_stock_adjustments,
            item_controller::search_items_fts,
            party_controller::search_parties,
            dues_controller::settle_bill_payment,
            dues_controller::settle_bills_batch,
            dues_controller::update_settlement,
            dues_controller::delete_settlement,
            dues_controller::get_settlement_history,
            item_controller::update_item,
            item_controller::update_item_low_stock_threshold,
            party_controller::update_party,
            settings_controller::update_shop_settings,
            settings_controller::verify_database_integrity,
            charge_type_controller::search_charge_types,
            charge_type_controller::create_charge_type,
            expense_controller::search_expense_types,
            expense_controller::list_expense_types,
            expense_controller::create_expense_type,
            expense_controller::update_expense_type,
            expense_controller::get_next_expense_code,
            expense_controller::create_expense,
            expense_controller::update_expense,
            expense_controller::delete_expense,
            expense_controller::list_expenses,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                // On-close backup; skipped when nothing changed since the last auto backup.
                if let Some(st) = app.try_state::<book_keeper::app_state::AppState>() {
                    let dir = st.backup_dir.read().unwrap_or_else(|e| e.into_inner()).clone();
                    book_keeper::services::r#impl::backup_scheduler::backup_on_exit(
                        &*st.backup_service,
                        &st.db_path,
                        &dir,
                        &st.config.backup,
                    );
                }
            }
        });
}
