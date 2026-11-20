// tests/e2e_scenarios_test.rs
//! Test Suite: Tier 4 Real-World Application Scenarios (S1 to S5)
//! Derived from TEST_INFRA.md § Real-World Application Scenarios and Spec Miner Report

mod common;

use common::fixtures::*;
use common::*;
use book_keeper::core::dtos::*;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::search_indexer::ItemSearchIndexer;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::accounting_service::AccountingService;
use book_keeper::services::contracts::inventory_service::InventoryService;
use book_keeper::services::contracts::migration_service::MigrationService;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::report_service::ReportService;
use book_keeper::services::contracts::return_service::ReturnService;
use book_keeper::services::contracts::sale_service::SaleService;

/// Scenario S1: Morning Inward to Evening Drawer Audit
#[test]
fn test_scenario_s1_morning_inward_to_evening_drawer_audit() {
    println!("=== EXECUTING SCENARIO S1: MORNING INWARD TO EVENING DRAWER AUDIT ===");
    let uow = MockUnitOfWork::new();
    let inventory_service = InventoryServiceImpl::new(FifoStrategyImpl, uow.clone());
    let payment_service = PaymentServiceImpl;
    let purchase_service = PurchaseServiceImpl::new(inventory_service.clone(), payment_service, uow.clone());
    let sale_service = SaleServiceImpl::new(inventory_service.clone(), payment_service, uow.clone());
    let report_service = ReportServiceImpl::new(uow.clone());

    // 1. Setup item "Nataraj Pencil" (initial stock = 0, is_low_stock = true)
    {
        let mut tx = uow.begin_transaction().unwrap();
        let item = create_test_item(1, "Nataraj Pencil", Some("Stationery"), Some("8901030111111"), 0.0, 10.0);
        tx.items().create(&item).unwrap();

        let vendor = create_test_party(1, "National Stationery", PartyType::Vendor, Some("9876543210"));
        tx.parties().create(&vendor).unwrap();
        tx.commit().unwrap();
    }

    // 2. Morning Inward Receipt: 500 Pencils @ ₹5/pc (Total = ₹2,500; Cash paid = ₹1,500; Credit = ₹1,000)
    let pur_payload = CreatePurchasePayload {
        transaction_code: Some("PUR-2001".to_string()),
        transaction_date: "2026-11-20".to_string(),
        party_id: Some(1),
        payment_mode: Some("CASH".to_string()),
        paid_amount: 1500.0,
        notes: Some("Morning bulk delivery".to_string()),
        items: vec![CreatePurchaseItemPayload {
            item_id: 1,
            bundle_id: None,
            bundle_quantity: 500.0,
            unit_cost: 5.0,
        }],
    };
    purchase_service.create_purchase(pur_payload).expect("purchase inward");

    {
        let mut tx = uow.begin_transaction().unwrap();
        let item = tx.items().find_by_id(1).unwrap().unwrap();
        assert_eq!(item.current_stock, 500.0);
        assert_eq!(item.is_low_stock, false);
    }

    // 3. Rapid Day Counter Billing: 10 sales totaling ₹4,000 (400 pcs depleted via FIFO @ ₹10)
    for i in 1..=10 {
        let sale_payload = CreateSalePayload {
            transaction_code: Some(format!("SAL-20{:02}", i)),
            transaction_date: "2026-11-20".to_string(),
            party_id: None,
            payment_mode: Some("CASH".to_string()),
            paid_amount: 400.0,
            notes: None,
            items: vec![CreateSaleItemPayload {
                item_id: 1,
                bundle_id: None,
                bundle_quantity: 40.0,
                unit_price: Some(10.0),
            }],
        };
        sale_service.create_sale(sale_payload).unwrap();
    }

    {
        let mut tx = uow.begin_transaction().unwrap();
        let item = tx.items().find_by_id(1).unwrap().unwrap();
        assert_eq!(item.current_stock, 100.0);
    }

    // 4. Physical Count Correction: 5 damaged pencils written off via negative adjustment
    let adj_payload = StockAdjustmentPayload {
        adjustment_date: "2026-11-20".to_string(),
        item_id: 1,
        quantity: -5.0,
        reason: AdjustmentReason::Damaged,
        notes: Some("Damaged in storage".to_string()),
    };
    inventory_service.create_adjustment(adj_payload).expect("negative adjustment");

    {
        let mut tx = uow.begin_transaction().unwrap();
        let item = tx.items().find_by_id(1).unwrap().unwrap();
        assert_eq!(item.current_stock, 95.0);
    }

    // 5. Evening Drawer Audit:
    let metrics = report_service.get_dashboard_metrics().unwrap();
    assert_eq!(metrics.today_sales, 4000.0);
    assert_eq!(metrics.today_purchases, 2500.0);
    assert_eq!(metrics.live_cash_in_drawer, 2500.0, "Evening drawer must equal ₹4,000 - ₹1,500 = ₹2,500");

    let valuation = report_service.get_stock_valuation_report().unwrap();
    assert_eq!(valuation.total_valuation, 475.0, "Stock valuation: 95 * 5.0 = ₹475");
    println!("=== SCENARIO S1 PASSED SUCCESSFULLY! ===");
}

