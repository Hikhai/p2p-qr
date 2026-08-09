# Workflow — Binance P2P Manager

## User: mua USDT (BUY)

1. Cấu hình API + (khuyến nghị) tên chủ TK người chuyển trong tab **Cài đặt**.
2. Load `p2p-extension` trên Chrome (Developer mode → Load unpacked).
3. Binance P2P: match lệnh → xác nhận mua → vào trang thanh toán.
4. Extension gửi STK/NH về app → mở lệnh trên app → hiện VietQR (status chờ thanh toán).
5. Chuyển khoản / quét QR (nội dung CK theo config nếu đã lưu tên).
6. Binance: **Đã thanh toán** → app đổi status, ẩn QR.
7. Chờ seller release / hoàn tất.

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
