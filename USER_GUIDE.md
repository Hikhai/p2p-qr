# Binance P2P Manager - Hướng Dẫn Sử Dụng

## 📥 Cài Đặt

1. Tải file `Binance P2P Manager_1.0.0_x64_en-US.msi` 
2. Chạy file installer và làm theo hướng dẫn
3. Mở app từ Desktop hoặc Start Menu

## 🚀 Bắt Đầu Nhanh

### 1. Cấu Hình API Credentials

**Bước 1:** Lấy API Key từ Binance
- Đăng nhập vào Binance.com
- Vào **User Center** → **API Management**
- Tạo API Key mới với quyền **Read** (không cần Write)
- Copy **API Key** và **Secret Key**

**Bước 2:** Nhập vào App
- Mở app → Tab **⚙️ Cài đặt**
- Nhập API Key và API Secret
- Click **💾 Lưu Credentials**
- Click **🔌 Test Connection** để kiểm tra

⚠️ **Lưu ý:** Sau khi lưu, bạn không thể sửa credentials. Để thay đổi, cần xóa toàn bộ dữ liệu.

### 2. Đồng Bộ Dữ Liệu

**Đồng bộ lần đầu:**
- Tab **⚙️ Cài đặt** → Click **📥 Đồng bộ ngay**
- Chọn số ngày muốn đồng bộ (mặc định: 7 ngày)
- Đợi quá trình hoàn tất

**Tự động làm mới:**
- App tự động refresh mỗi 30 giây
- Có thể tắt bằng nút **⏸️ Dừng tự động làm mới**

### 3. Xem Danh Sách Lệnh

**Dashboard:**
- Tab **📊 Dashboard**: Tổng quan tất cả lệnh
- Tab **🟢 Mua**: Chỉ lệnh mua
- Tab **🔴 Bán**: Chỉ lệnh bán
- Tab **⏳ Đang xử lý**: Lệnh chưa hoàn thành

**Thông tin hiển thị:**
- Mã lệnh, trạng thái
- Loại (Mua/Bán), tài sản, tiền tệ
- Giá, số lượng, tổng
- Đối tác giao dịch
- Thời gian tạo, thanh toán, hoàn tất

### 4. Xem Chi Tiết Lệnh

- Click vào bất kỳ lệnh nào
- Xem đầy đủ thông tin giao dịch
- Với lệnh đang xử lý: hiển thị thông tin thanh toán (nếu có)
  - Tên tài khoản, số tài khoản
  - Tên ngân hàng, chi nhánh
  - Mã QR VietQR (nếu là ngân hàng Việt Nam)

## 🔌 Chrome Extension (Tùy chọn)

**Cài đặt:**
1. Mở Chrome → Extensions → Developer mode
2. Load unpacked → chọn folder `fe/p2p-extension`
3. Extension sẽ tự động gửi thông tin thanh toán về app

**Sử dụng:**
- Vào trang chi tiết lệnh P2P trên Binance
- Extension tự động scrape thông tin
- App tự động nhận và hiển thị

## 🗑️ Xóa Toàn Bộ Dữ Liệu

**Khi nào cần:**
- Muốn thay đổi API credentials
- Reset app về trạng thái ban đầu
- Xóa tất cả lệnh đã đồng bộ

**Cách làm:**
1. Tab **⚙️ Cài đặt** → Cuộn xuống
2. Click **🗑️ Xóa Toàn Bộ Dữ Liệu**
3. Xác nhận
4. **App sẽ tắt ngay lập tức**
5. **Đợi 2 giây** rồi mở lại app

⚠️ **Hành động này không thể hoàn tác!**

## 🐛 Xử Lý Sự Cố

**Không kết nối được API:**
- Kiểm tra API Key/Secret đúng chưa
- Kiểm tra internet
- Kiểm tra IP có bị Binance chặn không

**App không hiển thị dữ liệu:**
- Click **🔄 Làm mới** để tải lại
- Kiểm tra đã đồng bộ chưa

**App bị treo:**
- Đợi vài giây (có thể do đang đồng bộ)
- Restart app
- Xóa file `p2p_app.db` rồi mở lại

## 📂 Dữ Liệu

**Vị trí:** 
- Database: `p2p_app.db` (cùng thư mục với app)
- Credentials: Được mã hóa trong database

**Backup:**
- Copy file `p2p_app.db` để backup
- Restore bằng cách paste lại file này

## ⚙️ Tùy Chọn Nâng Cao

**Build từ source:**
```bash
git clone <repo>
cd tauri-app-main
npm install
cd fe && npm install && cd ..
npm run tauri:build
```

**Development:**
```bash
npm run tauri:dev
```

## 📞 Hỗ Trợ

Nếu gặp vấn đề, vui lòng:
1. Kiểm tra file log (console trong app)
2. Thử xóa dữ liệu và cấu hình lại
3. Liên hệ support

---

**Version:** 1.0.0  
**Platform:** Windows 10/11 64-bit
