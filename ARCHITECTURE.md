# Architecture — Binance P2P Manager

Version **1.0.1**. Chi tiết dùng app: [USER_GUIDE.md](./USER_GUIDE.md).

---

## Thành phần

```
Chrome (p2p-extension)
    │  POST /api/payment-detail
    ▼
Tauri / Rust (src-tauri)
    ├── Axum HTTP 127.0.0.1:1425     # bridge extension
    ├── SyncEngine                   # Binance C2C history + poll
    ├── SQLite (orders, payment)     # local store
    ├── VietQR URL builder           # img.vietqr.io + bank BIN map
    └── CredentialsRepo              # OS keyring
            │
            ▼  Tauri events (orders-updated)
SvelteKit UI (fe/)
```

| Thư mục | Vai trò |
|---------|---------|
| `fe/` | UI: Dashboard, Lệnh mua/bán, Đang xử lý, Cài đặt, modal chi tiết + QR |
| `p2p-extension/` | MV3: inject hook `fetch`/`XHR`, queue + POST sang app |
| `src-tauri/` | Backend Tauri commands, sync, DB, HTTP bridge, VietQR |

---

## Sync

| Job | Interval | Việc |
|-----|----------|------|
| Incremental | ~60s | Lệnh mới theo cửa sổ `sync_state` |
| Active poll | ~15s | Làm mới lệnh gần đây (status + list) |
| Cleanup | ~5 phút | Xoá payment detail hết hạn / không còn in-progress |

Manual (từ UI): `force_sync_recent`, `force_initial_sync(days)`.

Đổi API key: clear orders + sync state của tài khoản cũ trước khi lưu key mới.

---

## Payment path

1. Extension hook network trên Binance → extract field thanh toán lệnh BUY.
2. `POST` JSON `PAYMENT_DETAIL` → `127.0.0.1:1425`.
3. App tạo placeholder order nếu chưa có từ API → upsert `order_payment_detail` → sinh URL VietQR.
4. Emit `orders-updated` → UI reload.
5. QR chỉ hiện khi BUY và `status_code === 1` (chờ thanh toán).

### Nội dung CK (`addInfo`)

- Có config tên chủ TK người chuyển → `{tên} chuyen tien` (không normalize case).
- Không config → bỏ `addInfo` khỏi URL QR.

---

## Status mapping (Binance string → code)

| Binance | Code | Ý nghĩa UI |
|---------|------|------------|
| `TRADING` / `PENDING` | 1 | Đang chờ thanh toán |
| `BUYER_PAYED` | 2 | Chờ người bán xác nhận |
| `DISTRIBUTING` | 3 | Đang giải phóng coin |
| `COMPLETED` | 4 | Đã hoàn thành |
| `IN_APPEAL` | 5 | Đang khiếu nại |
| `CANCELLED` | 6 | Đã hủy |
| `CANCELLED_BY_SYSTEM` | 7 | Hủy bởi hệ thống |

---

## Bảo mật & dữ liệu

- Bridge chỉ bind loopback; CORS predicate: origin `chrome-extension://…`
- Credentials: Windows Credential Manager; UI chỉ thấy thông tin đã mask
- DB local dưới `%LOCALAPPDATA%\BinanceP2PManager`
- Outbound: Binance API + `img.vietqr.io` (ảnh QR)
