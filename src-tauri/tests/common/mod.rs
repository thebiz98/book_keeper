// tests/common/mod.rs
//! Shared test harness, in-memory SQLite schema, mock repositories,
//! and complete domain service test doubles for Book Keeper backend test suites.
//! Built according to Book Keeper LLD Sections 3, 4, 5, 6, and 11.

#![allow(dead_code, unused_imports, unused_variables)]

pub mod fixtures;

use std::collections::HashMap;
use std::fs::File;
use std::sync::{Arc, Mutex};
use rusqlite::Connection;

use book_keeper::core::dtos::*;
use book_keeper::core::entities::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::search_indexer::ItemSearchIndexer;
use book_keeper::data::contracts::unit_of_work::{TransactionContext, UnitOfWork};
use book_keeper::data::contracts::*;
use book_keeper::services::contracts::accounting_service::AccountingService;
use book_keeper::services::contracts::inventory_allocation_strategy::{
    AdjustmentDepletionResult, DepletionResult, InventoryAllocationStrategy,
};
use book_keeper::services::contracts::inventory_service::InventoryService;
use book_keeper::services::contracts::migration_service::MigrationService;
use book_keeper::services::contracts::payment_service::PaymentService;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::report_service::ReportService;
use book_keeper::services::contracts::return_service::ReturnService;
use book_keeper::services::contracts::sale_service::SaleService;