/// Scenario S2: Customer Credit & Khata Cycle
#[test]
fn test_scenario_s2_customer_credit_and_khata_cycle() {
    println!("=== EXECUTING SCENARIO S2: CUSTOMER CREDIT & KHATA CYCLE ===");
    let uow = MockUnitOfWork::new();
    let inventory_service = InventoryServiceImpl::new(FifoStrategyImpl, uow.clone());
    let payment_service = PaymentServiceImpl;
    let sale_service = SaleServiceImpl::new(inventory_service.clone(), payment_service, uow.clone());
    let accounting_service = AccountingServiceImpl::new(payment_service, uow.clone());

    {
        let mut tx = uow.begin_transaction().unwrap();
        let customer = create_test_party(1, "Sharma Ji", PartyType::Customer, Some("9876500000"));
        tx.parties().create(&customer).unwrap();

        let item = create_test_item(1, "Classmate Notebook", None, None, 50.0, 60.0);
        tx.items().create(&item).unwrap();

        let pur = create_test_purchase_tx(1, "PUR-1", None, 1500.0, 1500.0, "2026-11-20");
        tx.purchases().create_transaction(&pur).unwrap();
        let lot = create_test_purchase_lot(1, 1, 1, None, 50.0, 50.0, 30.0, 50.0);
        tx.purchases().create_purchase_item(&lot).unwrap();
        tx.commit().unwrap();
    }

    let sale_payload = CreateSalePayload {
        transaction_code: Some("SAL-3001".to_string()),
        transaction_date: "2026-11-20".to_string(),
        party_id: Some(1),
        payment_mode: None,
        paid_amount: 0.0,
        notes: Some("Monthly school credit".to_string()),
        items: vec![CreateSaleItemPayload {
            item_id: 1,
            bundle_id: None,
            bundle_quantity: 20.0,
            unit_price: Some(60.0),
        }],
    };
    sale_service.create_sale(sale_payload).unwrap();

    let dues = accounting_service.get_party_dues_summary(PartyType::Customer).unwrap();
    assert_eq!(dues.len(), 1);
    assert_eq!(dues[0].party_id, 1);
    assert_eq!(dues[0].total_due, 1200.0);

    let settle_payload = SettlePaymentPayload {
        party_id: 1,
        amount: 500.0,
        settlement_date: "2026-11-23".to_string(),
        payment_mode: Some("CASH".to_string()),
        specific_bill_id: None,
        notes: Some("Installment payment".to_string()),
    };
    accounting_service.settle_bill_payment(settle_payload).unwrap();

    let dues_after = accounting_service.get_party_dues_summary(PartyType::Customer).unwrap();
    assert_eq!(dues_after[0].total_due, 700.0);

    {
        let mut tx = uow.begin_transaction().unwrap();
        let sale = tx.sales().find_by_id(1).unwrap().unwrap();
        assert_eq!(sale.balance_due, 700.0);
        assert_eq!(sale.payment_status, PaymentStatus::Partial);
    }

    println!("=== SCENARIO S2 PASSED SUCCESSFULLY! ===");
}

