# Binance P2P Manager — Hướng dẫn sử dụng

**Version 1.0.1** · Windows 10/11 64-bit

Tài liệu này dành cho người dùng cuối và người build từ source. Tổng quan kỹ thuật: [README.md](./README.md), [ARCHITECTURE.md](./ARCHITECTURE.md).

---

## 1. Cài đặt app

### Bản build sẵn

1. Chạy file `.msi` hoặc installer NSIS (`.exe`) từ bản phát hành.
2. Mở **Binance P2P Manager** từ Start Menu / Desktop.
3. Giữ thư mục `p2p-extension/` kèm theo để load vào Chrome (mục 2.3).

### Build từ source

Xem **mục 7**.

---

## 2. Cấu hình lần đầu

### 2.1 API Binance

1. Binance → **API Management** → tạo key quyền **Read** (không cần rút tiền / trade).
2. Trong app → tab **Cài đặt**:
   - Nhập **API Key** + **API Secret**
   - (Khuyến nghị) **Tên chủ TK ngân hàng (người chuyển)** — nhập đúng chữ hoa/thường như trên thẻ hoặc app ngân hàng
   - **Lưu Credentials** → **Test Kết Nối**

Đổi sang API key tài khoản khác: app **tự xoá lệnh/sync cũ** rồi đồng bộ lại — không bị trộn dữ liệu.

### 2.2 Đồng bộ lệnh

- Tab **Cài đặt** → chọn số ngày (1–30) → **Đồng bộ**.
- Dashboard: **Tải lại** / **Cập nhật từ sàn** = kéo dữ liệu mới từ Binance.
- Khi đã có credentials, app tự poll khoảng **15 giây**.

### 2.3 Chrome extension

1. Mở `chrome://extensions` → bật **Developer mode**.
2. **Load unpacked** → chọn thư mục `p2p-extension` (cùng repo hoặc kèm bản phát hành).
3. Giữ extension bật khi giao dịch trên Binance P2P.
4. Kiểm tra app đang chạy: mở `http://127.0.0.1:1425/api/health` → phải thấy `{"status":"ok"}`.

---

## 3. Quy trình mua (BUY) khuyến nghị

1. Trên Binance: xác nhận mua → vào **trang thanh toán**.
2. Extension đọc STK/ngân hàng và gửi về app (`127.0.0.1:1425`).
3. Trong app: mở lệnh → thấy thông tin + **mã VietQR** (chỉ khi trạng thái còn chờ thanh toán).
4. Chuyển khoản / quét QR.
5. Trên Binance bấm **Đã thanh toán** → app cập nhật **Chờ người bán xác nhận**, **ẩn QR**.

### Nội dung chuyển khoản

| Config tên chủ TK | Nội dung CK / QR `addInfo` |
|-------------------|----------------------------|
| Đã cấu hình | `{tên đã lưu} chuyen tien` — giữ đúng hoa/thường như đã nhập |
| Chưa cấu hình | QR không gắn `addInfo` — app ngân hàng dùng mặc định |

Không nhúng mã lệnh vào nội dung CK.

Ô **Nội dung chuyển khoản đề xuất** trong chi tiết lệnh dùng cùng quy tắc trên.

---

## 4. Trạng thái lệnh (thường gặp)

| Mã | Nhãn trên app |
|----|----------------|
| 1 | Đang chờ thanh toán |
| 2 | Chờ người bán xác nhận |
| 3 | Đang giải phóng coin |
| 4 | Đã hoàn thành |
| 5 | Đang khiếu nại |
| 6 | Đã hủy |
| 7 | Hủy bởi hệ thống |

---

## 5. Tabs chính

| Tab | Việc |
|-----|------|
| **Dashboard** | Tổng quan, tải lại, cập nhật từ sàn |
| **Lệnh mua** | Danh sách BUY |
| **Lệnh bán** | Danh sách SELL |
| **Đang xử lý** | Lệnh chưa kết thúc |
| **Cài đặt** | API, tên chủ TK, đồng bộ theo ngày, xoá dữ liệu |

---

## 6. Xử lý sự cố

