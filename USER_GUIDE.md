# Binance P2P Manager — Hướng dẫn sử dụng

Version **1.0.1** · Windows 10/11 64-bit

---

## 1. Cài đặt app

### Từ bản build sẵn

1. Chạy file `.msi` hoặc `.exe` (NSIS) trong thư mục release.
2. Mở **Binance P2P Manager** từ Start Menu / Desktop.

### Build từ source (xem mục 7)

---

## 2. Cấu hình lần đầu

### 2.1 API Binance

1. Binance → **API Management** → tạo key quyền **Read** (không cần rút tiền / trade).
2. Trong app → tab **Cài đặt**:
   - Nhập API Key + API Secret
   - (Khuyến nghị) **Tên chủ TK ngân hàng (người chuyển)** — đúng chữ hoa/thường như trên thẻ
   - **Lưu Credentials** → **Test Kết Nối**

Đổi sang API key tài khoản khác: app **tự xoá lệnh/sync cũ** rồi đồng bộ lại — không bị trộn dữ liệu.

### 2.2 Đồng bộ lệnh

- Tab **Cài đặt** → chọn số ngày (1–30) → **Đồng bộ**.
- Dashboard: **Tải lại** / **Cập nhật từ sàn** = kéo dữ liệu mới từ Binance.
- App tự poll ~15 giây khi đã có credentials.

### 2.3 Chrome extension

1. Mở `chrome://extensions` → bật **Developer mode**.
2. **Load unpacked** → chọn thư mục `p2p-extension` (cùng repo / kèm bản phát hành).
3. Giữ extension bật khi giao dịch trên Binance P2P.

---

## 3. Quy trình mua (BUY) khuyến nghị

1. Trên Binance: xác nhận mua → vào **trang thanh toán**.
2. Extension đọc STK/ngân hàng và gửi về app (`127.0.0.1:1425`).
3. Trong app: mở lệnh → thấy thông tin + **mã VietQR** (chỉ khi trạng thái còn chờ thanh toán).
4. Chuyển khoản / quét QR.
5. Trên Binance bấm **Đã thanh toán** → app cập nhật trạng thái **Chờ người bán xác nhận**, **ẩn QR**.

### Nội dung chuyển khoản

| Config tên chủ TK | Nội dung CK / QR `addInfo` |
|-------------------|----------------------------|
| Đã cấu hình | `{tên đã lưu} chuyen tien` (giữ đúng hoa/thường) |
| Chưa cấu hình | QR không gắn `addInfo` — app ngân hàng dùng mặc định |

Không nhúng mã lệnh vào nội dung CK.

---

## 4. Trạng thái lệnh (thường gặp)

| Mã | Nhãn |
|----|------|
| 1 | Đang chờ thanh toán |
| 2 | Chờ người bán xác nhận |
| 3 | Đang giải phóng coin |
| 4 | Đã hoàn thành |
| 5 | Đang khiếu nại |
| 6 | Đã hủy |
| 7 | Hủy bởi hệ thống |

---

## 5. Tabs chính

- **Dashboard** — tổng quan, tải lại, cập nhật từ sàn  
- **Mua / Bán / Đang xử lý** — lọc danh sách  
- **Cài đặt** — API, tên chủ TK, đồng bộ, xoá dữ liệu  

---

## 6. Xử lý sự cố

| Triệu chứng | Cách xử lý |
|-------------|------------|
| Extension không gửi được | App đang chạy? `http://127.0.0.1:1425/api/health` → `{"status":"ok"}`. Reload extension. |
| Chưa có QR | Lệnh phải là BUY + status chờ thanh toán; đã vào trang thanh toán Binance; ngân hàng được hỗ trợ VietQR. |
| QR sai nội dung CK | Cập nhật tên chủ TK trong Cài đặt → mở lại chi tiết lệnh. |
| Đổi tài khoản vẫn thấy lệnh cũ | Lưu API key mới (app sẽ clear). Hoặc **Xóa toàn bộ dữ liệu** rồi đồng bộ lại. |
| Không sync | Test kết nối API; kiểm tra mạng / IP Binance. |

**Dữ liệu local**

- SQLite: thư mục data của app (Tauri app data), không nên copy lung tung secret.
- Credentials: Windows Credential Manager (`p2p-qr.binance-api`).

---

## 7. Build file cài đặt / exe từ source

### Yêu cầu môi trường Windows

1. **Node.js** 20+ — https://nodejs.org  
2. **Rust** — https://rustup.rs (`rustup default stable`)  
3. **WebView2 Runtime** — thường đã có trên Win10/11  
4. Toolchain C++:
   - Khuyến nghị: **Visual Studio Build Tools** + workload “Desktop development with C++”, rồi:
     ```bash
     rustup default stable-x86_64-pc-windows-msvc
     ```
   - Hoặc MinGW đầy đủ nếu dùng target `windows-gnu`  
5. (Tuỳ chọn) WiX / NSIS — Tauri thường tự kéo khi build bundle

### Các bước

```bash
git clone https://github.com/Hikhai/p2p-qr.git
cd p2p-qr

npm install
cd fe && npm install && cd ..

# Dev (thử trước khi release)
npm run tauri:dev

# Build release
npm run tauri:build
```

### Lấy file sau khi build

Trong `src-tauri/target/release/bundle/`:

| Thư mục | File |
|---------|------|
| `msi/` | `Binance P2P Manager_1.0.1_x64_en-US.msi` (tên có thể hơi khác) |
| `nsis/` | installer `.exe` |

Binary thuần: `src-tauri/target/release/p2p-qr.exe`  
(Khi chạy exe thuần, vẫn cần WebView2; phân phối cho user nên dùng MSI/NSIS.)

### Extension khi phát hành

Đóng gói kèm thư mục `p2p-extension/` và hướng dẫn user **Load unpacked**, hoặc zip riêng.

---

## 8. Dev nhanh

```bash
npm install
cd fe && npm install && cd ..
npm run tauri:dev
```

Load `p2p-extension/` vào Chrome. Bridge: `127.0.0.1:1425`.
