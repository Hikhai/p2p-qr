# 🚀 P2P Binance Order Manager - Luồng Hoạt Động

## 📝 Tóm tắt ngắn gọn

App này giúp **tự động thu thập thông tin thanh toán** từ Binance P2P và **tạo QR code chuyển khoản** cho các ngân hàng Việt Nam.

---

## 🔄 Luồng hoạt động (5 bước chính)

```
┌────────────────────────────────────────────────────────────────┐
│  1️⃣  BẠN MỞ TRANG CHI TIẾT ĐƠN HÀNG TRÊN BINANCE P2P         │
│      https://p2p.binance.com/vi/fiatorderdetail/...           │
└────────────────────────────────────────────────────────────────┘
                              ↓
┌────────────────────────────────────────────────────────────────┐
│  2️⃣  EXTENSION TỰ ĐỘNG BẮT THÔNG TIN                          │
│                                                                │
│  injected.js: Hook vào API Binance                            │
│  - API: /bapi/c2c/v2/private/c2c/order-match/order-detail    │
│  - Trích xuất thông tin:                                      │
│    ✓ Số lệnh                                                  │
│    ✓ Tên ngân hàng (BIDV, Vietcombank, Techcombank, v.v.)   │
│    ✓ Số tài khoản                                            │
│    ✓ Tên chủ tài khoản                                       │
│    ✓ Số tiền (VND)                                           │
│    ✓ Nội dung chuyển khoản                                   │
│                                                                │
│  content.js: Kiểm tra dữ liệu đầy đủ                         │
│  background.js: Gửi qua HTTP đến app (có retry nếu lỗi)     │
└────────────────────────────────────────────────────────────────┘
                              ↓
┌────────────────────────────────────────────────────────────────┐
│  3️⃣  APP NHẬN DỮ LIỆU VÀ TẠO QR CODE                          │
│                                                                │
│  HTTP Server (Rust): Nhận request từ extension               │
│  - Endpoint: POST http://127.0.0.1:1425/api/payment-detail  │
│                                                                │
│  VietQR Generator: Tạo mã QR chuyển khoản                    │
│  - Tìm mã BIN của ngân hàng (ví dụ: BIDV = 970418)         │
│  - Tạo URL VietQR:                                           │
│    https://img.vietqr.io/image/970418-96247668-compact2.jpg  │
│    ?amount=200000&addInfo=ABCXYZ123                          │
│                                                                │
│  SQLite Database: Lưu thông tin vào database                │
│  - Bảng: order_payment_detail                                │
│  - Tự động xóa sau 24 giờ                                    │
└────────────────────────────────────────────────────────────────┘
                              ↓
┌────────────────────────────────────────────────────────────────┐
│  4️⃣  APP TỰ ĐỘNG CẬP NHẬT GIAO DIỆN                           │
│                                                                │
│  Tauri Event System: Phát s�� kiện "orders-updated"           │
│  Frontend (Svelte): Nhận sự kiện và refresh UI               │
└────────────────────────────────────────────────────────────────┘
                              ↓
┌────────────────────────────────────────────────────────────────┐
│  5️⃣  BẠN THẤY THÔNG TIN THANH TOÁN VÀ QR CODE                 │
│                                                                │
│  Hiển thị:                                                     │
│  ┌──────────────────────────────────────────────────────┐    │
│  │ 💳 Thông tin chuyển khoản                             │    │
│  │                                                       │    │
│  │ Ngân hàng:        BIDV                               │    │
│  │ Số tài khoản:     96247668                           │    │
│  │ Chủ tài khoản:    NGUYEN VAN A                       │    │
│  │ Số tiền:          200,000 VND                        │    │
│  │ Nội dung:         ABCXYZ123                          │    │
│  │                                                       │    │
│  │ 📱 Mã QR chuyển khoản                                │    │
│  │ ┌───────────────────────────┐                       │    │
│  │ │                           │                       │    │
│  │ │        [QR CODE]          │  ← Quét bằng         │    │
│  │ │                           │     app ngân hàng     │    │
│  │ └───────────────────────────┘                       │    │
│  └──────────────────────────────────────────────────────┘    │
└────────────────────────────────────────────────────────────────┘
```

---

## 🎯 Các tính năng chính

### ✅ 1. Tự động thu thập (Auto Capture)
- **Không cần copy/paste thủ công**
- Extension tự động bắt thông tin từ Binance
- Hoạt động ngầm, không làm gián đoạn

### ✅ 2. Trích xuất thông minh (Smart Extraction)
- **Động - không phụ thuộc vào thứ tự trường**
- Tìm trường theo tên (hỗ trợ tiếng Việt có/không dấu, tiếng Anh)
- Hoạt động với mọi loại đơn hàng

### ✅ 3. Tạo QR code tự động (VietQR)
- **Không dùng QR của Binance** (không có hoặc không đúng format)
- Tạo QR code chuẩn VietQR cho ngân hàng Việt Nam
- Quét bằng app ngân hàng → tự động điền thông tin

