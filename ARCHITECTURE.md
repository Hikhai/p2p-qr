# P2P Binance Order Management - Architecture Overview

## 📋 Tổng quan hệ thống

Ứng dụng quản lý đơn hàng P2P Binance với 2 thành phần chính:
1. **Chrome Extension** - Tự động thu thập thông tin thanh toán từ Binance
2. **Tauri Desktop App** - Lưu trữ, quản lý và hiển thị thông tin đơn hàng

---

## 🏗️ Kiến trúc hệ thống

```
┌─────────────────────────────────────────────────────────────────┐
│                        BINANCE P2P WEB                          │
│  (https://p2p.binance.com/vi/order-detail/...)                  │
└────────────────┬────────────────────────────────────────────────┘
                 │
                 │ 1. API Interceptor
                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                     CHROME EXTENSION                            │
│                                                                 │
│  ┌──────────────┐    ┌──────────────┐    ┌─────────────────┐  │
│  │ injected.js  │───▶│ content.js   │───▶│ background.js   │  │
│  │ (API Hook)   │    │ (Validator)  │    │ (HTTP Sender)   │  │
│  └──────────────┘    └──────────────┘    └─────────────────┘  │
│         │                    │                     │            │
│         │ Extract Payment    │ Validate            │ POST       │
│         │ Details            │ Complete            │ Retry      │
│         │                    │                     │            │
└─────────┴────────────────────┴─────────────────────┼────────────┘
                                                      │
                                                      │ HTTP POST
                                                      │ localhost:1425
                                                      ▼
┌─────────────────────────────────────────────────────────────────┐
│                      TAURI DESKTOP APP                          │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                  Rust Backend (main.rs)                  │  │
│  │                                                          │  │
│  │  ┌────────────────┐    ┌──────────────┐    ┌─────────┐ │  │
│  │  │ HTTP Server    │───▶│ VietQR       │───▶│ SQLite  │ │  │
│  │  │ (Axum:1425)    │    │ Generator    │    │ DB      │ │  │
│  │  └────────────────┘    └──────────────┘    └─────────┘ │  │
│  │         │                     │                  │      │  │
│  │         │ Receive             │ Generate         │ Save │  │
│  │         │ Payment             │ QR Code          │ Data │  │
│  │         │                     │                  │      │  │
│  └─────────┴─────────────────────┴──────────────────┼──────┘  │
│                                                      │          │
│                                                      ▼          │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │              Svelte Frontend (UI)                        │  │
│  │  - Display order list                                    │  │
│  │  - Show payment details                                  │  │
│  │  - QR code for bank transfer                             │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 🔄 Luồng hoạt động chi tiết

### Phase 1: Thu thập dữ liệu (Chrome Extension)

#### 1.1 **injected.js** - API Interceptor
```javascript
// Chạy trong page context, hook vào fetch/XMLHttpRequest
// Mục tiêu: Bắt API response từ Binance

TARGET API: /bapi/c2c/v2/private/c2c/order-match/order-detail

RESPONSE STRUCTURE:
{
  "data": {
    "orderNumber": "22811811414225453056",
    "totalPrice": "200000",           // VND amount
    "refMessage": "ABCXYZ123",        // Transfer content
    "payMethods": [{
      "fields": [
        { "fieldName": "Họ và tên", "fieldValue": "NGUYEN VAN A" },
        { "fieldName": "Tên ngân hàng", "fieldValue": "BIDV" },
        { "fieldName": "Số tài khoản", "fieldValue": "96247668" },
        { "fieldName": "Chi nhánh", "fieldValue": "CN Hà Nội" }
      ]
    }]
  }
}

EXTRACTION ALGORITHM (Dynamic Field Matching):
1. Loop through payMethods[0].fields[]
2. Match fieldName (case-insensitive, with/without diacritics):
   - "họ và tên" / "ho va ten" / "account name" → accountName
   - "tên ngân hàng" / "bank name" → bankName
   - "số tài khoản" / "account number" → accountNo
   - "chi nhánh" / "branch" → branchName
3. Extract root-level data:
   - data.totalPrice → amount (VND)
   - data.refMessage → transferContent
4. Send to content.js via window.postMessage()
```

**Why dynamic extraction?**
- Different orders have different field arrangements
- Supports Vietnamese (with/without diacritics) and English
- More robust than fixed array indices

#### 1.2 **content.js** - Validator & Bridge
```javascript
// Chạy trong content script context
// Mục tiêu: Validate và forward data