/// SQL Schema for all 15 SQLite tables + FTS5 virtual table
pub const SCHEMA_DDL: &str = r#"
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS ITEM (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    category TEXT,
    barcode TEXT,
    base_unit TEXT NOT NULL DEFAULT 'PCS',
    mrp REAL,
    default_sale_price REAL NOT NULL DEFAULT 0.0,
    current_stock REAL NOT NULL DEFAULT 0.0,
    is_low_stock INTEGER NOT NULL DEFAULT 0,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS ITEM_BUNDLE (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id INTEGER NOT NULL,
    bundle_name TEXT NOT NULL,
    units_per_bundle REAL NOT NULL,
    mrp REAL,
    default_bundle_price REAL,
    barcode TEXT,
    FOREIGN KEY (item_id) REFERENCES ITEM(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS PARTY (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    phone TEXT,
    party_type TEXT NOT NULL,
    notes TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS PURCHASE_TRANSACTION (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    transaction_code TEXT NOT NULL UNIQUE,
    transaction_date TEXT NOT NULL,
    party_id INTEGER,
    payment_mode TEXT,
    payment_status TEXT NOT NULL,
    total_amount REAL NOT NULL,
    paid_amount REAL NOT NULL DEFAULT 0.0,
    balance_due REAL NOT NULL DEFAULT 0.0,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (party_id) REFERENCES PARTY(id)
);

CREATE TABLE IF NOT EXISTS PURCHASE_ITEM (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    purchase_transaction_id INTEGER NOT NULL,
    item_id INTEGER NOT NULL,
    bundle_id INTEGER,
    bundle_quantity REAL NOT NULL,
    quantity REAL NOT NULL,
    unit_cost REAL NOT NULL,
    total_amount REAL NOT NULL,
    remaining_quantity REAL NOT NULL,
    FOREIGN KEY (purchase_transaction_id) REFERENCES PURCHASE_TRANSACTION(id) ON DELETE CASCADE,
    FOREIGN KEY (item_id) REFERENCES ITEM(id),
    FOREIGN KEY (bundle_id) REFERENCES ITEM_BUNDLE(id)
);

CREATE TABLE IF NOT EXISTS SALE_TRANSACTION (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    transaction_code TEXT NOT NULL UNIQUE,
    transaction_date TEXT NOT NULL,
    party_id INTEGER,
    payment_mode TEXT,
    payment_status TEXT NOT NULL,
    total_amount REAL NOT NULL,
    paid_amount REAL NOT NULL DEFAULT 0.0,
    balance_due REAL NOT NULL DEFAULT 0.0,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (party_id) REFERENCES PARTY(id)
);

CREATE TABLE IF NOT EXISTS SALE_ITEM (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sale_transaction_id INTEGER NOT NULL,
    item_id INTEGER NOT NULL,
    bundle_id INTEGER,
    bundle_quantity REAL NOT NULL,
    quantity REAL NOT NULL,
    unit_price REAL NOT NULL,
    total_amount REAL NOT NULL,
    FOREIGN KEY (sale_transaction_id) REFERENCES SALE_TRANSACTION(id) ON DELETE CASCADE,
    FOREIGN KEY (item_id) REFERENCES ITEM(id),
    FOREIGN KEY (bundle_id) REFERENCES ITEM_BUNDLE(id)
);

CREATE TABLE IF NOT EXISTS SALE_STOCK_ALLOCATION (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sale_item_id INTEGER NOT NULL,
    purchase_item_id INTEGER NOT NULL,
    quantity_depleted REAL NOT NULL,
    unit_cost REAL NOT NULL,
    FOREIGN KEY (sale_item_id) REFERENCES SALE_ITEM(id) ON DELETE CASCADE,
    FOREIGN KEY (purchase_item_id) REFERENCES PURCHASE_ITEM(id)
);

CREATE TABLE IF NOT EXISTS RETURN_TRANSACTION (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    return_code TEXT NOT NULL UNIQUE,
    return_date TEXT NOT NULL,
    return_type TEXT NOT NULL,
    party_id INTEGER,
    original_transaction_id INTEGER,
    settlement_type TEXT NOT NULL,
    total_amount REAL NOT NULL,
    reason TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (party_id) REFERENCES PARTY(id)
);

CREATE TABLE IF NOT EXISTS RETURN_ITEM (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    return_transaction_id INTEGER NOT NULL,
    item_id INTEGER NOT NULL,
    sale_item_id INTEGER,
    purchase_item_id INTEGER,
    quantity REAL NOT NULL,
    unit_price REAL NOT NULL,
    total_amount REAL NOT NULL,
    condition TEXT NOT NULL,
    FOREIGN KEY (return_transaction_id) REFERENCES RETURN_TRANSACTION(id) ON DELETE CASCADE,
    FOREIGN KEY (item_id) REFERENCES ITEM(id),
    FOREIGN KEY (sale_item_id) REFERENCES SALE_ITEM(id),
    FOREIGN KEY (purchase_item_id) REFERENCES PURCHASE_ITEM(id)
);

CREATE TABLE IF NOT EXISTS PAYMENT_SETTLEMENT (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    settlement_date TEXT NOT NULL,
    party_id INTEGER,
    transaction_type TEXT NOT NULL,
    transaction_id INTEGER NOT NULL,
    amount_paid REAL NOT NULL,
    payment_mode TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (party_id) REFERENCES PARTY(id)
);

CREATE TABLE IF NOT EXISTS STOCK_ADJUSTMENT (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    adjustment_date TEXT NOT NULL,
    item_id INTEGER NOT NULL,
    quantity REAL NOT NULL,
    reason TEXT NOT NULL,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (item_id) REFERENCES ITEM(id)
);

CREATE TABLE IF NOT EXISTS ADJUSTMENT_STOCK_ALLOCATION (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    stock_adjustment_id INTEGER NOT NULL,
    purchase_item_id INTEGER NOT NULL,
    quantity_depleted REAL NOT NULL,
    unit_cost REAL NOT NULL,
    FOREIGN KEY (stock_adjustment_id) REFERENCES STOCK_ADJUSTMENT(id) ON DELETE CASCADE,
    FOREIGN KEY (purchase_item_id) REFERENCES PURCHASE_ITEM(id)
);

CREATE TABLE IF NOT EXISTS SHOP_SETTING (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    shop_name TEXT NOT NULL,
    phone TEXT,
    address TEXT,
    tax_id TEXT
);

CREATE TABLE IF NOT EXISTS APP_MIGRATION_HISTORY (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE VIRTUAL TABLE IF NOT EXISTS ITEM_SEARCH_FTS USING fts5(
    item_id UNINDEXED,
    name,
    category,
    barcode,
    tokenize='trigram'
);
"#;

pub fn setup_in_memory_db() -> rusqlite::Result<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(SCHEMA_DDL)?;
    Ok(conn)
}

// Storage State
#[derive(Default, Clone)]
pub struct MockStorageState {
    pub items: Arc<Mutex<HashMap<i64, Item>>>,
    pub bundles: Arc<Mutex<HashMap<i64, ItemBundle>>>,
    pub parties: Arc<Mutex<HashMap<i64, Party>>>,
    pub purchases: Arc<Mutex<HashMap<i64, PurchaseTransaction>>>,
    pub purchase_items: Arc<Mutex<HashMap<i64, PurchaseItem>>>,
    pub sales: Arc<Mutex<HashMap<i64, SaleTransaction>>>,
    pub sale_items: Arc<Mutex<HashMap<i64, SaleItem>>>,
    pub allocations: Arc<Mutex<HashMap<i64, SaleStockAllocation>>>,
    pub settlements: Arc<Mutex<HashMap<i64, PaymentSettlement>>>,
    pub returns: Arc<Mutex<HashMap<i64, ReturnTransaction>>>,
    pub return_items: Arc<Mutex<HashMap<i64, ReturnItem>>>,
    pub adjustments: Arc<Mutex<HashMap<i64, StockAdjustment>>>,
    pub adjustment_allocations: Arc<Mutex<HashMap<i64, AdjustmentStockAllocation>>>,
    pub settings: Arc<Mutex<Option<ShopSetting>>>,
}

impl MockStorageState {
    pub fn new() -> Self {
        Self::default()
    }
}

// Concrete Repositories
pub struct MockItemRepository { pub staged: Arc<Mutex<HashMap<i64, Item>>> }
impl ItemRepository for MockItemRepository {
    fn find_by_id(&self, id: i64) -> Result<Option<Item>, AppError> {
        let map = self.staged.lock().unwrap();
        Ok(map.get(&id).cloned())
    }
    fn find_by_name(&self, name: &str) -> Result<Option<Item>, AppError> {
        let map = self.staged.lock().unwrap();
        Ok(map.values().find(|i| i.name.eq_ignore_ascii_case(name)).cloned())
    }
    fn find_by_barcode(&self, barcode: &str) -> Result<Option<Item>, AppError> {
        let map = self.staged.lock().unwrap();
        Ok(map.values().find(|i| i.barcode.as_deref() == Some(barcode)).cloned())
    }
    fn list_catalog(&self, category: Option<&str>, low_stock_only: bool, offset: usize, limit: usize) -> Result<Vec<Item>, AppError> {
        let map = self.staged.lock().unwrap();
        let items: Vec<Item> = map.values()
            .filter(|i| i.is_active)
            .filter(|i| category.map_or(true, |c| i.category.as_deref() == Some(c)))
            .filter(|i| !low_stock_only || i.is_low_stock)
            .skip(offset)
            .take(limit)
            .cloned()
            .collect();
        Ok(items)
    }
    fn create(&mut self, item: &Item) -> Result<i64, AppError> {
        let mut map = self.staged.lock().unwrap();
        let id = if item.id > 0 { item.id } else { (map.len() as i64) + 1 };
        let mut cloned = item.clone();
        cloned.id = id;
        map.insert(id, cloned);
        Ok(id)
    }
    fn update(&mut self, item: &Item) -> Result<(), AppError> {
        let mut map = self.staged.lock().unwrap();
        map.insert(item.id, item.clone());
        Ok(())
    }
    fn update_stock_cache(&mut self, item_id: i64, new_stock: f64, is_low_stock: bool) -> Result<(), AppError> {
        let mut map = self.staged.lock().unwrap();
        if let Some(item) = map.get_mut(&item_id) {
            item.current_stock = new_stock;
            item.is_low_stock = is_low_stock;
            Ok(())
        } else {
            Err(AppError::NotFound { entity: "Item".to_string(), id: item_id.to_string() })
        }
    }
    fn archive(&mut self, item_id: i64) -> Result<(), AppError> {
        let mut map = self.staged.lock().unwrap();
        if let Some(item) = map.get_mut(&item_id) {
            item.is_active = false;
            Ok(())
        } else {
            Err(AppError::NotFound { entity: "Item".to_string(), id: item_id.to_string() })
        }
    }
}

pub struct MockBundleRepository { pub staged: Arc<Mutex<HashMap<i64, ItemBundle>>> }
impl BundleRepository for MockBundleRepository {
    fn find_by_id(&self, id: i64) -> Result<Option<ItemBundle>, AppError> {
        let map = self.staged.lock().unwrap();
        Ok(map.get(&id).cloned())
    }
    fn find_by_item_id(&self, item_id: i64) -> Result<Vec<ItemBundle>, AppError> {
        let map = self.staged.lock().unwrap();
        Ok(map.values().filter(|b| b.item_id == item_id).cloned().collect())
    }
    fn list_by_item_id(&self, item_id: i64) -> Result<Vec<ItemBundle>, AppError> {
        self.find_by_item_id(item_id)
    }
    fn find_by_barcode(&self, barcode: &str) -> Result<Option<ItemBundle>, AppError> {
        let map = self.staged.lock().unwrap();
        Ok(map.values().find(|b| b.barcode.as_deref() == Some(barcode)).cloned())
    }
    fn create(&mut self, bundle: &ItemBundle) -> Result<i64, AppError> {
        let mut map = self.staged.lock().unwrap();
        let id = if bundle.id > 0 { bundle.id } else { (map.len() as i64) + 1 };
        let mut cloned = bundle.clone();
        cloned.id = id;
        map.insert(id, cloned);
        Ok(id)
    }
    fn update(&mut self, bundle: &ItemBundle) -> Result<(), AppError> {
        let mut map = self.staged.lock().unwrap();
        map.insert(bundle.id, bundle.clone());
        Ok(())
    }
    fn delete(&mut self, id: i64) -> Result<(), AppError> {
        let mut map = self.staged.lock().unwrap();
        map.remove(&id);
        Ok(())
    }
    fn get_bundle_suggestions(&self) -> Result<Vec<BundleSuggestion>, AppError> {
        let map = self.staged.lock().unwrap();
        let mut counts: HashMap<(String, u64), usize> = HashMap::new();
        for b in map.values() {
            let key = (b.bundle_name.clone(), b.units_per_bundle.round() as u64);
            *counts.entry(key).or_insert(0) += 1;
        }
        let mut suggestions: Vec<BundleSuggestion> = counts.into_iter()
            .map(|((name, units), count)| BundleSuggestion {
                bundle_name: name,
                units_per_bundle: units as f64,
                usage_count: count as i64,
            })
            .collect();
        suggestions.sort_by(|a, b| b.usage_count.cmp(&a.usage_count));
        Ok(suggestions)
    }
    fn list_suggestions(&self) -> Result<Vec<BundleSuggestion>, AppError> {
        self.get_bundle_suggestions()
    }
}

pub struct MockPartyRepository { pub staged: Arc<Mutex<HashMap<i64, Party>>> }
impl PartyRepository for MockPartyRepository {
    fn find_by_id(&self, id: i64) -> Result<Option<Party>, AppError> {
        let map = self.staged.lock().unwrap();
        Ok(map.get(&id).cloned())
    }
    fn list(&self, party_type: Option<PartyType>, active_only: bool) -> Result<Vec<Party>, AppError> {
        let map = self.staged.lock().unwrap();
        Ok(map.values()
            .filter(|p| party_type.as_ref().map_or(true, |pt| &p.party_type == pt))
            .filter(|p| !active_only || p.is_active)
            .cloned()
            .collect())
    }
    fn list_by_type(&self, party_type: PartyType, active_only: bool) -> Result<Vec<Party>, AppError> {
        self.list(Some(party_type), active_only)
    }
    fn create(&mut self, party: &Party) -> Result<i64, AppError> {
        let mut map = self.staged.lock().unwrap();
        let id = if party.id > 0 { party.id } else { (map.len() as i64) + 1 };
        let mut cloned = party.clone();
        cloned.id = id;
        map.insert(id, cloned);
        Ok(id)
    }
    fn update(&mut self, party: &Party) -> Result<(), AppError> {
        let mut map = self.staged.lock().unwrap();
        map.insert(party.id, party.clone());
        Ok(())
    }
    fn archive(&mut self, id: i64) -> Result<(), AppError> {
        let mut map = self.staged.lock().unwrap();
        if let Some(p) = map.get_mut(&id) {
            p.is_active = false;
            Ok(())
        } else {
            Err(AppError::NotFound { entity: "Party".to_string(), id: id.to_string() })
        }
    }
}

pub struct MockPurchaseRepository {
    pub staged: Arc<Mutex<HashMap<i64, PurchaseTransaction>>>,
    pub staged_items: Arc<Mutex<HashMap<i64, PurchaseItem>>>,
}
impl MockPurchaseRepository {
    pub fn find_purchase_item_by_id(&self, id: i64) -> Result<Option<PurchaseItem>, AppError> {
        let map = self.staged_items.lock().unwrap();
        Ok(map.get(&id).cloned())
    }
}
impl PurchaseRepository for MockPurchaseRepository {
    fn create_transaction(&mut self, tx: &PurchaseTransaction) -> Result<i64, AppError> {
        let mut map = self.staged.lock().unwrap();
        let id = if tx.id > 0 { tx.id } else { (map.len() as i64) + 1 };
        let mut cloned = tx.clone();
        cloned.id = id;
        map.insert(id, cloned);
        Ok(id)
    }
    fn create_purchase_item(&mut self, line: &PurchaseItem) -> Result<i64, AppError> {
        let mut map = self.staged_items.lock().unwrap();
        let id = if line.id > 0 { line.id } else { (map.len() as i64) + 1 };
        let mut cloned = line.clone();
        cloned.id = id;
        map.insert(id, cloned);
        Ok(id)
    }
    fn find_transaction_by_id(&self, id: i64) -> Result<Option<PurchaseTransaction>, AppError> {
        let map = self.staged.lock().unwrap();
        Ok(map.get(&id).cloned())
    }
    fn find_by_id(&self, id: i64) -> Result<Option<PurchaseTransaction>, AppError> {
        self.find_transaction_by_id(id)
    }
    fn find_by_code(&self, code: &str) -> Result<Option<PurchaseTransaction>, AppError> {
        let map = self.staged.lock().unwrap();
        Ok(map.values().find(|p| p.transaction_code == code).cloned())
    }
    fn get_next_transaction_code(&self) -> Result<String, AppError> {
        let map = self.staged.lock().unwrap();
        let max_num = map.values()
            .filter_map(|p| {
                if p.transaction_code.starts_with("PUR-") {
                    p.transaction_code[4..].parse::<u64>().ok()
                } else {
                    None
                }
            })
            .max()
            .unwrap_or(1000);
        Ok(format!("PUR-{}", max_num + 1))
    }
    fn get_latest_code(&self) -> Result<Option<String>, AppError> {
        let map = self.staged.lock().unwrap();
        Ok(map.values().map(|p| p.transaction_code.clone()).max())
    }
    fn get_active_lots_for_item(&self, item_id: i64) -> Result<Vec<PurchaseItem>, AppError> {
        let map = self.staged_items.lock().unwrap();
        let mut lots: Vec<PurchaseItem> = map.values()
            .filter(|p| p.item_id == item_id && p.remaining_quantity > 0.0)
            .cloned()
            .collect();
        lots.sort_by_key(|l| l.id); // FIFO order
        Ok(lots)
    }
    fn list_active_lots_for_item(&self, item_id: i64) -> Result<Vec<PurchaseItem>, AppError> {
        self.get_active_lots_for_item(item_id)
    }
    fn update_remaining_quantity(&mut self, purchase_item_id: i64, new_remaining: f64) -> Result<(), AppError> {
        let mut map = self.staged_items.lock().unwrap();
        if let Some(lot) = map.get_mut(&purchase_item_id) {
            lot.remaining_quantity = new_remaining;
            Ok(())
        } else {
            Err(AppError::NotFound { entity: "PurchaseItem".to_string(), id: purchase_item_id.to_string() })
        }
    }
    fn update_balance_due(&mut self, purchase_id: i64, new_balance: f64, status: PaymentStatus) -> Result<(), AppError> {
        let mut map = self.staged.lock().unwrap();
        if let Some(tx) = map.get_mut(&purchase_id) {
            let reduction = (tx.balance_due - new_balance).max(0.0);
            tx.paid_amount += reduction;
            tx.balance_due = new_balance;
            tx.payment_status = status;
            Ok(())
        } else {
            Err(AppError::NotFound { entity: "PurchaseTransaction".to_string(), id: purchase_id.to_string() })
        }
    }
    fn list_items_by_transaction_id(&self, tx_id: i64) -> Result<Vec<PurchaseItem>, AppError> {
        let map = self.staged_items.lock().unwrap();
        let mut items: Vec<PurchaseItem> = map.values().filter(|p| p.purchase_transaction_id == tx_id).cloned().collect();
        items.sort_by_key(|i| i.id);
        Ok(items)
    }
    fn get_latest_purchase_item_for_item(&self, item_id: i64) -> Result<Option<PurchaseItem>, AppError> {
        let map = self.staged_items.lock().unwrap();
        let mut lots: Vec<PurchaseItem> = map.values().filter(|p| p.item_id == item_id).cloned().collect();
        lots.sort_by_key(|l| l.id);
        Ok(lots.pop())
    }
    fn list_purchases(
        &self,
        start_date: Option<&str>,
        end_date: Option<&str>,
        status: Option<PaymentStatus>,
        party_id: Option<i64>,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<PurchaseTransaction>, AppError> {
        let map = self.staged.lock().unwrap();
        let mut list: Vec<PurchaseTransaction> = map.values()
            .filter(|p| party_id.map_or(true, |pid| p.party_id == Some(pid)))
            .filter(|p| status.as_ref().map_or(true, |st| &p.payment_status == st))
            .filter(|p| start_date.map_or(true, |sd| p.transaction_date.as_str() >= sd))
            .filter(|p| end_date.map_or(true, |ed| p.transaction_date.as_str() <= ed))
            .cloned()
            .collect();
        list.sort_by_key(|p| p.id);
        Ok(list.into_iter().skip(offset).take(limit).collect())
    }
    fn list_transactions(&self, filter: &PurchaseFilter) -> Result<PaginatedList<PurchaseTransaction>, AppError> {
        let from_date = filter.from_date.as_deref().or(filter.start_date.as_deref());
        let to_date = filter.to_date.as_deref().or(filter.end_date.as_deref());
        let page = filter.page.unwrap_or(1);
        let limit = filter.limit.unwrap_or(50);
        let offset = page.saturating_sub(1) * limit;

        let all_filtered = self.list_purchases(from_date, to_date, filter.payment_status, filter.party_id, 0, usize::MAX)?;
        let total = all_filtered.len();
        let items: Vec<PurchaseTransaction> = all_filtered.into_iter().skip(offset).take(limit).collect();
        Ok(PaginatedList::new(items, total, page, limit))
    }
}

pub struct MockSaleRepository {
    pub staged: Arc<Mutex<HashMap<i64, SaleTransaction>>>,
    pub staged_items: Arc<Mutex<HashMap<i64, SaleItem>>>,
}
impl SaleRepository for MockSaleRepository {
    fn create_transaction(&mut self, tx: &SaleTransaction) -> Result<i64, AppError> {
        let mut map = self.staged.lock().unwrap();
        let id = if tx.id > 0 { tx.id } else { (map.len() as i64) + 1 };
        let mut cloned = tx.clone();
        cloned.id = id;
        map.insert(id, cloned);
        Ok(id)
    }
    fn create_sale_item(&mut self, item: &SaleItem) -> Result<i64, AppError> {
        let mut map = self.staged_items.lock().unwrap();
        let id = if item.id > 0 { item.id } else { (map.len() as i64) + 1 };
        let mut cloned = item.clone();
        cloned.id = id;
        map.insert(id, cloned);
        Ok(id)
    }
    fn find_transaction_by_id(&self, id: i64) -> Result<Option<SaleTransaction>, AppError> {
        let map = self.staged.lock().unwrap();
        Ok(map.get(&id).cloned())
    }
    fn find_by_id(&self, id: i64) -> Result<Option<SaleTransaction>, AppError> {
        self.find_transaction_by_id(id)
    }
    fn find_by_code(&self, code: &str) -> Result<Option<SaleTransaction>, AppError> {
        let map = self.staged.lock().unwrap();
        Ok(map.values().find(|s| s.transaction_code == code).cloned())
    }
    fn get_next_transaction_code(&self) -> Result<String, AppError> {
        let map = self.staged.lock().unwrap();
        let max_num = map.values()
            .filter_map(|s| {
                if s.transaction_code.starts_with("SAL-") {
                    s.transaction_code[4..].parse::<u64>().ok()
                } else {
                    None
                }
            })
            .max()
            .unwrap_or(1000);
        Ok(format!("SAL-{}", max_num + 1))
    }
    fn get_latest_code(&self) -> Result<Option<String>, AppError> {
        let map = self.staged.lock().unwrap();
        Ok(map.values().map(|s| s.transaction_code.clone()).max())
    }
    fn update_balance_due(&mut self, sale_id: i64, new_balance: f64, status: PaymentStatus) -> Result<(), AppError> {
        let mut map = self.staged.lock().unwrap();
        if let Some(tx) = map.get_mut(&sale_id) {
            let reduction = (tx.balance_due - new_balance).max(0.0);
            tx.paid_amount += reduction;
            tx.balance_due = new_balance;
            tx.payment_status = status;
            Ok(())
        } else {
            Err(AppError::NotFound { entity: "SaleTransaction".to_string(), id: sale_id.to_string() })
        }
    }
    fn list_items_by_transaction_id(&self, tx_id: i64) -> Result<Vec<SaleItem>, AppError> {
        let map = self.staged_items.lock().unwrap();
        let mut items: Vec<SaleItem> = map.values().filter(|s| s.sale_transaction_id == tx_id).cloned().collect();
        items.sort_by_key(|i| i.id);
        Ok(items)
    }
    fn list_unpaid_by_party_id(&self, party_id: i64) -> Result<Vec<SaleTransaction>, AppError> {
        let map = self.staged.lock().unwrap();
        let mut sales: Vec<SaleTransaction> = map.values()
            .filter(|s| s.party_id == Some(party_id) && s.balance_due > 0.0)
            .cloned()
            .collect();
        sales.sort_by_key(|s| s.id);
        Ok(sales)
    }
    fn list_sales(
        &self,
        start_date: Option<&str>,
        end_date: Option<&str>,
        status: Option<PaymentStatus>,
        party_id: Option<i64>,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SaleTransaction>, AppError> {
        let map = self.staged.lock().unwrap();
        let mut list: Vec<SaleTransaction> = map.values()
            .filter(|s| party_id.map_or(true, |pid| s.party_id == Some(pid)))
            .filter(|s| status.as_ref().map_or(true, |st| &s.payment_status == st))
            .filter(|s| start_date.map_or(true, |sd| s.transaction_date.as_str() >= sd))
            .filter(|s| end_date.map_or(true, |ed| s.transaction_date.as_str() <= ed))
            .cloned()
            .collect();
        list.sort_by_key(|s| s.id);
        Ok(list.into_iter().skip(offset).take(limit).collect())
    }
    fn list_transactions(&self, filter: &SalesFilter) -> Result<PaginatedList<SaleTransaction>, AppError> {
        let from_date = filter.from_date.as_deref().or(filter.start_date.as_deref());
        let to_date = filter.to_date.as_deref().or(filter.end_date.as_deref());
        let page = filter.page.unwrap_or(1);
        let limit = filter.limit.unwrap_or(50);
        let offset = page.saturating_sub(1) * limit;

        let all_filtered = self.list_sales(from_date, to_date, filter.payment_status, filter.party_id, 0, usize::MAX)?;
        let total = all_filtered.len();
        let items: Vec<SaleTransaction> = all_filtered.into_iter().skip(offset).take(limit).collect();
        Ok(PaginatedList::new(items, total, page, limit))
    }
}

pub struct MockAllocationRepository { pub staged: Arc<Mutex<HashMap<i64, SaleStockAllocation>>> }
impl AllocationRepository for MockAllocationRepository {
    fn create_allocation(&mut self, allocation: &SaleStockAllocation) -> Result<i64, AppError> {
        let mut map = self.staged.lock().unwrap();
        let id = if allocation.id > 0 { allocation.id } else { (map.len() as i64) + 1 };
        let mut cloned = allocation.clone();
        cloned.id = id;
        map.insert(id, cloned);
        Ok(id)
    }
    fn create(&mut self, alloc: &SaleStockAllocation) -> Result<i64, AppError> {
        self.create_allocation(alloc)
    }
    fn find_by_sale_item_id(&self, sale_item_id: i64) -> Result<Vec<SaleStockAllocation>, AppError> {
        let map = self.staged.lock().unwrap();
        let mut allocs: Vec<SaleStockAllocation> = map.values()
            .filter(|a| a.sale_item_id == sale_item_id)
            .cloned()
            .collect();
        allocs.sort_by_key(|a| a.id);
        Ok(allocs)
    }
    fn list_by_sale_item_id(&self, sale_item_id: i64) -> Result<Vec<SaleStockAllocation>, AppError> {
        self.find_by_sale_item_id(sale_item_id)
    }
}

pub struct MockSettlementRepository { pub staged: Arc<Mutex<HashMap<i64, PaymentSettlement>>> }
impl SettlementRepository for MockSettlementRepository {
    fn create_settlement(&mut self, settlement: &PaymentSettlement) -> Result<i64, AppError> {
        let mut map = self.staged.lock().unwrap();
        let id = if settlement.id > 0 { settlement.id } else { (map.len() as i64) + 1 };
        let mut cloned = settlement.clone();
        cloned.id = id;
        map.insert(id, cloned);
        Ok(id)
    }
    fn create(&mut self, settlement: &PaymentSettlement) -> Result<i64, AppError> {
        self.create_settlement(settlement)
    }
    fn list_by_party(&self, party_id: i64) -> Result<Vec<PaymentSettlement>, AppError> {
        let map = self.staged.lock().unwrap();
        let mut list: Vec<PaymentSettlement> = map.values()
            .filter(|s| s.party_id == Some(party_id))
            .cloned()
            .collect();
        list.sort_by_key(|s| s.id);
        Ok(list)
    }
    fn list_by_party_id(&self, party_id: i64) -> Result<Vec<PaymentSettlement>, AppError> {
        self.list_by_party(party_id)
    }
    fn list_by_transaction(&self, tx_type: &str, tx_id: i64) -> Result<Vec<PaymentSettlement>, AppError> {
        let map = self.staged.lock().unwrap();
        let mut list: Vec<PaymentSettlement> = map.values()
            .filter(|s| s.transaction_type == tx_type && s.transaction_id == tx_id)
            .cloned()
            .collect();
        list.sort_by_key(|s| s.id);
        Ok(list)
    }
    fn get_daily_settlement_total(&self, date: &str, tx_type: &str) -> Result<f64, AppError> {
        let map = self.staged.lock().unwrap();
        let sum: f64 = map.values()
            .filter(|s| s.settlement_date == date && s.transaction_type == tx_type)
            .map(|s| s.amount_paid)
            .sum();
        Ok(sum)
    }
}

pub struct MockReturnRepository {
    pub staged: Arc<Mutex<HashMap<i64, ReturnTransaction>>>,
    pub staged_items: Arc<Mutex<HashMap<i64, ReturnItem>>>,
}
impl ReturnRepository for MockReturnRepository {
    fn create_return_transaction(&mut self, tx: &ReturnTransaction) -> Result<i64, AppError> {
        let mut map = self.staged.lock().unwrap();
        let id = if tx.id > 0 { tx.id } else { (map.len() as i64) + 1 };
        let mut cloned = tx.clone();
        cloned.id = id;
        map.insert(id, cloned);
        Ok(id)
    }
    fn create_transaction(&mut self, tx: &ReturnTransaction) -> Result<i64, AppError> {
        self.create_return_transaction(tx)
    }
    fn create_return_item(&mut self, item: &ReturnItem) -> Result<i64, AppError> {
        let mut map = self.staged_items.lock().unwrap();
        let id = if item.id > 0 { item.id } else { (map.len() as i64) + 1 };
        let mut cloned = item.clone();
        cloned.id = id;
        map.insert(id, cloned);
        Ok(id)
    }
    fn find_return_by_id(&self, id: i64) -> Result<Option<ReturnTransaction>, AppError> {
        let map = self.staged.lock().unwrap();
        Ok(map.get(&id).cloned())
    }
    fn find_by_id(&self, id: i64) -> Result<Option<ReturnTransaction>, AppError> {
        self.find_return_by_id(id)
    }
    fn get_next_return_code(&self) -> Result<String, AppError> {
        let map = self.staged.lock().unwrap();
        let max_num = map.values()
            .filter_map(|r| {
                if r.return_code.starts_with("RET-") {
                    r.return_code[4..].parse::<u64>().ok()
                } else {
                    None
                }
            })
            .max()
            .unwrap_or(1000);
        Ok(format!("RET-{}", max_num + 1))
    }
    fn get_latest_code(&self) -> Result<Option<String>, AppError> {
        let map = self.staged.lock().unwrap();
        Ok(map.values().map(|r| r.return_code.clone()).max())
    }
    fn list_returns(
        &self,
        return_type: Option<ReturnType>,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<ReturnTransaction>, AppError> {
        let map = self.staged.lock().unwrap();
        let mut list: Vec<ReturnTransaction> = map.values()
            .filter(|r| return_type.as_ref().map_or(true, |rt| &r.return_type == rt))
            .cloned()
            .collect();
        list.sort_by_key(|r| r.id);
        Ok(list.into_iter().skip(offset).take(limit).collect())
    }
    fn list_items_by_transaction_id(&self, tx_id: i64) -> Result<Vec<ReturnItem>, AppError> {
        let map = self.staged_items.lock().unwrap();
        let mut items: Vec<ReturnItem> = map.values().filter(|r| r.return_transaction_id == tx_id).cloned().collect();
        items.sort_by_key(|i| i.id);
        Ok(items)
    }
}

pub struct MockAdjustmentRepository { pub staged: Arc<Mutex<HashMap<i64, StockAdjustment>>> }
impl AdjustmentRepository for MockAdjustmentRepository {
    fn create_adjustment(&mut self, adj: &StockAdjustment) -> Result<i64, AppError> {
        let mut map = self.staged.lock().unwrap();
        let id = if adj.id > 0 { adj.id } else { (map.len() as i64) + 1 };
        let mut cloned = adj.clone();
        cloned.id = id;
        map.insert(id, cloned);
        Ok(id)
    }
    fn create(&mut self, adj: &StockAdjustment) -> Result<i64, AppError> {
        self.create_adjustment(adj)
    }
    fn list_adjustments(
        &self,
        item_id: Option<i64>,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<StockAdjustment>, AppError> {
        let map = self.staged.lock().unwrap();
        let mut list: Vec<StockAdjustment> = map.values()
            .filter(|a| item_id.map_or(true, |id| a.item_id == id))
            .cloned()
            .collect();
        list.sort_by_key(|a| a.id);
        Ok(list.into_iter().skip(offset).take(limit).collect())
    }
    fn list_by_item_id(&self, item_id: i64) -> Result<Vec<StockAdjustment>, AppError> {
        self.list_adjustments(Some(item_id), 0, usize::MAX)
    }
}

pub struct MockAdjustmentAllocationRepository { pub staged: Arc<Mutex<HashMap<i64, AdjustmentStockAllocation>>> }
impl AdjustmentAllocationRepository for MockAdjustmentAllocationRepository {
    fn create_allocation(&mut self, alloc: &AdjustmentStockAllocation) -> Result<i64, AppError> {
        let mut map = self.staged.lock().unwrap();
        let id = if alloc.id > 0 { alloc.id } else { (map.len() as i64) + 1 };
        let mut cloned = alloc.clone();
        cloned.id = id;
        map.insert(id, cloned);
        Ok(id)
    }
    fn create(&mut self, alloc: &AdjustmentStockAllocation) -> Result<i64, AppError> {
        self.create_allocation(alloc)
    }
    fn find_by_adjustment_id(&self, adjustment_id: i64) -> Result<Vec<AdjustmentStockAllocation>, AppError> {
        let map = self.staged.lock().unwrap();
        let mut allocs: Vec<AdjustmentStockAllocation> = map.values()
            .filter(|a| a.stock_adjustment_id == adjustment_id)
            .cloned()
            .collect();
        allocs.sort_by_key(|a| a.id);
        Ok(allocs)
    }
    fn list_by_adjustment_id(&self, adj_id: i64) -> Result<Vec<AdjustmentStockAllocation>, AppError> {
        self.find_by_adjustment_id(adj_id)
    }
}

pub struct MockSettingRepository { pub staged: Arc<Mutex<Option<ShopSetting>>> }
impl SettingRepository for MockSettingRepository {
    fn get(&self) -> Result<Option<ShopSetting>, AppError> {
        let s = self.staged.lock().unwrap();
        Ok(s.clone())
    }
    fn update(&mut self, setting: &ShopSetting) -> Result<(), AppError> {
        let mut s = self.staged.lock().unwrap();
        *s = Some(setting.clone());
        Ok(())
    }
}

// Transaction Context with authentic staged snapshot rollback and zero Box::leak
pub struct MockTransactionContext {
    state_ref: MockStorageState,
    pub staged_items: Arc<Mutex<HashMap<i64, Item>>>,
    pub staged_bundles: Arc<Mutex<HashMap<i64, ItemBundle>>>,
    pub staged_parties: Arc<Mutex<HashMap<i64, Party>>>,
    pub staged_purchases: Arc<Mutex<HashMap<i64, PurchaseTransaction>>>,
    pub staged_purchase_items: Arc<Mutex<HashMap<i64, PurchaseItem>>>,
    pub staged_sales: Arc<Mutex<HashMap<i64, SaleTransaction>>>,
    pub staged_sale_items: Arc<Mutex<HashMap<i64, SaleItem>>>,
    pub staged_allocations: Arc<Mutex<HashMap<i64, SaleStockAllocation>>>,
    pub staged_settlements: Arc<Mutex<HashMap<i64, PaymentSettlement>>>,
    pub staged_returns: Arc<Mutex<HashMap<i64, ReturnTransaction>>>,
    pub staged_return_items: Arc<Mutex<HashMap<i64, ReturnItem>>>,
    pub staged_adjustments: Arc<Mutex<HashMap<i64, StockAdjustment>>>,
    pub staged_adjustment_allocations: Arc<Mutex<HashMap<i64, AdjustmentStockAllocation>>>,
    pub staged_settings: Arc<Mutex<Option<ShopSetting>>>,
    committed: bool,
    pub items_repo: MockItemRepository,
    pub bundles_repo: MockBundleRepository,
    pub purchases_repo: MockPurchaseRepository,
    pub sales_repo: MockSaleRepository,
    pub allocations_repo: MockAllocationRepository,
    pub settlements_repo: MockSettlementRepository,
    pub returns_repo: MockReturnRepository,
    pub adjustments_repo: MockAdjustmentRepository,
    pub adjustment_allocations_repo: MockAdjustmentAllocationRepository,
    pub settings_repo: MockSettingRepository,
    pub parties_repo: MockPartyRepository,
}

impl MockTransactionContext {
    pub fn new(state: MockStorageState) -> Self {
        let staged_items = Arc::new(Mutex::new(state.items.lock().unwrap().clone()));
        let staged_bundles = Arc::new(Mutex::new(state.bundles.lock().unwrap().clone()));
        let staged_parties = Arc::new(Mutex::new(state.parties.lock().unwrap().clone()));
        let staged_purchases = Arc::new(Mutex::new(state.purchases.lock().unwrap().clone()));
        let staged_purchase_items = Arc::new(Mutex::new(state.purchase_items.lock().unwrap().clone()));
        let staged_sales = Arc::new(Mutex::new(state.sales.lock().unwrap().clone()));
        let staged_sale_items = Arc::new(Mutex::new(state.sale_items.lock().unwrap().clone()));
        let staged_allocations = Arc::new(Mutex::new(state.allocations.lock().unwrap().clone()));
        let staged_settlements = Arc::new(Mutex::new(state.settlements.lock().unwrap().clone()));
        let staged_returns = Arc::new(Mutex::new(state.returns.lock().unwrap().clone()));
        let staged_return_items = Arc::new(Mutex::new(state.return_items.lock().unwrap().clone()));
        let staged_adjustments = Arc::new(Mutex::new(state.adjustments.lock().unwrap().clone()));
        let staged_adjustment_allocations = Arc::new(Mutex::new(state.adjustment_allocations.lock().unwrap().clone()));
        let staged_settings = Arc::new(Mutex::new(state.settings.lock().unwrap().clone()));

        Self {
            state_ref: state,
            staged_items: staged_items.clone(),
            staged_bundles: staged_bundles.clone(),
            staged_parties: staged_parties.clone(),
            staged_purchases: staged_purchases.clone(),
            staged_purchase_items: staged_purchase_items.clone(),
            staged_sales: staged_sales.clone(),
            staged_sale_items: staged_sale_items.clone(),
            staged_allocations: staged_allocations.clone(),
            staged_settlements: staged_settlements.clone(),
            staged_returns: staged_returns.clone(),
            staged_return_items: staged_return_items.clone(),
            staged_adjustments: staged_adjustments.clone(),
            staged_adjustment_allocations: staged_adjustment_allocations.clone(),
            staged_settings: staged_settings.clone(),
            committed: false,
            items_repo: MockItemRepository { staged: staged_items },
            bundles_repo: MockBundleRepository { staged: staged_bundles },
            purchases_repo: MockPurchaseRepository { staged: staged_purchases, staged_items: staged_purchase_items },
            sales_repo: MockSaleRepository { staged: staged_sales, staged_items: staged_sale_items },
            allocations_repo: MockAllocationRepository { staged: staged_allocations },
            settlements_repo: MockSettlementRepository { staged: staged_settlements },
            returns_repo: MockReturnRepository { staged: staged_returns, staged_items: staged_return_items },
            adjustments_repo: MockAdjustmentRepository { staged: staged_adjustments },
            adjustment_allocations_repo: MockAdjustmentAllocationRepository { staged: staged_adjustment_allocations },
            settings_repo: MockSettingRepository { staged: staged_settings },
            parties_repo: MockPartyRepository { staged: staged_parties },
        }
    }
}

impl TransactionContext for MockTransactionContext {
    fn commit(mut self: Box<Self>) -> Result<(), AppError> {
        *self.state_ref.items.lock().unwrap() = self.staged_items.lock().unwrap().clone();
        *self.state_ref.bundles.lock().unwrap() = self.staged_bundles.lock().unwrap().clone();
        *self.state_ref.parties.lock().unwrap() = self.staged_parties.lock().unwrap().clone();
        *self.state_ref.purchases.lock().unwrap() = self.staged_purchases.lock().unwrap().clone();
        *self.state_ref.purchase_items.lock().unwrap() = self.staged_purchase_items.lock().unwrap().clone();
        *self.state_ref.sales.lock().unwrap() = self.staged_sales.lock().unwrap().clone();
        *self.state_ref.sale_items.lock().unwrap() = self.staged_sale_items.lock().unwrap().clone();
        *self.state_ref.allocations.lock().unwrap() = self.staged_allocations.lock().unwrap().clone();
        *self.state_ref.settlements.lock().unwrap() = self.staged_settlements.lock().unwrap().clone();
        *self.state_ref.returns.lock().unwrap() = self.staged_returns.lock().unwrap().clone();
        *self.state_ref.return_items.lock().unwrap() = self.staged_return_items.lock().unwrap().clone();
        *self.state_ref.adjustments.lock().unwrap() = self.staged_adjustments.lock().unwrap().clone();
        *self.state_ref.adjustment_allocations.lock().unwrap() = self.staged_adjustment_allocations.lock().unwrap().clone();
        *self.state_ref.settings.lock().unwrap() = self.staged_settings.lock().unwrap().clone();
        self.committed = true;
        Ok(())
    }

    fn rollback(mut self: Box<Self>) -> Result<(), AppError> {
        self.committed = false;
        Ok(())
    }

    fn items(&mut self) -> &mut dyn ItemRepository { &mut self.items_repo }
    fn bundles(&mut self) -> &mut dyn BundleRepository { &mut self.bundles_repo }
    fn purchases(&mut self) -> &mut dyn PurchaseRepository { &mut self.purchases_repo }
    fn sales(&mut self) -> &mut dyn SaleRepository { &mut self.sales_repo }
    fn allocations(&mut self) -> &mut dyn AllocationRepository { &mut self.allocations_repo }
    fn settlements(&mut self) -> &mut dyn SettlementRepository { &mut self.settlements_repo }
    fn returns(&mut self) -> &mut dyn ReturnRepository { &mut self.returns_repo }
    fn adjustments(&mut self) -> &mut dyn AdjustmentRepository { &mut self.adjustments_repo }
    fn adjustment_allocations(&mut self) -> &mut dyn AdjustmentAllocationRepository { &mut self.adjustment_allocations_repo }
    fn settings(&mut self) -> &mut dyn SettingRepository { &mut self.settings_repo }
    fn parties(&mut self) -> &mut dyn PartyRepository { &mut self.parties_repo }
}

#[derive(Clone)]
pub struct MockUnitOfWork {
    pub state: MockStorageState,
}
impl MockUnitOfWork {
    pub fn new() -> Self {
        Self { state: MockStorageState::new() }
    }
}
impl UnitOfWork for MockUnitOfWork {
    fn begin_transaction(&self) -> Result<Box<dyn TransactionContext>, AppError> {
        Ok(Box::new(MockTransactionContext::new(self.state.clone())))
    }
}

// =========================================================================
// Domain Services Implementations
// =========================================================================

#[derive(Clone, Copy)]
pub struct FifoStrategyImpl;
impl InventoryAllocationStrategy for FifoStrategyImpl {
    fn validate_stock_availability(&self, uow: &mut dyn TransactionContext, item_id: i64, required_base_qty: f64) -> Result<(), AppError> {
        let lots = uow.purchases().list_active_lots_for_item(item_id)?;
        let total_available: f64 = lots.iter().map(|l| l.remaining_quantity).sum();
        if total_available < required_base_qty {
            let item_name = uow.items().find_by_id(item_id)?.map(|i| i.name).unwrap_or_else(|| "Unknown Item".to_string());
            return Err(AppError::InsufficientStockError { item_id, item_name, required: required_base_qty, available: total_available });
        }
        Ok(())
    }

    fn deplete_lots_for_sale(&self, uow: &mut dyn TransactionContext, sale_item_id: i64, item_id: i64, required_base_qty: f64) -> Result<DepletionResult, AppError> {
        self.validate_stock_availability(uow, item_id, required_base_qty)?;
        let lots = uow.purchases().list_active_lots_for_item(item_id)?;
        let mut remaining_needed = required_base_qty;
        let mut total_cogs = 0.0;
        let mut allocations = Vec::new();
        for lot in lots {
            if remaining_needed <= 0.0 { break; }
            let take = lot.remaining_quantity.min(remaining_needed);
            let new_lot_remaining = lot.remaining_quantity - take;
            uow.purchases().update_remaining_quantity(lot.id, new_lot_remaining)?;
            let allocation = SaleStockAllocation { id: 0, sale_item_id, purchase_item_id: lot.id, quantity_depleted: take, unit_cost: lot.unit_cost };
            let alloc_id = uow.allocations().create(&allocation)?;
            let mut saved = allocation.clone();
            saved.id = alloc_id;
            allocations.push(saved);
            total_cogs += take * lot.unit_cost;
            remaining_needed -= take;
        }
        Ok(DepletionResult { total_cogs, allocations })
    }

    fn restore_lots_for_return(&self, uow: &mut dyn TransactionContext, sale_item_id: i64, quantity_to_restore: f64) -> Result<(), AppError> {
        if quantity_to_restore <= 0.0 {
            return Err(AppError::ValidationError {
                field: "quantity_to_restore".to_string(),
                message: "Restoration quantity must be strictly positive".to_string(),
            });
        }

        let mut allocs = uow.allocations().find_by_sale_item_id(sale_item_id)?;
        allocs.sort_by(|a, b| b.id.cmp(&a.id)); // Reverse FIFO order: latest depleted first
        let mut remaining_to_restore = quantity_to_restore;

        for alloc in allocs {
            if remaining_to_restore <= 0.0 { break; }
            let restore_amount = alloc.quantity_depleted.min(remaining_to_restore);

            // Dynamic lookup by alloc.purchase_item_id across purchases
            let mut found_lot: Option<PurchaseItem> = None;
            let purchases = uow.purchases().list_purchases(None, None, None, None, 0, 10000)?;
            for p in purchases {
                let items = uow.purchases().list_items_by_transaction_id(p.id)?;
                if let Some(lot) = items.into_iter().find(|l| l.id == alloc.purchase_item_id) {
                    found_lot = Some(lot);
                    break;
                }
            }

            if found_lot.is_none() {
                let synth_items = uow.purchases().list_items_by_transaction_id(0)?;
                if let Some(lot) = synth_items.into_iter().find(|l| l.id == alloc.purchase_item_id) {
                    found_lot = Some(lot);
                }
            }

            let lot = found_lot.ok_or_else(|| AppError::NotFound {
                entity: "PurchaseItem".to_string(),
                id: alloc.purchase_item_id.to_string(),
            })?;

            let new_remaining = lot.remaining_quantity + restore_amount;
            if new_remaining > lot.quantity {
                return Err(AppError::ValidationError {
                    field: "remaining_quantity".to_string(),
                    message: format!(
                        "Restoration of {} units exceeds lot #{} capacity of {} (current: {})",
                        restore_amount, lot.id, lot.quantity, lot.remaining_quantity
                    ),
                });
            }

            uow.purchases().update_remaining_quantity(lot.id, new_remaining)?;
            remaining_to_restore -= restore_amount;
        }

        if remaining_to_restore > 0.0 {
            return Err(AppError::ValidationError {
                field: "quantity_to_restore".to_string(),
                message: format!(
                    "Return quantity {} exceeds total units depleted for sale_item_id {}",
                    quantity_to_restore, sale_item_id
                ),
            });
        }

        Ok(())
    }

    fn deplete_lots_for_adjustment(&self, uow: &mut dyn TransactionContext, stock_adjustment_id: i64, item_id: i64, write_off_qty: f64) -> Result<AdjustmentDepletionResult, AppError> {
        self.validate_stock_availability(uow, item_id, write_off_qty)?;
        let lots = uow.purchases().list_active_lots_for_item(item_id)?;
        let mut remaining_needed = write_off_qty;
        let mut total_loss_value = 0.0;
        let mut allocations = Vec::new();
        for lot in lots {
            if remaining_needed <= 0.0 { break; }
            let take = lot.remaining_quantity.min(remaining_needed);
            let new_lot_remaining = lot.remaining_quantity - take;
            uow.purchases().update_remaining_quantity(lot.id, new_lot_remaining)?;
            let alloc = AdjustmentStockAllocation { id: 0, stock_adjustment_id, purchase_item_id: lot.id, quantity_depleted: take, unit_cost: lot.unit_cost };
            let alloc_id = uow.adjustment_allocations().create(&alloc)?;
            let mut saved = alloc.clone();
            saved.id = alloc_id;
            allocations.push(saved);
            total_loss_value += take * lot.unit_cost;
            remaining_needed -= take;
        }
        Ok(AdjustmentDepletionResult { total_loss_value, allocations })
    }
}

#[derive(Clone, Copy)]
pub struct PaymentServiceImpl;
impl PaymentService for PaymentServiceImpl {
    fn record_payment(&self, uow: &mut dyn TransactionContext, payload: RecordPaymentPayload) -> Result<i64, AppError> {
        if payload.amount_paid <= 0.0 {
            return Err(AppError::ValidationError {
                field: "amount_paid".to_string(),
                message: format!("Payment amount must be positive, got {}", payload.amount_paid),
            });
        }

        let (current_balance, _) = if payload.transaction_type == "SALE" {
            let sale = uow.sales().find_by_id(payload.transaction_id)?.ok_or_else(|| AppError::NotFound { entity: "SaleTransaction".to_string(), id: payload.transaction_id.to_string() })?;
            (sale.balance_due, sale.payment_status)
        } else {
            let pur = uow.purchases().find_by_id(payload.transaction_id)?.ok_or_else(|| AppError::NotFound { entity: "PurchaseTransaction".to_string(), id: payload.transaction_id.to_string() })?;
            (pur.balance_due, pur.payment_status)
        };
        if payload.amount_paid > current_balance {
            return Err(AppError::ValidationError { field: "amount_paid".to_string(), message: "Payment exceeds outstanding balance".to_string() });
        }
        let new_balance = current_balance - payload.amount_paid;
        let new_status = if new_balance <= 0.0 { PaymentStatus::Paid } else { PaymentStatus::Partial };
        if payload.transaction_type == "SALE" {
            uow.sales().update_balance_due(payload.transaction_id, new_balance, new_status)?;
        } else {
            uow.purchases().update_balance_due(payload.transaction_id, new_balance, new_status)?;
        }
        let settlement = PaymentSettlement {
            id: 0,
            settlement_date: payload.settlement_date,
            party_id: payload.party_id,
            transaction_type: payload.transaction_type,
            transaction_id: payload.transaction_id,
            amount_paid: payload.amount_paid,
            payment_mode: payload.payment_mode,
            notes: payload.notes,
            created_at: "2026-11-20T12:00:00Z".to_string(),
        };
        uow.settlements().create(&settlement)
    }

    fn record_refund(&self, uow: &mut dyn TransactionContext, payload: RecordRefundPayload) -> Result<i64, AppError> {
        let settlement = PaymentSettlement {
            id: 0,
            settlement_date: payload.refund_date,
            party_id: payload.party_id,
            transaction_type: format!("{}_REFUND", payload.transaction_type),
            transaction_id: payload.transaction_id.unwrap_or(0),
            amount_paid: -payload.amount.abs(),
            payment_mode: payload.payment_mode,
            notes: payload.notes,
            created_at: "2026-11-20T12:00:00Z".to_string(),
        };
        uow.settlements().create(&settlement)
    }

    fn reduce_transaction_balance(&self, uow: &mut dyn TransactionContext, transaction_type: &str, transaction_id: i64, reduction_amount: f64) -> Result<(), AppError> {
        if transaction_type == "SALE" {
            let sale = uow.sales().find_by_id(transaction_id)?.ok_or_else(|| AppError::NotFound { entity: "SaleTransaction".to_string(), id: transaction_id.to_string() })?;
            let new_balance = (sale.balance_due - reduction_amount).max(0.0);
            let new_status = if new_balance == 0.0 { PaymentStatus::Paid } else { PaymentStatus::Partial };
            uow.sales().update_balance_due(transaction_id, new_balance, new_status)?;
        } else {
            let pur = uow.purchases().find_by_id(transaction_id)?.ok_or_else(|| AppError::NotFound { entity: "PurchaseTransaction".to_string(), id: transaction_id.to_string() })?;
            let new_balance = (pur.balance_due - reduction_amount).max(0.0);
            let new_status = if new_balance == 0.0 { PaymentStatus::Paid } else { PaymentStatus::Partial };
            uow.purchases().update_balance_due(transaction_id, new_balance, new_status)?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct InventoryServiceImpl<S: InventoryAllocationStrategy> {
    pub strategy: S,
    pub uow: MockUnitOfWork,
}
impl<S: InventoryAllocationStrategy> InventoryServiceImpl<S> {
    pub fn new(strategy: S, uow: MockUnitOfWork) -> Self { Self { strategy, uow } }
}
impl<S: InventoryAllocationStrategy> InventoryService for InventoryServiceImpl<S> {
    fn validate_stock_availability(&self, uow: &mut dyn TransactionContext, item_id: i64, required_base_qty: f64) -> Result<(), AppError> {
        self.strategy.validate_stock_availability(uow, item_id, required_base_qty)
    }
    fn deplete_stock_for_sale(&self, uow: &mut dyn TransactionContext, sale_item_id: i64, item_id: i64, required_base_qty: f64) -> Result<DepletionResult, AppError> {
        let depletion = self.strategy.deplete_lots_for_sale(uow, sale_item_id, item_id, required_base_qty)?;
        let item = uow.items().find_by_id(item_id)?.ok_or_else(|| AppError::NotFound { entity: "Item".to_string(), id: item_id.to_string() })?;
        let new_stock = (item.current_stock - required_base_qty).max(0.0);
        let is_low_stock = new_stock < 5.0;
        uow.items().update_stock_cache(item_id, new_stock, is_low_stock)?;
        Ok(depletion)
    }
    fn add_stock_for_purchase(&self, uow: &mut dyn TransactionContext, purchase_item: &PurchaseItem) -> Result<(), AppError> {
        uow.purchases().create_purchase_item(purchase_item)?;
        let item = uow.items().find_by_id(purchase_item.item_id)?.ok_or_else(|| AppError::NotFound { entity: "Item".to_string(), id: purchase_item.item_id.to_string() })?;
        let new_stock = item.current_stock + purchase_item.quantity;
        let is_low_stock = new_stock < 5.0;
        uow.items().update_stock_cache(item.id, new_stock, is_low_stock)?;
        Ok(())
    }
    fn restore_stock_for_return(&self, uow: &mut dyn TransactionContext, sale_item_id: i64, item_id: i64, quantity_to_restore: f64) -> Result<(), AppError> {
        self.strategy.restore_lots_for_return(uow, sale_item_id, quantity_to_restore)?;
        let item = uow.items().find_by_id(item_id)?.ok_or_else(|| AppError::NotFound { entity: "Item".to_string(), id: item_id.to_string() })?;
        let new_stock = item.current_stock + quantity_to_restore;
        let is_low_stock = new_stock < 5.0;
        uow.items().update_stock_cache(item.id, new_stock, is_low_stock)?;
        Ok(())
    }
    fn remove_stock_for_vendor_return(&self, uow: &mut dyn TransactionContext, item_id: i64, purchase_item_id: Option<i64>, quantity: f64) -> Result<(), AppError> {
        let item = uow.items().find_by_id(item_id)?.ok_or_else(|| AppError::NotFound { entity: "Item".to_string(), id: item_id.to_string() })?;
        if item.current_stock < quantity {
            return Err(AppError::InsufficientStockError {
                item_id,
                item_name: item.name.clone(),
                required: quantity,
                available: item.current_stock,
            });
        }
        if let Some(lot_id) = purchase_item_id {
            let lots = uow.purchases().list_active_lots_for_item(item_id)?;
            if let Some(lot) = lots.iter().find(|l| l.id == lot_id) {
                let new_rem = (lot.remaining_quantity - quantity).max(0.0);
                uow.purchases().update_remaining_quantity(lot_id, new_rem)?;
            }
        }
        let new_stock = item.current_stock - quantity;
        let is_low_stock = new_stock < 5.0;
        uow.items().update_stock_cache(item_id, new_stock, is_low_stock)?;
        Ok(())
    }
    fn create_adjustment(&self, payload: StockAdjustmentPayload) -> Result<i64, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let adj = StockAdjustment {
            id: 0,
            adjustment_date: payload.adjustment_date.clone(),
            item_id: payload.item_id,
            quantity: payload.quantity,
            reason: payload.reason,
            notes: payload.notes,
            created_at: "2026-11-20T12:00:00Z".to_string(),
        };
        let adj_id = tx.adjustments().create(&adj)?;
        let item = tx.items().find_by_id(payload.item_id)?.ok_or_else(|| AppError::NotFound { entity: "Item".to_string(), id: payload.item_id.to_string() })?;

        if payload.quantity < 0.0 {
            let write_off_qty = payload.quantity.abs();
            self.strategy.deplete_lots_for_adjustment(&mut *tx, adj_id, payload.item_id, write_off_qty)?;
            let new_stock = (item.current_stock - write_off_qty).max(0.0);
            let is_low = new_stock < 5.0;
            tx.items().update_stock_cache(item.id, new_stock, is_low)?;
        } else {
            let latest_lot = tx.purchases().get_latest_purchase_item_for_item(payload.item_id)?;
            let unit_cost = latest_lot.map(|l| l.unit_cost).unwrap_or(item.default_sale_price * 0.7);
            let synthetic_tx = PurchaseTransaction {
                id: 0,
                transaction_code: format!("ADJ-{}", adj_id),
                transaction_date: payload.adjustment_date,
                party_id: None,
                payment_mode: None,
                payment_status: PaymentStatus::Paid,
                total_amount: payload.quantity * unit_cost,
                paid_amount: payload.quantity * unit_cost,
                balance_due: 0.0,
                notes: Some("Synthetic Adjustment Lot".to_string()),
                created_at: "2026-11-20T12:00:00Z".to_string(),
            };
            let synth_tx_id = tx.purchases().create_transaction(&synthetic_tx)?;
            let synth_lot = PurchaseItem {
                id: 0,
                purchase_transaction_id: synth_tx_id,
                item_id: payload.item_id,
                bundle_id: None,
                bundle_quantity: payload.quantity,
                quantity: payload.quantity,
                unit_cost,
                total_amount: payload.quantity * unit_cost,
                remaining_quantity: payload.quantity,
            };
            tx.purchases().create_purchase_item(&synth_lot)?;
            let new_stock = item.current_stock + payload.quantity;
            let is_low = new_stock < 5.0;
            tx.items().update_stock_cache(item.id, new_stock, is_low)?;
        }
        tx.commit()?;
        Ok(adj_id)
    }
    fn list_adjustments(&self, item_id: Option<i64>, page: usize, limit: usize) -> Result<PaginatedList<StockAdjustment>, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let all = if let Some(id) = item_id { tx.adjustments().list_by_item_id(id)? } else { Vec::new() };
        let total = all.len();
        let items = all.into_iter().skip((page - 1) * limit).take(limit).collect();
        Ok(PaginatedList::new(items, total, page, limit))
    }
}

pub struct SaleServiceImpl<I: InventoryService, P: PaymentService> {
    pub inventory_service: I,
    pub payment_service: P,
    pub uow: MockUnitOfWork,
}
impl<I: InventoryService, P: PaymentService> SaleServiceImpl<I, P> {
    pub fn new(inventory_service: I, payment_service: P, uow: MockUnitOfWork) -> Self {
        Self { inventory_service, payment_service, uow }
    }
}
impl<I: InventoryService, P: PaymentService> SaleService for SaleServiceImpl<I, P> {
    fn get_next_sale_code(&self) -> Result<String, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let latest = tx.sales().get_latest_code()?;
        let next_num = match latest {
            Some(code) if code.starts_with("SAL-") => code[4..].parse::<u64>().unwrap_or(1000) + 1,
            _ => 1001,
        };
        Ok(format!("SAL-{}", next_num))
    }
    fn create_sale(&self, payload: CreateSalePayload) -> Result<SaleSummaryResponse, AppError> {
        if payload.items.is_empty() {
            return Err(AppError::ValidationError { field: "items".to_string(), message: "items cannot be empty".to_string() });
        }
        for item_payload in &payload.items {
            if item_payload.bundle_quantity <= 0.0 {
                return Err(AppError::ValidationError {
                    field: "bundle_quantity".to_string(),
                    message: format!("Sale quantity must be positive, got {}", item_payload.bundle_quantity),
                });
            }
            if let Some(rate) = item_payload.unit_price {
                if rate < 0.0 {
                    return Err(AppError::ValidationError {
                        field: "unit_price".to_string(),
                        message: "unit_price cannot be negative".to_string(),
                    });
                }
            }
        }
        if payload.paid_amount < 0.0 {
            return Err(AppError::ValidationError {
                field: "paid_amount".to_string(),
                message: "paid_amount cannot be negative".to_string(),
            });
        }

        let mut tx = self.uow.begin_transaction()?;
        let sale_code = payload.transaction_code.clone().unwrap_or_else(|| self.get_next_sale_code().unwrap_or_else(|_| "SAL-1001".to_string()));
        let mut total_amount = 0.0;
        let mut resolved_lines = Vec::new();
        for item_payload in &payload.items {
            let (multiplier, unit_price) = if let Some(bundle_id) = item_payload.bundle_id {
                let bundle = tx.bundles().find_by_id(bundle_id)?.ok_or_else(|| AppError::NotFound { entity: "ItemBundle".to_string(), id: bundle_id.to_string() })?;
                let effective_rate = item_payload.unit_price.unwrap_or_else(|| bundle.default_bundle_price.unwrap_or(bundle.units_per_bundle * 10.0) / bundle.units_per_bundle);
                (bundle.units_per_bundle, effective_rate)
            } else {
                let item = tx.items().find_by_id(item_payload.item_id)?.ok_or_else(|| AppError::NotFound { entity: "Item".to_string(), id: item_payload.item_id.to_string() })?;
                (1.0, item_payload.unit_price.unwrap_or(item.default_sale_price))
            };
            let base_qty = item_payload.bundle_quantity * multiplier;
            let line_total = base_qty * unit_price;
            total_amount += line_total;
            resolved_lines.push((item_payload.clone(), base_qty, unit_price, line_total));
        }
        let sale_tx = SaleTransaction {
            id: 0,
            transaction_code: sale_code.clone(),
            transaction_date: payload.transaction_date,
            party_id: payload.party_id,
            payment_mode: payload.payment_mode.clone(),
            payment_status: PaymentStatus::Unpaid,
            total_amount,
            paid_amount: 0.0,
            balance_due: total_amount,
            notes: payload.notes,
            created_at: "2026-11-20T12:00:00Z".to_string(),
        };
        let sale_id = tx.sales().create_transaction(&sale_tx)?;
        for (line_payload, base_qty, unit_price, line_total) in resolved_lines {
            let sale_item = SaleItem { id: 0, sale_transaction_id: sale_id, item_id: line_payload.item_id, bundle_id: line_payload.bundle_id, bundle_quantity: line_payload.bundle_quantity, quantity: base_qty, unit_price, total_amount: line_total };
            let sale_item_id = tx.sales().create_sale_item(&sale_item)?;
            self.inventory_service.deplete_stock_for_sale(&mut *tx, sale_item_id, line_payload.item_id, base_qty)?;
        }
        let upfront_payment = payload.paid_amount.min(total_amount);
        if upfront_payment > 0.0 {
            let payment_payload = RecordPaymentPayload {
                party_id: payload.party_id,
                transaction_type: "SALE".to_string(),
                transaction_id: sale_id,
                settlement_date: sale_tx.transaction_date.clone(),
                amount_paid: upfront_payment,
                payment_mode: payload.payment_mode,
                notes: Some("Upfront Counter Payment".to_string()),
            };
            self.payment_service.record_payment(&mut *tx, payment_payload)?;
        }
        tx.commit()?;
        let balance_due = (total_amount - payload.paid_amount).max(0.0);
        let change = (payload.paid_amount - total_amount).max(0.0);
        let payment_status = if balance_due == 0.0 {
            PaymentStatus::Paid
        } else if payload.paid_amount > 0.0 {
            PaymentStatus::Partial
        } else {
            PaymentStatus::Unpaid
        };
        Ok(SaleSummaryResponse { sale_id, transaction_code: sale_code, total_amount, paid_amount: payload.paid_amount, balance_due, change_amount: change, change_returned: change, payment_status })
    }
    fn get_sales_history(&self, _filter: SalesHistoryFilter) -> Result<PaginatedList<SaleTransaction>, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        tx.sales().list_transactions(&SalesFilter { party_id: None, payment_status: None, start_date: None, end_date: None, from_date: None, to_date: None, page: Some(1), limit: Some(50) })
    }
    fn get_sale_details(&self, sale_id: i64) -> Result<SaleDetailView, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let sale = tx.sales().find_by_id(sale_id)?.ok_or_else(|| AppError::NotFound { entity: "SaleTransaction".to_string(), id: sale_id.to_string() })?;
        let items = tx.sales().list_items_by_transaction_id(sale_id)?;
        let details: Vec<SaleItemDetail> = items.into_iter().map(|i| SaleItemDetail { sale_item: i, item_name: "Item".to_string(), bundle_name: None, allocations: Vec::new() }).collect();
        Ok(SaleDetailView { sale, party: None, items: details, settlements: Vec::new() })
    }
}

pub struct PurchaseServiceImpl<I: InventoryService, P: PaymentService> {
    pub inventory_service: I,
    pub payment_service: P,
    pub uow: MockUnitOfWork,
}
impl<I: InventoryService, P: PaymentService> PurchaseServiceImpl<I, P> {
    pub fn new(inventory_service: I, payment_service: P, uow: MockUnitOfWork) -> Self { Self { inventory_service, payment_service, uow } }
}
impl<I: InventoryService, P: PaymentService> PurchaseService for PurchaseServiceImpl<I, P> {
    fn get_next_purchase_code(&self) -> Result<String, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let latest = tx.purchases().get_latest_code()?;
        let next_num = match latest {
            Some(code) if code.starts_with("PUR-") => code[4..].parse::<u64>().unwrap_or(1000) + 1,
            _ => 1001,
        };
        Ok(format!("PUR-{}", next_num))
    }
    fn create_purchase(&self, payload: CreatePurchasePayload) -> Result<i64, AppError> {
        if payload.items.is_empty() {
            return Err(AppError::ValidationError { field: "items".to_string(), message: "items cannot be empty".to_string() });
        }
        let mut tx = self.uow.begin_transaction()?;
        let pur_code = payload.transaction_code.clone().unwrap_or_else(|| self.get_next_purchase_code().unwrap_or_else(|_| "PUR-1001".to_string()));
        let mut total_amount = 0.0;
        let mut resolved_lines = Vec::new();
        for item_payload in &payload.items {
            let (multiplier, unit_cost) = if let Some(bundle_id) = item_payload.bundle_id {
                let bundle = tx.bundles().find_by_id(bundle_id)?.ok_or_else(|| AppError::NotFound { entity: "ItemBundle".to_string(), id: bundle_id.to_string() })?;
                let effective_cost = item_payload.unit_cost / bundle.units_per_bundle;
                (bundle.units_per_bundle, effective_cost)
            } else {
                (1.0, item_payload.unit_cost)
            };
            let base_qty = item_payload.bundle_quantity * multiplier;
            let line_total = item_payload.bundle_quantity * item_payload.unit_cost;
            total_amount += line_total;
            resolved_lines.push((item_payload.clone(), base_qty, unit_cost, line_total));
        }
        let purchase_tx = PurchaseTransaction {
            id: 0,
            transaction_code: pur_code,
            transaction_date: payload.transaction_date.clone(),
            party_id: payload.party_id,
            payment_mode: payload.payment_mode.clone(),
            payment_status: PaymentStatus::Unpaid,
            total_amount,
            paid_amount: 0.0,
            balance_due: total_amount,
            notes: payload.notes,
            created_at: "2026-11-20T12:00:00Z".to_string(),
        };
        let purchase_id = tx.purchases().create_transaction(&purchase_tx)?;
        for (line_payload, base_qty, unit_cost, line_total) in resolved_lines {
            let purchase_item = PurchaseItem {
                id: 0,
                purchase_transaction_id: purchase_id,
                item_id: line_payload.item_id,
                bundle_id: line_payload.bundle_id,
                bundle_quantity: line_payload.bundle_quantity,
                quantity: base_qty,
                unit_cost,
                total_amount: line_total,
                remaining_quantity: base_qty,
            };
            self.inventory_service.add_stock_for_purchase(&mut *tx, &purchase_item)?;
        }
        let upfront_payment = payload.paid_amount.min(total_amount);
        if upfront_payment > 0.0 {
            let payment_payload = RecordPaymentPayload {
                party_id: payload.party_id,
                transaction_type: "PURCHASE".to_string(),
                transaction_id: purchase_id,
                settlement_date: payload.transaction_date,
                amount_paid: upfront_payment,
                payment_mode: payload.payment_mode,
                notes: Some("Supplier Inward Payment".to_string()),
            };
            self.payment_service.record_payment(&mut *tx, payment_payload)?;
        }
        tx.commit()?;
        Ok(purchase_id)
    }
    fn get_purchases_history(&self, _filter: PurchaseHistoryFilter) -> Result<PaginatedList<PurchaseTransaction>, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        tx.purchases().list_transactions(&PurchaseFilter { party_id: None, payment_status: None, start_date: None, end_date: None, from_date: None, to_date: None, page: Some(1), limit: Some(50) })
    }
    fn get_purchase_details(&self, purchase_id: i64) -> Result<PurchaseDetailView, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let pur = tx.purchases().find_by_id(purchase_id)?.ok_or_else(|| AppError::NotFound { entity: "PurchaseTransaction".to_string(), id: purchase_id.to_string() })?;
        let items = tx.purchases().list_items_by_transaction_id(purchase_id)?;
        let details: Vec<PurchaseItemDetail> = items.into_iter().map(|i| PurchaseItemDetail { purchase_item: i, item_name: "Item".to_string(), bundle_name: None }).collect();
        Ok(PurchaseDetailView { purchase: pur, party: None, items: details, settlements: Vec::new() })
    }
}

pub struct ReturnServiceImpl<I: InventoryService, P: PaymentService> {
    pub inventory_service: I,
    pub payment_service: P,
    pub uow: MockUnitOfWork,
}
impl<I: InventoryService, P: PaymentService> ReturnServiceImpl<I, P> {
    pub fn new(inventory_service: I, payment_service: P, uow: MockUnitOfWork) -> Self { Self { inventory_service, payment_service, uow } }
}
impl<I: InventoryService, P: PaymentService> ReturnService for ReturnServiceImpl<I, P> {
    fn get_next_return_code(&self) -> Result<String, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        tx.returns().get_next_return_code()
    }
    fn create_customer_return(&self, payload: CustomerReturnPayload) -> Result<String, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let mut total_refund = 0.0;
        let return_code = payload.return_code.clone().unwrap_or_else(|| "RET-1001".to_string());
        let ret_tx = ReturnTransaction {
            id: 0,
            return_code: return_code.clone(),
            return_date: payload.return_date.clone(),
            return_type: ReturnType::Customer,
            party_id: payload.party_id,
            original_transaction_id: payload.original_sale_id,
            settlement_type: payload.settlement_type,
            total_amount: 0.0,
            reason: payload.reason,
            notes: payload.notes,
            created_at: "2026-11-20T12:00:00Z".to_string(),
        };
        let return_id = tx.returns().create_transaction(&ret_tx)?;
        for item_payload in &payload.items {
            let line_total = item_payload.quantity * item_payload.unit_price;
            total_refund += line_total;
            let ret_item = ReturnItem {
                id: 0,
                return_transaction_id: return_id,
                item_id: item_payload.item_id,
                sale_item_id: item_payload.sale_item_id,
                purchase_item_id: None,
                quantity: item_payload.quantity,
                unit_price: item_payload.unit_price,
                total_amount: line_total,
                condition: item_payload.condition,
            };
            tx.returns().create_return_item(&ret_item)?;
            if item_payload.condition == StockCondition::Restockable {
                if let Some(sale_item_id) = item_payload.sale_item_id {
                    self.inventory_service.restore_stock_for_return(&mut *tx, sale_item_id, item_payload.item_id, item_payload.quantity)?;
                }
            }
        }
        if payload.settlement_type == SettlementType::CashRefund {
            let refund_payload = RecordRefundPayload {
                party_id: payload.party_id,
                transaction_type: "SALE".to_string(),
                transaction_id: payload.original_sale_id,
                refund_date: payload.return_date,
                amount: total_refund,
                payment_mode: Some("CASH".to_string()),
                notes: Some("Customer return refund".to_string()),
            };
            self.payment_service.record_refund(&mut *tx, refund_payload)?;
        }
        tx.commit()?;
        Ok(return_code)
    }
    fn create_vendor_return(&self, payload: VendorReturnPayload) -> Result<String, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let return_code = format!("VRET-{}", (tx.returns().list_returns(Some(ReturnType::Vendor), 0, 1000)?.len() as i64) + 1001);
        let mut total_amount = 0.0;
        for item in &payload.items {
            total_amount += item.quantity * item.unit_cost;
        }

        let ret_tx = ReturnTransaction {
            id: 0,
            return_code: return_code.clone(),
            return_date: payload.return_date.clone(),
            return_type: ReturnType::Vendor,
            party_id: payload.party_id,
            original_transaction_id: payload.original_purchase_id,
            settlement_type: payload.settlement_type,
            total_amount,
            reason: payload.reason,
            notes: payload.notes,
            created_at: "2026-11-20T12:00:00Z".to_string(),
        };
        let return_id = tx.returns().create_return_transaction(&ret_tx)?;

        for item in &payload.items {
            let line_total = item.quantity * item.unit_cost;
            let ret_item = ReturnItem {
                id: 0,
                return_transaction_id: return_id,
                item_id: item.item_id,
                sale_item_id: None,
                purchase_item_id: item.purchase_item_id,
                quantity: item.quantity,
                unit_price: item.unit_cost,
                total_amount: line_total,
                condition: StockCondition::Restockable,
            };
            tx.returns().create_return_item(&ret_item)?;
            self.inventory_service.remove_stock_for_vendor_return(&mut *tx, item.item_id, item.purchase_item_id, item.quantity)?;
        }

        if payload.settlement_type == SettlementType::CashRefund {
            let refund_payload = RecordRefundPayload {
                party_id: payload.party_id,
                transaction_type: "PURCHASE".to_string(),
                transaction_id: payload.original_purchase_id,
                refund_date: payload.return_date,
                amount: total_amount,
                payment_mode: Some("CASH".to_string()),
                notes: Some("Vendor return refund".to_string()),
            };
            self.payment_service.record_refund(&mut *tx, refund_payload)?;
        }

        tx.commit()?;
        Ok(return_code)
    }
    fn get_return_details(&self, return_id: i64) -> Result<ReturnDetailView, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let ret_tx = tx.returns().find_return_by_id(return_id)?
            .ok_or_else(|| AppError::NotFound { entity: "ReturnTransaction".to_string(), id: return_id.to_string() })?;
        let party = if let Some(pid) = ret_tx.party_id {
            tx.parties().find_by_id(pid)?
        } else {
            None
        };
        let items = tx.returns().list_items_by_transaction_id(return_id)?;
        let mut item_details = Vec::new();
        for it in items {
            let name = tx.items().find_by_id(it.item_id)?
                .map(|i| i.name)
                .unwrap_or_else(|| "Unknown Item".to_string());
            item_details.push(ReturnItemDetail {
                return_item: it,
                item_name: name,
            });
        }
        Ok(ReturnDetailView {
            return_tx: ret_tx,
            party,
            items: item_details,
        })
    }
}

