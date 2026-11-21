// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use book_keeper::api::*;
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let state = book_keeper::app_state::build_sqlite_app_state(&dir.join("book_keeper.db"))?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            settings_controller::backup_database,
            item_controller::create_bundle,
            return_controller::create_customer_return,
            item_controller::create_item,
            party_controller::create_party,
            purchase_controller::create_purchase,
            sale_controller::create_sale,
            adjustment_controller::create_stock_adjustment,
            return_controller::create_vendor_return,
            item_controller::get_bundle_suggestions,
            report_controller::get_cash_flow_report,
            report_controller::get_dashboard_metrics,
            item_controller::get_items_catalog,
            report_controller::get_low_stock_alerts,
            purchase_controller::get_next_purchase_code,
            return_controller::get_next_return_code,
            sale_controller::get_next_sale_code,
            party_controller::get_party,
            dues_controller::get_party_dues_summary,
            dues_controller::get_party_ledger,
            report_controller::get_pnl_report,
            purchase_controller::get_purchase_details,
            purchase_controller::get_purchases_history,
            return_controller::get_return_details,
            sale_controller::get_sale_details,
            sale_controller::get_sales_history,
            report_controller::get_sales_purchase_trend,
            settings_controller::get_shop_settings,
            report_controller::get_stock_valuation_report,
            import_controller::import_items_from_excel,
            party_controller::list_parties,
            adjustment_controller::list_stock_adjustments,
            item_controller::search_items_fts,
            party_controller::search_parties,
            dues_controller::settle_bill_payment,
            item_controller::update_item,
            party_controller::update_party,
            settings_controller::update_shop_settings,
            settings_controller::verify_database_integrity,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
