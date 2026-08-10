# Binance P2P Manager — Hướng dẫn sử dụng

**Version 1.0.2** · Windows 10/11 64-bit

Tài liệu này dành cho người dùng cuối và người build từ source. Tổng quan: [README.md](./README.md) · kiến trúc: [ARCHITECTURE.md](./ARCHITECTURE.md) · luồng code: [FLOW_CODE.md](./FLOW_CODE.md).

---

## 1. Cài đặt app

### Bản build sẵn

1. Chạy installer **NSIS** (`Binance P2P Manager_x.x.x_x64-setup.exe`) hoặc `.msi` — **không** chạy tay file `p2p-qr.exe` tách rời.
2. Mở **Binance P2P Manager** từ Start Menu / Desktop.
3. Giữ thư mục `p2p-extension/` kèm theo để load vào Chrome (mục 2.3).

> **Lỗi `WebView2Loader.dll was not found`:** thường do mở `p2p-qr.exe` một mình (thiếu DLL cạnh exe). Dùng installer NSIS, hoặc copy cả `p2p-qr.exe` **và** `WebView2Loader.dll` trong cùng thư mục. Máy cũng cần [WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/) (Windows 10/11 thường đã có).

### Build từ source

Xem **mục 7**.

---

## 2. Cấu hình lần đầu

### 2.1 API Binance

1. Binance → **API Management** → tạo key quyền **Read** (không cần rút tiền / trade).
2. Trong app → tab **Cài đặt**:
   - Nhập **API Key** + **API Secret**
   - (Khuyến nghị) **Nội dung chuyển khoản** — nhập đủ nội dung muốn dùng trên CK/QR, đúng chữ hoa/thường
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

| Config nội dung CK | Nội dung CK / QR `addInfo` |
|--------------------|----------------------------|
| Đã cấu hình | Dùng nguyên `{nội dung}` đã lưu — giữ đúng hoa/thường |
| Chưa cấu hình | QR không gắn `addInfo` — app ngân hàng dùng mặc định |

Không tự thêm hậu tố; không nhúng mã lệnh vào nội dung CK.

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
| **Bot** | Auto chat + QR cho lệnh BÁN (start/stop, tin nhắn, log) |
| **Cài đặt** | API, nội dung CK (BUY), đồng bộ theo ngày, xoá dữ liệu |

---

## 5b. Bot lệnh bán (SELL)

1. Vào tab **Bot** (cần đã lưu API key ở Cài đặt).
2. Cấu hình:
   - **Tin chào** — gửi khi lệnh mới chờ thanh toán
   - **Nội dung CK trong QR** — placeholder: `{ten_nguoi_mua}`, `{ma_lenh}`, `{so_tien}` (để trống = mặc định ngân hàng)
   - **Tin hoàn tất** — chỉ gửi khi giao dịch đã xong thật
   - (Tuỳ chọn) tài khoản ngân hàng dự phòng nếu lệnh thiếu `payMethods`
3. Bấm **Bắt đầu** — bot poll lệnh SELL và lắng nghe chat WebSocket.
4. Khi có lệnh `TRADING`: gửi tin chào + ảnh VietQR vào chat Binance.
5. Tin cảm ơn **không** gửi lúc buyer mới báo đã trả / còn nút mở khóa; chỉ sau khi lệnh hoàn tất và không còn mở khóa được.
6. Nên tắt trả lời tự động (auto-reply) trên Binance để tránh tin trùng với bot.

State bot: `%LOCALAPPDATA%\BinanceP2PManager\bot_config.json`, `bot_state.json`.

---

## 6. Xử lý sự cố

| Triệu chứng | Cách xử lý |
|-------------|------------|
| Extension không gửi được | App đang chạy? Health `127.0.0.1:1425/api/health` → `ok`. Reload extension. |
| Chưa có QR | Lệnh BUY + status chờ thanh toán; đã vào trang thanh toán Binance; ngân hàng có trong map VietQR. |
| QR sai nội dung CK | Sửa **Nội dung chuyển khoản** trong Cài đặt → mở lại chi tiết lệnh. |
| Đổi tài khoản vẫn thấy lệnh cũ | Lưu API key mới (app sẽ clear). Hoặc **Xóa toàn bộ dữ liệu** rồi đồng bộ lại. |
| Không sync | **Test Kết Nối**; kiểm tra mạng / IP whitelist Binance API. |
| `WebView2Loader.dll was not found` | Cài bằng NSIS/MSI, hoặc để `WebView2Loader.dll` cùng thư mục với `p2p-qr.exe`. |
| Bot không gửi tin1/QR | Bot đang chạy? Chat WS đã kết nối (log)? Lệnh còn `TRADING`? |
| QR CK thiếu chữ (vd. TRAN UC…) | Dùng bản ≥1.0.2 (chuẩn hóa `Ð`→`D`); kiểm tra placeholder `{ten_nguoi_mua}`. |
| Tin cảm ơn gửi sớm | Cần bản ≥1.0.2 (verify `checkIfCanReleaseCoin`); tắt auto-reply Binance. |
| `npm` lỗi Execution Policy | Dùng `npm.cmd` hoặc `Set-ExecutionPolicy -Scope CurrentUser RemoteSigned`. |
| `rustup` / `cargo` not recognized | Thêm `%USERPROFILE%\.cargo\bin` vào PATH User, mở lại terminal. |

### Dữ liệu local

- SQLite + bot config/state: `%LOCALAPPDATA%\BinanceP2PManager`
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
   - Hoặc giữ `windows-gnu` nếu đã có MinGW đầy đủ (`dlltool.exe` trong PATH, ví dụ `C:\mingw64\bin`)
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
