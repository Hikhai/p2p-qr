# Release Notes — Binance P2P Manager

## v1.0.2

### Bot SELL (mới)

- Tab **Bot**: tự gửi tin chào + ảnh VietQR vào chat khi lệnh BÁN chờ thanh toán
- Tin hoàn tất chỉ gửi khi detail = `COMPLETED` **và** `checkIfCanReleaseCoin = false` (không gửi lúc còn mở khóa)
- Placeholder CK QR: `{ten_nguoi_mua}`, `{ma_lenh}`, `{so_tien}`; chuẩn hóa `Ð`/`Đ` → `D`
- Dedup theo `bot_state.json`; upload ảnh chat (presign → CDN → WS `imageType=IMAGE`)
- Log gọn trong panel bot

### Docs

- Cập nhật README, USER_GUIDE, ARCHITECTURE, WORKFLOW
- Thêm [FLOW_CODE.md](./FLOW_CODE.md) — luồng hoạt động + file/hàm liên quan

---

## v1.0.1

### Đồng bộ & trạng thái

- Map đúng enum Binance: `BUYER_PAYED`, `DISTRIBUTING`, `IN_APPEAL`, …
- Nhãn “Chờ người bán xác nhận” khi buyer đã thanh toán
- Poll tự động ~15s; nút Tải lại / Cập nhật từ sàn gọi sync thật
- Đổi API key → xoá orders/sync cũ, tránh trộn tài khoản

### Extension & VietQR

- HTTP bridge `127.0.0.1:1425` (không còn WebSocket)
- Capture sớm khi vào trang thanh toán; placeholder order nếu chưa sync API
- Ẩn QR sau khi status không còn “chờ thanh toán”
- Nội dung CK / QR `addInfo`: nguyên chuỗi user cấu hình (giữ hoa/thường); không tự thêm `chuyen tien`
- Chưa config → QR không `addInfo` (mặc định app ngân hàng)
- Tắt log DEBUG mặc định; bỏ DOM scraper không dùng

### UI / bảo mật

- Icon MUA/BÁN bằng CSS (tránh emoji lỗi WebView2)
- Credentials trong Windows Credential Manager
- CORS bridge chỉ cho origin extension

### Repo / docs

- Dọn script debug, audit cũ, dependency không dùng (`ws`, plugin-opener)
- Đồng bộ version **1.0.1** (app, Cargo, fe, extension)
- Viết lại README, USER_GUIDE, ARCHITECTURE, WORKFLOW

---

## v1.0.0 (baseline)

Bản đầu: đồng bộ lệnh, modal chi tiết, VietQR cơ bản, extension scrape, dark UI.
