use anyhow::{anyhow, Result};
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde_json::Value;
use sha2::Sha256;
use std::fmt;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

type HmacSha256 = Hmac<Sha256>;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Phân loại lỗi để phía gọi biết có nên thử lại hay không.
///
/// Bản trước gói mọi lỗi vào một `anyhow::Error` rồi retry 3 lần cho tất cả. Sai
/// chữ ký hay sai API key thì thử lại bao nhiêu lần cũng vẫn sai, chỉ tốn thêm
/// 3 giây và 3 lần gọi API cho mỗi trang.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiFailure {
    /// Lỗi mạng hoặc timeout — thử lại được.
    Network,
    /// Bị giới hạn tần suất — thử lại được, nhưng phải chờ lâu hơn.
    RateLimited,
    /// Lệch giờ so với server — thử lại được sau khi đồng bộ lại thời gian.
    ClockSkew,
    /// Sai API key hoặc chữ ký — thử lại vô nghĩa.
    Auth,
    /// Chưa phân loại được — không thử lại để tránh vòng lặp im lặng.
    Other,
}

impl ApiFailure {
    pub fn is_retryable(self) -> bool {
        matches!(self, Self::Network | Self::RateLimited | Self::ClockSkew)
    }
}

#[derive(Debug)]
pub struct ApiError {
    pub kind: ApiFailure,
    pub message: String,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for ApiError {}

impl ApiError {
    fn new(kind: ApiFailure, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

#[derive(Clone)]
pub struct C2CApiClient {
    api_key: String,
    api_secret: String,
    http: Client,
    base: String,
    /// Chênh lệch giữa giờ server Binance và giờ máy, tính bằng millisecond.
    time_offset: Arc<AtomicI64>,
}

impl C2CApiClient {
    pub fn new(api_key: String, api_secret: String) -> Self {
        // Không có timeout thì một kết nối treo sẽ giữ scheduler đứng vô thời hạn.
        let http = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            api_key,
            api_secret,
            http,
            base: "https://api.binance.com".into(),
            time_offset: Arc::new(AtomicI64::new(0)),
        }
    }

    /// Đo chênh lệch giờ với server Binance.
    pub async fn sync_time(&self) -> Result<()> {
        let local_time = Utc::now().timestamp_millis();
        let url = format!("{}/api/v3/time", self.base);

        let res = self.http.get(&url).send().await?;
        let json: Value = res.json().await?;
        let server_time = json["serverTime"]
            .as_i64()
            .ok_or_else(|| anyhow!("không đọc được serverTime từ phản hồi"))?;

        let offset = server_time - local_time;
        self.time_offset.store(offset, Ordering::Relaxed);
        info!(offset_ms = offset, "đã đồng bộ giờ với Binance");
        Ok(())
    }

    fn get_timestamp(&self) -> i64 {
        Utc::now().timestamp_millis() + self.time_offset.load(Ordering::Relaxed)
    }

    pub async fn list_user_order_history(
        &self,
        trade_type: &str,
        start_ts: i64,
        end_ts: i64,
        page: u32,
        rows: u32,
    ) -> Result<Value, ApiError> {
        let timestamp = self.get_timestamp();
        let recv_window = 60_000;
        let query = format!(
            "tradeType={trade_type}&startTimestamp={start_ts}&endTimestamp={end_ts}&page={page}&rows={rows}&timestamp={timestamp}&recvWindow={recv_window}"
        );
        let signature = self
            .sign(&query)
            .map_err(|e| ApiError::new(ApiFailure::Auth, format!("không ký được request: {e}")))?;

        let url = format!(
            "{}/sapi/v1/c2c/orderMatch/listUserOrderHistory?{}&signature={}",
            self.base, query, signature
        );

        let res = self
            .http
            .get(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await
            .map_err(|e| {
                let kind = if e.is_timeout() || e.is_connect() || e.is_request() {
                    ApiFailure::Network
                } else {
                    ApiFailure::Other
                };
                ApiError::new(kind, e.to_string())
            })?;

        let status = res.status();
        // 429 = vượt giới hạn tần suất, 418 = đã bị ban tạm thời vì tiếp tục gọi sau 429.
        if status.as_u16() == 429 || status.as_u16() == 418 {
            return Err(ApiError::new(
                ApiFailure::RateLimited,
                format!("Binance giới hạn tần suất (HTTP {status})"),
            ));
        }
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(ApiError::new(
                ApiFailure::Auth,
                format!("Binance từ chối xác thực (HTTP {status})"),
            ));
        }

        let text = res
            .text()
            .await
            .map_err(|e| ApiError::new(ApiFailure::Network, e.to_string()))?;

        let json: Value = serde_json::from_str(&text).map_err(|e| {
            // Không đưa nguyên body vào log: nó chứa dữ liệu lệnh của người dùng.
            debug!(body_len = text.len(), "phản hồi không phải JSON hợp lệ");
            ApiError::new(ApiFailure::Other, format!("phản hồi không phải JSON: {e}"))
        })?;

        if json.get("code").and_then(|x| x.as_str()) == Some("000000") {
            return Ok(json);
        }

        Err(Self::classify_error(&json))
    }

    /// Suy ra loại lỗi từ mã lỗi Binance trả về.
    fn classify_error(json: &Value) -> ApiError {
        let code = json
            .get("code")
            .map(|c| match c {
                Value::Number(n) => n.to_string(),
                Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default();

        let msg = json
            .get("msg")
            .or_else(|| json.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("không rõ nguyên nhân")
            .to_string();

        let kind = match code.as_str() {
            // Sai chữ ký / API key không hợp lệ / thiếu quyền.
            "-1022" | "-2014" | "-2015" | "-1099" => ApiFailure::Auth,
            // timestamp nằm ngoài recvWindow.
            "-1021" => ApiFailure::ClockSkew,
            // Vượt giới hạn tần suất.
            "-1003" | "-1015" => ApiFailure::RateLimited,
            // Lỗi nội bộ phía Binance, thử lại được.
            "-1000" | "-1001" => ApiFailure::Network,
            _ => ApiFailure::Other,
        };

        if kind == ApiFailure::Auth {
            warn!(code = %code, "Binance từ chối xác thực");
        }

        ApiError::new(kind, format!("Binance trả về mã {code}: {msg}"))
    }

    fn sign(&self, query: &str) -> Result<String> {
        let mut mac = HmacSha256::new_from_slice(self.api_secret.as_bytes())?;
        mac.update(query.as_bytes());
        Ok(hex::encode(mac.finalize().into_bytes()))
    }
}