pub struct AccountingServiceImpl<P: PaymentService> {
    pub payment_service: P,
    pub uow: MockUnitOfWork,
}
impl<P: PaymentService> AccountingServiceImpl<P> {
    pub fn new(payment_service: P, uow: MockUnitOfWork) -> Self { Self { payment_service, uow } }

    pub fn calculate_live_cash_drawer(&self, today: &str) -> Result<f64, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let mut drawer = 0.0;

        let state = &self.uow.state;
        let settlements_map = state.settlements.lock().unwrap();

        let mut settled_sales: std::collections::HashSet<i64> = std::collections::HashSet::new();
        let mut settled_purchases: std::collections::HashSet<i64> = std::collections::HashSet::new();

        // 1. Accumulate all date-matched settlements that used physical CASH
        for s in settlements_map.values() {
            if s.settlement_date == today {
                let is_cash = s.payment_mode.as_deref() == Some("CASH");
                if !is_cash {
                    continue;
                }

                match s.transaction_type.as_str() {
                    "SALE" | "CUSTOMER_SETTLEMENT" => {
                        drawer += s.amount_paid.abs();
                        settled_sales.insert(s.transaction_id);
                    }
                    "PURCHASE" | "VENDOR_SETTLEMENT" => {
                        drawer -= s.amount_paid.abs();
                        settled_purchases.insert(s.transaction_id);
                    }
                    "SALE_REFUND" => {
                        // Customer cash refund is cash outflow: decrement drawer
                        drawer -= s.amount_paid.abs();
                    }
                    "PURCHASE_REFUND" => {
                        // Vendor refund is cash inflow: increment drawer
                        drawer += s.amount_paid.abs();
                    }
                    _ => {}
                }
            }
        }