### ✅ 4. Cơ chế retry thông minh
- **Extension có thể mở trước hoặc sau app**
- Tự động kiểm tra backend mỗi 5 giây
- Queue request và gửi lại khi app sẵn sàng

### ✅ 5. Cập nhật realtime
- **UI tự động refresh** khi có dữ liệu mới
- Không cần F5 hoặc reload trang

---

## 🏦 Ngân hàng được hỗ trợ (26+)

| Tên ngân hàng | Mã BIN | Tên ngân hàng | Mã BIN |
|--------------|--------|--------------|--------|
| BIDV | 970418 | Vietcombank | 970436 |
| Techcombank | 970407 | VPBank | 970432 |
| ACB | 970416 | MB | 970422 |
| Agribank | 970405 | SHB | 970443 |
| VietinBank | 970415 | TPBank | 970423 |
| Sacombank | 970403 | HDBank | 970437 |
| VIB | 970441 | MSB | 970426 |
| OCB | 970448 | SeABank | 970440 |
| *và 10+ ngân hàng khác...* | | | |

---

## 💻 Cấu trúc code đơn giản

```
📁 fe/p2p-extension/
  ├── injected.js       → Bắt API Binance (chạy trong page)
  ├── content.js        → Kiểm tra dữ liệu (chạy trong extension)
  └── background.js     → Gửi HTTP đến app (service worker)

📁 src-tauri/src/
  └── main.rs           → HTTP server + VietQR + Database

📁 fe/src/
  └── lib/OrderDetail.svelte → Hiển thị UI
```

---

## 🔧 Cách sử dụng

### Bước 1: Cài đặt Extension
1. Mở `chrome://extensions/`
2. Bật "Developer mode"
3. Click "Load unpacked"
4. Chọn thư mục `fe/p2p-extension/`

### Bước 2: Chạy App
```bash
npm run tauri dev
```

### Bước 3: Sử dụng
1. Mở trang chi tiết đơn hàng trên Binance P2P
2. Extension tự động bắt thông tin
3. Mở app để xem chi tiết và QR code
4. Quét QR bằng app ngân hàng để chuyển tiền

---

## 🐛 Debug nhanh

### Extension không bắt dữ liệu?
```
1. Mở Console (F12)
2. Tìm log: [P2P Ext v0.4.0] Injected script loaded
3. Refresh trang Binance
```

### App không nhận dữ liệu?
```
1. Kiểm tra terminal có log: [HTTP Server] Listening on http://127.0.0.1:1425
2. Test health: curl http://127.0.0.1:1425/api/health
3. Kiểm tra Windows Firewall
```

### QR code không hiện?
```
1. Xem terminal log: [DEBUG] Generating VietQR from backend...
2. Kiểm tra log: [DEBUG] Found BIN code: 970418
3. Nếu "Failed to find BIN code" → ngân hàng chưa được hỗ trợ
```

---

## 📊 Flow chart đơn giản

```
User opens Binance order page
        ↓
Extension intercepts API
        ↓
Extract payment info
        ↓
Validate data complete
        ↓
Send HTTP to app (with retry)
        ↓
App generates VietQR
        ↓
Save to SQLite
        ↓
Emit update event
        ↓
UI refreshes
        ↓
User sees QR code
```

---

## 🎓 Kiến thức kỹ thuật

### Extension Architecture
- **Injected Script**: Chạy trong page context, access Binance API
- **Content Script**: Bridge giữa page và extension
- **Background Script**: Service worker, HTTP client

### Backend Architecture
- **Axum**: Lightweight HTTP server framework (Rust)
- **SQLite**: Embedded database, không cần setup
- **Tauri**: Cross-platform desktop framework

### API Integration
- **Binance API**: `/bapi/c2c/v2/private/c2c/order-match/order-detail`
- **VietQR API**: `https://img.vietqr.io/image/{BIN}-{ACCOUNT}-compact2.jpg`

---

## 📌 Lưu ý quan trọng

1. **Chỉ hoạt động local**: HTTP server bind 127.0.0.1 (localhost)
2. **Không có authentication**: Giả định môi trường tin cậy
3. **Tự động xóa**: Dữ liệu trong database tự xóa sau 24 giờ
4. **VietQR public API**: Không cần đăng ký, miễn phí sử dụng

---

## 🚀 Next Steps

Sau khi hiểu luồng hoạt động, bạn có thể:
1. ✅ Test với đơn hàng thật trên Binance
2. ✅ Thêm ngân hàng mới vào `get_bank_bin()`
3. ✅ Tùy chỉnh UI theo ý muốn
4. ✅ Export dữ liệu ra CSV/Excel

---

**Tài liệu chi tiết**: Xem `ARCHITECTURE.md` để hiểu sâu hơn về cấu trúc hệ thống.

**Version**: 0.4.0
**Last Updated**: October 15, 2025