| Triệu chứng | Cách xử lý |
|-------------|------------|
| Extension không gửi được | App đang chạy? Health `127.0.0.1:1425/api/health` → `ok`. Reload extension. |
| Chưa có QR | Lệnh BUY + status chờ thanh toán; đã vào trang thanh toán Binance; ngân hàng có trong map VietQR. |
| QR sai nội dung CK | Sửa tên chủ TK trong **Cài đặt** → mở lại chi tiết lệnh. |
| Đổi tài khoản vẫn thấy lệnh cũ | Lưu API key mới (app sẽ clear). Hoặc **Xóa toàn bộ dữ liệu** rồi đồng bộ lại. |
| Không sync | **Test Kết Nối**; kiểm tra mạng / IP whitelist Binance API. |
| `npm` lỗi Execution Policy | Dùng `npm.cmd` hoặc `Set-ExecutionPolicy -Scope CurrentUser RemoteSigned`. |
| `rustup` / `cargo` not recognized | Thêm `%USERPROFILE%\.cargo\bin` vào PATH User, mở lại terminal. |

### Dữ liệu local

- SQLite: thư mục data app (`%LOCALAPPDATA%\BinanceP2PManager`) — không copy lung tung khi còn secret trong process.
- Credentials: Windows Credential Manager (`p2p-qr.binance-api`).

---

## 7. Build file cài đặt / exe từ source

### Yêu cầu môi trường Windows

1. **Node.js** 20+ — https://nodejs.org — kiểm tra: `node --version`
2. **Rust** — https://rustup.rs  
   - Sau khi cài: đóng hết terminal hoặc thêm `%USERPROFILE%\.cargo\bin` vào PATH  
   - Kiểm tra: `rustup --version`, `rustc --version`
3. **WebView2 Runtime** — thường đã có trên Win10/11
4. Toolchain C++ / linker:
   - Khuyến nghị: **Visual Studio Build Tools** + workload *Desktop development with C++*, rồi:
     ```powershell
     rustup default stable-x86_64-pc-windows-msvc
     ```
   - Hoặc giữ `windows-gnu` nếu đã có MinGW/linker đầy đủ
5. WiX / NSIS — Tauri thường tự kéo khi bundle

### Lỗi PowerShell thường gặp

**`npm` — running scripts is disabled**

```powershell
npm.cmd --version
npm.cmd install
```

Hoặc:

```powershell
Set-ExecutionPolicy -Scope CurrentUser RemoteSigned
```

**`rustup` / `cargo` not recognized**

```powershell
$env:Path += ";$env:USERPROFILE\.cargo\bin"
rustup --version
```

Vĩnh viễn: *Environment Variables* → Path (User) → thêm `C:\Users\<TênBạn>\.cargo\bin` → mở terminal mới.

### Các bước build

```powershell
git clone https://github.com/Hikhai/p2p-qr.git
cd p2p-qr

# Nếu cần:
# $env:Path += ";$env:USERPROFILE\.cargo\bin"

npm.cmd install
cd fe; npm.cmd install; cd ..

# Thử dev trước (tuỳ chọn)
npm.cmd run tauri:dev

# Build release (lần đầu có thể mất nhiều phút)
npm.cmd run tauri:build
```

### Lấy file sau khi build

Trong `src-tauri/target/release/bundle/`:

| Thư mục | File |
|---------|------|
| `msi/` | Installer MSI (tên dạng `Binance P2P Manager_1.0.1_x64_…`) |
| `nsis/` | Installer `.exe` |

Binary thuần: `src-tauri/target/release/p2p-qr.exe`  
(Vẫn cần WebView2; phân phối cho user nên dùng MSI/NSIS.)

### Extension khi phát hành

Zip kèm thư mục `p2p-extension/` và hướng dẫn **Load unpacked**, hoặc gửi nguyên folder.

---

## 8. Dev nhanh

```powershell
npm.cmd install
cd fe; npm.cmd install; cd ..
npm.cmd run tauri:dev
```

Load `p2p-extension/` vào Chrome. Bridge: `127.0.0.1:1425`.