VALIDATION RULES:
✓ Must have: orderNumber + accountNo + bankName
✓ Complete: Must have amount OR transferContent
✗ Skip: Incomplete first responses (waiting for full data)

FORWARD METHODS (Parallel):
1. Direct HTTP to Tauri backend (primary)
2. Chrome messaging to background.js (fallback)

NOTIFICATION:
- Show in-page toast notification when captured
```

#### 1.3 **background.js** - HTTP Sender with Retry
```javascript
// Chạy trong service worker context
// Mục tiêu: Đảm bảo data được gửi đến backend

FEATURES:
1. Health Check Polling
   - Check http://127.0.0.1:1425/api/health every 5 seconds
   - Track backend status: isBackendReady

2. Request Queue
   - Queue failed requests when backend offline
   - Auto-process queue when backend online

3. Deduplication
   - 3-second cooldown per order
   - JSON key comparison to prevent duplicates

4. Retry Logic
   - Try immediate send
   - If failed → queue for later
   - Process queue when health check passes

ENDPOINT: POST http://127.0.0.1:1425/api/payment-detail
```

**Why retry mechanism?**
- User might open extension before app
- Network issues / app restart scenarios
- Ensures no data loss

---

### Phase 2: Xử lý dữ liệu (Tauri Backend)

#### 2.1 **HTTP Server** (Axum on port 1425)
```rust
// Routes:
GET  /api/health          → Health check for extension
POST /api/payment-detail  → Receive payment data

// CORS: Allow any origin (for extension)
```

#### 2.2 **Payment Detail Handler**
```rust
async fn handle_payment_detail() {
    // 1. Extract fields from request
    let order_number = data["orderNumber"]
    let account_name = data["accountName"]
    let account_no = data["accountNo"]
    let bank_name = data["bankName"]
    let amount = data["amount"]
    let transfer_content = data["transferContent"]
    
    // 2. ALWAYS generate VietQR (ignore extension QR)
    if account_no.is_some() && bank_name.is_some() {
        qr_code_url = generate_vietqr_url(
            bank_name, account_no, account_name, 
            amount, transfer_content
        )
    }
    
    // 3. Save to SQLite database
    INSERT OR REPLACE INTO order_payment_detail (
        order_number, account_name, account_no,
        bank_name, sub_bank, qr_code_url,
        amount, transfer_content, ...
    )
    
    // 4. Notify frontend to refresh UI
    app_handle.emit("orders-updated", {...})
}
```

#### 2.3 **VietQR Generator**
```rust
fn generate_vietqr_url() -> Option<String> {
    // 1. Get bank BIN code
    let bin = get_bank_bin(bank_name)?
    // Examples: BIDV=970418, Techcombank=970407, VPBank=970432
    
    // 2. Build VietQR URL
    let url = format!(
        "https://img.vietqr.io/image/{}-{}-compact2.jpg",
        bin, account_no
    )
    
    // 3. Add query parameters
    // ?amount=200000
    // &addInfo=ABCXYZ123
    // &accountName=NGUYEN%20VAN%20A
    
    // 4. Return complete URL
    Some(url)
}

// Supported banks (26+):
BIDV, Vietcombank, Techcombank, VPBank, ACB, MB, 
Agribank, SHB, VietinBank, TPBank, Sacombank, HDBank,
VIB, MSB, OCB, SeABank, etc.
```

**Why backend-generated QR?**
- Binance doesn't always provide QR codes
- VietQR is universal for Vietnamese banks
- Consistent format across all orders

#### 2.4 **SQLite Database**
```sql
-- Table: order_payment_detail
CREATE TABLE order_payment_detail (
    order_number TEXT PRIMARY KEY,
    account_name TEXT,
    account_no TEXT,
    bank_name TEXT,
    sub_bank TEXT,
    qr_code_url TEXT,
    amount TEXT,
    transfer_content TEXT,
    suggested_transfer_content TEXT,
    captured_at INTEGER,
    purge_after INTEGER
);

-- Auto-purge after 24 hours
```

---

### Phase 3: Hiển thị dữ liệu (Svelte Frontend)

#### 3.1 **Order List Page** (+page.svelte)
```svelte
// Load orders from API credentials
// Display list with status, amount, time
// Click order → navigate to detail page
```

#### 3.2 **Order Detail Page** (OrderDetail.svelte)
```svelte
DISPLAY:
1. Order Information
   - Order number
   - Trade type (BUY/SELL)
   - Asset & Fiat amounts