/// Scenario S3: Restockable Return & Defective Replacement
#[test]
fn test_scenario_s3_restockable_return_and_defective_replacement() {
    println!("=== EXECUTING SCENARIO S3: RESTOCKABLE RETURN & DEFECTIVE REPLACEMENT ===");
    let uow = MockUnitOfWork::new();
    let inventory_service = InventoryServiceImpl::new(FifoStrategyImpl, uow.clone());
    let payment_service = PaymentServiceImpl;
    let sale_service = SaleServiceImpl::new(inventory_service.clone(), payment_service, uow.clone());
    let return_service = ReturnServiceImpl::new(inventory_service.clone(), payment_service, uow.clone());

    {
        let mut tx = uow.begin_transaction().unwrap();
        let item = create_test_item(1, "Casio Calculator", None, None, 5.0, 300.0);
        tx.items().create(&item).unwrap();

        let pur = create_test_purchase_tx(1, "PUR-1", None, 1000.0, 1000.0, "2026-11-20");
        tx.purchases().create_transaction(&pur).unwrap();
        let lot = create_test_purchase_lot(1, 1, 1, None, 5.0, 5.0, 200.0, 5.0);
        tx.purchases().create_purchase_item(&lot).unwrap();
        tx.commit().unwrap();
    }

    let sale_payload = CreateSalePayload {
        transaction_code: Some("SAL-4001".to_string()),
        transaction_date: "2026-11-20".to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        paid_amount: 600.0,
        notes: None,
        items: vec![CreateSaleItemPayload {
            item_id: 1,
            bundle_id: None,
            bundle_quantity: 2.0,
            unit_price: Some(300.0),
        }],
    };
    sale_service.create_sale(sale_payload).unwrap();

    {
        let mut tx = uow.begin_transaction().unwrap();
        let item = tx.items().find_by_id(1).unwrap().unwrap();
        assert_eq!(item.current_stock, 3.0);
    }

    let return_payload = CustomerReturnPayload {
        return_code: Some("RET-4001".to_string()),
        return_date: "2026-11-20".to_string(),
        party_id: None,
        original_sale_id: Some(1),
        settlement_type: SettlementType::CashRefund,
        reason: Some("Unused in packaging".to_string()),
        notes: None,
        items: vec![CustomerReturnItemPayload {
            item_id: 1,
            sale_item_id: Some(1),
            quantity: 1.0,
            unit_price: 300.0,
            condition: StockCondition::Restockable,
        }],
    };
    return_service.create_customer_return(return_payload).unwrap();

    {
        let mut tx = uow.begin_transaction().unwrap();
        let item = tx.items().find_by_id(1).unwrap().unwrap();
        assert_eq!(item.current_stock, 4.0);

        let lots = tx.purchases().list_active_lots_for_item(1).unwrap();
        assert_eq!(lots[0].remaining_quantity, 4.0);
    }

    let swap_payload = CustomerReturnPayload {
        return_code: Some("RET-4002".to_string()),
        return_date: "2026-11-20".to_string(),
        party_id: None,
        original_sale_id: Some(1),
        settlement_type: SettlementType::DefectiveReplacement,
        reason: Some("Keypad unresponsive".to_string()),
        notes: None,
        items: vec![CustomerReturnItemPayload {
            item_id: 1,
            sale_item_id: Some(1),
            quantity: 1.0,
            unit_price: 300.0,
            condition: StockCondition::DamagedScrap,
        }],
    };
    return_service.create_customer_return(swap_payload).unwrap();

    {
        let mut tx = uow.begin_transaction().unwrap();
        let item = tx.items().find_by_id(1).unwrap().unwrap();
        assert_eq!(item.current_stock, 4.0);
    }

    println!("=== SCENARIO S3 PASSED SUCCESSFULLY! ===");
}

/// Scenario S4: Bulk Catalog Import & Instant Barcode Billing
#[test]
fn test_scenario_s4_bulk_catalog_import_and_instant_barcode_billing() {
    println!("=== EXECUTING SCENARIO S4: BULK CATALOG & INSTANT BARCODE BILLING ===");
    let uow = MockUnitOfWork::new();
    let sqlite_conn = std::sync::Arc::new(std::sync::Mutex::new(common::setup_in_memory_db().unwrap()));
    let indexer = SqliteSearchIndexer::new(sqlite_conn, uow.clone());
    let inventory_service = InventoryServiceImpl::new(FifoStrategyImpl, uow.clone());
    let payment_service = PaymentServiceImpl;
    let sale_service = SaleServiceImpl::new(inventory_service, payment_service, uow.clone());

    {
        let mut tx = uow.begin_transaction().unwrap();
        for i in 1..=100 {
            let barcode = format!("89010300{:05}", i);
            let item = create_test_item(i, &format!("Stationery Product {}", i), Some("Office"), Some(&barcode), 50.0, 25.0);
            tx.items().create(&item).unwrap();
            indexer.index_item(&item).unwrap();

            let pur = create_test_purchase_tx(i, &format!("PUR-{}", i), None, 500.0, 500.0, "2026-11-20");
            tx.purchases().create_transaction(&pur).unwrap();
            let lot = create_test_purchase_lot(i, i, i, None, 50.0, 50.0, 10.0, 50.0);
            tx.purchases().create_purchase_item(&lot).unwrap();
        }
        tx.commit().unwrap();
    }

    let scan_query = "8901030000042";
    let matches = indexer.search(scan_query).expect("barcode lookup");
    assert!(!matches.is_empty(), "Scanned item must resolve immediately");
    assert_eq!(matches[0].item_id, 42);
    assert_eq!(matches[0].default_sale_price, 25.0);

    let sale_payload = CreateSalePayload {
        transaction_code: Some("SAL-5001".to_string()),
        transaction_date: "2026-11-20".to_string(),
        party_id: None,
        payment_mode: Some("CASH".to_string()),
        paid_amount: 100.0,
        notes: None,
        items: vec![CreateSaleItemPayload {
            item_id: 42,
            bundle_id: None,
            bundle_quantity: 4.0,
            unit_price: Some(25.0),
        }],
    };
    let summary = sale_service.create_sale(sale_payload).expect("sale succeeds");
    assert_eq!(summary.total_amount, 100.0);
    assert_eq!(summary.payment_status, PaymentStatus::Paid);

    println!("=== SCENARIO S4 PASSED SUCCESSFULLY! ===");
}

