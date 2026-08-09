# Báo cáo Audit — p2p-qr (Binance P2P Manager)

Nhánh: `audit/code-review` · Commit gốc: `bb7a52f` · Ngày: 2026-08-09

Báo cáo này **chỉ phân tích, không thay đổi code sản phẩm**. Mỗi phát hiện đều ghi rõ vị trí, bằng chứng, ảnh hưởng và đề xuất sửa.

---

## 1. Phạm vi và phương pháp

Đã đọc toàn bộ source: 8 file Rust được compile, 7 component Svelte, migrations, cấu hình Tauri/Vite/SvelteKit, manifest extension.

Môi trường kiểm chứng được dựng trên máy này:

| Thành phần | Phiên bản | Kết quả |
|---|---|---|
| Rust | 1.97.1 (host `x86_64-pc-windows-gnu`) | `cargo check --all-targets` → **pass**, 2 warning |
| Node.js | 24.19.0 | `npm run build` → **pass**; `svelte-check` → 0 lỗi |
| SQLite | `node:sqlite` | dùng để chạy lại schema + query thật của app |

Do máy không có MSVC, Rust được cài bằng toolchain GNU kèm mingw-w64 (crate `windows-sys` cần `dlltool.exe`). Điều này đủ để `cargo check`, nhưng **chưa** build được bundle Tauri hoàn chỉnh.

Ký hiệu mức độ tin cậy:

- **[V]** — đã kiểm chứng bằng cách chạy thật (SQLite / cargo / build).
- **[S]** — suy ra từ đọc code, chắc chắn theo quy tắc ngôn ngữ hoặc so khớp tên field.

---

## 2. Tổng quan kiến trúc hiện tại

```
Chrome Extension (MV3)          Tauri App (Rust)                    SvelteKit UI
  hook fetch/XHR                                                    
  trên *.binance.com  ──POST──> axum 127.0.0.1:1425                 
                                  /api/payment-detail               
                                        │                            
                                        ├─ sinh VietQR URL           
                                        ├─ ghi SQLite                
                                        └─ emit "orders-updated" ──> reload toàn bộ orders
                                                                     
                                Scheduler (tokio)                    invoke(list_orders_from_db)
                                  ├─ incremental_sync  mỗi 60s       invoke(force_sync_recent) mỗi 30s
                                  ├─ active_poll       mỗi 15s       
                                  └─ cleanup           mỗi 300s      
                                        │                            
                                        └─> Binance REST /sapi/v1/c2c/orderMatch/listUserOrderHistory
```

SQLite nằm ở `%LOCALAPPDATA%\BinanceP2PManager\p2p_app.db`.

Ba điểm yếu nằm ngay ở mức kiến trúc, trước khi bàn tới từng dòng code:

1. **Không có tầng repository cho `order_payment_detail`.** SQL thao tác bảng này được viết lặp lại inline ở 4 nơi trong `main.rs`, mỗi nơi một biến thể cột khác nhau. Đây là nguyên nhân gốc của phần lớn lỗi P0 bên dưới.
2. **Ba nguồn đồng bộ chồng lên nhau** (scheduler 15s, scheduler 60s, frontend 30s) không hề biết về nhau, cùng gọi một endpoint Binance.
3. **`main.rs` là god file 1018 dòng**: bảng BIN ngân hàng, HTTP server, 14 Tauri command và scheduler nằm chung một file.

---

## 3. Bảng tổng hợp phát hiện

| # | Mức | Vấn đề | Vị trí | Tin cậy |
|---|---|---|---|---|
| P0-1 | Nghiêm trọng | `INSERT OR REPLACE` không có UNIQUE index → sinh dòng trùng, đọc ra dữ liệu cũ | `main.rs:645`, `migrations/001_init.sql:31` | [V] |
| P0-2 | Nghiêm trọng | Query cleanup dùng `status_code` (cột thật `order_status_code`) → cleanup **chưa bao giờ chạy** | `main.rs:181`, `main.rs:999` | [V] |
| P0-3 | Nghiêm trọng | `list_recent_payment_details` dùng cột `created_at` không tồn tại → luôn lỗi | `main.rs:804` | [V] |
| P0-4 | Nghiêm trọng | Số tiền trên QR sai 100x hoặc 27.000x | `main.rs:526-531` | [V] |
| P0-5 | Nghiêm trọng | `DELETE FROM orders` trước khi sync → mất sạch dữ liệu nếu sync lỗi | `sync_engine.rs:123` | [S] |
| P0-6 | Cao | Thứ tự if-chain khiến SHB / Shinhan / Standard Chartered không thể match | `main.rs:434-446` | [S] |
| P0-7 | Cao | Frontend gọi command với object lồng, Rust nhận tham số phẳng → ghi dòng toàn NULL | `OrderDetail.svelte:123`, `main.rs:194` | [S] |
| P0-8 | Cao | Link "Mở trên Binance" dùng field không tồn tại `create_time` | `OrderDetail.svelte:466` | [S] |
| P0-9 | Trung bình | Migration runner chỉ nạp 001, split theo `;`; 002 chết và sẽ lỗi nếu chạy | `db/mod.rs:24-31` | [V] |
| P0-10 | Trung bình | `db/stage_map.json` không bao giờ được nạp (sai đường dẫn + không bundle) | `main.rs:908` | [S] |
| P1-1 | Nghiêm trọng | API key/secret "mã hoá" bằng base64 | `crypto/mod.rs` | [S] |
| P1-2 | Nghiêm trọng | HTTP server localhost mở CORS `Any`, không auth | `main.rs:698-707` | [S] |
| P1-3 | Cao | `get_saved_credentials` trả secret nguyên văn về UI | `main.rs:89` | [S] |
| P1-4 | Cao | `println!` in số tài khoản, tên chủ TK, toàn bộ payload | `main.rs` (~60 chỗ) | [S] |
| P1-5 | Trung bình | `csp: null` | `tauri.conf.json:25` | [S] |
| P1-6 | Trung bình | `clear_all_data` spawn PowerShell + `process::exit(0)` | `main.rs:234-296` | [S] |
| P1-7 | Trung bình | Listener `window.message` không kiểm tra origin | `+page.svelte:258` | [S] |
| P1-8 | Thấp | `store()` DELETE rồi INSERT ngoài transaction | `credentials.rs:20-30` | [S] |
| P2-1 | Cao | 3 nguồn sync trùng nhau đập vào API Binance | `main.rs:960`, `+page.svelte:284` | [S] |
| P2-2 | Cao | Frontend tải toàn bộ orders (`limit: 0`) rồi filter ở client | `+page.svelte:34` | [S] |
| P2-3 | Trung bình | Upsert từng dòng, không transaction | `repo.rs:95` | [S] |
| P2-4 | Trung bình | Thiếu pragma WAL / busy_timeout | `db/mod.rs:15-17` | [S] |
| P2-5 | Trung bình | `reqwest::Client` không set timeout | `c2c_api_client.rs:24` | [S] |
| P2-6 | Trung bình | Retry cả lỗi không thể retry, backoff không giới hạn | `sync_engine.rs:26-41` | [S] |
| P2-7 | Thấp | Tạo mới `Intl.NumberFormat` mỗi cell | `OrderTable.svelte:88`, `OrderDetail.svelte:35` | [S] |
| P3-1 | Trung bình | 7 file Rust không bao giờ được compile (~14,5 KB) | `src/orders/*`, `src/credentials/mod.rs` | [V] |
| P3-2 | Trung bình | `lib.rs` là app Tauri thứ hai + WS server, compile nhưng không bao giờ chạy | `lib.rs` | [V] |
| P3-3 | Trung bình | 75 file build artifact bị commit (648 KB), `.gitignore` không phủ | `fe/build`, `fe/.svelte-kit` | [V] |
| P3-4 | Thấp | 4 dependency Rust không dùng + `ws` ở root | `Cargo.toml`, `package.json` | [V] |
| P3-5 | Thấp | Extension thiếu toàn bộ file JS mà manifest tham chiếu | `chrome-extension/` | [V] |
| P3-6 | Thấp | Migration trùng lặp đã lệch schema | `db/migrations/001_init.sql` | [V] |
| P3-7 | Thấp | 6 cột schema không ai đọc/ghi | `001_init.sql` | [V] |
| P3-8 | Thấp | Toast `position: fixed` → mọi toast đè lên nhau | `Toast.svelte:56-58` | [S] |
| P3-9 | Thấp | `.btn-primary` được dùng nhưng không định nghĩa | `OrderDetail.svelte:464` | [S] |
| P3-10 | Thấp | `partnerName` trong bảng bỏ qua `trade_type` | `OrderTable.svelte:97` | [S] |
| P3-11 | Thấp | Class CSS sinh từ text tiếng Việt, có case sinh selector không hợp lệ | `OrderTable.svelte:227` | [S] |
| P3-12 | Thấp | Toàn bộ frontend là `any` → `svelte-check` 0 lỗi dù có bug tên field | `+page.svelte`, `OrderTable.svelte` | [V] |

---

## 4. Chi tiết nhóm P0 — Sai dữ liệu và sai tiền

### P0-1 · `INSERT OR REPLACE` không có UNIQUE index để replace lên

`order_payment_detail` chỉ có index **không unique**:

`src-tauri/migrations/001_init.sql:31-46`

```sql
CREATE TABLE IF NOT EXISTS order_payment_detail (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  order_number TEXT NOT NULL,
  ...
);

CREATE INDEX IF NOT EXISTS idx_payment_order ON order_payment_detail(order_number);
```

Nhưng code ghi bằng `INSERT OR REPLACE` với ý định upsert. `OR REPLACE` chỉ thay thế khi vi phạm ràng buộc UNIQUE — ở đây không có, nên mỗi lần ghi là **thêm một dòng mới**.

Kết hợp với hàm đọc không có `ORDER BY`:

`src-tauri/src/main.rs:762-770`