2. Payment Details (from database)
   - Bank name
   - Account number
   - Account name
   - Amount (formatted VND)
   - Transfer content

3. QR Code for Transfer
   - Display VietQR image
   - One-click bank transfer via mobile app
   - Copy account number button

EVENT LISTENER:
- Listen "orders-updated" event
- Auto-refresh when new data captured
```

---

## 🔑 Key Features

### ✅ Dynamic Field Extraction
- **Problem**: Binance API returns different field orders for different orders
- **Solution**: Match by `fieldName` instead of array index
- **Benefit**: Works with all order types, Vietnamese/English fields

### ✅ Retry Mechanism
- **Problem**: Extension might load before app is ready
- **Solution**: Health check + request queue + auto-retry
- **Benefit**: No data loss, works in any order

### ✅ VietQR Auto-Generation
- **Problem**: Binance doesn't provide QR for Vietnamese banks
- **Solution**: Generate VietQR using bank BIN codes
- **Benefit**: One-click transfer on mobile banking apps

### ✅ Real-time Updates
- **Problem**: UI needs to refresh when extension captures data
- **Solution**: Tauri event system (orders-updated)
- **Benefit**: Seamless UX, instant feedback

---

## 🛠️ Technology Stack

### Chrome Extension
- **Language**: JavaScript (ES6+)
- **APIs**: Fetch/XHR interception, Chrome messaging
- **Pattern**: Injected script + Content script + Background service worker

### Tauri Backend
- **Language**: Rust
- **Framework**: Tauri 2.x
- **HTTP Server**: Axum
- **Database**: SQLite (via sqlx)
- **CORS**: tower-http

### Frontend
- **Framework**: SvelteKit
- **Language**: TypeScript
- **Styling**: CSS
- **Build**: Vite

---

## 📊 Data Flow Example

```
USER ACTION:
Open Binance order detail page
↓
EXTENSION INTERCEPTS:
/bapi/c2c/v2/private/c2c/order-match/order-detail
↓
EXTRACT:
{
  orderNumber: "22811811414225453056",
  bankName: "BIDV",
  accountNo: "96247668",
  accountName: "NGUYEN VAN A",
  amount: "200000",
  transferContent: "ABCXYZ123"
}
↓
VALIDATE:
✓ Has order number
✓ Has bank + account
✓ Has amount
✓ Complete data
↓
SEND HTTP:
POST http://127.0.0.1:1425/api/payment-detail
↓
BACKEND PROCESS:
1. Receive request
2. Generate VietQR:
   - BIDV → BIN 970418
   - URL: https://img.vietqr.io/image/970418-96247668-compact2.jpg
           ?amount=200000&addInfo=ABCXYZ123
3. Save to database
4. Emit event: "orders-updated"
↓
FRONTEND REFRESH:
1. Receive event
2. Fetch payment detail from DB
3. Update UI with QR code
↓
USER SEES:
✓ Bank: BIDV
✓ Account: 96247668
✓ Amount: 200,000 VND
✓ Content: ABCXYZ123
✓ QR Code: [Scannable image]
```

---

## 🚀 Workflow Summary

| Step | Component | Action | Output |
|------|-----------|--------|--------|
| 1 | injected.js | Intercept Binance API | Payment data JSON |
| 2 | content.js | Validate completeness | Forward to background |
| 3 | background.js | Send HTTP with retry | POST to backend |
| 4 | Rust HTTP Server | Receive request | Parse JSON |
| 5 | VietQR Generator | Generate QR URL | VietQR image URL |
| 6 | SQLite | Save to database | Persisted data |
| 7 | Tauri Events | Emit update event | Notify frontend |
| 8 | Svelte UI | Fetch & display | Show to user |

---

## 🔒 Security Notes

1. **Local Only**: HTTP server binds to 127.0.0.1 (localhost only)
2. **No Authentication**: Assumes trusted local environment
3. **CORS**: Allows any origin for extension communication
4. **Data Retention**: Auto-purge after 24 hours
5. **No External Calls**: VietQR generation uses public API (read-only)

---

## 📝 Configuration

### Extension
- Manifest v3
- Host permissions: `https://p2p.binance.com/*`, `https://*.binance.com/*`
- Run at: document_start (early injection)

### Backend
- HTTP Port: 1425 (configurable in main.rs)
- Database: `p2p_app.db` in app directory
- Health check: Every 5 seconds

### Frontend
- Dev server: Port 5173 (Vite default)
- Build target: Tauri bundle

---

## 🐛 Debugging Tips