        // 2. Include upfront cash from sales on `today` ONLY if no settlement was logged for them
        let sales = tx.sales().list_transactions(&SalesFilter {
            party_id: None,
            payment_status: None,
            start_date: Some(today.to_string()),
            end_date: Some(today.to_string()),
            from_date: Some(today.to_string()),
            to_date: Some(today.to_string()),
            page: Some(1),
            limit: Some(1000),
        })?;
        for s in sales.items {
            if s.payment_mode.as_deref() == Some("CASH") && s.paid_amount > 0.0 {
                if !settled_sales.contains(&s.id) {
                    drawer += s.paid_amount;
                }
            }
        }

        // 3. Deduct upfront cash from purchases on `today` ONLY if no settlement was logged for them
        let purchases = tx.purchases().list_transactions(&PurchaseFilter {
            party_id: None,
            payment_status: None,
            start_date: Some(today.to_string()),
            end_date: Some(today.to_string()),
            from_date: Some(today.to_string()),
            to_date: Some(today.to_string()),
            page: Some(1),
            limit: Some(1000),
        })?;
        for p in purchases.items {
            if p.payment_mode.as_deref() == Some("CASH") && p.paid_amount > 0.0 {
                if !settled_purchases.contains(&p.id) {
                    drawer -= p.paid_amount;
                }
            }
        }