```rust
    let row = sqlx::query(r#"
        SELECT account_name, account_no, bank_name, sub_bank, qr_code_url, amount, transfer_content, suggested_transfer_content, captured_at
        FROM order_payment_detail 
        WHERE order_number = ?
    "#)
        .bind(&order_number)
        .fetch_optional(pool)
```

Kết quả chạy thật (3 lần lưu cùng một `order_number`, QR mới nhất là `NEWEST.jpg`):

```
[BUG]  3 saves of the SAME order -> 3 rows (rows accumulate, no UNIQUE index to replace on)
[BUG]  returns STALE data: qr=https://img.vietqr.io/OLD.jpg, amount=1000000
       (newest was NEWEST.jpg / 3000000)
```

**Ảnh hưởng:** người dùng nhìn thấy QR và số tiền của lần cập nhật **đầu tiên**, không phải mới nhất. Với một app chuyển tiền, đây là lỗi nặng nhất trong repo. Bảng cũng phình vô hạn vì không có gì xoá (xem P0-2).

**Đề xuất:**

```sql
-- migration mới
DELETE FROM order_payment_detail
  WHERE id NOT IN (SELECT MAX(id) FROM order_payment_detail GROUP BY order_number);
CREATE UNIQUE INDEX IF NOT EXISTS idx_payment_order_uniq
  ON order_payment_detail(order_number);
```

Và đổi sang upsert tường minh, chỉ ghi đè khi giá trị mới không NULL:

```sql
INSERT INTO order_payment_detail (order_number, ..., captured_at, purge_after)
VALUES (?1, ..., ?10, ?11)
ON CONFLICT(order_number) DO UPDATE SET
  account_name = COALESCE(excluded.account_name, order_payment_detail.account_name),
  ...
  captured_at  = excluded.captured_at;
```

`COALESCE` quan trọng: nó vô hiệu hoá luôn tác hại của P0-7.

### P0-2 · Cleanup tham chiếu cột không tồn tại → chưa bao giờ chạy

Bảng `orders` có cột `order_status_code`, không có `status_code`. Nhưng cả hai chỗ cleanup đều viết `status_code`:

`src-tauri/src/main.rs:177-184`

```rust
    let result = sqlx::query(r#"
        DELETE FROM order_payment_detail 
        WHERE order_number NOT IN (
            SELECT order_number FROM orders 
            WHERE status_code IN (1, 2, 3)  -- Only keep for processing orders
        )
        OR purge_after < ?
    "#)
```

Bản trong scheduler còn **nuốt luôn lỗi**:

`src-tauri/src/main.rs:995-1007`

```rust
                        if let Ok(_res) = sqlx::query(r#"
                            DELETE FROM order_payment_detail 
                            WHERE order_number NOT IN (
                                SELECT order_number FROM orders 
                                WHERE status_code IN (1, 2, 3)
                            )
                            OR purge_after < ?
                        "#)
                        .bind(chrono::Utc::now().timestamp_millis())
                        .execute(pool)
                        .await {
                            // Cleanup completed silently
                        }
```

Kết quả chạy thật:

```
[info] orders columns: order_status_code
[BUG]  cleanup ALWAYS fails at runtime -> no such column: status_code
```

**Ảnh hưởng:** `purge_after` (24h) chưa bao giờ có hiệu lực. Thông tin thanh toán — số tài khoản, tên chủ tài khoản, tức dữ liệu cá nhân của đối tác — được giữ **vĩnh viễn**, đồng thời nhân bản mỗi lần cập nhật vì P0-1. `if let Ok(_)` khiến lỗi im lặng suốt vòng đời app. Comment `// Cleanup completed silently` mô tả đúng hành vi quan sát được nhưng sai bản chất: nó thất bại im lặng.

**Đề xuất:** sửa tên cột, gom vào một hàm repository duy nhất, và log lỗi ở mức `warn` thay vì bỏ qua:

```rust
if let Err(e) = repo.purge_expired_payment_details().await {
    tracing::warn!(error = %e, "purge payment details failed");
}
```

### P0-3 · `list_recent_payment_details` luôn lỗi

`src-tauri/src/main.rs:799-806`

```rust
    let rows = sqlx::query(r#"
        SELECT order_number, bank_name, account_no, 
               CASE WHEN qr_code_url IS NOT NULL THEN 'YES' ELSE 'NO' END as has_qr,
               created_at
        FROM order_payment_detail 
        ORDER BY created_at DESC 
        LIMIT 20
    "#)
```

`order_payment_detail` không có `created_at` ở bất kỳ migration nào — cột thời gian là `captured_at`.

```
[BUG]  query ALWAYS fails at runtime -> no such column: created_at
```

Command này đã đăng ký trong `invoke_handler` nhưng frontend chưa gọi, nên lỗi chưa lộ ra. **Đề xuất:** đổi `created_at` → `captured_at`, hoặc xoá command nếu không dùng.

### P0-4 · Số tiền trên QR sai 100x hoặc 27.000x

`src-tauri/src/main.rs:524-532`

```rust
    if let Some(amt) = amount {
        // Convert USDT to VND if needed (amount might be in USDT)
        let amt_clean = amt.replace(",", "").replace(".", "");
        if let Ok(num) = amt_clean.parse::<f64>() {
            // If amount < 1000, assume it's USDT and convert (rough estimate 27000 VND/USDT)
            let vnd_amount = if num < 1000.0 { (num * 27000.0) as i64 } else { num as i64 };
            params.push(format!("amount={}", vnd_amount));
        }
    }
```

Hai lỗi độc lập trong 6 dòng:

1. `.replace(".", "")` xoá dấu thập phân chứ không chỉ dấu phân cách nghìn.
2. Heuristic "nhỏ hơn 1000 thì coi là USDT" nhân với tỷ giá **hardcode 27.000**.

Kết quả chạy thật:

```
[BUG]  "500.00"   -> 50000 VND      (đúng phải ~500)
[BUG]  "1234.56"  -> 123456 VND     (đúng phải ~1234.56)
[OK]   "12,000,000" -> 12000000 VND
[BUG]  "1,234.56" -> 123456 VND
[BUG]  "999"      -> 26973000 VND   (sai 27.000x)
```

Cần nói cho chính xác: `handle_payment_detail` đã chuẩn hoá `amount` **trước** khi gọi hàm này, và số nguyên thì `format!("{:.0}")` nên đi qua an toàn. Hai nhánh còn lỗi thật sự là:

- số có phần thập phân (`{:.2}`) → sai đúng 100x;
- bất kỳ số nào < 1000 → nhân 27.000.

**Ảnh hưởng:** QR sinh ra có `amount` sai, người dùng quét và chuyển sai số tiền. Tỷ giá hardcode còn trôi theo thời gian.

**Đề xuất:** không suy đoán đơn vị. Truyền vào một kiểu đã rõ nghĩa và từ chối nếu không chắc:

```rust
/// `amount_vnd` phải là số tiền VND đã chuẩn hoá ở tầng gọi.
fn vietqr_amount_param(amount_vnd: Option<&str>) -> Option<String> {
    let raw = amount_vnd?;
    // chỉ bỏ phân cách nghìn, giữ nguyên dấu thập phân
    let normalized = raw.replace(',', "").replace(' ', "");
    let value: f64 = normalized.parse().ok()?;
    if value <= 0.0 { return None; }
    Some(format!("amount={}", value.round() as i64))
}
```

Nếu thật sự cần quy đổi USDT → VND thì phải lấy tỷ giá từ `order.price` của chính lệnh đó (đã có trong DB), tuyệt đối không hardcode.

### P0-5 · `force_initial_sync` xoá hết dữ liệu trước khi biết sync có thành công

`src-tauri/src/api/sync_engine.rs:120-126`

```rust
        // ✅ Clear ALL old data first before syncing fresh data
        // This ensures we only keep exactly what user requested (X days)
        println!("[SYNC] Clearing all old orders before fresh sync...");
        sqlx::query("DELETE FROM orders")
            .execute(self.repo.pool())
            .await?;
```

Sau `DELETE` là vòng lặp gọi mạng nhiều phút (`sync_chunk` cho BUY và SELL, mỗi chunk 7 ngày). Bất kỳ lỗi mạng, hết hạn API key, hay đóng app giữa chừng đều để lại DB rỗng hoặc thiếu, không có đường lùi.

**Đề xuất:** bọc trong transaction, hoặc sync vào bảng tạm rồi swap. Đơn giản nhất mà vẫn an toàn:

```rust
let mut tx = self.repo.pool().begin().await?;
sqlx::query("DELETE FROM orders").execute(&mut *tx).await?;
// ... toàn bộ upsert dùng &mut *tx ...
tx.commit().await?;   // chỉ commit khi mọi chunk đã xong
```

Đồng thời `DELETE FROM orders` không xoá `order_payment_detail` tương ứng, để lại bản ghi mồ côi mà cleanup lại đang hỏng (P0-2).

### P0-6 · Ba nhánh ngân hàng không thể match do thứ tự if-chain

`get_bank_bin` là chuỗi 190 dòng `if ... contains(...) { return }`. Thứ tự quyết định kết quả, và có ba chỗ nhánh sau bị nhánh trước chặn:

`src-tauri/src/main.rs:433-446`

```rust
    // SCB - Ngân hàng TMCP Sài Gòn (already handled above but kept for reference)
    if name_lower.contains("scb") || name_lower.contains("sài gòn") || name_lower.contains("sai gon") { return Some("970429"); }
    ...
    // SHB - Ngân hàng TMCP Sài Gòn - Hà Nội
    if name_lower.contains("shb") || name_lower.contains("sài gòn hà nội") || name_lower.contains("sai gon ha noi") { return Some("970443"); }
    ...
    // Shinhan - Ngân hàng TNHH MTV Shinhan Việt Nam
    if name_lower.contains("shinhan") || name_lower.contains("shbvn") { return Some("970424"); }
    ...
    // Standard Chartered
    if name_lower.contains("standard chartered") || name_lower.contains("scbvl") { return Some("970410"); }
```

