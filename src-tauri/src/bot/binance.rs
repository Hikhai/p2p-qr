use anyhow::{anyhow, Result};
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde_json::Value;
use sha2::Sha256;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

type HmacSha256 = Hmac<Sha256>;

const BASE_URL: &str = "https://api.binance.com";

/// Client Binance dành cho bot SELL (chi tiết lệnh, chat credential, upload ảnh).
pub struct BotBinanceClient {
    api_key: String,
    api_secret: String,
    http: Client,
    time_offset_ms: AtomicI64,
}

impl BotBinanceClient {
    pub fn new(api_key: String, api_secret: String) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(20))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            api_key,
            api_secret,
            http,
            time_offset_ms: AtomicI64::new(0),
        }
    }

    pub async fn sync_time(&self) -> Result<()> {
        let local = Utc::now().timestamp_millis();
        let res: Value = self
            .http
            .get(format!("{BASE_URL}/api/v3/time"))
            .send()
            .await?
            .json()
            .await?;
        let server = res["serverTime"]
            .as_i64()
            .ok_or_else(|| anyhow!("Không đọc được serverTime"))?;
        self.time_offset_ms.store(server - local, Ordering::Relaxed);
        Ok(())
    }

    fn now_ms(&self) -> i64 {
        Utc::now().timestamp_millis() + self.time_offset_ms.load(Ordering::Relaxed)
    }

    fn sign(&self, query: &str) -> Result<String> {
        let mut mac = HmacSha256::new_from_slice(self.api_secret.as_bytes())?;
        mac.update(query.as_bytes());
        Ok(hex::encode(mac.finalize().into_bytes()))
    }

    fn is_api_ok(json: &Value) -> bool {
        match json.get("code") {
            None => true,
            Some(c) => {
                c.as_str() == Some("000000")
                    || c.as_i64() == Some(0)
                    || c.as_str() == Some("0")
            }
        }
    }

    fn parse_api_response(text: &str, http_status: reqwest::StatusCode) -> Result<Value> {
        let json: Value = serde_json::from_str(text)
            .map_err(|e| anyhow!("Lỗi parse JSON (HTTP {http_status}): {e} body={text}"))?;
        if Self::is_api_ok(&json) {
            Ok(json)
        } else {
            Err(anyhow!("Binance API trả lỗi (HTTP {http_status}): {text}"))
        }
    }

    async fn signed_request(
        &self,
        method: reqwest::Method,
        path: &str,
        query: &str,
    ) -> Result<Value> {
        let query = format!("{query}&timestamp={}&recvWindow=60000", self.now_ms());
        let query = query.trim_start_matches('&').to_string();
        let signature = self.sign(&query)?;
        let url = format!("{BASE_URL}{path}?{query}&signature={signature}");
        let res = self
            .http
            .request(method, &url)
            .header("X-MBX-APIKEY", &self.api_key)
            .header("clientType", "web")
            .header("User-Agent", "p2p-qr/1.0")
            .send()
            .await?;
        let status = res.status();
        let text = res.text().await?;
        Self::parse_api_response(&text, status)
    }

    async fn signed_post_json(&self, path: &str, body: &Value) -> Result<Value> {
        let query = format!("timestamp={}&recvWindow=60000", self.now_ms());
        let signature = self.sign(&query)?;
        let url = format!("{BASE_URL}{path}?{query}&signature={signature}");
        let res = self
            .http
            .post(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .header("clientType", "web")
            .header("User-Agent", "p2p-qr/1.0")
            .json(body)
            .send()
            .await?;
        let status = res.status();
        let text = res.text().await?;
        // Adaptive callers inspect the body themselves when not OK.
        serde_json::from_str(&text)
            .map_err(|e| anyhow!("Lỗi parse JSON (HTTP {status}): {e} body={text}"))
    }

    pub async fn get_order_detail(&self, order_no: &str) -> Result<Value> {
        // Binance nhận nhiều tên field — `adOrderNo` dễ lệch sang mã quảng cáo.
        let bodies = [
            serde_json::json!({ "orderNumber": order_no }),
            serde_json::json!({ "orderNo": order_no }),
            serde_json::json!({ "adOrderNo": order_no }),
        ];
        let url = format!("{BASE_URL}/sapi/v1/c2c/orderMatch/getUserOrderDetail");
        let mut last_err = None;

        for body in &bodies {
            let res = self
                .http
                .post(&url)
                .header("X-MBX-APIKEY", &self.api_key)
                .header("clientType", "web")
                .json(body)
                .send()
                .await;
            match res {
                Ok(res) => {
                    let status = res.status();
                    let text = res.text().await.unwrap_or_default();
                    let parsed: Option<Value> = serde_json::from_str(&text).ok();
                    if status.is_success() {
                        if let Some(v) = &parsed {
                            if Self::api_ok(v) {
                                return Ok(v["data"].clone());
                            }
                        }
                    }
                    match self
                        .signed_post_json("/sapi/v1/c2c/orderMatch/getUserOrderDetail", body)
                        .await
                    {
                        Ok(v) if Self::api_ok(&v) => return Ok(v["data"].clone()),
                        Ok(v) => last_err = Some(format!("{v}")),
                        Err(e) => last_err = Some(e.to_string()),
                    }
                }
                Err(e) => last_err = Some(e.to_string()),
            }
        }

        Err(anyhow!(
            "Không lấy được chi tiết lệnh {order_no}: {}",
            last_err.unwrap_or_else(|| "unknown".into())
        ))
    }

    /// `true` = seller vẫn còn nút mở khóa (lệnh chưa hoàn tất thật).
    /// Phản hồi mơ hồ → Err (không gửi tin 3).
    pub async fn can_release_coin(&self, order_no: &str) -> Result<bool> {
        let bodies = [
            serde_json::json!({ "orderNumber": order_no }),
            serde_json::json!({ "orderNo": order_no }),
        ];
        let mut last_err = None;
        for body in &bodies {
            match self
                .signed_post_json("/sapi/v1/c2c/orderMatch/checkIfCanReleaseCoin", body)
                .await
            {
                Ok(v) if Self::api_ok(&v) => match Self::parse_bool_flag(v.get("data")) {
                    Some(b) => return Ok(b),
                    None => last_err = Some(format!("data không rõ: {}", v["data"])),
                },
                Ok(v) => last_err = Some(format!("{v}")),
                Err(e) => last_err = Some(e.to_string()),
            }
        }
        Err(anyhow!(
            "checkIfCanReleaseCoin lỗi {order_no}: {}",
            last_err.unwrap_or_else(|| "unknown".into())
        ))
    }

    fn api_ok(v: &Value) -> bool {
        v.get("code")
            .map(|c| {
                c.as_str() == Some("000000")
                    || c.as_str() == Some("0")
                    || c.as_i64() == Some(0)
            })
            .unwrap_or(false)
            && !v.get("data").map(|d| d.is_null()).unwrap_or(true)
    }

    /// Chỉ nhận true/false rõ ràng — object rỗng / null → None (an toàn, hoãn tin 3).
    fn parse_bool_flag(data: Option<&Value>) -> Option<bool> {
        match data {
            Some(Value::Bool(b)) => Some(*b),
            Some(Value::String(s)) => match s.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" => Some(true),
                "false" | "0" | "no" => Some(false),
                _ => None,
            },
            Some(Value::Number(n)) => {
                if n.as_i64() == Some(1) || n.as_u64() == Some(1) {
                    Some(true)
                } else if n.as_i64() == Some(0) || n.as_u64() == Some(0) {
                    Some(false)
                } else {
                    None
                }
            }
            Some(obj) if obj.is_object() => obj
                .get("canRelease")
                .or_else(|| obj.get("canReleaseCoin"))
                .or_else(|| obj.get("allowRelease"))
                .and_then(|x| Self::parse_bool_flag(Some(x))),
            _ => None,
        }
    }

    pub async fn list_recent_sell_orders(&self, window_ms: i64) -> Result<Vec<Value>> {
        let now = self.now_ms();
        let query = format!(
            "tradeType=SELL&startTimestamp={}&endTimestamp={}&page=1&rows=20",
            now - window_ms,
            now
        );
        let res = self
            .signed_request(
                reqwest::Method::GET,
                "/sapi/v1/c2c/orderMatch/listUserOrderHistory",
                &query,
            )
            .await?;
        let list = res
            .get("data")
            .and_then(|d| {
                d.get("data")
                    .and_then(|x| x.as_array())
                    .cloned()
                    .or_else(|| d.as_array().cloned())
            })
            .unwrap_or_default();
        Ok(list)
    }

    pub async fn retrieve_chat_credential(&self) -> Result<(String, String, String)> {
        let res = self
            .signed_request(
                reqwest::Method::GET,
                "/sapi/v1/c2c/chat/retrieveChatCredential",
                "",
            )
            .await?;
        let data = res
            .get("data")
            .ok_or_else(|| anyhow!("Thiếu 'data' trong phản hồi credential: {res}"))?;
        let get = |keys: &[&str]| -> Option<String> {
            keys.iter()
                .find_map(|k| data.get(*k).and_then(|v| v.as_str()).map(String::from))
        };
        let wss_url =
            get(&["chatWssUrl", "wssUrl"]).ok_or_else(|| anyhow!("Thiếu chatWssUrl: {data}"))?;
        let listen_key =
            get(&["listenKey"]).ok_or_else(|| anyhow!("Thiếu listenKey: {data}"))?;
        let listen_token =
            get(&["listenToken"]).ok_or_else(|| anyhow!("Thiếu listenToken: {data}"))?;
        Ok((wss_url, listen_key, listen_token))
    }

    /// Xin pre-signed URL để upload ảnh chat: `(upload_url, image_url)`.
    ///
    /// Quan trọng: endpoint này ký **chỉ** `timestamp` (+ recvWindow) trên query,
    /// còn `imageName` gửi trong JSON body (không đưa vào chuỗi ký) — giống
    /// `getUserOrderDetail` / mẫu Python gửi biên lai P2P.
    pub async fn presign_chat_image(
        &self,
        image_name: &str,
        order_no: Option<&str>,
    ) -> Result<(String, String)> {
        let path = "/sapi/v1/c2c/chat/image/pre-signed-url";
        let mut errors = Vec::new();

        // 1) Form body + chỉ ký timestamp trên query (mẫu Python gửi biên lai)
        match self
            .presign_with_form_body(path, image_name, order_no)
            .await
        {
            Ok(pair) => return Ok(pair),
            Err(e) => errors.push(format!("form: {e}")),
        }

        // 2) JSON body + chỉ ký timestamp
        let mut bodies = vec![serde_json::json!({ "imageName": image_name })];
        if let Some(ono) = order_no.filter(|s| !s.is_empty()) {
            bodies.push(serde_json::json!({
                "imageName": image_name,
                "orderNo": ono,
            }));
            bodies.push(serde_json::json!({
                "imageName": image_name,
                "adOrderNo": ono,
            }));
        }

        for (i, body) in bodies.iter().enumerate() {
            match self.presign_with_json_body(path, body).await {
                Ok(pair) => return Ok(pair),
                Err(e) => errors.push(format!("json#{}: {e}", i + 1)),
            }
        }

        // 2) Fallback cũ: imageName nằm trong query đã ký
        let query = format!("imageName={}", urlencoding::encode(image_name));
        match self
            .signed_request(reqwest::Method::POST, path, &query)
            .await
            .and_then(|v| Self::extract_presign_urls(&v))
        {
            Ok(pair) => return Ok(pair),
            Err(e) => errors.push(format!("query-post: {e}")),
        }

        Err(anyhow!(
            "presign ảnh chat thất bại — {}",
            errors.join(" | ")
        ))
    }

    async fn signed_query_only(&self, path: &str) -> Result<String> {
        let query = format!("timestamp={}&recvWindow=60000", self.now_ms());
        let signature = self.sign(&query)?;
        Ok(format!("{BASE_URL}{path}?{query}&signature={signature}"))
    }

    async fn presign_with_form_body(
        &self,
        path: &str,
        image_name: &str,
        order_no: Option<&str>,
    ) -> Result<(String, String)> {
        let url = self.signed_query_only(path).await?;
        let mut form = format!("imageName={}", urlencoding::encode(image_name));
        if let Some(ono) = order_no.filter(|s| !s.is_empty()) {
            form.push_str(&format!("&orderNo={}", urlencoding::encode(ono)));
        }
        let res = self
            .http
            .post(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .header("clientType", "web")
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) p2p-qr/1.0",
            )
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(form)
            .send()
            .await?;
        let status = res.status();
        let text = res.text().await?;
        let json = Self::parse_api_response(&text, status)?;
        Self::extract_presign_urls(&json)
    }

    async fn presign_with_json_body(
        &self,
        path: &str,
        body: &Value,
    ) -> Result<(String, String)> {
        let url = self.signed_query_only(path).await?;
        let res = self
            .http
            .post(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .header("clientType", "web")
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) p2p-qr/1.0",
            )
            .json(body)
            .send()
            .await?;
        let status = res.status();
        let text = res.text().await?;
        let json = Self::parse_api_response(&text, status)?;
        Self::extract_presign_urls(&json)
    }

    fn extract_presign_urls(res: &Value) -> Result<(String, String)> {
        let data = res
            .get("data")
            .ok_or_else(|| anyhow!("Thiếu 'data' trong phản hồi pre-signed: {res}"))?;

        let get = |keys: &[&str]| -> Option<String> {
            keys.iter()
                .find_map(|k| data.get(*k).and_then(|v| v.as_str()).map(String::from))
        };

        let upload_url = get(&[
            "preSignedUrl",
            "presignedUrl",
            "uploadUrl",
            "s3Url",
            "putUrl",
        ])
        .or_else(|| {
            get(&["url"]).filter(|u| u.contains("X-Amz") || u.contains("Signature") || u.contains("s3"))
        })
        .ok_or_else(|| anyhow!("Thiếu URL upload trong phản hồi: {data}"))?;

        // Ưu tiên URL http(s) tuyệt đối — KHÔNG dùng filePath thô (ảnh sẽ không hiện trong chat).
        let image_url = ["imageUrl", "downloadUrl", "accessUrl", "fileUrl", "url"]
            .iter()
            .find_map(|k| {
                data.get(*k)
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
                    .filter(|u| *u != upload_url.as_str())
                    .map(str::to_string)
            })
            .or_else(|| {
                // Ghép CDN từ filePath / path tương đối.
                let path = get(&["filePath", "path", "imagePath", "key"])?;
                Some(resolve_binance_static_url(&path))
            })
            .ok_or_else(|| anyhow!("Thiếu URL ảnh công khai trong phản hồi: {data}"))?;

        Ok((upload_url, image_url))
    }

    pub async fn download_image(&self, url: &str) -> Result<Vec<u8>> {
        let res = self.http.get(url).send().await?;
        if !res.status().is_success() {
            return Err(anyhow!("Tải ảnh QR thất bại: HTTP {}", res.status()));
        }
        Ok(res.bytes().await?.to_vec())
    }

    pub async fn upload_image_with_type(
        &self,
        upload_url: &str,
        bytes: Vec<u8>,
        content_type: &str,
    ) -> Result<()> {
        // Thử đúng Content-Type; nếu S3 từ chối vì lệch signed headers → thử không gửi header.
        let attempts = [Some(content_type), Some("image/jpg"), None];
        let mut last_err = String::new();
        for ct in attempts {
            let mut req = self.http.put(upload_url).body(bytes.clone());
            if let Some(ct) = ct {
                req = req.header("Content-Type", ct);
            }
            let res = req.send().await?;
            if res.status().is_success() {
                return Ok(());
            }
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            last_err = format!("HTTP {status} ct={ct:?} {body}");
            if status.as_u16() != 403 && status.as_u16() != 400 {
                break;
            }
        }
        Err(anyhow!("Upload ảnh thất bại: {last_err}"))
    }

    /// Kiểm tra URL ảnh đã upload — bắt buộc đọc được bytes ảnh thật (JPEG/PNG).
    pub async fn verify_image_url(&self, image_url: &str) -> Result<()> {
        let res = self.http.get(image_url).send().await?;
        if !res.status().is_success() {
            return Err(anyhow!(
                "URL ảnh không đọc được sau upload: HTTP {} ({})",
                res.status(),
                summarize_url(image_url)
            ));
        }
        let ct = res
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("?")
            .to_string();
        let bytes = res.bytes().await?;
        let kind = if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            "jpeg"
        } else if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
            "png"
        } else {
            "other"
        };
        if kind == "other" || bytes.len() < 256 {
            return Err(anyhow!(
                "CDN chưa phục vụ ảnh thật (ct={ct}, magic={kind}, {} bytes) — {}",
                bytes.len(),
                summarize_url(image_url)
            ));
        }
        Ok(())
    }

    /// Lấy lịch sử chat lệnh — dùng để xác nhận ảnh đã vào hội thoại.
    pub async fn list_chat_messages(&self, order_no: &str) -> Result<Vec<Value>> {
        let query = format!(
            "orderNo={}&page=1&rows=50",
            urlencoding::encode(order_no)
        );
        let res = self
            .signed_request(
                reqwest::Method::GET,
                "/sapi/v1/c2c/chat/retrieveChatMessagesWithPagination",
                &query,
            )
            .await?;
        let list = res
            .get("data")
            .and_then(|d| {
                d.get("data")
                    .and_then(|x| x.as_array())
                    .cloned()
                    .or_else(|| d.as_array().cloned())
                    .or_else(|| d.get("list").and_then(|x| x.as_array()).cloned())
                    .or_else(|| d.get("messages").and_then(|x| x.as_array()).cloned())
            })
            .unwrap_or_default();
        Ok(list)
    }

    pub fn chat_has_image(messages: &[Value], image_url: &str) -> bool {
        let needle = image_url
            .rsplit('/')
            .next()
            .unwrap_or(image_url)
            .split('?')
            .next()
            .unwrap_or(image_url);
        let short = needle.get(..20).unwrap_or(needle);
        messages.iter().any(|m| {
            let ty_str = m.get("type").and_then(|x| x.as_str()).unwrap_or("");
            let ty_num = m.get("type").and_then(|x| x.as_i64());
            let img = m
                .get("imageUrl")
                .or_else(|| m.get("imgUrl"))
                .or_else(|| m.get("content"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            let looks_image = ty_str.eq_ignore_ascii_case("image")
                || ty_num == Some(2)
                || m.get("imageUrl").is_some()
                || img.contains("client_upload");
            looks_image && (img.contains(needle) || img.contains(short) || img == image_url)
        })
    }

    /// Có tin ảnh do mình gửi trong chat chưa (bất kỳ URL).
    pub fn chat_has_self_image(messages: &[Value]) -> bool {
        messages.iter().any(|m| {
            let is_self = m.get("self").and_then(|x| x.as_bool()).unwrap_or(false);
            let ty = m.get("type").and_then(|x| x.as_str()).unwrap_or("");
            let has_url = m
                .get("imageUrl")
                .or_else(|| m.get("thumbnailUrl"))
                .and_then(|x| x.as_str())
                .map(|s| s.contains("client_upload"))
                .unwrap_or(false);
            is_self && (ty.eq_ignore_ascii_case("image") || has_url)
        })
    }
}

fn summarize_url(url: &str) -> String {
    // Bỏ query (chứa chữ ký) — chỉ log host + path.
    let base = url.split('?').next().unwrap_or(url);
    let s: String = base.chars().take(120).collect();
    s
}

/// Ghép path tương đối Binance thành URL CDN công khai.
fn resolve_binance_static_url(path: &str) -> String {
    let path = path.trim();
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    // Các CDN hay gặp với ảnh chat / upload C2C.
    if path.contains("/client_upload/") || path.contains("/image/") {
        format!("https://api.binance.com{path}")
    } else {
        format!("https://public.bnbstatic.com{path}")
    }
}