fn create_scenario_vb_archive(archive_path: &str, company_name: &str, item_count: usize, party_count: usize) {
    let temp_db_path = std::env::temp_dir().join(format!("scen_src_{}.db", uuid::Uuid::new_v4()));
    {
        let conn = rusqlite::Connection::open(&temp_db_path).expect("open temp sqlite");
        conn.execute_batch(r#"
            CREATE TABLE items (id TEXT PRIMARY KEY, name TEXT, salePrice REAL, purchasePrice REAL, mrp REAL, primaryQuantity REAL);
            CREATE TABLE parties (id TEXT PRIMARY KEY, name TEXT, phone TEXT, type TEXT);
            CREATE TABLE documents (id TEXT PRIMARY KEY, documentType TEXT, documentNumber TEXT, documentDate TEXT, total REAL);
            CREATE TABLE document_items (id TEXT PRIMARY KEY, documentId TEXT, itemId TEXT, quantity REAL, rate REAL, amount REAL);
        "#).expect("create vdbill tables");

        for i in 1..=item_count {
            conn.execute(
                "INSERT INTO items (id, name, salePrice, purchasePrice, mrp, primaryQuantity) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![format!("itm-{}", i), format!("Migrated Item {}", i), 20.0, 10.0, 25.0, 10.0],
            ).expect("insert test item");
        }

        for i in 1..=party_count {
            conn.execute(
                "INSERT INTO parties (id, name, phone, type) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![format!("pty-{}", i), format!("Migrated Party {}", i), "9876543210", "customer"],
            ).expect("insert test party");
        }
    }

    use std::fs::File;
    use std::io::{Read, Write};
    let file = File::create(archive_path).expect("create zip file");
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("company_manifest.json", options).unwrap();
    let manifest_json = format!(
        r#"{{"company_name":"{}","version":"2.4","backup_date":"2026-11-14"}}"#,
        company_name
    );
    zip.write_all(manifest_json.as_bytes()).unwrap();

    zip.start_file("company.db", options).unwrap();
    let mut db_bytes = Vec::new();
    File::open(&temp_db_path).unwrap().read_to_end(&mut db_bytes).unwrap();
    zip.write_all(&db_bytes).unwrap();

    zip.finish().unwrap();
    let _ = std::fs::remove_file(&temp_db_path);
}

/// Scenario S5: Legacy VedBill Cutover & Reconciliation
#[test]
fn test_scenario_s5_legacy_vedbill_cutover_and_reconciliation() {
    println!("=== EXECUTING SCENARIO S5: VEDBILL CUTOVER & RECONCILIATION ===");
    let uow = MockUnitOfWork::new();
    let migration_service = MigrationServiceImpl::new(uow.clone());

    let archive_path = std::env::temp_dir().join(format!("vedbill_cutover_test_{}.vb", uuid::Uuid::new_v4()));
    let archive_path_str = archive_path.to_str().unwrap();
    create_scenario_vb_archive(archive_path_str, "Gupta Stationery Mart", 166, 12);

    let preview = migration_service.inspect_backup_file(archive_path_str).unwrap();
    assert_eq!(preview.table_stats.items_count, 166);
    assert_eq!(preview.table_stats.parties_count, 12);

    let result = migration_service.execute_migration(archive_path_str).unwrap();
    assert!(result.success);
    assert_eq!(result.items_imported, 166);
    assert_eq!(result.parties_imported, 12);

    let mut tx = uow.begin_transaction().unwrap();
    let catalog = tx.items().list_catalog(None, false, 0, 200).unwrap();
    assert_eq!(catalog.len(), 166);

    let _ = std::fs::remove_file(archive_path);
    println!("=== SCENARIO S5 PASSED SUCCESSFULLY! ===");
}