### Extension not capturing?
1. Check console for `[P2P Ext]` logs
2. Verify injected.js loaded: Look for version log
3. Test with fresh order detail page

### Backend not receiving?
1. Check terminal for `[DEBUG]` logs
2. Test health endpoint: `curl http://127.0.0.1:1425/api/health`
3. Check Windows Firewall

### QR not generating?
1. Verify bank name matches supported banks (see `get_bank_bin()`)
2. Check terminal logs: "Generating VietQR...", "Found BIN code: XXX"
3. Test VietQR URL manually in browser

### UI not updating?
1. Check Tauri event emission: `[DEBUG] Event emission result`
2. Verify frontend event listener registered
3. Check database has data: `sqlite3 p2p_app.db "SELECT * FROM order_payment_detail"`

---

## 📚 File Structure

```
tauri-app-main/
├── fe/                              # Frontend workspace
│   ├── p2p-extension/               # Chrome Extension
│   │   ├── injected.js              # API interceptor (page context)
│   │   ├── content.js               # Validator & bridge
│   │   ├── background.js            # HTTP sender with retry
│   │   ├── manifest.json            # Extension config
│   │   └── popup.html               # Extension popup UI
│   │
│   └── src/                         # Svelte frontend
│       ├── routes/
│       │   ├── +page.svelte         # Order list page
│       │   └── +page.ts             # Page data loader
│       └── lib/
│           └── OrderDetail.svelte   # Payment detail component
│
├── src-tauri/                       # Tauri backend
│   ├── src/
│   │   ├── main.rs                  # HTTP server, VietQR, DB
│   │   ├── api/                     # Binance API client
│   │   ├── db/                      # Database module
│   │   ├── orders/                  # Order repository
│   │   └── crypto/                  # Encryption utilities
│   │
│   ├── migrations/
│   │   └── 001_init.sql             # Database schema
│   │
│   └── tauri.conf.json              # Tauri config
│
├── ARCHITECTURE.md                  # This file
├── package.json                     # Node.js dependencies
└── README.md                        # Project README
```

---

## 🔄 LocalStorage Fallback Strategy

### Problem
- User opens Binance order page **before** app is running
- Extension captures data but app is not ready to receive
- User opens app later → no payment details shown

### Solution: Dual Storage
```
Extension captures payment data
        ↓
    [Store in 2 places]
        ↓
   ┌────┴────┐
   ↓         ↓
HTTP POST   localStorage
to App      (p2p_payment_{orderNumber})
   ↓         ↓
Backend DB  Browser Memory
   ↓         ↓
   └────┬────┘
        ↓
   App reads from DB first,
   falls back to localStorage if not found
```

### Implementation Details

#### Extension (content.js)
```javascript
// ALWAYS save to localStorage (even if HTTP succeeds)
const storageKey = `p2p_payment_${orderNumber}`;
localStorage.setItem(storageKey, JSON.stringify(paymentDetail));

// Then try HTTP to backend
fetch('http://127.0.0.1:1425/api/payment-detail', {...})
```

#### Frontend (OrderDetail.svelte)
```javascript
async function loadPaymentDetail() {
  // 1. Try database first (fast, already processed)
  let result = await invoke('get_order_payment_detail', {orderNumber});
  
  if (!result) {
    // 2. Fallback: Check localStorage
    const key = `p2p_payment_${orderNumber}`;
    const stored = localStorage.getItem(key);
    
    if (stored) {
      // 3. Save to backend and reload
      await invoke('save_payment_detail_from_extension', {paymentDetail});
      result = await invoke('get_order_payment_detail', {orderNumber});
    }
  }
  
  return result;
}
```

### Benefits
- ✅ Works even if app opened after browser
- ✅ No data loss - always has backup in localStorage
- ✅ Automatic sync when app loads
- ✅ No user intervention required

---

## 🎯 Next Steps / Future Improvements

1. **Authentication**: Add API key for extension → backend
2. **Multi-Order Support**: Batch capture multiple orders
3. **Export**: Export orders to CSV/Excel
4. **Notifications**: Desktop notifications when order captured
5. **Statistics**: Dashboard with daily/monthly stats
6. **Backup**: Auto-backup database
7. **Bank Detection**: Improve bank name matching (fuzzy search)
8. **QR Templates**: Support different VietQR templates
9. **Bulk localStorage Sync**: Scan and import all stored payment details on app startup

---

**Last Updated**: October 15, 2025
**Version**: 0.4.1 - Added localStorage fallback