        Ok(drawer)
    }
}
impl<P: PaymentService> AccountingService for AccountingServiceImpl<P> {
    fn get_party_dues_summary(&self, party_type: PartyType) -> Result<Vec<PartyDueSummary>, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let parties = tx.parties().list_by_type(party_type, true)?;
        let mut summaries = Vec::new();
        for party in parties {
            let unpaid_sales = tx.sales().list_unpaid_by_party_id(party.id)?;
            let total_due: f64 = unpaid_sales.iter().map(|s| s.balance_due).sum();
            if total_due > 0.0 {
                let oldest_bill_date = unpaid_sales.first().map(|s| s.transaction_date.clone());
                summaries.push(PartyDueSummary { party_id: party.id, party_name: party.name, phone: party.phone, party_type, total_due, oldest_bill_date, unpaid_bills_count: unpaid_sales.len() });
            }
        }
        Ok(summaries)
    }
    fn settle_bill_payment(&self, payload: SettlePaymentPayload) -> Result<(), AppError> {
        let mut tx = self.uow.begin_transaction()?;
        if let Some(sale_id) = payload.specific_bill_id {
            let payment_payload = RecordPaymentPayload { party_id: Some(payload.party_id), transaction_type: "SALE".to_string(), transaction_id: sale_id, settlement_date: payload.settlement_date, amount_paid: payload.amount, payment_mode: payload.payment_mode, notes: payload.notes };
            self.payment_service.record_payment(&mut *tx, payment_payload)?;
        } else {
            let unpaid = tx.sales().list_unpaid_by_party_id(payload.party_id)?;
            let mut remaining = payload.amount;
            for bill in unpaid {
                if remaining <= 0.0 { break; }
                let settle_amount = bill.balance_due.min(remaining);
                let payment_payload = RecordPaymentPayload { party_id: Some(payload.party_id), transaction_type: "SALE".to_string(), transaction_id: bill.id, settlement_date: payload.settlement_date.clone(), amount_paid: settle_amount, payment_mode: payload.payment_mode.clone(), notes: payload.notes.clone() };
                self.payment_service.record_payment(&mut *tx, payment_payload)?;
                remaining -= settle_amount;
            }
        }
        tx.commit()?;
        Ok(())
    }
    fn get_party_ledger(&self, party_id: i64) -> Result<PartyLedgerView, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let party = tx.parties().find_by_id(party_id)?.ok_or_else(|| AppError::NotFound { entity: "Party".to_string(), id: party_id.to_string() })?;
        let sales = tx.sales().list_unpaid_by_party_id(party_id)?;
        let total_due: f64 = sales.iter().map(|s| s.balance_due).sum();
        Ok(PartyLedgerView { party, current_balance: total_due, entries: Vec::new() })
    }
}

