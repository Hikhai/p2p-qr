# Binance P2P Manager

Ứng dụng desktop Windows để quản lý lệnh Binance P2P: đồng bộ từ API, nhận STK/ngân hàng từ Chrome extension, và sinh mã VietQR chuyển khoản.

**Version 1.0.1** · Windows 10/11 x64 · Tauri 2 + SvelteKit + Rust

---

## Tính năng

- Đồng bộ lệnh BUY/SELL từ Binance C2C API (lần đầu theo số ngày, poll ~15s, incremental ~60s)
- Map trạng thái đúng (`BUYER_PAYED` → chờ người bán xác nhận, …)
- Chrome extension bắt thông tin thanh toán ngay khi vào trang CK trên Binance
- Sinh VietQR; nội dung CK: `{tên chủ TK config} chuyen tien` (giữ đúng hoa/thường)
- Đổi API key → tự xoá dữ liệu tài khoản cũ, tránh trộn lệnh
- API secret lưu trong Windows Credential Manager (không ghi rõ trong SQLite)

---

## Cấu trúc repo

```
p2p-qr/
├── fe/                 # UI SvelteKit (static → Tauri WebView)
├── p2p-extension/      # Chrome MV3 — hook network Binance → HTTP bridge
├── src-tauri/          # Rust: sync, SQLite, VietQR, Axum :1425, keyring
├── README.md           # Tổng quan (file này)
├── USER_GUIDE.md       # Hướng dẫn dùng + build installer/exe
├── ARCHITECTURE.md     # Kiến trúc & luồng dữ liệu
├── WORKFLOW.md         # Quy trình mua / dev / release
└── RELEASE_NOTES.md    # Changelog theo phiên bản
```

---

## Cầu nối Extension ↔ App

| | |
|---|---|
| Protocol | HTTP localhost (không dùng WebSocket) |
| Health | `GET http://127.0.0.1:1425/api/health` |
| Payment | `POST http://127.0.0.1:1425/api/payment-detail` |
| Body | `{ "type": "PAYMENT_DETAIL", "data": { orderNumber, accountNo, bankName, … } }` |

App phải đang chạy; extension chỉ gửi tới `127.0.0.1`.

---

## Quick start (dev)

### Yêu cầu

- Node.js 20+
- Rust stable (`rustup`) — MSVC hoặc GNU + MinGW
- WebView2 Runtime (thường có sẵn trên Windows 10/11)
- Git

### Chạy

```powershell
# Nếu rustup/cargo không nhận trong PowerShell:
# $env:Path += ";$env:USERPROFILE\.cargo\bin"
# Nếu npm bị chặn Execution Policy: dùng npm.cmd

npm.cmd install
cd fe; npm.cmd install; cd ..
npm.cmd run tauri:dev
```

Chrome → `chrome://extensions` → Developer mode → **Load unpacked** → thư mục `p2p-extension/`.

### Build release

```powershell
npm.cmd run tauri:build
```

Output:

| Đường dẫn | Nội dung |
|-----------|----------|
| `src-tauri/target/release/bundle/msi/` | Installer MSI |
| `src-tauri/target/release/bundle/nsis/` | Installer NSIS (`.exe`) |
| `src-tauri/target/release/p2p-qr.exe` | Binary chạy trực tiếp |

Chi tiết môi trường, lỗi PowerShell, và đóng gói extension: xem [USER_GUIDE.md](./USER_GUIDE.md) mục 7.

---

## Tài liệu

| File | Nội dung |
|------|----------|
| [USER_GUIDE.md](./USER_GUIDE.md) | Cài đặt, cấu hình, quy trình mua, xử lý sự cố, build |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | Thành phần, sync, payment path, status map |
| [WORKFLOW.md](./WORKFLOW.md) | Vòng lặp user / dev / release |
| [RELEASE_NOTES.md](./RELEASE_NOTES.md) | Ghi chú phiên bản |

---

## Bảo mật

- API secret chỉ nằm trong OS keyring và process Rust
- Bridge HTTP bind `127.0.0.1`, CORS chỉ origin `chrome-extension://`
- Không gửi dữ liệu ra server thứ ba (ngoài Binance API và `img.vietqr.io` để render QR)

---

## License

MIT
