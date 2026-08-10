#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod banks;
mod bot;
mod db;
mod vietqr;

mod api {
    pub mod c2c_api_client;
    pub mod credentials;
    pub mod sync_engine;
}

mod orders {
    pub mod payment_repo;
    pub mod repo;
    pub mod stage_map;
}

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use axum::extract::{DefaultBodyLimit, Json, State as AxumState};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::Json as ResponseJson;
use axum::routing::{get, post};
use axum::Router;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};
use tokio::sync::RwLock;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use api::c2c_api_client::C2CApiClient;
use api::credentials::{CredentialInfo, CredentialsRepo};
use api::sync_engine::SyncEngine;
use db::Db;
use orders::payment_repo::{PaymentDetail, PaymentDetailInput, PaymentRepo};
use orders::repo::OrderRepo;
use orders::stage_map::StageMap;

const HTTP_ADDR: SocketAddr = SocketAddr::new(
    std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
    1425,
);
/// Payload của extension chỉ là vài trăm byte; giới hạn để một request khổng lồ
/// không ăn hết RAM.
const HTTP_BODY_LIMIT: usize = 64 * 1024;

pub struct AppCtx {
    order_repo: Arc<OrderRepo>,
    payment_repo: Arc<PaymentRepo>,
    creds_repo: Arc<CredentialsRepo>,
    api_client: Arc<RwLock<Option<C2CApiClient>>>,
}