pub struct ReportServiceImpl {
    pub uow: MockUnitOfWork,
}
impl ReportServiceImpl {
    pub fn new(uow: MockUnitOfWork) -> Self { Self { uow } }
}
impl ReportService for ReportServiceImpl {
    fn get_dashboard_metrics(&self) -> Result<DashboardMetrics, AppError> {
        let state = &self.uow.state;
        let sales_map = state.sales.lock().unwrap();
        let pur_map = state.purchases.lock().unwrap();
        let lots_map = state.purchase_items.lock().unwrap();
        let alloc_map = state.allocations.lock().unwrap();
        let settlements_map = state.settlements.lock().unwrap();

        let today_sales: f64 = sales_map.values().map(|s| s.total_amount).sum();
        let today_purchases: f64 = pur_map.values().map(|p| p.total_amount).sum();
        let total_cogs: f64 = alloc_map.values().map(|a| a.quantity_depleted * a.unit_cost).sum();
        let today_net_profit = today_sales - total_cogs;

        let total_stock_valuation: f64 = lots_map.values()
            .map(|l| l.remaining_quantity * l.unit_cost)
            .sum();

        let customer_receivables: f64 = sales_map.values().map(|s| s.balance_due).sum();
        let vendor_payables: f64 = pur_map.values().map(|p| p.balance_due).sum();

        let sales_cash: f64 = sales_map.values().filter(|s| s.payment_mode.as_deref() == Some("CASH")).map(|s| s.paid_amount).sum();
        let pur_cash: f64 = pur_map.values().filter(|p| p.payment_mode.as_deref() == Some("CASH")).map(|p| p.paid_amount).sum();

        let refunds_cash: f64 = settlements_map.values()
            .filter(|s| s.transaction_type == "SALE_REFUND" && s.payment_mode.as_deref() == Some("CASH"))
            .map(|s| s.amount_paid.abs())
            .sum();

        let customer_dues_cash: f64 = settlements_map.values()
            .filter(|s| s.transaction_type == "CUSTOMER_SETTLEMENT" && s.payment_mode.as_deref() == Some("CASH"))
            .map(|s| s.amount_paid.abs())
            .sum();

        let vendor_dues_cash: f64 = settlements_map.values()
            .filter(|s| s.transaction_type == "VENDOR_SETTLEMENT" && s.payment_mode.as_deref() == Some("CASH"))
            .map(|s| s.amount_paid.abs())
            .sum();

        let live_cash_in_drawer = sales_cash - pur_cash - refunds_cash + customer_dues_cash - vendor_dues_cash;

        Ok(DashboardMetrics {
            today_sales,
            today_purchases,
            today_sales_amount: today_sales,
            today_sales_count: sales_map.len(),
            today_purchase_amount: today_purchases,
            today_purchase_count: pur_map.len(),
            today_net_profit,
            total_stock_valuation,
            customer_receivables,
            vendor_payables,
            live_cash_in_drawer,
        })
    }
    fn get_low_stock_alerts(&self) -> Result<Vec<LowStockItem>, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let items = tx.items().list_catalog(None, true, 0, 50)?;
        Ok(items.into_iter().map(|i| LowStockItem {
            id: i.id,
            item_id: i.id,
            name: i.name,
            category: i.category,
            current_stock: i.current_stock,
            base_unit: i.base_unit,
            default_sale_price: i.default_sale_price,
            mrp: i.mrp,
        }).collect())
    }
    fn get_sales_purchase_trend(&self, _period: &str) -> Result<Vec<TrendDataPoint>, AppError> {
        let state = &self.uow.state;
        let sales_map = state.sales.lock().unwrap();
        let pur_map = state.purchases.lock().unwrap();
        let mut date_map: HashMap<String, (f64, f64)> = HashMap::new();
        for s in sales_map.values() {
            let entry = date_map.entry(s.transaction_date.clone()).or_insert((0.0, 0.0));
            entry.0 += s.total_amount;
        }
        for p in pur_map.values() {
            let entry = date_map.entry(p.transaction_date.clone()).or_insert((0.0, 0.0));
            entry.1 += p.total_amount;
        }
        let mut trend: Vec<TrendDataPoint> = date_map.into_iter()
            .map(|(date, (sales, purchases))| TrendDataPoint {
                date,
                sales_amount: sales,
                purchase_amount: purchases,
            })
            .collect();
        trend.sort_by(|a, b| a.date.cmp(&b.date));
        Ok(trend)
    }
    fn get_pnl_report(&self, from_date: &str, to_date: &str) -> Result<PnLReport, AppError> {
        let state = &self.uow.state;
        let sales_map = state.sales.lock().unwrap();
        let alloc_map = state.allocations.lock().unwrap();
        let gross_sales: f64 = sales_map.values().map(|s| s.total_amount).sum();
        let cogs: f64 = alloc_map.values().map(|a| a.quantity_depleted * a.unit_cost).sum();
        let net_profit = gross_sales - cogs;
        Ok(PnLReport {
            from_date: from_date.to_string(),
            to_date: to_date.to_string(),
            gross_sales,
            cogs,
            net_profit,
            total_discounts: 0.0,
            returns_refunded: 0.0,
        })
    }
    fn get_stock_valuation_report(&self) -> Result<StockValuationReport, AppError> {
        let state = &self.uow.state;
        let lots_map = state.purchase_items.lock().unwrap();
        let total_valuation: f64 = lots_map.values().map(|l| l.remaining_quantity * l.unit_cost).sum();
        let total_items_count = lots_map.len();
        Ok(StockValuationReport {
            total_valuation,
            active_lots_count: total_items_count,
            total_items_count,
            categories_breakdown: Vec::new(),
            categories: Vec::new(),
        })
    }
    fn get_cash_flow_report(&self, from_date: &str, to_date: &str) -> Result<CashFlowReport, AppError> {
        let state = &self.uow.state;
        let settlements = state.settlements.lock().unwrap();
        let mut total_inflow = 0.0;
        let mut total_outflow = 0.0;
        for s in settlements.values() {
            if s.settlement_date.as_str() >= from_date && s.settlement_date.as_str() <= to_date {
                if s.amount_paid > 0.0 {
                    total_inflow += s.amount_paid;
                } else {
                    total_outflow += s.amount_paid.abs();
                }
            }
        }
        let net_cash_flow = total_inflow - total_outflow;
        Ok(CashFlowReport {
            from_date: from_date.to_string(),
            to_date: to_date.to_string(),
            total_inflow,
            total_outflow,
            net_cash_flow,
            closing_drawer_balance: net_cash_flow,
            sales_cash_inflow: total_inflow,
            customer_settlements_inflow: 0.0,
            purchase_cash_outflow: total_outflow,
            vendor_settlements_outflow: 0.0,
            refunds_cash_outflow: 0.0,
            net_cash_balance: net_cash_flow,
        })
    }
}

