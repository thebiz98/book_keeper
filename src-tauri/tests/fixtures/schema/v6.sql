-- FROZEN: DDL delta of migration 6 as shipped. Never edit.
CREATE TABLE code_sequences (
    prefix TEXT PRIMARY KEY,
    last_value INTEGER NOT NULL CHECK (last_value >= 0)
);
INSERT INTO code_sequences (prefix, last_value)
  SELECT 'SAL-', COALESCE(MAX(CAST(SUBSTR(transaction_code, 5) AS INTEGER)), 0) FROM sale_transactions
  WHERE transaction_code GLOB 'SAL-[0-9]*' AND transaction_code NOT GLOB 'SAL-*[^0-9]*';
INSERT INTO code_sequences (prefix, last_value)
  SELECT 'PUR-', COALESCE(MAX(CAST(SUBSTR(transaction_code, 5) AS INTEGER)), 0) FROM purchase_transactions
  WHERE transaction_code GLOB 'PUR-[0-9]*' AND transaction_code NOT GLOB 'PUR-*[^0-9]*';
INSERT INTO code_sequences (prefix, last_value)
  SELECT 'ADJ-', COALESCE(MAX(CAST(SUBSTR(transaction_code, 5) AS INTEGER)), 0) FROM purchase_transactions
  WHERE transaction_code GLOB 'ADJ-[0-9]*' AND transaction_code NOT GLOB 'ADJ-*[^0-9]*';
INSERT INTO code_sequences (prefix, last_value)
  SELECT 'OPN-', COALESCE(MAX(CAST(SUBSTR(transaction_code, 5) AS INTEGER)), 0) FROM purchase_transactions
  WHERE transaction_code GLOB 'OPN-[0-9]*' AND transaction_code NOT GLOB 'OPN-*[^0-9]*';
INSERT INTO code_sequences (prefix, last_value)
  SELECT 'RTL-', COALESCE(MAX(CAST(SUBSTR(transaction_code, 5) AS INTEGER)), 0) FROM purchase_transactions
  WHERE transaction_code GLOB 'RTL-[0-9]*' AND transaction_code NOT GLOB 'RTL-*[^0-9]*';
INSERT INTO code_sequences (prefix, last_value)
  SELECT 'RET-', COALESCE(MAX(CAST(SUBSTR(return_code, 5) AS INTEGER)), 0) FROM return_transactions
  WHERE return_code GLOB 'RET-[0-9]*' AND return_code NOT GLOB 'RET-*[^0-9]*';
INSERT INTO code_sequences (prefix, last_value) VALUES ('EXP-', 0);
CREATE TABLE expense_types (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE CHECK (length(trim(name)) > 0),
    is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);
CREATE TABLE expenses (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    expense_code TEXT NOT NULL UNIQUE,
    expense_date TEXT NOT NULL,
    expense_type_id INTEGER NOT NULL REFERENCES expense_types(id),
    expense_type_name TEXT NOT NULL,
    amount REAL NOT NULL CHECK (amount > 0),
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    updated_at TEXT
);
CREATE INDEX idx_expenses_date ON expenses(expense_date);
CREATE INDEX idx_expenses_type_date ON expenses(expense_type_id, expense_date);
CREATE VIRTUAL TABLE expense_type_search_fts USING fts5(
    expense_type_id UNINDEXED, name,
    tokenize = 'unicode61 remove_diacritics 2'
);
INSERT INTO schema_version (version, name) VALUES (6, 'expenses_code_sequences');
PRAGMA user_version = 6;
