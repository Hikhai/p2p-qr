# Workflow vận hành

## User: mua USDT (BUY)

1. Cấu hình API + (tuỳ chọn) tên chủ TK người chuyển trong app.  
2. Load `p2p-extension` trên Chrome.  
3. Binance P2P: match lệnh → xác nhận mua → trang thanh toán.  
4. Extension gửi STK/NH về app → app hiện QR.  
5. Chuyển khoản / quét QR.  
6. Binance: Đã thanh toán → app đổi status, ẩn QR.  
7. Chờ seller release.

## Dev loop

```bash
npm run tauri:dev          # UI + Rust hot reload
# Chrome: reload unpacked extension sau khi sửa p2p-extension/
```

## Release loop

```bash
npm run tauri:build
# Lấy MSI/NSIS trong src-tauri/target/release/bundle/
# Kèm zip p2p-extension/ cho user
```