#[derive(Clone)]
struct HttpAppState {
    order_repo: Arc<OrderRepo>,
    payment_repo: Arc<PaymentRepo>,
    creds_repo: Arc<CredentialsRepo>,
    api_client: Arc<RwLock<Option<C2CApiClient>>>,
    handle: tauri::AppHandle,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StoreCredentialsResult {
    account_switched: bool,
}

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

// ───────────────────────────── Credentials ─────────────────────────────

#[tauri::command]
async fn store_api_credentials(
    state: State<'_, AppCtx>,
    app: tauri::AppHandle,
    label: String,
    api_key: String,
    api_secret: String,
    payer_bank_name: Option<String>,
) -> Result<StoreCredentialsResult, String> {
    let previous_key = state
        .creds_repo
        .current_api_key()
        .await
        .map_err(|e| e.to_string())?;
    let account_switched = previous_key
        .as_ref()
        .map(|old| old != &api_key)
        .unwrap_or(false);

    state
        .creds_repo
        .store(
            &label,
            &api_key,
            &api_secret,
            payer_bank_name.as_deref(),
        )
        .await
        .map_err(|e| e.to_string())?;

    // Đổi API key = đổi tài khoản Binance → xoá lệnh/sync cũ để không trộn dữ liệu.
    if account_switched {
        state
            .order_repo
            .clear_all()
            .await
            .map_err(|e| e.to_string())?;
        let _ = app.emit(
            "orders-updated",
            &serde_json::json!({"source": "cleared"}),
        );
        info!("đã xoá dữ liệu lệnh cũ vì đổi API key/tài khoản Binance");
    }

    let client = C2CApiClient::new(api_key, api_secret);
    if let Err(e) = client.sync_time().await {
        warn!(error = %e, "chưa đồng bộ được giờ, sẽ thử lại ở lần gọi API đầu tiên");
    }
    *state.api_client.write().await = Some(client);
    Ok(StoreCredentialsResult { account_switched })
}

#[tauri::command]
async fn update_payer_bank_name(
    state: State<'_, AppCtx>,
    payer_bank_name: String,
) -> Result<(), String> {
    state
        .creds_repo
        .update_payer_bank_name(&payer_bank_name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn check_api_credentials(state: State<'_, AppCtx>) -> Result<bool, String> {
    Ok(state.api_client.read().await.is_some())
}

/// Trả về thông tin đã che để UI hiển thị.
///
/// Bản trước có `get_saved_credentials` trả nguyên api_secret về frontend, nghĩa là
/// secret nằm trong bộ nhớ webview và trong mọi ảnh chụp devtools.
#[tauri::command]
async fn get_credential_info(state: State<'_, AppCtx>) -> Result<Option<CredentialInfo>, String> {
    state.creds_repo.info().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn clear_api_credentials(state: State<'_, AppCtx>) -> Result<(), String> {
    state.creds_repo.clear().await.map_err(|e| e.to_string())?;
    *state.api_client.write().await = None;
    Ok(())
}

#[tauri::command]
async fn test_api_credentials(state: State<'_, AppCtx>) -> Result<String, String> {
    let client = current_client(&state).await?;

    client
        .sync_time()
        .await
        .map_err(|e| format!("Không đồng bộ được giờ với Binance: {e}"))?;

    let now = now_ms();
    let res = client
        .list_user_order_history("BUY", now - 5 * 60 * 1000, now, 1, 1)
        .await
        .map_err(|e| e.to_string())?;

    // Không trả nguyên phản hồi về UI: nó chứa dữ liệu lệnh, nickname và số tiền.
    let total = res
        .get("data")
        .and_then(|d| d.get("total"))
        .and_then(|t| t.as_i64());

    Ok(match total {
        Some(t) => format!("Kết nối thành công. Binance ghi nhận {t} lệnh trong 5 phút gần nhất."),
        None => "Kết nối thành công.".to_string(),
    })
}

async fn current_client(state: &State<'_, AppCtx>) -> Result<C2CApiClient, String> {
    state
        .api_client
        .read()
        .await
        .clone()
        .ok_or_else(|| "Chưa cấu hình API credentials".to_string())
}

// ───────────────────────────── Sync ─────────────────────────────

#[tauri::command]
async fn force_initial_sync(state: State<'_, AppCtx>, days: i64) -> Result<u64, String> {
    let client = current_client(&state).await?;
    client
        .sync_time()
        .await
        .map_err(|e| format!("Không đồng bộ được giờ với Binance: {e}"))?;

    SyncEngine::new(&client, &state.order_repo)
        .force_initial_sync(days)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn force_sync_recent(state: State<'_, AppCtx>, app: tauri::AppHandle) -> Result<u64, String> {
    let client = current_client(&state).await?;
    let engine = SyncEngine::new(&client, &state.order_repo);

    let mut changed = engine.active_poll().await.map_err(|e| e.to_string())?;
    changed += engine.incremental_sync().await.map_err(|e| e.to_string())?;
    // Luôn báo UI tải lại — kể cả khi không có dòng nào đổi — để nút tải lại/cập nhật
    // không bị cảm giác "không chạy".
    let _ = app.emit(
        "orders-updated",
        &serde_json::json!({"source": "poll", "changed": changed}),
    );
    Ok(changed)
}

// ───────────────────────────── Orders ─────────────────────────────

#[tauri::command]
async fn list_orders_from_db(
    state: State<'_, AppCtx>,
    limit: i64,
) -> Result<Vec<orders::repo::OrderRow>, String> {
    state
        .order_repo
        .list_orders(limit)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct DbStats {
    total: i64,
    buy_count: i64,
    sell_count: i64,
    in_progress: i64,
}

#[tauri::command]
async fn get_db_stats(state: State<'_, AppCtx>) -> Result<DbStats, String> {
    use sqlx::Row;
    let pool = state.order_repo.pool();

    // Một lượt quét bảng cho cả ba con số, thay vì ba câu COUNT riêng.
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) AS total,
               SUM(CASE WHEN trade_type = 'BUY' THEN 1 ELSE 0 END) AS buy_count,
               SUM(CASE WHEN trade_type = 'SELL' THEN 1 ELSE 0 END) AS sell_count
        FROM orders
        "#,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    let in_progress = state
        .order_repo
        .count_in_progress()
        .await
        .map_err(|e| e.to_string())?;

    Ok(DbStats {
        total: row.get("total"),
        buy_count: row.get::<Option<i64>, _>("buy_count").unwrap_or(0),
        sell_count: row.get::<Option<i64>, _>("sell_count").unwrap_or(0),
        in_progress,
    })
}

// ───────────────────────────── Payment detail ─────────────────────────────

/// Payload từ UI. Tauri chuyển camelCase của JS sang đây.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PaymentPayload {
    order_number: String,
    account_name: Option<String>,
    account_no: Option<String>,
    bank_name: Option<String>,
    sub_bank: Option<String>,
    amount: Option<String>,
    transfer_content: Option<String>,
    suggested_transfer_content: Option<String>,
}

#[tauri::command]
async fn save_payment_detail(
    state: State<'_, AppCtx>,
    payload: PaymentPayload,
) -> Result<(), String> {
    let payer_name = state
        .creds_repo
        .payer_bank_name()
        .await
        .map_err(|e| e.to_string())?;
    let input = build_payment_input(
        &state.order_repo,
        &state.payment_repo,
        payload,
        payer_name,
    )
    .await
    .map_err(|e| e.to_string())?;

    state
        .payment_repo
        .upsert(&input, now_ms())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_order_payment_detail(
    state: State<'_, AppCtx>,
    order_number: String,
) -> Result<Option<PaymentDetail>, String> {
    let mut detail = state
        .payment_repo
        .get(&order_number)
        .await
        .map_err(|e| e.to_string())?;

    // Áp dụng lại config hiện tại lên bản ghi cũ: nội dung CK + đề xuất + QR addInfo.
    if let Some(ref mut d) = detail {
        let payer = state
            .creds_repo
            .payer_bank_name()
            .await
            .map_err(|e| e.to_string())?;
        if let Some(name) = payer.filter(|s| !s.trim().is_empty()) {
            // Dùng nguyên nội dung user cấu hình (không tự thêm hậu tố).
            let content = name.trim().to_string();
            d.transfer_content = Some(content.clone());
            d.suggested_transfer_content = Some(content);
        } else {
            d.suggested_transfer_content =
                vietqr::sanitize_add_info(d.suggested_transfer_content.as_deref());
        }
        if let (Some(bank), Some(account)) = (d.bank_name.as_deref(), d.account_no.as_deref()) {
            let amount = d.amount.as_deref().and_then(vietqr::parse_vnd_amount);
            if let Some(url) =
                vietqr::image_url(bank, account, amount, d.transfer_content.as_deref())
            {
                d.qr_code_url = Some(url);
            }
        }
    }

    Ok(detail)
}

#[tauri::command]
async fn cleanup_old_payment_details(state: State<'_, AppCtx>) -> Result<u64, String> {
    state
        .payment_repo
        .purge_expired(now_ms())
        .await
        .map_err(|e| e.to_string())
}

/// Xoá toàn bộ dữ liệu người dùng.
///
/// Bản trước nội suy đường dẫn vào một câu lệnh PowerShell rồi gọi `std::process::exit(0)`.
/// exit(0) bỏ qua toàn bộ cleanup của Tauri: pool SQLite không được đóng, WAL chưa
/// checkpoint, và người dùng phải tự mở lại app. Xoá bằng SQL trong một transaction
/// thì không cần cả tiến trình con lẫn việc thoát app.
#[tauri::command]
async fn clear_all_data(state: State<'_, AppCtx>, app: tauri::AppHandle) -> Result<(), String> {
    state
        .order_repo
        .clear_all()
        .await
        .map_err(|e| e.to_string())?;
    state.creds_repo.clear().await.map_err(|e| e.to_string())?;
    *state.api_client.write().await = None;

    let _ = app.emit("orders-updated", &serde_json::json!({"source": "cleared"}));
    info!("đã xoá toàn bộ dữ liệu người dùng");
    Ok(())
}

/// Dựng bản ghi payment detail, kèm QR sinh ở backend.
async fn build_payment_input(
    order_repo: &OrderRepo,
    payment_repo: &PaymentRepo,
    payload: PaymentPayload,
    payer_bank_name: Option<String>,
) -> Result<PaymentDetailInput> {
    let order_number = payload.order_number.trim().to_string();
    if order_number.is_empty() {
        anyhow::bail!("thiếu orderNumber");
    }

    // Extension thường tới trang thanh toán trước khi incremental sync kịp ghi lệnh.
    // Tạo placeholder BUY (status=1) để lưu QR ngay; sync API sẽ ghi đè sau.
    if !payment_repo.order_exists(&order_number).await? {
        order_repo
            .ensure_buy_placeholder(&order_number, payload.amount.as_deref(), now_ms())
            .await?;
    }

    // Ưu tiên tổng tiền do API Binance trả về; chuỗi extension bóc từ giao diện web
    // chỉ dùng khi API chưa có dữ liệu.
    let amount_source = match order_repo.total_fiat_vnd(&order_number).await? {
        Some(total) => Some(total),
        None => payload.amount.clone(),
    };
    let amount_vnd = amount_source.as_deref().and_then(vietqr::parse_vnd_amount);

    // Có config → dùng nguyên nội dung user nhập (giữ hoa/thường). Không config → None
    // để QR/app ngân hàng dùng mặc định.
    let transfer_content = payer_bank_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|name| name.to_string());

    let qr_code_url = match (payload.bank_name.as_deref(), payload.account_no.as_deref()) {
        (Some(bank), Some(account)) => {
            let url = vietqr::image_url(bank, account, amount_vnd, transfer_content.as_deref());
            if url.is_none() {
                warn!(order_number = %order_number, "không sinh được VietQR: ngân hàng chưa hỗ trợ hoặc số tài khoản không hợp lệ");
            }
            url
        }
        _ => None,
    };

    // Có config → đề xuất = nội dung config. Không config → memo Binance (đã bỏ mã lệnh).
    let suggested_transfer_content = match &transfer_content {
        Some(content) => Some(content.clone()),
        None => vietqr::sanitize_add_info(
            payload
                .transfer_content
                .as_deref()
                .or(payload.suggested_transfer_content.as_deref()),
        ),
    };

    Ok(PaymentDetailInput {
        order_number,
        account_name: payload.account_name,
        account_no: payload.account_no,
        bank_name: payload.bank_name,
        sub_bank: payload.sub_bank,
        qr_code_url,
        amount: amount_vnd.map(|a| a.to_string()),
        transfer_content,
        suggested_transfer_content,
    })
}

// ───────────────────────────── HTTP bridge ─────────────────────────────

#[derive(Deserialize)]
struct ExtensionRequest {
    #[serde(rename = "type")]
    request_type: String,
    data: serde_json::Value,
}

#[derive(Serialize)]
struct ApiResponse {
    success: bool,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<&'static str>,
}

impl ApiResponse {
    fn ok() -> Self {
        Self {
            success: true,
            message: "Đã lưu thông tin thanh toán".into(),
            code: None,
        }
    }

    fn failed(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            code: None,
        }
    }
}

/// Chỉ nhận request từ extension của trình duyệt.
///
/// Server nghe trên `127.0.0.1:1425` và trước đây bật CORS `allow_origin(Any)`, nên
/// bất kỳ trang web nào người dùng đang mở đều gọi được endpoint này và đọc được
/// phản hồi. Service worker của extension gửi `Origin: chrome-extension://…` hoặc
/// không gửi Origin, còn một trang web luôn gửi `Origin: https://…`.
fn is_extension_origin(origin: &str) -> bool {
    origin.starts_with("chrome-extension://")
        || origin.starts_with("moz-extension://")
        || origin.starts_with("safari-web-extension://")
}

fn origin_allowed(headers: &HeaderMap) -> bool {
    match headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        None => true,
        Some(origin) => is_extension_origin(origin),
    }
}

async fn handle_payment_detail(
    AxumState(state): AxumState<HttpAppState>,
    headers: HeaderMap,
    Json(request): Json<ExtensionRequest>,
) -> (StatusCode, ResponseJson<ApiResponse>) {
    if !origin_allowed(&headers) {
        warn!("từ chối request payment detail vì Origin không phải extension");
        return (
            StatusCode::FORBIDDEN,
            ResponseJson(ApiResponse::failed("Origin không được phép")),
        );
    }

    if request.request_type != "PAYMENT_DETAIL" {
        return (
            StatusCode::BAD_REQUEST,
            ResponseJson(ApiResponse::failed("Loại request không được hỗ trợ")),
        );
    }

    let get_str = |key: &str| -> Option<String> {
        request
            .data
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    let payload = PaymentPayload {
        order_number: get_str("orderNumber").unwrap_or_default(),
        account_name: get_str("accountName"),
        account_no: get_str("accountNo"),
        bank_name: get_str("bankName"),
        sub_bank: get_str("subBank"),
        amount: get_str("amount"),
        transfer_content: get_str("transferContent"),
        suggested_transfer_content: get_str("suggestedTransferContent"),
    };
    let order_number = payload.order_number.clone();

    let payer_name = match state.creds_repo.payer_bank_name().await {
        Ok(name) => name,
        Err(e) => {
            warn!(error = %e, "không đọc được nội dung CK cấu hình");
            None
        }
    };

    let input = match build_payment_input(
        &state.order_repo,
        &state.payment_repo,
        payload,
        payer_name,
    )
    .await
    {
        Ok(input) => input,
        Err(e) => {
            let msg = e.to_string();
            warn!(error = %msg, "payload payment detail không hợp lệ");
            return (
                StatusCode::BAD_REQUEST,
                ResponseJson(ApiResponse::failed(msg)),
            );
        }
    };

    if let Err(e) = state.payment_repo.upsert(&input, now_ms()).await {
        error!(error = %e, "không lưu được payment detail");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            ResponseJson(ApiResponse::failed("Lỗi ghi cơ sở dữ liệu")),
        );
    }

    // Kéo lệnh thật từ sàn ngay để thay placeholder + cập nhật trạng thái.
    if let Some(client) = state.api_client.read().await.clone() {
        let order_repo = state.order_repo.clone();
        let handle = state.handle.clone();
        tokio::spawn(async move {
            run_sync(&handle, &order_repo, &client, SyncKind::ActivePoll).await;
        });
    }

    let _ = state.handle.emit(
        "orders-updated",
        &serde_json::json!({"source": "extension", "orderNumber": order_number}),
    );

    (StatusCode::OK, ResponseJson(ApiResponse::ok()))
}

async fn health_check() -> ResponseJson<serde_json::Value> {
    ResponseJson(serde_json::json!({"status": "ok"}))
}

async fn start_http_server(state: HttpAppState) {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
            origin.to_str().map(is_extension_origin).unwrap_or(false)
        }))
        .allow_methods([axum::http::Method::POST, axum::http::Method::GET])
        .allow_headers([header::CONTENT_TYPE]);

    let app = Router::new()
        .route("/api/health", get(health_check))
        .route("/api/payment-detail", post(handle_payment_detail))
        .layer(DefaultBodyLimit::max(HTTP_BODY_LIMIT))
        .layer(cors)
        .with_state(state);

    let listener = match tokio::net::TcpListener::bind(HTTP_ADDR).await {
        Ok(listener) => listener,
        Err(e) => {
            // Trước đây dùng `expect`, làm panic task và app chạy tiếp mà không có
            // cách nào biết cầu nối với extension đã chết.
            error!(error = %e, addr = %HTTP_ADDR, "không mở được cổng cho cầu nối extension");
            return;
        }
    };

    info!(addr = %HTTP_ADDR, "cầu nối extension đang lắng nghe");
    if let Err(e) = axum::serve(listener, app).await {
        error!(error = %e, "cầu nối extension dừng");
    }
}