pub struct MigrationServiceImpl { pub uow: MockUnitOfWork }
impl MigrationServiceImpl { pub fn new(uow: MockUnitOfWork) -> Self { Self { uow } } }
impl MigrationService for MigrationServiceImpl {
    fn scan_backup_directory(&self) -> Result<Vec<BackupFileInfo>, AppError> { Ok(Vec::new()) }

    fn inspect_backup_file(&self, archive_path: &str) -> Result<MigrationPreview, AppError> {
        let file = File::open(archive_path).map_err(|e| AppError::IoError(e.to_string()))?;
        let backup_file_size_bytes = file.metadata().map(|m| m.len()).unwrap_or(0);
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|_| AppError::MigrationError("Invalid ZIP header".to_string()))?;

        // 1. Read and parse company_manifest.json genuinely
        #[derive(serde::Deserialize)]
        struct ManifestData {
            #[serde(alias = "company_name", alias = "companyName")]
            company_name: String,
            #[serde(alias = "version", alias = "schema_version", alias = "schemaVersion")]
            schema_version: serde_json::Value,
            #[serde(alias = "backup_date", alias = "created_at", alias = "createdAt", default)]
            created_at: String,
        }

        let (manifest, schema_version) = {
            let mut manifest_entry = archive
                .by_name("company_manifest.json")
                .map_err(|_| AppError::MigrationError("Missing company_manifest.json in archive".to_string()))?;

            let manifest: ManifestData = serde_json::from_reader(&mut manifest_entry)
                .map_err(|e| AppError::MigrationError(format!("Corrupt manifest: {}", e)))?;

            let version: i32 = match manifest.schema_version {
                serde_json::Value::Number(ref n) => n.as_i64().unwrap_or(1) as i32,
                serde_json::Value::String(ref s) => s.parse::<i32>().unwrap_or_else(|_| {
                    s.split('.').next().and_then(|p| p.parse().ok()).unwrap_or(1)
                }),
                _ => 1,
            };
            (manifest, version)
        };

