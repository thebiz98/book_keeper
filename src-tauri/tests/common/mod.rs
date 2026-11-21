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
    fn list_all_lots(&self) -> Result<Vec<PurchaseItem>, AppError> {
        let map = self.staged_items.lock().unwrap();
        let mut lots: Vec<PurchaseItem> = map.values().cloned().collect();
        lots.sort_by_key(|l| l.id);
        Ok(lots)
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
    fn list_all(&self) -> Result<Vec<SaleStockAllocation>, AppError> {
        let map = self.staged.lock().unwrap();
        let mut list: Vec<SaleStockAllocation> = map.values().cloned().collect();
        list.sort_by_key(|a| a.id);
        Ok(list)
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
    fn list_all(&self) -> Result<Vec<PaymentSettlement>, AppError> {
        let map = self.staged.lock().unwrap();
        let mut list: Vec<PaymentSettlement> = map.values().cloned().collect();
        list.sort_by_key(|s| s.id);
        Ok(list)
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
// Production Domain Services Re-exports
// =========================================================================

pub use book_keeper::services::{
    FifoStrategy, FifoStrategyImpl,
    PaymentServiceImpl,
    BackupServiceImpl,
    ImportExportServiceImpl,
    DataExportImportServiceImpl,
};

pub type InventoryServiceImpl<S = FifoStrategy> = book_keeper::services::InventoryServiceImpl<S, MockUnitOfWork>;
pub type SaleServiceImpl<I, P> = book_keeper::services::SaleServiceImpl<I, P, MockUnitOfWork>;
pub type PurchaseServiceImpl<I, P> = book_keeper::services::PurchaseServiceImpl<I, P, MockUnitOfWork>;
pub type ReturnServiceImpl<I, P> = book_keeper::services::ReturnServiceImpl<I, P, MockUnitOfWork>;

pub type AccountingServiceImpl<P> = book_keeper::services::AccountingServiceImpl<P, MockUnitOfWork>;
pub type ReportServiceImpl = book_keeper::services::ReportServiceImpl<MockUnitOfWork>;
pub type MigrationServiceImpl = book_keeper::services::MigrationServiceImpl<MockUnitOfWork>;
pub type ItemServiceImpl = book_keeper::services::ItemServiceImpl<MockUnitOfWork>;
pub type PartyServiceImpl = book_keeper::services::PartyServiceImpl<MockUnitOfWork>;
pub type ShopSettingServiceImpl = book_keeper::services::ShopSettingServiceImpl<MockUnitOfWork>;

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