// ───────────────────────────── Scheduler ─────────────────────────────

const INCREMENTAL_INTERVAL: Duration = Duration::from_secs(60);
const ACTIVE_POLL_INTERVAL: Duration = Duration::from_secs(15);
const CLEANUP_INTERVAL: Duration = Duration::from_secs(300);
const IDLE_RETRY: Duration = Duration::from_secs(5);

async fn start_scheduler(
    handle: tauri::AppHandle,
    order_repo: Arc<OrderRepo>,
    payment_repo: Arc<PaymentRepo>,
    api_client: Arc<RwLock<Option<C2CApiClient>>>,
) {
    use tokio::time::{interval, MissedTickBehavior};

    let mut incremental = interval(INCREMENTAL_INTERVAL);
    let mut active_poll = interval(ACTIVE_POLL_INTERVAL);
    let mut cleanup = interval(CLEANUP_INTERVAL);
    for timer in [&mut incremental, &mut active_poll, &mut cleanup] {
        timer.set_missed_tick_behavior(MissedTickBehavior::Delay);
    }

    loop {
        tokio::select! {
            _ = incremental.tick() => {
                if let Some(client) = api_client.read().await.clone() {
                    run_sync(&handle, &order_repo, &client, SyncKind::Incremental).await;
                } else {
                    tokio::time::sleep(IDLE_RETRY).await;
                }
            }
            _ = active_poll.tick() => {
                // Luôn poll 24h khi đã có credentials — bắt lệnh mới + đổi trạng thái
                // (đã thanh toán) mà không phụ thuộc nút thủ công.
                if let Some(client) = api_client.read().await.clone() {
                    run_sync(&handle, &order_repo, &client, SyncKind::ActivePoll).await;
                }
            }
            _ = cleanup.tick() => {
                match payment_repo.purge_expired(now_ms()).await {
                    Ok(removed) if removed > 0 => info!(removed, "đã xoá payment detail hết hạn"),
                    Ok(_) => {}
                    // Bản trước bỏ qua lỗi bằng `if let Ok(_) = ...`, nên query sai
                    // tên cột chạy suốt nhiều tháng mà không ai biết.
                    Err(e) => error!(error = %e, "không dọn được payment detail"),
                }
            }
        }
    }
}

