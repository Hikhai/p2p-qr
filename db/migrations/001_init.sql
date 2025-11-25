-- moved from src-tauri/migrations/001_init.sql
-- Orders core
CREATE TABLE IF NOT EXISTS orders (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  order_number TEXT UNIQUE,
  trade_type TEXT,
  asset TEXT,
  fiat TEXT,
  price TEXT,
  amount_asset TEXT,
  total_fiat TEXT,
  order_status_code INTEGER,
  create_time_ms INTEGER,
  update_time_ms INTEGER,
  buyer_nickname TEXT,
  seller_nickname TEXT,
  buyer_paid_time_ms INTEGER,
  released_time_ms INTEGER,
  cancelled_time_ms INTEGER,
  last_api_sync_ts INTEGER,
  last_ext_update_ts INTEGER,
  source_flags INTEGER DEFAULT 0,
  has_payment_detail INTEGER DEFAULT 0,
  remark TEXT
);

CREATE INDEX IF NOT EXISTS idx_orders_number ON orders(order_number);
CREATE INDEX IF NOT EXISTS idx_orders_create ON orders(create_time_ms);
CREATE INDEX IF NOT EXISTS idx_orders_status ON orders(order_status_code);

-- Payment detail (phase6)
CREATE TABLE IF NOT EXISTS order_payment_detail (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  order_number TEXT NOT NULL,
  account_name TEXT,
  account_no TEXT,
  bank_name TEXT,
  sub_bank TEXT,
  qr_code_url TEXT,
  captured_at INTEGER,
  purge_after INTEGER,
  FOREIGN KEY(order_number) REFERENCES orders(order_number) ON DELETE CASCADE
);

-- API Credentials
CREATE TABLE IF NOT EXISTS api_credentials (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  label TEXT,
  api_key_enc BLOB,
  api_secret_enc BLOB,
  created_at INTEGER,
  last_used_at INTEGER
);

-- Sync state
CREATE TABLE IF NOT EXISTS sync_state (
  id TEXT PRIMARY KEY,
  last_start_timestamp INTEGER,
  last_end_timestamp INTEGER,
  last_complete_ts INTEGER
);
