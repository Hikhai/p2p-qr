# Architecture (overview)

Chi tiết vận hành xem [README.md](./README.md) và [USER_GUIDE.md](./USER_GUIDE.md).

## Components

```
Chrome (p2p-extension)
    │  POST /api/payment-detail
    ▼
Tauri / Rust (src-tauri)
    ├── Axum HTTP 127.0.0.1:1425     # bridge extension
    ├── SyncEngine                   # Binance C2C history
    ├── SQLite (orders, payment)     # local store
    ├── VietQR URL builder           # img.vietqr.io
    └── CredentialsRepo              # OS keyring
            │
            ▼  Tauri events (orders-updated)
SvelteKit UI (fe/)
```

## Sync

| Job | Interval | Việc |
|-----|----------|------|
| Incremental | ~60s | Lệnh mới theo cửa sổ `sync_state` |
| Active poll | ~15s | Làm mới lệnh 24h (status + list) |
| Cleanup | ~5 phút | Xoá payment detail hết hạn / lệnh không còn in-progress |

Manual: `force_sync_recent`, `force_initial_sync(days)`.

## Payment path

1. Extension hook `fetch`/`XHR` trên Binance → extract BUY payment fields.  
2. `POST` JSON `PAYMENT_DETAIL` → app.  
3. App tạo placeholder order nếu chưa có → upsert `order_payment_detail` → sinh VietQR.  
4. Emit `orders-updated` → UI reload.  
5. QR chỉ hiện khi BUY + `status_code === 1`.

## Status mapping (string → code)

`TRADING/PENDING→1`, `BUYER_PAYED→2`, `DISTRIBUTING→3`, `COMPLETED→4`, `IN_APPEAL→5`, `CANCELLED→6`, `CANCELLED_BY_SYSTEM→7`.