enum SyncKind {
    Incremental,
    ActivePoll,
}

async fn run_sync(
    handle: &tauri::AppHandle,
    order_repo: &Arc<OrderRepo>,
    client: &C2CApiClient,
    kind: SyncKind,
) {
    let engine = SyncEngine::new(client, order_repo);
    let (source, result) = match kind {
        SyncKind::Incremental => ("incremental", engine.incremental_sync().await),
        SyncKind::ActivePoll => ("poll", engine.active_poll().await),
    };

    match result {
        Ok(changed) => {
            // Bắn cả khi changed=0 để UI biết sync còn sống; đọc SQLite local rất nhẹ.
            let _ = handle.emit(
                "orders-updated",
                &serde_json::json!({"source": source, "changed": changed}),
            );
        }
        Err(e) => error!(error = %e, source, "đồng bộ thất bại"),
    }
}

// ───────────────────────────── main ─────────────────────────────

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("P2PQR_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let app_data_dir = dirs::data_local_dir()
        .expect("không xác định được thư mục dữ liệu ứng dụng")
        .join("BinanceP2PManager");
    std::fs::create_dir_all(&app_data_dir).expect("không tạo được thư mục dữ liệu ứng dụng");

    let db_path = app_data_dir.join("p2p_app.db");
    // Không log đường dẫn đầy đủ: nó chứa tên người dùng Windows.
    info!("đang mở cơ sở dữ liệu trong thư mục dữ liệu ứng dụng");

    let db = Db::init(db_path.to_string_lossy().as_ref())
        .await
        .expect("khởi tạo cơ sở dữ liệu thất bại");

    let creds_repo = Arc::new(CredentialsRepo::new(db.pool().clone()));
    let order_repo = Arc::new(OrderRepo::new(
        db.pool().clone(),
        Arc::new(StageMap::default()),
    ));
    let payment_repo = Arc::new(PaymentRepo::new(db.pool().clone()));

    let api_client = Arc::new(RwLock::new(match creds_repo.load().await {
        Ok(Some((key, secret))) => Some(C2CApiClient::new(key, secret)),
        Ok(None) => None,
        Err(e) => {
            warn!(error = %e, "không đọc được credentials đã lưu");
            None
        }
    }));

    let app_ctx = AppCtx {
        order_repo: order_repo.clone(),
        payment_repo: payment_repo.clone(),
        creds_repo: creds_repo.clone(),
        api_client: api_client.clone(),
    };

    tauri::Builder::default()
        .setup(move |app| {
            let handle = app.handle().clone();
            bot::init_logging(&handle);

            tauri::async_runtime::spawn(start_scheduler(
                handle.clone(),
                order_repo.clone(),
                payment_repo.clone(),
                api_client.clone(),
            ));

            tauri::async_runtime::spawn(start_http_server(HttpAppState {
                order_repo: order_repo.clone(),
                payment_repo: payment_repo.clone(),
                creds_repo: creds_repo.clone(),
                api_client: api_client.clone(),
                handle,
            }));

            Ok(())
        })
        .manage(app_ctx)
        .manage(bot::BotRuntime::default())
        .invoke_handler(tauri::generate_handler![
            store_api_credentials,
            update_payer_bank_name,
            check_api_credentials,
            get_credential_info,
            clear_api_credentials,
            test_api_credentials,
            force_initial_sync,
            force_sync_recent,
            list_orders_from_db,
            get_db_stats,
            get_order_payment_detail,
            save_payment_detail,
            cleanup_old_payment_details,
            clear_all_data,
            bot::get_bot_config,
            bot::save_bot_config,
            bot::get_bot_status,
            bot::start_bot,
            bot::stop_bot
        ])
        .run(tauri::generate_context!())
        .expect("lỗi khi chạy ứng dụng Tauri");
}
