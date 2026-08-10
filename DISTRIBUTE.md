# Đóng gói gửi khách hàng (Windows)

Gửi khách **installer NSIS** — không gửi file `p2p-qr.exe` lẻ (sẽ lỗi thiếu `WebView2Loader.dll`).

---

## 1. Build trên máy bạn

Mở PowerShell tại thư mục project:

```powershell
cd C:\Users\Admin\Projects\p2p-qr

# Quan trọng: đừng để CARGO_TARGET_DIR trỏ sang thư mục tạm
Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue

npm.cmd install
cd fe; npm.cmd install; cd ..
npm.cmd run tauri:build
```

Chờ build xong (có thể 5–15 phút lần đầu).

---

## 2. Lấy file gửi khách

File cần gửi:

```
src-tauri\target\release\bundle\nsis\Binance P2P Manager_1.0.2_x64-setup.exe
```

Nên zip kèm extension:

| Trong zip | Mục đích |
|-----------|----------|
| `Binance P2P Manager_1.0.2_x64-setup.exe` | Cài app desktop |
| `p2p-extension\` (cả thư mục) | Load vào Chrome (BUY / bắt STK) |

Ví dụ tạo zip:

```powershell
$out = "C:\Users\Admin\Projects\p2p-qr\dist-customer"
New-Item -ItemType Directory -Force -Path $out | Out-Null
Copy-Item "src-tauri\target\release\bundle\nsis\Binance P2P Manager_1.0.2_x64-setup.exe" $out
Copy-Item "p2p-extension" "$out\p2p-extension" -Recurse
Compress-Archive -Path "$out\*" -DestinationPath "$out\Binance-P2P-Manager-1.0.2.zip" -Force
```

Gửi khách file `Binance-P2P-Manager-1.0.2.zip`.

---

## 3. Hướng dẫn khách cài

1. Giải nén zip.
2. Chạy **`Binance P2P Manager_1.0.2_x64-setup.exe`** → Next → Install.
3. Mở app từ Start Menu / Desktop (**Binance P2P Manager**).
4. Vào tab **Cài đặt**: nhập API Key + Secret → Lưu → Test kết nối.
5. (Nếu dùng BUY) Chrome → `chrome://extensions` → Developer mode → **Load unpacked** → chọn thư mục `p2p-extension`.
6. (Nếu dùng bot SELL) Tab **Bot** → cấu hình tin nhắn → **Bắt đầu**.

Yêu cầu máy khách: Windows 10/11 64-bit. Installer sẽ tự cài WebView2 nếu máy chưa có.

---

## 4. Không làm vậy

| Sai | Đúng |
|-----|------|
| Gửi `p2p-qr.exe` từ `target\release\` | Gửi `*-setup.exe` (NSIS) |
| Copy mỗi file `.exe` sang USB | Zip setup + `p2p-extension` |
| Chạy exe trong thư mục build tạm | Cài bằng installer rồi mở từ Start Menu |

---

## 5. Lỗi thường gặp

| Lỗi | Cách xử lý |
|-----|------------|
| `WebView2Loader.dll was not found` | Bản build cũ thiếu DLL trong installer (gnu). Dùng setup mới (≥ bản đã fix resources), hoặc copy `WebView2Loader.dll` vào cùng thư mục app (`%LOCALAPPDATA%\Binance P2P Manager\`). |
| Windows SmartScreen chặn | More info → Run anyway (chưa ký code certificate) |
| App không mở sau cài | Cài [WebView2 Runtime](https://go.microsoft.com/fwlink/p/?LinkId=2124703) rồi mở lại |