        // 2. Extract company.db to a temp file and query real SQLite table statistics
        let mut db_entry = archive
            .by_name("company.db")
            .map_err(|_| AppError::MigrationError("Missing company.db in archive".to_string()))?;

        let temp_db_path = std::env::temp_dir().join(format!("vedbill_inspect_{}.db", uuid::Uuid::new_v4()));
        {
            let mut out_file = File::create(&temp_db_path)
                .map_err(|e| AppError::IoError(e.to_string()))?;
            std::io::copy(&mut db_entry, &mut out_file)
                .map_err(|e| AppError::IoError(e.to_string()))?;
        }

        let conn = rusqlite::Connection::open_with_flags(&temp_db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let items_count: usize = conn.query_row("SELECT COUNT(*) FROM items", [], |r| r.get(0)).unwrap_or(0);
        let parties_count: usize = conn.query_row("SELECT COUNT(*) FROM parties", [], |r| r.get(0)).unwrap_or(0);
        let sales_count: usize = conn.query_row("SELECT COUNT(*) FROM documents WHERE documentType = 'sale_invoice'", [], |r| r.get(0))
            .or_else(|_| conn.query_row("SELECT COUNT(*) FROM documents", [], |r| r.get(0)))
            .unwrap_or(0);
        let purchases_count: usize = conn.query_row("SELECT COUNT(*) FROM documents WHERE documentType = 'purchase_invoice'", [], |r| r.get(0)).unwrap_or(0);
        let document_items_count: usize = conn.query_row("SELECT COUNT(*) FROM document_items", [], |r| r.get(0)).unwrap_or(0);

        drop(conn);
        let _ = std::fs::remove_file(&temp_db_path);

        Ok(MigrationPreview {
            company_name: manifest.company_name,
            schema_version,
            created_at: manifest.created_at,
            backup_file_path: archive_path.to_string(),
            backup_file_size_bytes,
            table_stats: TableStats {
                items_count,
                parties_count,
                sales_count,
                purchases_count,
                document_items_count,
            },
        })
    }

    fn execute_migration(&self, archive_path: &str) -> Result<MigrationResult, AppError> {
        let start = std::time::Instant::now();
        let file = File::open(archive_path).map_err(|e| AppError::IoError(e.to_string()))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|_| AppError::MigrationError("Invalid ZIP header".to_string()))?;

        let mut db_entry = archive
            .by_name("company.db")
            .map_err(|_| AppError::MigrationError("company.db not found".to_string()))?;

        let temp_db_path = std::env::temp_dir().join(format!("vedbill_exec_{}.db", uuid::Uuid::new_v4()));
        {
            let mut out_file = File::create(&temp_db_path).map_err(|e| AppError::IoError(e.to_string()))?;
            std::io::copy(&mut db_entry, &mut out_file).map_err(|e| AppError::IoError(e.to_string()))?;
        }

        let conn = rusqlite::Connection::open_with_flags(&temp_db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let mut tx = self.uow.begin_transaction()?;

        // Genuine reading of items from company.db
        let mut stmt = conn.prepare("SELECT name, salePrice, mrp, primaryQuantity FROM items")
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let item_rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, Option<f64>>(2)?,
                row.get::<_, f64>(3)?,
            ))
        }).map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let mut items_imported = 0;
        for (idx, r) in item_rows.enumerate() {
            let (name, sale_price, mrp, stock) = r.map_err(|e| AppError::DatabaseError(e.to_string()))?;
            let item_id = (idx as i64) + 1;
            let item = Item {
                id: item_id,
                name,
                category: Some("General".to_string()),
                barcode: None,
                base_unit: "PCS".to_string(),
                mrp,
                default_sale_price: sale_price,
                current_stock: stock,
                is_low_stock: stock < 5.0,
                is_active: true,
                created_at: "2026-11-20T10:00:00Z".to_string(),
                updated_at: "2026-11-20T10:00:00Z".to_string(),
            };
            tx.items().create(&item)?;
            items_imported += 1;
        }

        // Genuine reading of parties from company.db
        let mut party_stmt = conn.prepare("SELECT name, phone, type FROM parties")
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let party_rows = party_stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
            ))
        }).map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let mut parties_imported = 0;
        for (idx, r) in party_rows.enumerate() {
            let (name, phone, p_type) = r.map_err(|e| AppError::DatabaseError(e.to_string()))?;
            let party_type = if p_type == "supplier" { PartyType::Vendor } else { PartyType::Customer };
            let party = Party {
                id: (idx as i64) + 1,
                name,
                phone,
                party_type,
                notes: Some("Migrated from VedBill".to_string()),
                is_active: true,
                created_at: "2026-11-20T10:00:00Z".to_string(),
                updated_at: "2026-11-20T10:00:00Z".to_string(),
            };
            tx.parties().create(&party)?;
            parties_imported += 1;
        }

        tx.commit()?;
        drop(stmt);
        drop(party_stmt);
        drop(conn);
        let _ = std::fs::remove_file(&temp_db_path);

        Ok(MigrationResult {
            success: true,
            items_imported,
            parties_imported,
            sales_imported: 0,
            purchases_imported: 0,
            duration_ms: start.elapsed().as_millis() as u64,
            errors: Vec::new(),
        })
    }
}

pub struct SqliteSearchIndexer {
    pub conn: Arc<Mutex<Connection>>,
    pub uow: MockUnitOfWork,
}

impl SqliteSearchIndexer {
    pub fn new(conn: Arc<Mutex<Connection>>, uow: MockUnitOfWork) -> Self {
        Self { conn, uow }
    }
}

impl ItemSearchIndexer for SqliteSearchIndexer {
    fn index_item(&self, item: &Item) -> Result<(), AppError> {
        self.index_item_with_bundles(item, &[])
    }

    fn index_item_with_bundles(&self, item: &Item, _bundles: &[ItemBundle]) -> Result<(), AppError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM ITEM_SEARCH_FTS WHERE item_id = ?1", rusqlite::params![item.id])
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        if item.is_active {
            conn.execute(
                "INSERT INTO ITEM_SEARCH_FTS (item_id, name, category, barcode) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![item.id, &item.name, &item.category, &item.barcode],
            ).map_err(|e| AppError::DatabaseError(e.to_string()))?;
        }
        Ok(())
    }

    fn remove_item(&self, item_id: i64) -> Result<(), AppError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM ITEM_SEARCH_FTS WHERE item_id = ?1", rusqlite::params![item_id])
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    fn search(&self, query: &str) -> Result<Vec<ItemSearchResult>, AppError> {
        self.search_with_limit(query, 50)
    }

    fn search_with_limit(&self, query: &str, limit: usize) -> Result<Vec<ItemSearchResult>, AppError> {
        let clean_q = query.trim().trim_end_matches('*');
        if clean_q.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self.conn.lock().unwrap();
        // Trigram FTS5 matching with LIKE fallback for short queries:
        let matched_ids: Vec<i64> = if clean_q.len() >= 3 {
            let fts_query = format!("\"{}\"*", clean_q.replace('"', "\"\""));
            let mut stmt = conn.prepare(
                "SELECT item_id FROM ITEM_SEARCH_FTS WHERE ITEM_SEARCH_FTS MATCH ?1 LIMIT ?2"
            ).map_err(|e| AppError::DatabaseError(e.to_string()))?;
            let res: Result<Vec<i64>, _> = stmt.query_map(rusqlite::params![fts_query, limit as i64], |row| row.get(0))
                .and_then(|mapped| mapped.collect());
            match res {
                Ok(ids) => ids,
                Err(_) => {
                    let mut fallback_stmt = conn.prepare(
                        "SELECT item_id FROM ITEM_SEARCH_FTS WHERE name LIKE '%' || ?1 || '%' OR barcode LIKE '%' || ?1 || '%' OR category LIKE '%' || ?1 || '%' LIMIT ?2"
                    ).map_err(|e| AppError::DatabaseError(e.to_string()))?;
                    let mapped = fallback_stmt.query_map(rusqlite::params![clean_q, limit as i64], |row| row.get(0))
                        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
                    let ids: Vec<i64> = mapped.filter_map(Result::ok).collect();
                    ids
                }
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT item_id FROM ITEM_SEARCH_FTS WHERE name LIKE '%' || ?1 || '%' OR barcode LIKE '%' || ?1 || '%' OR category LIKE '%' || ?1 || '%' LIMIT ?2"
            ).map_err(|e| AppError::DatabaseError(e.to_string()))?;
            let mapped = stmt.query_map(rusqlite::params![clean_q, limit as i64], |row| row.get(0))
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;
            let ids: Vec<i64> = mapped.filter_map(Result::ok).collect();
            ids
        };

        // Hydrate from active transaction
        let mut tx = self.uow.begin_transaction()?;
        let mut scored_results = Vec::new();
        for id in matched_ids {
            if let Some(item) = tx.items().find_by_id(id)? {
                if !item.is_active {
                    continue; // Exclude archived items
                }
                let bundles = tx.bundles().find_by_item_id(item.id)?;
                let bundle_results: Vec<ItemBundleSearchResult> = bundles.into_iter().map(|b| {
                    ItemBundleSearchResult {
                        bundle_id: b.id,
                        bundle_name: b.bundle_name,
                        units_per_bundle: b.units_per_bundle,
                        bundle_price: b.default_bundle_price,
                        barcode: b.barcode,
                    }
                }).collect();

                let score = if item.barcode.as_deref() == Some(clean_q) { 10.0 } else { 1.0 };
                scored_results.push((score, ItemSearchResult {
                    item_id: item.id,
                    name: item.name,
                    category: item.category,
                    barcode: item.barcode,
                    base_unit: item.base_unit,
                    mrp: item.mrp,
                    default_sale_price: item.default_sale_price,
                    current_stock: item.current_stock,
                    is_low_stock: item.is_low_stock,
                    bundles: bundle_results,
                }));
            }
        }

        // Sort by score desc (exact barcode match first)
        scored_results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        Ok(scored_results.into_iter().map(|r| r.1).collect())
    }

    fn rebuild_index(&self) -> Result<(), AppError> {
        self.rebuild_index_with_bundles(&[])
    }

    fn rebuild_index_with_bundles(&self, items_with_bundles: &[(Item, Vec<ItemBundle>)]) -> Result<(), AppError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM ITEM_SEARCH_FTS", [])
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        for (item, _) in items_with_bundles {
            if item.is_active {
                conn.execute(
                    "INSERT INTO ITEM_SEARCH_FTS (item_id, name, category, barcode) VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![item.id, &item.name, &item.category, &item.barcode],
                ).map_err(|e| AppError::DatabaseError(e.to_string()))?;
            }
        }
        Ok(())
    }
}