Lần vết theo đúng thứ tự thực thi:

| Input | Nhánh chặn | Trả về | Đúng ra phải là |
|---|---|---|---|
| `Ngân hàng TMCP Sài Gòn - Hà Nội` | dòng 434 `contains("sài gòn")` | SCB `970429` | SHB `970443` |
| `shbvn` | dòng 440 `contains("shb")` | SHB `970443` | Shinhan `970424` |
| `scbvl` | dòng 434 `contains("scb")` | SCB `970429` | Standard Chartered `970410` |

**Ảnh hưởng:** sinh QR trỏ sang **ngân hàng khác**. Với QR chuyển tiền, sai BIN nghĩa là tiền đi sai đích hoặc giao dịch bị từ chối.

Comment ở dòng 433 (`already handled above but kept for reference`) cho thấy chính tác giả cũng không còn theo dõi được thứ tự — dấu hiệu rõ ràng rằng cấu trúc này đã vượt ngưỡng bảo trì được.

**Đề xuất:** thay if-chain bằng bảng dữ liệu, so khớp theo độ dài alias giảm dần để alias cụ thể luôn thắng alias chung, và tách ra file riêng kèm test:

```rust
struct Bank { bin: &'static str, aliases: &'static [&'static str] }

static BANKS: &[Bank] = &[
    Bank { bin: "970443", aliases: &["shb", "sài gòn hà nội", "sài gòn - hà nội"] },
    Bank { bin: "970424", aliases: &["shinhan", "shbvn"] },
    Bank { bin: "970410", aliases: &["standard chartered", "scbvl"] },
    Bank { bin: "970429", aliases: &["scb", "sài gòn"] },
    // ...
];

pub fn bank_bin(name: &str) -> Option<&'static str> {
    let hay = normalize(name);  // lowercase + bỏ dấu + gộp khoảng trắng
    BANKS.iter()
        .flat_map(|b| b.aliases.iter().map(move |a| (a.len(), *a, b.bin)))
        .filter(|(_, alias, _)| hay.contains(alias))
        .max_by_key(|(len, _, _)| *len)      // alias dài nhất thắng
        .map(|(_, _, bin)| bin)
}
```

Cách này loại bỏ hoàn toàn sự phụ thuộc vào thứ tự. Kèm theo cần test bảng cho ít nhất các cặp gây nhầm ở trên. **Lưu ý:** toàn bộ danh sách BIN trong file hiện tại nên được đối chiếu lại với bảng BIN chính thức của NAPAS — tôi không kiểm chứng được giá trị từng mã, và ít nhất `BVBank`/`VietBank` đang dùng chung `970433` (dòng 371 và 481), một trong hai gần như chắc chắn sai.

### P0-7 · Frontend và backend không khớp chữ ký command

Command Rust nhận **tham số phẳng**:

`src-tauri/src/main.rs:194-204`

```rust
async fn save_payment_detail_from_extension(
    state: State<'_, AppCtx>, 
    order_number: String,
    account_name: Option<String>,
    account_no: Option<String>, 
    bank_name: Option<String>,
    ...
```

`OrderDetail.svelte` lại gửi một **object lồng** tên `paymentDetail`:

`fe/src/lib/OrderDetail.svelte:123-135`

```svelte
          await invoke('save_payment_detail_from_extension', {
            orderNumber: order.order_number,
            paymentDetail: {
              accountName: data.accountName,
              accountNo: data.accountNo,
              ...
            }
          });
```

Mọi field là `Option<String>` nên Tauri deserialize thành `None` thay vì báo lỗi. Kết quả: ghi vào DB một dòng chỉ có `order_number`, còn lại NULL — và vì P0-1 nó là **dòng mới**, có thể trở thành dòng được đọc ra.

`+page.svelte:264` gọi cùng command với tham số phẳng nhưng **thiếu** `amount` và `transferContent`, nên cũng ghi NULL vào hai cột đó.

**Đề xuất:** định nghĩa một struct request duy nhất, dùng chung cho cả HTTP endpoint và Tauri command, và thêm `#[serde(deny_unknown_fields)]` để lệch chữ ký báo lỗi ngay:

```rust
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PaymentDetailInput {
    order_number: String,
    account_name: Option<String>,
    // ...
}
```

### P0-8 · Link "Mở trên Binance" luôn thiếu tham số

`fe/src/lib/OrderDetail.svelte:465-468`

```svelte
        on:click={() => {
          const url = `https://c2c.binance.com/vi/fiatOrderDetail?orderNo=${order.order_number}&createdAt=${order.create_time}`;
          window.open(url, '_blank');
        }}
