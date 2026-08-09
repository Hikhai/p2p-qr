# Binance P2P Manager

Ứng dụng desktop (Tauri 2 + SvelteKit + Rust) để quản lý lệnh Binance P2P, đồng bộ từ API, nhận thông tin thanh toán từ Chrome extension, và sinh mã VietQR chuyển khoản.

**Version:** 1.0.1 · **Platform:** Windows 10/11 x64

## Tính năng chính

- Đồng bộ lệnh BUY/SELL từ Binance C2C API (lần đầu + poll 15s / incremental 60s)
- Hiển thị trạng thái đúng (`BUYER_PAYED` → chờ người bán xác nhận, …)
- Chrome extension bắt thông tin STK/ngân hàng ngay khi vào trang thanh toán
- Sinh VietQR; nội dung CK theo config `{tên chủ TK} chuyen tien` (hoặc mặc định app ngân hàng nếu chưa cấu hình)
- Đổi API key → tự xoá dữ liệu tài khoản cũ, tránh trộn lệnh
- Credentials lưu trong Windows Credential Manager (không lưu secret dạng rõ trong SQLite)

## Cấu trúc repo

```
p2p-qr/
├── fe/                 # SvelteKit UI
├── p2p-extension/      # Chrome MV3 extension
├── src-tauri/          # Rust backend (Tauri, Axum :1425, SQLite, sync)
├── README.md
└── USER_GUIDE.md
```

## Cầu nối Extension ↔ App

| | |
|---|---|
| Protocol | HTTP (không dùng WebSocket) |
| Health | `GET http://127.0.0.1:1425/api/health` |
| Payment | `POST http://127.0.0.1:1425/api/payment-detail` |
| Body | `{ "type": "PAYMENT_DETAIL", "data": { orderNumber, accountNo, bankName, … } }` |

## Development

### Yêu cầu

- Node.js 20+
- Rust stable (MSVC hoặc GNU + MinGW đầy đủ)
- WebView2 Runtime (Windows)
- Git

### Chạy dev

```bash
npm install
cd fe && npm install && cd ..
npm run tauri:dev
```

Load extension: Chrome → `chrome://extensions` → Developer mode → Load unpacked → chọn thư mục `p2p-extension/`.

### Build installer / exe

Xem mục **Build** bên dưới hoặc `USER_GUIDE.md`.

```bash
npm run tauri:build
```

File output mặc định:

- `src-tauri/target/release/bundle/msi/` — installer MSI  
- `src-tauri/target/release/bundle/nsis/` — installer NSIS (exe)  
- `src-tauri/target/release/p2p-qr.exe` — binary chạy trực tiếp  

## Tài liệu

| File | Nội dung |
|------|----------|
| [USER_GUIDE.md](./USER_GUIDE.md) | Hướng dẫn dùng app + extension + build |
| [RELEASE_NOTES.md](./RELEASE_NOTES.md) | Ghi chú phiên bản |

## Bảo mật

- API secret chỉ nằm trong kho khoá hệ thống và process Rust
- Bridge HTTP chỉ bind `127.0.0.1`, CORS giới hạn origin extension
- Không gửi dữ liệu ra server thứ ba (ngoài Binance API và img.vietqr.io)

## License

MIT (private/internal tuỳ chính sách phân phối của bạn).
