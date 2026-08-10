# Workflow — Binance P2P Manager

## User: mua USDT (BUY)

1. Cấu hình API + (khuyến nghị) nội dung chuyển khoản trong tab **Cài đặt**.
2. Load `p2p-extension` trên Chrome (Developer mode → Load unpacked).
3. Binance P2P: match lệnh → xác nhận mua → vào trang thanh toán.
4. Extension gửi STK/NH về app → mở lệnh trên app → hiện VietQR (status chờ thanh toán).
5. Chuyển khoản / quét QR (nội dung CK theo config nếu đã lưu).
6. Binance: **Đã thanh toán** → app đổi status, ẩn QR.
7. Chờ seller release / hoàn tất.

## User: bán USDT (SELL) với bot

1. Tab **Bot**: cấu hình tin chào, nội dung CK QR (`{ten_nguoi_mua}`…), tin hoàn tất; (tuỳ chọn) TK ngân hàng dự phòng.
2. **Bắt đầu bot** — cần đã lưu API key (Cài đặt).
3. Khi có lệnh BÁN `TRADING`: bot gửi tin chào + ảnh VietQR vào chat Binance.
4. Buyer chuyển khoản / quét QR.
5. Sau khi bạn mở khóa và lệnh **hoàn tất thật**: bot gửi tin 3 (không gửi lúc còn nút mở khóa).
6. Nên tắt **auto-reply** trên Binance merchant để tránh tin trùng.

Chi tiết code: [FLOW_CODE.md](./FLOW_CODE.md).

## Dev loop

```powershell
npm.cmd install
cd fe; npm.cmd install; cd ..
npm.cmd run tauri:dev
```

Sau khi sửa `p2p-extension/`: Chrome → Extensions → Reload extension.

## Release loop

```powershell
npm.cmd run tauri:build
```

1. Lấy MSI/NSIS trong `src-tauri/target/release/bundle/`.
2. Zip kèm `p2p-extension/` cho user.
3. Cập nhật [RELEASE_NOTES.md](./RELEASE_NOTES.md) nếu đổi phiên bản.