```

Struct trả về từ backend không có field `create_time`:

`src-tauri/src/orders/repo.rs:9-24`

```rust
pub struct OrderRow {
    pub order_number: String,
    ...
    pub create_time_ms: i64,
```

Nên URL luôn là `...&createdAt=undefined`. Đây chính là loại lỗi mà TypeScript bắt được — nhưng `order` được khai báo `any` (P3-12) nên trình kiểm tra im lặng.

### P0-9 · Migration runner sơ sài, migration 002 chết

`src-tauri/src/db/mod.rs:23-32`

```rust
    async fn run_migrations(pool: &SqlitePool) -> Result<()> {
        let sql = include_str!("../../migrations/001_init.sql");
        for statement in sql.split(';') {
```

Ba vấn đề:

1. Chỉ `001` được nhúng. `002_add_payment_fields.sql` **không bao giờ** được nạp.
2. Không có bảng theo dõi phiên bản — không biết migration nào đã áp dụng.
3. Tách câu lệnh bằng `split(';')` sẽ vỡ với trigger, `BEGIN...END`, hoặc dấu `;` trong string literal.

Kiểm chứng cả (1) và (3):

```
[BUG]  FAILS: "ALTER TABLE order_payment_detail ADD COLUMN transfer_co..." -> duplicate column name
[BUG]  FAILS: "ALTER TABLE order_payment_detail ADD COLUMN suggested_t..." -> duplicate column name
[BUG]  splitting on ";" breaks statements containing ";" -> unrecognized token: "'x"
```

002 thêm ba cột mà 001 đã có sẵn, nên nếu ai đó "sửa" bằng cách nhúng thêm 002 thì app sẽ **crash khi khởi động**. File này cần xoá, không phải kích hoạt.

**Đề xuất:** dùng `sqlx::migrate!` — có bảng `_sqlx_migrations`, chạy đúng thứ tự, idempotent, parse đúng:

```rust
sqlx::migrate!("./migrations").run(&pool).await?;
```

### P0-10 · `db/stage_map.json` không bao giờ được nạp

`src-tauri/src/main.rs:907-909`

```rust
    // Load status label mapping from db/stage_map.json (exe directory)
    let stage_map_path = exe_dir.parent().unwrap_or(&exe_dir).join("db/stage_map.json");
    let stage_map = Arc::new(orders::stage_map::StageMap::load_from(stage_map_path.to_str().unwrap()));
```

Đường dẫn là `<thư mục cha của exe>/db/stage_map.json`:

- Khi dev, exe ở `target/debug/` → tìm ở `target/db/stage_map.json` → không có.
- Khi cài đặt, `tauri.conf.json` không khai báo `bundle.resources` nên thư mục `db/` **không được đóng gói** → cũng không có.

`StageMap::load_from` bỏ qua lỗi đọc file một cách im lặng và dùng nhãn hardcode. Vì vậy nhãn emoji trong `db/stage_map.json` (`🔄 Đang giao dịch`, …) chưa từng xuất hiện trên UI; app luôn dùng nhãn trong `stage_map.rs`. Cùng vấn đề đường dẫn áp dụng cho `db/seed_credentials.json` ở dòng 894.

**Đề xuất:** chọn một trong hai và làm cho rõ ràng — hoặc `include_str!` nhãn vào binary (bỏ hẳn file JSON), hoặc khai báo `bundle.resources` và resolve qua `app.path().resource_dir()`. Cấu hình mà im lặng không có tác dụng còn tệ hơn là không có cấu hình.

---

## 5. Chi tiết nhóm P1 — Bảo mật và dữ liệu cá nhân

Nhóm này đáng chú ý vì app giữ **API key có quyền đọc lịch sử giao dịch Binance** và **số tài khoản ngân hàng của đối tác**.

### P1-1 · API key và secret lưu gần như plaintext

`src-tauri/src/crypto/mod.rs:5-18`

```rust
// Dummy crypto context using base64 (NOT secure). Replace with real AEAD + KDF later.
pub struct CryptoCtx;

impl CryptoCtx {
    pub fn new_dummy() -> Self { CryptoCtx }
    pub fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>> {
        Ok(STANDARD.encode(plain).into_bytes())
    }
```

Base64 là encoding, không phải encryption. Cột `api_key_enc` / `api_secret_enc` chỉ cần một lệnh `base64 -d` là ra. File DB nằm ở đường dẫn cố định `%LOCALAPPDATA%\BinanceP2PManager\p2p_app.db`, không phân quyền đặc biệt.

Comment đã tự nhận biết vấn đề, nhưng `new_dummy()` được gọi trực tiếp trong `main()` (dòng 883) và là code path duy nhất.

**Đề xuất theo thứ tự ưu tiên:**

1. Dùng OS credential store, không tự quản khoá: crate [`keyring`](https://crates.io/crates/keyring) → Windows Credential Manager / macOS Keychain / Secret Service. Secret không nằm trong file DB nữa.
2. Nếu buộc phải để trong DB: AEAD thật (`chacha20poly1305` hoặc `aes-gcm`) với khoá lấy từ keyring, nonce ngẫu nhiên mỗi lần ghi.
3. Đổi tên `CryptoCtx::new_dummy()` → `new()` chỉ sau khi đã có mã hoá thật, để không còn API mời gọi dùng bản giả.

### P1-2 · HTTP server localhost không có xác thực, CORS mở hoàn toàn

`src-tauri/src/main.rs:697-714`

```rust
async fn start_http_server(state: HttpAppState) {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    
    let app = Router::new()
        .route("/api/health", get(health_check))
        .route("/api/payment-detail", post(handle_payment_detail))
```

Bind vào `127.0.0.1:1425` chỉ chặn được truy cập từ máy khác. Nó **không** chặn được JavaScript của bất kỳ website nào mà người dùng đang mở: một trang bất kỳ có thể `fetch('http://127.0.0.1:1425/api/payment-detail', {method:'POST', ...})`, và `allow_origin(Any)` cho phép đọc cả response.

Kịch bản lợi dụng: một trang web bơm bản ghi payment detail với số tài khoản của kẻ tấn công cho một `orderNumber` hợp lệ. Vì `handle_payment_detail` tự sinh lại VietQR từ `bankName` + `accountNo` nhận được, QR hiển thị trong app sẽ trỏ vào tài khoản của kẻ tấn công. Người dùng tin app và quét.

Không có giới hạn kích thước body, không rate limit, nên cũng có thể bơm dữ liệu cho tới khi đầy đĩa — càng dễ vì cleanup đang hỏng (P0-2).

**Đề xuất:**

1. Sinh một token ngẫu nhiên khi app khởi động, yêu cầu header `Authorization: Bearer <token>` trên mọi request ghi. Extension lấy token qua một bước pairing thủ công (người dùng copy từ UI) — extension MV3 không thể tự lấy secret an toàn từ localhost.
2. Giới hạn CORS về đúng origin của extension: `chrome-extension://<id>` thay vì `Any`.
3. Thêm `DefaultBodyLimit` và một tầng rate limit.
4. Chỉ nhận `orderNumber` **đã tồn tại** trong bảng `orders` — bản ghi payment cho lệnh không có thật nên bị từ chối.

### P1-3 · Secret được trả nguyên văn về UI

`src-tauri/src/main.rs:88-91`

```rust
#[tauri::command]
async fn get_saved_credentials(state: State<'_, AppCtx>) -> Result<Option<(String, String)>, String> {
    state.creds_repo.latest().await.map_err(|e| e.to_string())
}
```

Frontend nhận rồi bind thẳng vào input:

`fe/src/routes/+page.svelte:227-234`

```svelte
      const credentials = await invoke<[string, string] | null>('get_saved_credentials');
      
      if (credentials) {
        // Credentials exist - fill the input fields
        const [savedApiKey, savedApiSecret] = credentials;
        apiKey = savedApiKey;
        apiSecret = savedApiSecret;
```

Secret đi vào DOM, vào memory của webview, và hiện ra chỉ bằng một cú click "👁️ Hiện". `csp: null` (P1-5) khiến rủi ro XSS trở nên thực tế hơn.

UI thực ra **không cần** secret: nó chỉ cần biết đã lưu credentials hay chưa (`credentialsSaved`) — mà đã có sẵn command `check_api_credentials`.

**Đề xuất:** đổi command thành trả về metadata không nhạy cảm:

```rust
#[derive(Serialize)]
struct CredentialsInfo { label: String, api_key_masked: String, saved_at: i64 }
// api_key_masked: "vmPUZE…8kU" — không bao giờ trả secret
```

### P1-4 · Log in ra dữ liệu cá nhân

Khoảng 60 lệnh `println!("[DEBUG] ...")` trong `main.rs`, trong đó có:

`src-tauri/src/main.rs:614-624`

```rust
    println!("[DEBUG] === EXTRACTED FIELDS FROM REQUEST ===");
    println!("  - order_number: {:?}", order_number);
    println!("  - account_name: {:?}", account_name);
    println!("  - account_no: {:?}", account_no);
    println!("  - bank_name: {:?}", bank_name);
```

Và cả payload thô:

`src-tauri/src/main.rs:550-551`

```rust
    println!("[DEBUG] Received payment detail request: {:?}", request.request_type);
    println!("[DEBUG] Request data: {:?}", request.data);
```

`request.data` là toàn bộ JSON do extension gửi lên, tức mọi field thanh toán ở dạng thô.

Đáng chú ý là `main.rs:623` có nỗ lực che `qr_code_url` (`if s.len() > 50 { "present (truncated)" }`), nhưng số tài khoản và tên chủ tài khoản ở các dòng ngay phía trên thì in đầy đủ. Đây là dữ liệu cá nhân của **người thứ ba** (đối tác giao dịch), không phải của người dùng app.

Thêm nữa: `println!` không có mức log, không tắt được ở bản release, và trên Windows với `windows_subsystem="windows"` thì stdout thường bị mất — nên vừa rò rỉ khi chạy từ terminal, vừa vô dụng khi cần debug thật.

**Đề xuất:** chuyển sang `tracing` + `tracing-subscriber`, mặc định `INFO`, đưa toàn bộ chi tiết payload xuống `debug!` và chỉ bật qua biến môi trường. Không log số tài khoản dù ở mức nào; nếu cần đối chiếu thì log 4 ký tự cuối.

### P1-5 · CSP bị tắt

`src-tauri/tauri.conf.json:24-26`

```json
    "security": {
      "csp": null
    }
```

`null` nghĩa là Tauri không chèn CSP nào. Webview được phép nạp script và kết nối tới bất kỳ đâu. App có nạp ảnh từ domain ngoài (`img.vietqr.io`) nên cần một CSP tường minh chứ không phải tắt hẳn:

```json
"csp": "default-src 'self'; img-src 'self' data: https://img.vietqr.io; style-src 'self' 'unsafe-inline'; connect-src 'self' ipc: http://ipc.localhost"
```

Cũng nên lưu ý: QR được tải từ `img.vietqr.io`, tức số tài khoản và số tiền của mọi giao dịch được gửi tới dịch vụ bên thứ ba dưới dạng URL. Điều này **trái với** cam kết trong `README.md` dòng 50: *"All processing is local. No external network calls besides Binance endpoints."* Nếu muốn giữ đúng cam kết thì phải sinh QR offline (encode chuỗi EMVCo VietQR rồi render bằng crate `qrcode`) — hoàn toàn khả thi và bỏ được một dependency mạng.

### P1-6 · `clear_all_data` xoá file bằng cách spawn shell rồi tự kill process

`src-tauri/src/main.rs:260-273`

```rust
    #[cfg(target_os = "windows")]
    {
        let script = format!(
            r#"Start-Sleep -Seconds 2; Remove-Item -Path '{}','{}','{}' -ErrorAction SilentlyContinue"#,
            db_str, wal_str, shm_str
        );
        
        println!("[CLEAR_DATA] PowerShell script: {}", script);
        
        Command::new("powershell")
            .args(&["-WindowStyle", "Hidden", "-Command", &script])
            .spawn()
```

Ba vấn đề:

1. Đường dẫn được nội suy vào chuỗi lệnh trong dấu nháy đơn. Ở đây đường dẫn xuất phát từ `dirs::data_local_dir()` nên khó điều khiển, nhưng đây là mẫu code (`format!` vào shell) không nên tồn tại trong codebase — chỉ cần một lần refactor cho phép người dùng chọn đường dẫn DB là thành lỗ hổng thật.
2. `std::process::exit(0)` ở dòng 295 kết thúc process ngay, không đóng connection pool, không checkpoint WAL. Hành vi "đợi 2 giây rồi xoá" là cách lách vấn đề file đang bị lock, và nó phụ thuộc vào thời gian nên không đáng tin.
3. UX bắt người dùng tự mở lại app, và message trong `+page.svelte:882` phải giải thích điều đó.

**Đề xuất:** làm hẳn trong Rust, không cần shell và không cần exit:

```rust
// 1. đóng pool để nhả file lock
pool.close().await;
// 2. xoá file (bao gồm -wal, -shm)
for p in [&db_path, &wal, &shm] { let _ = std::fs::remove_file(p); }
// 3. chạy lại migration để có DB rỗng, app tiếp tục sống
let db = Db::init(db_path.to_str()...).await?;
```

Nếu vẫn muốn khởi động lại thì dùng `tauri-plugin-process` (`app.restart()`) thay vì hướng dẫn người dùng làm thủ công.

### P1-7 · Listener `postMessage` không kiểm tra origin

`fe/src/routes/+page.svelte:258-281`

```svelte
    const handleExtensionMessage = async (event: any) => {
      if (event.data.__TAURI_SAVE_PAYMENT__) {
        try {
          const paymentData = event.data.__TAURI_SAVE_PAYMENT__;
          const result = await invoke('save_payment_detail_from_extension', {
            ...
    window.addEventListener('message', handleExtensionMessage);
```

Không có `event.origin` / `event.source` check. Bất kỳ nội dung nào chạy được trong webview đều có thể `postMessage` để ghi payment detail.

Ngoài ra đường dẫn này đã **lỗi thời**: extension hiện gửi qua HTTP `:1425`, không qua `postMessage`. Nó cũng đang gọi command với tham số thiếu (P0-7). Đây là cơ chế thứ ba làm cùng một việc, bên cạnh HTTP endpoint và `tryLoadFromLocalStorage()` trong `OrderDetail.svelte`.

**Đề xuất:** xoá hẳn listener này và hàm `fetch_payment_from_localstorage` (`main.rs:717`) — hàm đó `eval` một script trả về giá trị nhưng **không đọc lại kết quả**, nên nó không làm gì cả ngoài việc trả về `"Script executed"`. Giữ đúng một đường dữ liệu: extension → HTTP endpoint đã xác thực → DB → event.

### P1-8 · `store()` không dùng transaction

`src-tauri/src/api/credentials.rs:19-30`

```rust
        // Delete existing credentials with the same label to allow updating
        sqlx::query(r#"DELETE FROM api_credentials WHERE label = ?1"#)
            .bind(label)
            .execute(&self.pool).await?;
        
        // Insert new credentials
        sqlx::query(r#"INSERT INTO api_credentials(label, api_key_enc, api_secret_enc, created_at, last_used_at) VALUES (?1, ?2, ?3, ?4, ?4)"#)
```

Nếu process chết giữa hai câu lệnh, credentials mất hẳn. Bảng cũng không có UNIQUE trên `label` nên không thể upsert đúng cách.

**Đề xuất:** thêm `UNIQUE(label)` rồi dùng `ON CONFLICT(label) DO UPDATE`, thành một câu lệnh nguyên tử.

---

## 6. Chi tiết nhóm P2 — Hiệu năng

### P2-1 · Ba nguồn đồng bộ chồng nhau

Scheduler backend:

`src-tauri/src/main.rs:966-974`

```rust
            // 60s incremental sync
            let mut inc = interval(Duration::from_secs(60));
            inc.set_missed_tick_behavior(MissedTickBehavior::Delay);
            // 15s active poll
            let mut poll = interval(Duration::from_secs(15));
```

Frontend lại tự bật thêm một vòng nữa:

`fe/src/routes/+page.svelte:284-288`

```svelte
    const interval = setInterval(async () => {
      if (isAutoRefresh && !refreshing) {
        await refreshFromExchange(true); // Silent refresh - no toast spam
      }
    }, 30000);
```

`refreshFromExchange` gọi `force_sync_recent`, mà hàm này chạy **cả** `active_poll()` **và** `incremental_sync()` (`main.rs:164-167`) — đúng hai việc scheduler đang tự làm.

Chi phí thực của một chu kỳ `active_poll`: với mỗi `trade_type` trong `["BUY","SELL"]`, `sync_chunk` phân trang cửa sổ 24 giờ với `rows = 20` và `sleep(100ms)` giữa các trang. Người dùng có 200 lệnh/ngày sẽ tốn 10 request mỗi chiều, tức **20 request mỗi 15 giây**, và toàn bộ đều là dữ liệu đã có trong DB. Cộng thêm `incremental_sync` 60s và vòng frontend 30s.

Mỗi lần xong lại `emit("orders-updated")`, và frontend phản ứng bằng cách tải lại **toàn bộ** bảng orders (P2-2).

**Đề xuất:**

1. Xoá `setInterval` ở frontend. Backend đã là nguồn duy nhất; UI chỉ cần lắng nghe event. Giữ nút "Cập nhật từ sàn" cho hành động thủ công.
2. `active_poll` chỉ nên refresh những lệnh **đang xử lý**, đọc từ DB trước:
   ```sql
   SELECT order_number, create_time_ms FROM orders
   WHERE order_status_code IN (1,2,3) ORDER BY create_time_ms DESC
   ```
   Nếu không có lệnh nào đang xử lý thì bỏ qua hoàn toàn chu kỳ đó — trường hợp phổ biến nhất.
3. Tăng `rows` lên mức tối đa API cho phép để giảm số lần phân trang.
4. Chỉ `emit` khi thực sự có thay đổi (so `rows_affected` hoặc hash tập status), tránh reload UI vô ích.

### P2-2 · Frontend tải toàn bộ orders rồi lọc ở client

`fe/src/routes/+page.svelte:32-36`

```svelte
  async function loadOrders(silent: boolean = false) {
    try { 
      const result = await invoke('list_orders_from_db', { limit: 0 }); 
      orders = result as any[];
```

`limit: 0` đi vào nhánh không `LIMIT` (`repo.rs:107`). Toàn bộ bảng được serialize sang JSON, qua IPC, rồi lọc bằng JS:

`fe/src/routes/+page.svelte:51-54`

```svelte
  $: buyOrders = orders.filter(o=>o.trade_type==='BUY');
  $: sellOrders = orders.filter(o=>o.trade_type==='SELL');
  $: inProgressOrders = orders.filter(o=>o.status_code===1 || o.status_code===2 || o.status_code===3);
```

`OrderTable` lọc và phân trang thêm một lần nữa trên chính mảng đó, rồi chỉ render 20 dòng. Sau 30 ngày sync, đây là hàng nghìn bản ghi được truyền và lọc lại mỗi 15–30 giây.

**Đề xuất:** đẩy filter, search, sort và phân trang xuống SQL. Một command nhận tham số và trả về kèm tổng số:

```rust
#[tauri::command]
async fn query_orders(
    state: State<'_, AppCtx>,
    trade_type: Option<String>,
    status_codes: Option<Vec<i64>>,
    search: Option<String>,
    limit: i64,
    offset: i64,
) -> Result<OrdersPage, String>   // { rows, total }
```

Các con số ở dashboard nên là `SELECT COUNT(*) ... GROUP BY trade_type` chứ không phải `.filter().length` trên toàn bộ dataset.

### P2-3 · Upsert từng dòng, không transaction

`src-tauri/src/api/sync_engine.rs:82-84`

```rust
            for order in &data_vec {
                self.repo.upsert_from_api(order, end).await?;
            }
```

Mỗi `upsert_from_api` là một `sqlx::query(...).execute(&self.pool)` riêng, tức mỗi dòng là một transaction ngầm của SQLite, kéo theo một lần fsync. Sync 30 ngày với vài nghìn lệnh sẽ chậm rõ rệt.

**Đề xuất:** mở transaction cho mỗi trang và nhận `&mut Transaction` trong `upsert_from_api`:

```rust
let mut tx = self.repo.pool().begin().await?;
for order in &data_vec { self.repo.upsert_from_api(&mut tx, order, end).await?; }
tx.commit().await?;
```

### P2-4 · Thiếu pragma SQLite

`src-tauri/src/db/mod.rs:15-17`

```rust
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&url).await?;
```

Kết nối bằng URL string nên không cấu hình được gì. Với 5 connection ghi đồng thời (scheduler + HTTP handler + command từ UI) mà không có `busy_timeout`, `SQLITE_BUSY` là chuyện sẽ xảy ra. Đáng chú ý là `clear_all_data` đã xoá cả `-wal` và `-shm`, tức tác giả cho rằng WAL đang bật — nhưng nó không được bật tường minh ở đâu.

**Đề xuất:**

```rust
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};

let opts = SqliteConnectOptions::new()
    .filename(db_path)
    .create_if_missing(true)
    .journal_mode(SqliteJournalMode::Wal)
    .synchronous(SqliteSynchronous::Normal)
    .busy_timeout(Duration::from_secs(5))
    .foreign_keys(true);
let pool = SqlitePoolOptions::new().max_connections(5).connect_with(opts).await?;
```

`foreign_keys(true)` cũng đáng chú ý: bản migration trong `db/migrations/001_init.sql` có khai báo `FOREIGN KEY ... ON DELETE CASCADE` nhưng SQLite mặc định **tắt** enforcement, nên ràng buộc đó chưa từng có tác dụng (xem thêm P3-6).

### P2-5 · HTTP client không có timeout

`src-tauri/src/api/c2c_api_client.rs:20-28`

```rust
    pub fn new(api_key: String, api_secret: String) -> Self {
        Self { 
            api_key, 
            api_secret, 
            http: Client::new(), 
```

`reqwest::Client::new()` mặc định **không** có timeout. Một kết nối treo sẽ chặn `sync_chunk`, mà `sync_chunk` được `await` trong `tokio::select!` của scheduler — nên treo một request là đứng luôn cả vòng lặp sync, không có gì phát hiện ra.

**Đề xuất:** `Client::builder().timeout(Duration::from_secs(20)).connect_timeout(Duration::from_secs(5)).build()?`.

### P2-6 · Retry cả những lỗi không thể retry

`src-tauri/src/api/sync_engine.rs:29-41`

```rust
        loop {
            match self.client.list_user_order_history(trade_type, start, end, page, rows).await {
                Ok(res) => return Ok(res),
                Err(e) => {
                    retries += 1;
                    if retries >= max_retries {
                        return Err(e);
                    }
                    println!("[SYNC] Retry {}/{} after {}ms: {}", retries, max_retries, delay, e);
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                    delay *= 2; // Exponential backoff
```

`list_user_order_history` gộp mọi thất bại thành một `anyhow::Error` (dòng 74: `Err(anyhow!("API error: {text}"))`), nên tầng retry không phân biệt được lỗi mạng tạm thời với API key sai hay chữ ký sai. Với credentials sai, mỗi trang bị thử lại 3 lần kèm chờ 1s + 2s — vô nghĩa và làm chậm việc báo lỗi cho người dùng.

`delay *= 2` không có trần và không có jitter; nếu `max_retries` được nâng lên thì thời gian chờ tăng theo luỹ thừa không giới hạn.

**Đề xuất:** trả về error type có phân loại và chỉ retry nhóm tạm thời:

```rust
enum ApiError {
    Transient { status: Option<u16> },   // 5xx, timeout, 429 → retry
    Auth,                                // -2014/-1022 → dừng ngay
    RateLimited { retry_after: Duration },
    Other(anyhow::Error),
}
```

Kèm trần backoff (`delay.min(30_000)`) và jitter ngẫu nhiên.

### P2-7 · Formatter được tạo lại mỗi lần render

`OrderTable` cache đúng một formatter rồi bỏ quên phần còn lại:

`fe/src/lib/OrderTable.svelte:13`

```svelte
  const nfFiat = new Intl.NumberFormat('vi-VN', { minimumFractionDigits: 0, maximumFractionDigits: 0 });
```

`fe/src/lib/OrderTable.svelte:88-91`

```svelte
    return new Intl.NumberFormat('vi-VN', { 
      minimumFractionDigits: 0,
      maximumFractionDigits: digits 
    }).format(num);
```

`formatAsset` tạo mới một `Intl.NumberFormat` mỗi lần gọi, tức 20 lần mỗi lần render bảng. `fmtDate` gọi `toLocaleString('vi-VN')` cũng dựng formatter nội bộ mỗi dòng. `OrderDetail.svelte` có cùng vấn đề ở `formatNumber` và `formatCryptoAmount`. Khởi tạo `Intl.*` đắt hơn nhiều so với việc gọi `.format()`.

**Đề xuất:** cache theo số chữ số thập phân, đặt ở module scope để dùng chung cho mọi component:

```ts
const numberFormatters = new Map<number, Intl.NumberFormat>();
export function formatAmount(value: number, digits: number) {
  let f = numberFormatters.get(digits);
  if (!f) {
    f = new Intl.NumberFormat('vi-VN', { minimumFractionDigits: 0, maximumFractionDigits: digits });
    numberFormatters.set(digits, f);
  }
  return f.format(value);
}
```

Ngoài ra `transition:slide` trên `<tr>` cộng với `animation-delay: {i * 30}ms` (`OrderTable.svelte:207-208`) khiến dòng thứ 20 chờ 570ms mới hiện. Vì `.order-row` đặt `opacity: 0` và dựa vào `animation-fill-mode: forwards`, nếu animation không chạy thì các dòng **vô hình**. Nên bỏ animation trên dòng bảng, hoặc ít nhất bọc trong `@media (prefers-reduced-motion: no-preference)`.

---

## 7. Chi tiết nhóm P3 — Code chết và vệ sinh repo

### P3-1 · Bảy file Rust không bao giờ được compile

`main.rs` khai báo module dạng inline, chỉ nhận đúng hai submodule:

`src-tauri/src/main.rs:6`

```rust
mod orders { pub mod repo; pub mod stage_map; }
```

Vì `mod orders { … }` có thân, Rust **không** tìm `orders/mod.rs`. Và `lib.rs` không khai báo `mod orders` nào cả. Nên `orders/mod.rs` cùng mọi thứ nó khai báo đều nằm ngoài cây module.

Bằng chứng lấy từ dep-info do chính cargo sinh ra sau `cargo check --all-targets` (liệt kê đầy đủ input của từng target):

```
--- tauri_app-74f4e8d8ecc2affb.d ---     (bin target)
src\api\c2c_api_client.rs
src\api\credentials.rs
src\api\sync_engine.rs
src\crypto\mod.rs
src\db\mod.rs
src\main.rs
src\orders\repo.rs
src\orders\stage_map.rs

--- tauri_app_lib-1f2c82cb8b66d91a.d --- (lib target)
src\lib.rs
```

Vậy các file sau **chưa từng được compiler đọc tới**:

| File | Kích thước |
|---|---|
| `src/orders/mod.rs` | 113 B |
| `src/orders/store.rs` | 7.183 B |
| `src/orders/parser_list.rs` | 3.129 B |
| `src/orders/dedup.rs` | 1.921 B |
| `src/orders/parser_detail.rs` | 1.673 B |
| `src/orders/status.rs` | 340 B |
| `src/credentials/mod.rs` | 0 B |

Tổng ~14,4 KB. Đây không phải "code chưa dùng" mà là code **không tồn tại** với trình biên dịch: nó không được type-check, không được kiểm tra cú pháp, và sẽ hỏng dần theo mỗi lần refactor mà không ai biết. `OrderStore` trong `store.rs` còn là một tầng lưu trữ in-memory song song, cạnh tranh khái niệm với `OrderRepo` đang dùng thật.

**Đề xuất:** xoá cả bảy file. Nếu muốn giữ `OrderStore` cho tương lai thì phải đưa vào cây module và để `cargo check` nhìn thấy — bằng không nó chỉ là nợ.

### P3-2 · `lib.rs` là một app Tauri thứ hai

`src-tauri/src/lib.rs:27-47`

```rust
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let ws_state = WsState::new();
    ...
            tauri::async_runtime::spawn(async move {
                if let Err(e) = start_ws_server("127.0.0.1:8123", ws_state_for_task).await {
```

File này dựng `tauri::Builder`, đăng ký command `get_raw_messages`, và chạy một WebSocket server trên cổng 8123. `main.rs` có `#[tokio::main] async fn main()` riêng và **không** gọi `run()`. Nên đây là target được compile nhưng không bao giờ thực thi — nó chỉ tồn tại để làm chậm build và gây nhầm lẫn khi đọc.

Đáng chú ý: `README.md` mô tả kiến trúc theo đúng file chết này (*"Extension forwards compact JSON payloads via `ws://127.0.0.1:8123`"*), trong khi hệ thống thật dùng HTTP `:1425`. Tài liệu đang mô tả code không chạy.

**Đề xuất:** xoá `lib.rs` và bỏ khối `[lib]` trong `Cargo.toml`. Nếu sau này cần target mobile thì dựng lại đúng cách bằng cách chuyển logic của `main.rs` vào `lib.rs::run()` và để `main.rs` chỉ gọi nó — đó là bố cục Tauri 2 tiêu chuẩn mà template ban đầu hướng tới.

### P3-3 · 75 file build artifact bị commit

`.gitignore` dùng pattern neo ở gốc repo:

`.gitignore:1-11`

```gitignore
.DS_Store
node_modules
/build
/.svelte-kit
```

`/build` chỉ khớp `<root>/build`, không khớp `fe/build`. Kiểm chứng:

```
$ git check-ignore -v fe/build/index.html fe/.svelte-kit/tsconfig.json fe/node_modules/svelte/package.json
.gitignore:2:node_modules   fe/node_modules/svelte/package.json
```

Chỉ `node_modules` được match; hai đường dẫn còn lại **không** bị ignore. Số lượng và dung lượng:

```
tracked files in fe/build + fe/.svelte-kit : 75
total bytes                                : 663.063  (~648 KB)
```

Tác hại cụ thể: chạy `npm run build` một lần trên repo sạch làm bẩn đúng **toàn bộ 75 file** (do tên chunk chứa hash nội dung):

```
$ npm run build && git status --porcelain -- fe/build fe/.svelte-kit | wc -l
75
```

Nghĩa là mọi developer đều có 75 file thay đổi trong `git status` sau khi build, mọi PR đều lẫn diff của file sinh tự động, và merge conflict trên các file này là không thể giải quyết có ý nghĩa.

**Đề xuất:**

```gitignore
node_modules/
build/
.svelte-kit/
dist/
target/
```

Rồi bỏ theo dõi:

```bash
git rm -r --cached fe/build fe/.svelte-kit
```

Lưu ý một hệ quả: `tauri.conf.json` trỏ `frontendDist: "../fe/build"`, nên sau khi bỏ track thì **bắt buộc** phải chạy `npm run fe:build` trước `tauri build`. `beforeBuildCommand` đã lo việc này, nhưng CI cần chạy build frontend trước khi build Tauri.

### P3-4 · Dependency không dùng

Đối chiếu `Cargo.toml` với kết quả grep toàn bộ `src/`:

| Dependency | Nơi tham chiếu | Kết luận |
|---|---|---|
| `sha1` | chỉ `orders/dedup.rs` | Không compile (P3-1) → **xoá** |
| `urlencoding` | không có chỗ nào | **xoá** |
| `tower` | không có `use tower::` | transitive của `tower-http` → **xoá khỏi dep trực tiếp** |
| `tungstenite` | không có `use tungstenite::` | **xoá** |
| `tokio-tungstenite` | chỉ `lib.rs` | chết lúc runtime → xoá cùng P3-2 |
| `futures` | chỉ `lib.rs` | chết lúc runtime → xoá cùng P3-2 |
| `tower-http` | `main.rs:23` (CORS) | **giữ** |

Root `package.json` cũng khai báo `ws: ^8.18.3` trong `dependencies` dù không có file JS nào ở root dùng nó — di sản của kiến trúc WebSocket cũ.

Ngoài ra `sqlx` đang bật feature `macros` nhưng code chỉ dùng `sqlx::query()` runtime, không dùng `query!`/`query_as!` compile-time. Nếu chuyển sang `sqlx::migrate!` (P0-9) thì cần thêm feature `migrate`.

Metadata package cũng còn nguyên của template:

`src-tauri/Cargo.toml:1-6`

```toml
[package]
name = "tauri-app"
version = "0.1.0"
description = "A Tauri App"
authors = ["you"]
```

`version` ở đây là `0.1.0` trong khi `tauri.conf.json` và `package.json` đều là `1.0.1` — ba nguồn phiên bản không đồng bộ.

### P3-5 · Extension không thể load được

`chrome-extension/manifest.json` tham chiếu 4 file script và 3 icon:

`chrome-extension/manifest.json:18-50`

```json
  "background": {
    "service_worker": "background.js",
    "type": "module"
  },
  ...
      "js": ["content.js"],
  ...
      "resources": ["injected.js", "scraper.js"],
  ...
  "icons": {
    "16": "icon16.png",
    "48": "icon48.png",
    "128": "icon128.png"
  }
```

Nhưng `git ls-files chrome-extension/` chỉ trả về đúng một dòng: `chrome-extension/manifest.json`. Không có `background.js`, `content.js`, `injected.js`, `scraper.js`, hay icon nào. Chrome sẽ từ chối load thư mục này.

Vì extension là **nguồn duy nhất** của thông tin thanh toán, tính năng chính của app không thể hoạt động khi clone repo. `README.md` còn hướng dẫn load từ `p2p-extension/` — một đường dẫn thứ ba không tồn tại.

Manifest cũng thiếu quyền gọi về app:

`chrome-extension/manifest.json:11-16`

```json
  "host_permissions": [
    "https://c2c.binance.com/*",
    ...
  ],
```

Không có `http://127.0.0.1:1425/*`. MV3 service worker cần host permission để `fetch` tới localhost; hiện tại nó chỉ đi được vì server trả CORS `Any` (P1-2) — nghĩa là khi siết CORS lại thì phải bổ sung quyền này.

**Đề xuất:** commit đầy đủ source extension, hoặc nếu nó được giữ riêng thì `README` phải nói rõ và manifest không nên nằm lẻ trong repo. Thêm `http://127.0.0.1:1425/*` vào `host_permissions`.

### P3-6 · Migration trùng lặp đã lệch schema

Tồn tại hai bản `001_init.sql`, và chúng **khác nhau**:

| | `src-tauri/migrations/` (bản đang dùng) | `db/migrations/` (bản chết) |
|---|---|---|
| `order_payment_detail.amount` | có | **không** |
| `.transfer_content` | có | **không** |
| `.suggested_transfer_content` | có | **không** |
| FOREIGN KEY → `orders` | không | **có** (`ON DELETE CASCADE`) |
| `idx_payment_order` | có | không |

Chỉ bản trong `src-tauri/migrations/` được `include_str!`. Bản trong `db/` có dòng đầu `-- moved from src-tauri/migrations/001_init.sql` cho thấy ý định di chuyển chưa hoàn tất, và giờ nó là bẫy: ai đọc `db/migrations/` sẽ hiểu sai schema thật.

Điều thú vị là bản chết lại có `FOREIGN KEY ... ON DELETE CASCADE` — chính là thứ sẽ giải quyết được vấn đề bản ghi mồ côi ở P0-5. Nên khôi phục ý tưởng đó vào migration thật (kèm `foreign_keys(true)` ở P2-4), rồi xoá thư mục `db/migrations/`.

### P3-7 · Sáu cột schema không ai đọc hoặc ghi

Grep toàn repo (`*.rs`, `*.svelte`, `*.sql`, `*.ts`) cho các cột của `orders`:

| Cột | Ghi | Đọc |
|---|---|---|
| `has_payment_detail` | không bao giờ | `repo.rs:103,128` |
| `buyer_paid_time_ms` | không bao giờ | không bao giờ |
| `released_time_ms` | không bao giờ | không bao giờ |
| `cancelled_time_ms` | không bao giờ | không bao giờ |
| `last_ext_update_ts` | không bao giờ | không bao giờ |
| `remark` | không bao giờ | không bao giờ |

`has_payment_detail` là trường hợp tệ nhất vì nó **được đọc và gửi về frontend**:

`src-tauri/src/orders/repo.rs:128`

```rust
                has_payment_detail: r.get::<i64,_>("has_payment_detail") == 1,
```

Không có `UPDATE` nào đặt cột này, nên nó vĩnh viễn bằng `0` và field trong `OrderRow` vĩnh viễn `false`. Frontend không dùng field đó, nên hệ quả duy nhất hiện tại là payload IPC to hơn cần thiết — nhưng nó là một cái bẫy: ai thấy tên field này sẽ tưởng có thể dựa vào nó để biết lệnh nào đã có thông tin thanh toán.

Thêm một rủi ro nhỏ: cột khai báo `INTEGER DEFAULT 0` mà không `NOT NULL`; `r.get::<i64,_>()` sẽ **panic** nếu gặp NULL (ví dụ dữ liệu do migration cũ hoặc insert thủ công tạo ra).

**Đề xuất:** hoặc cập nhật cột này thật (một `UPDATE orders SET has_payment_detail = 1 WHERE order_number = ?` khi lưu payment detail, việc này đồng thời cho phép UI đánh dấu lệnh nào có QR), hoặc bỏ cột và field. Các cột thời gian còn lại nên bỏ cho tới khi có nhu cầu thật.

### P3-8 · Mọi toast đè lên nhau

`ToastContainer` dựng một flex column có `gap: 12px`:

`fe/src/lib/ToastContainer.svelte:61-71`

```svelte
  .toast-container {
    position: fixed;
    top: 0;
    right: 0;
    ...
    display: flex;
    flex-direction: column;
    gap: 12px;
```

Nhưng mỗi `Toast` con lại tự `position: fixed` vào cùng một toạ độ:

`fe/src/lib/Toast.svelte:55-59`

```svelte
  .toast {
    position: fixed;
    top: 20px;
    right: 20px;
```

`position: fixed` đưa phần tử ra khỏi luồng layout, nên `flex-direction` và `gap` của container không có tác dụng. Hai toast cùng lúc sẽ nằm chồng khít lên nhau — và điều này xảy ra thường xuyên vì `toastError` giữ 5 giây trong khi `toastSuccess` giữ 3 giây.

**Đề xuất:** bỏ `position`/`top`/`right` khỏi `.toast`, để container lo việc định vị.

Cùng file còn một chi tiết nhỏ: `setTimeout` trong `onMount` không được clear khi component bị destroy (`Toast.svelte:27-34`), nên nếu toast bị xoá sớm bằng nút đóng thì vẫn còn timer chạy và gọi `onClose` lần hai.

### P3-9 · `.btn-primary` không được định nghĩa

`fe/src/lib/OrderDetail.svelte:463-471`

```svelte
      <button 
        class="btn-primary"
        on:click={() => {
```

Trong `<style>` của `OrderDetail.svelte` có `.btn-secondary` nhưng **không** có `.btn-primary`. Vì Svelte scope CSS theo component và nút này nằm trong modal (ngoài phạm vi style toàn cục ở `+page.svelte`), nút "🌐 Mở trên Binance" render với style mặc định của browser, lệch hẳn so với nút "Đóng" bên cạnh.

Đây cũng là điểm mà Svelte thường cảnh báo "Unused CSS selector" cho chiều ngược lại, nhưng chiều này — class dùng mà không có style — thì không có cảnh báo nào.

### P3-10 · `partnerName` trong bảng bỏ qua `trade_type`

Hai định nghĩa cho cùng một khái niệm. Ở trang chính, đúng:

`fe/src/routes/+page.svelte:50`

```svelte
  function partnerName(o:any) { return o.trade_type === 'BUY' ? o.seller_nickname : o.buyer_nickname }
```

Trong bảng, sai:

`fe/src/lib/OrderTable.svelte:97`

```svelte
  function partnerName(o:any) { return o.seller_nickname || o.buyer_nickname }
```

Với lệnh `SELL`, đối tác đúng phải là `buyer_nickname`, nhưng hàm trong bảng trả `seller_nickname` trước — mà với lệnh SELL thì `seller_nickname` chính là **bản thân người dùng**. Nên cột "Đối tác" ở tab "Lệnh bán" đang hiển thị chính tên người dùng.

Bản trong `+page.svelte` sau khi tách component ra thì không còn được dùng tới nữa — code chết bên cạnh một bản sao bị lỗi.

**Đề xuất:** đưa hàm này (và `fmtDate`, `toNum`, `statusText`) vào một module dùng chung, ví dụ `fe/src/lib/orders.ts`, để chỉ tồn tại một định nghĩa.

### P3-11 · Class CSS sinh từ nhãn tiếng Việt

`fe/src/lib/OrderTable.svelte:227`

```svelte
            <td class={"status-cell status-"+statusText(o).replace(/\s+/g, '-')}>{statusText(o)}</td>
```

Style tương ứng được viết tay theo từng nhãn:

`fe/src/lib/OrderTable.svelte:535-539`

```svelte
  .status-Đang-chờ-thanh-toán { 
    color: #60a5fa; 
```

Cách này ràng buộc CSS vào **nội dung văn bản**. Ba hệ quả:

1. `statusText` có nhánh fallback `` `Không xác định (${o.status_code})` `` (dòng 120) → sinh class `status-Không-xác-định-(7)`. Dấu ngoặc không escape khiến selector không hợp lệ, và style `.status-Không-xác-định` đã định nghĩa cũng không khớp.
2. Nhãn đến từ `StageMap` mà `StageMap` lại đọc từ file JSON (P0-10). Nếu file đó có tác dụng, nhãn sẽ thành `🔄 Đang giao dịch` và **mọi** style trạng thái mất tác dụng cùng lúc.
3. Đổi một chữ trong nhãn ở backend làm hỏng màu ở frontend, không có cảnh báo nào.

**Đề xuất:** map từ `status_code` (số, ổn định) sang class ngữ nghĩa:

```ts
const STATUS_CLASS: Record<number, string> = {
  1: 'status-pending', 2: 'status-paid', 3: 'status-verifying',
  4: 'status-completed', 5: 'status-cancelled', 6: 'status-cancelled',
};
const statusClass = (o) => STATUS_CLASS[o.status_code] ?? 'status-unknown';
```

### P3-12 · Frontend không có type nên `svelte-check` không bắt được gì

`svelte-check` chạy sạch:

```
svelte-check found 0 errors and 0 warnings
```

Nhưng kết quả đó không có nhiều giá trị, vì dữ liệu từ backend không hề được mô tả kiểu:

`fe/src/routes/+page.svelte:14-19`

```svelte
  let orders:any[] = [];
  ...
  let selectedOrder: any = null;
```

`fe/src/lib/OrderDetail.svelte:8`

```svelte
  export let order: any;
```

Chính vì `order: any` mà `order.create_time` ở P0-8 không bị phát hiện — với một interface đúng thì đó là lỗi biên dịch ngay dòng đó. Tương tự, lệch chữ ký `invoke` ở P0-7 cũng nằm trong vùng mù.

**Đề xuất:** khai báo một lần các kiểu khớp với struct Rust và dùng chúng ở mọi `invoke`:

```ts
// fe/src/lib/types.ts — phản chiếu OrderRow trong src-tauri/src/orders/repo.rs
export interface OrderRow {
  order_number: string;
  trade_type: 'BUY' | 'SELL';
  fiat: string; asset: string;
  amount_asset: string; total_fiat: string; price: string;
  create_time_ms: number;
  status_code: number; status_label: string;
  buyer_nickname: string; seller_nickname: string;
  has_payment_detail: boolean;
  last_api_sync_ts: number;
}

const orders = await invoke<OrderRow[]>('query_orders', { ... });
```

Lý tưởng hơn thì sinh tự động bằng [`ts-rs`](https://crates.io/crates/ts-rs) hoặc [`specta`](https://crates.io/crates/specta) từ chính struct Rust, để hai bên không thể lệch nhau.

Một ghi chú liên quan: `package.json` cài `svelte: ^5.0.0` nhưng toàn bộ code dùng cú pháp Svelte 4 (`export let`, `$:`, `on:click`, `<script context="module">`). Svelte 5 vẫn chạy được ở chế độ tương thích, nhưng sẽ không được lợi từ hệ reactivity mới, và các cú pháp này đã bị deprecated. Đây là việc nên làm sau khi các lỗi P0 đã xử lý xong, không nên làm cùng lúc.

---

## 8. Lộ trình đề xuất

Thứ tự dưới đây xếp theo tỷ lệ *giảm rủi ro / công sức*, không theo mức nghiêm trọng thuần.

### Giai đoạn 1 — Chặn máu (nửa ngày)

Đây là nhóm ảnh hưởng trực tiếp tới việc người dùng chuyển tiền sai.

1. **P0-4** sửa chuẩn hoá số tiền VietQR, bỏ tỷ giá hardcode.
2. **P0-6** thay if-chain ngân hàng bằng bảng dữ liệu + test cho SHB / Shinhan / Standard Chartered, và đối chiếu lại toàn bộ BIN với NAPAS (đặc biệt `BVBank` vs `VietBank` đang trùng `970433`).
3. **P0-1** thêm UNIQUE index + upsert `ON CONFLICT` với `COALESCE`, kèm migration dọn dòng trùng đã tồn tại.
4. **P0-2** sửa `status_code` → `order_status_code` và log lỗi thay vì nuốt.

Sau bước này QR hiển thị đúng ngân hàng, đúng số tiền, đúng dữ liệu mới nhất.

### Giai đoạn 2 — Đóng lỗ bảo mật (1 ngày)

5. **P1-2** thêm token cho HTTP endpoint, siết CORS, thêm body limit, chỉ nhận `orderNumber` có thật.
6. **P1-1** chuyển credentials sang keyring.
7. **P1-3** ngừng trả secret về UI.
8. **P1-4** thay `println!` bằng `tracing`, bỏ log số tài khoản.
9. **P1-5** đặt CSP tường minh.
10. **P1-7** xoá listener `postMessage` và `fetch_payment_from_localstorage`.

Bước 5 và 6 là hai việc quan trọng nhất trong nhóm: một cái chặn ghi dữ liệu giả từ web ngoài, một cái bảo vệ API key.

### Giai đoạn 3 — Dọn nền móng (nửa ngày)

11. **P3-3** sửa `.gitignore`, `git rm -r --cached` build artifacts.
12. **P3-1**, **P3-2** xoá 7 file không compile và `lib.rs`, bỏ khối `[lib]`.
13. **P3-4** bỏ dependency không dùng, đồng bộ version giữa `Cargo.toml` / `tauri.conf.json` / `package.json`.
14. **P0-9** chuyển sang `sqlx::migrate!`, xoá `002`, xoá `db/migrations/`.
15. **P0-10** quyết định dứt điểm chuyện `stage_map.json`.
16. **P3-6** đưa `FOREIGN KEY ON DELETE CASCADE` vào migration thật + bật `foreign_keys`.

Làm nhóm này trước giai đoạn 4 vì nó thu nhỏ đáng kể diện tích code cần sửa ở các bước sau.

### Giai đoạn 4 — Hiệu năng (1 ngày)

17. **P2-1** bỏ `setInterval` ở frontend, `active_poll` chỉ chạy khi có lệnh đang xử lý.
18. **P2-2** đẩy filter / search / phân trang xuống SQL.
19. **P2-3** upsert theo transaction.
20. **P2-4** bật WAL + `busy_timeout`.
21. **P2-5**, **P2-6** timeout cho HTTP client, phân loại lỗi để retry đúng.
22. **P2-7** cache `Intl` formatter, bỏ animation trên dòng bảng.

### Giai đoạn 5 — Chất lượng dài hạn (1–2 ngày)

23. **P3-12** khai báo type cho dữ liệu backend, cân nhắc `ts-rs`/`specta` sinh tự động.
24. Tách `main.rs` (1018 dòng) thành `commands/`, `http/`, `banks/`, `scheduler.rs`.
25. Gom SQL của `order_payment_detail` vào một `PaymentRepo` duy nhất — đây là biện pháp phòng ngừa tận gốc cho cả nhóm P0.
26. **P3-8** đến **P3-11**: toast, `.btn-primary`, `partnerName`, class trạng thái.
27. **P3-5** commit source extension, thêm `host_permissions` cho localhost.
28. Cập nhật `README.md` cho khớp thực tế (HTTP 1425 thay vì WS 8123, `chrome-extension/` thay vì `p2p-extension/`, và sửa hoặc bỏ tuyên bố "no external network calls" vì QR đang gọi `img.vietqr.io`).
29. Thêm test: bảng BIN ngân hàng, chuẩn hoá số tiền, upsert payment detail. Ba chỗ này là nơi lỗi P0 phát sinh và cũng là nơi dễ viết test nhất — hiện repo **không có test nào**.
30. Cân nhắc sinh QR offline bằng crate `qrcode` để thực sự không gửi số tài khoản ra ngoài.

### Việc nên làm ngay, độc lập với mọi giai đoạn

`db/seed_credentials.applied.json` đang được commit với giá trị rỗng:

`db/seed_credentials.applied.json:1-5`

```json
{
  "label": "test",
  "api": "",
  "api_secret": ""
}
```

`.gitignore` chỉ loại trừ `db/seed_credentials.json`, **không** loại trừ biến thể `.applied.json` — mà `main.rs:901-902` chính là chỗ `rename` file gốc thành tên đó sau khi nạp. Nghĩa là luồng seed tự động tạo ra một file **chứa API key thật** với cái tên **không nằm trong .gitignore**. Lần này nó rỗng, nhưng cơ chế đang chờ để rò rỉ.

Sửa ngay:

```gitignore
db/seed_credentials*.json
```

và `git rm --cached db/seed_credentials.applied.json`.

---

## 9. Cách tái hiện các kiểm chứng

Môi trường trên máy này (không có trong PATH hệ thống, cần thêm thủ công):

```powershell
$env:Path = "C:\Users\Admin\tools\node;" +
            "C:\Users\Admin\tools\mingw64\bin;" +
            "C:\Users\Admin\.cargo\bin;" +
            "C:\Program Files\Git\cmd;" + $env:Path
```

`mingw64\bin` là bắt buộc cho Rust: toolchain host là `x86_64-pc-windows-gnu` và crate `windows-sys` cần `dlltool.exe`. Không có nó, `cargo check` dừng với `error: error calling dlltool 'dlltool.exe': program not found`.

Kiểm tra backend:

```powershell
cd src-tauri
cargo check --all-targets
# → Finished, 2 warnings: unused variable `account_name`, `message` (main.rs:501,503)
```

Xem chính xác file nào được compile:

```powershell
$t = $env:CARGO_TARGET_DIR
Get-ChildItem -Path $t -Filter "*.d" -Recurse |
  Where-Object { (Get-Content $_.FullName -Raw) -match "p2p-qr" } |
  ForEach-Object { Get-Content $_.FullName }
```

Kiểm tra frontend:

```powershell
cd fe
npm install
npx svelte-kit sync
npx svelte-check --tsconfig ./tsconfig.json     # → 0 errors (xem P3-12 về ý nghĩa)
npm run build                                    # → pass
```

Chứng minh build artifact gây bẩn repo:

```powershell
git status --porcelain -- fe/build fe/.svelte-kit   # trước: rỗng
cd fe; npm run build; cd ..
git status --porcelain -- fe/build fe/.svelte-kit   # sau: 75 dòng
git checkout -- fe/build fe/.svelte-kit; git clean -fd fe/build fe/.svelte-kit
```

Kiểm chứng nhóm lỗi SQL và số tiền: script `verify_sqlite.mjs` chạy lại đúng schema và đúng các câu query của app trên SQLite in-memory (`node:sqlite`, có sẵn từ Node 22). Nó kiểm tra bảy điểm: migration runner, 002 trên nền 001, `INSERT OR REPLACE` không UNIQUE index, đọc không `ORDER BY`, `created_at`, `status_code`, và chuẩn hoá số tiền VietQR. Script này nằm ngoài repo (`C:\Users\Admin\tools\verify_sqlite.mjs`) để không lẫn vào source; nếu muốn giữ lâu dài thì nên chuyển thành test Rust thật trong `src-tauri/tests/`.

---

## 10. Ghi chú về giới hạn của báo cáo

Những điều **chưa** kiểm chứng được và cần lưu ý:

- **Chưa chạy app.** Không build được bundle Tauri (thiếu MSVC và WebView2 runtime trên máy này), nên các lỗi runtime được suy ra từ schema và code chứ không phải từ việc quan sát app thật. Bốn lỗi SQL thì đã chạy lại query thật trên SQLite nên độ chắc chắn cao.
- **Chưa gọi API Binance.** Không có credentials, nên hình dạng response thật của `listUserOrderHistory` chưa được đối chiếu. Phần parse trong `repo.rs:33-93` đang thử rất nhiều tên field thay thế (`buyerNickname` / `buyerNickName`, `sellerNickname` / `makerNickname` / `counterPartNickName`…), điều này gợi ý rằng hình dạng response từng thay đổi hoặc chưa từng được xác định rõ. Nên xác nhận lại với response thật rồi thu gọn.
- **Giá trị các mã BIN ngân hàng chưa được xác minh.** Tôi chỉ chứng minh được **thứ tự** so khớp gây lỗi, không kiểm chứng từng mã số đúng hay sai. Việc đối chiếu với bảng BIN chính thức của NAPAS là bắt buộc trước khi phát hành, và điểm đáng ngờ nhất là `970433` đang được dùng cho cả `BVBank` (dòng 371) và `VietBank` (dòng 481).
- **Extension chưa đọc được** vì source không có trong repo, nên phần dữ liệu vào của hệ thống là một hộp đen trong báo cáo này. Cụ thể: `amount` mà extension gửi lên là VND hay USDT thì chưa xác định được — điều này ảnh hưởng trực tiếp tới cách sửa P0-4 cho đúng.

