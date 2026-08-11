//! Lưu API key/secret của Binance trong kho khoá của hệ điều hành.
//!
//! Bản trước ghi vào SQLite qua một "CryptoCtx" chỉ làm base64 — nghĩa là ai đọc
//! được file `p2p_app.db` là đọc được secret, và secret cho phép rút tiền nếu API
//! key được bật quyền đó. Ở đây dùng Windows Credential Manager / macOS Keychain /
//! Secret Service, còn SQLite chỉ giữ nhãn và thời điểm tạo để hiển thị.

use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use tracing::{info, warn};

const KEYRING_SERVICE: &str = "p2p-qr.binance-api";
const KEYRING_ACCOUNT: &str = "default";

#[derive(Debug, Serialize, Deserialize)]
struct StoredCredentials {
    label: String,
    api_key: String,
    api_secret: String,
    created_at: i64,
    /// Tên chủ tài khoản ngân hàng gắn với Binance (người chuyển tiền khi BUY).
    #[serde(default)]
    payer_bank_name: Option<String>,
}

/// Thông tin an toàn để trả về UI — không bao giờ chứa secret.
#[derive(Debug, Serialize)]
pub struct CredentialInfo {
    pub label: String,
    /// API key đã che, ví dụ `abcd…wxyz`.
    pub api_key_masked: String,
    pub created_at: i64,
    pub payer_bank_name: Option<String>,
}

pub struct CredentialsRepo {
    /// Chỉ còn dùng để di chuyển dữ liệu cũ ra khỏi DB và dọn bảng.
    pool: SqlitePool,
}

impl CredentialsRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn entry() -> Result<keyring::Entry> {
        keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
            .map_err(|e| anyhow!("không mở được kho khoá hệ thống: {e}"))
    }

    pub async fn store(
        &self,
        label: &str,
        api_key: &str,
        api_secret: &str,
        payer_bank_name: Option<&str>,
    ) -> Result<()> {
        let existing = self.read_record().await?;
        let payer = normalize_payer_name(
            payer_bank_name
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .or_else(|| existing.as_ref().and_then(|r| r.payer_bank_name.clone())),
        );

        let record = StoredCredentials {
            label: label.to_string(),
            api_key: api_key.to_string(),
            api_secret: api_secret.to_string(),
            created_at: Utc::now().timestamp_millis(),
            payer_bank_name: payer,
        };

        Self::entry()?
            .set_password(&serde_json::to_string(&record)?)
            .map_err(|e| anyhow!("không ghi được vào kho khoá hệ thống: {e}"))?;

        // Nếu còn bản base64 cũ trong DB thì xoá đi, không giữ hai nguồn.
        self.purge_legacy_rows().await?;
        Ok(())
    }

    /// Cập nhật nội dung CK cấu hình mà không đụng API key/secret.
    pub async fn update_payer_bank_name(&self, payer_bank_name: &str) -> Result<()> {
        let Some(mut record) = self.read_record().await? else {
            return Err(anyhow!("chưa có credentials để cập nhật nội dung CK"));
        };
        record.payer_bank_name = normalize_payer_name(Some(payer_bank_name.trim().to_string()));
        Self::entry()?
            .set_password(&serde_json::to_string(&record)?)
            .map_err(|e| anyhow!("không ghi được vào kho khoá hệ thống: {e}"))?;
        Ok(())
    }

    /// Đọc credentials để gọi API. Chỉ dùng trong backend.
    pub async fn load(&self) -> Result<Option<(String, String)>> {
        if let Some(record) = self.read_record().await? {
            return Ok(Some((record.api_key, record.api_secret)));
        }
        Ok(None)
    }

    /// API key hiện tại (để phát hiện đổi tài khoản).
    pub async fn current_api_key(&self) -> Result<Option<String>> {
        Ok(self.read_record().await?.map(|r| r.api_key))
    }

    pub async fn payer_bank_name(&self) -> Result<Option<String>> {
        Ok(self
            .read_record()
            .await?
            .and_then(|r| r.payer_bank_name)
            .filter(|s| !s.is_empty()))
    }

    /// Thông tin hiển thị cho UI.
    pub async fn info(&self) -> Result<Option<CredentialInfo>> {
        Ok(self.read_record().await?.map(|r| CredentialInfo {
            label: r.label,
            api_key_masked: mask(&r.api_key),
            created_at: r.created_at,
            payer_bank_name: r.payer_bank_name,
        }))
    }

    pub async fn clear(&self) -> Result<()> {
        match Self::entry()?.delete_credential() {
            Ok(()) => {}
            Err(keyring::Error::NoEntry) => {}
            Err(e) => return Err(anyhow!("không xoá được khoá trong kho hệ thống: {e}")),
        }
        self.purge_legacy_rows().await?;
        Ok(())
    }

    async fn read_record(&self) -> Result<Option<StoredCredentials>> {
        match Self::entry()?.get_password() {
            Ok(raw) => Ok(Some(serde_json::from_str(&raw)?)),
            Err(keyring::Error::NoEntry) => self.migrate_legacy().await,
            Err(e) => Err(anyhow!("không đọc được kho khoá hệ thống: {e}")),
        }
    }

    /// Chuyển credentials base64 còn sót trong SQLite sang kho khoá, rồi xoá khỏi DB.
    ///
    /// Chạy một lần cho người dùng đã cài bản cũ, để họ không phải nhập lại API key.
    async fn migrate_legacy(&self) -> Result<Option<StoredCredentials>> {
        let row = sqlx::query(
            "SELECT label, api_key_enc, api_secret_enc, created_at
             FROM api_credentials ORDER BY id DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else { return Ok(None) };

        let decode = |raw: Vec<u8>| -> Option<String> {
            let text = String::from_utf8(raw).ok()?;
            let bytes = STANDARD.decode(text.trim()).ok()?;
            String::from_utf8(bytes).ok()
        };

        let key: Option<Vec<u8>> = row.get("api_key_enc");
        let secret: Option<Vec<u8>> = row.get("api_secret_enc");
        let (Some(api_key), Some(api_secret)) = (key.and_then(decode), secret.and_then(decode))
        else {
            warn!("bỏ qua credentials cũ trong DB vì không giải mã được");
            return Ok(None);
        };

        let record = StoredCredentials {
            label: row
                .get::<Option<String>, _>("label")
                .unwrap_or_else(|| "binance".into()),
            api_key,
            api_secret,
            created_at: row
                .get::<Option<i64>, _>("created_at")
                .unwrap_or_else(|| Utc::now().timestamp_millis()),
            payer_bank_name: None,
        };

        Self::entry()?
            .set_password(&serde_json::to_string(&record)?)
            .map_err(|e| anyhow!("không ghi được vào kho khoá hệ thống: {e}"))?;
        self.purge_legacy_rows().await?;
        info!("đã chuyển credentials từ SQLite sang kho khoá hệ thống");

        Ok(Some(record))
    }

    async fn purge_legacy_rows(&self) -> Result<()> {
        sqlx::query("DELETE FROM api_credentials")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

fn normalize_payer_name(value: Option<String>) -> Option<String> {
    value
        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|s| !s.is_empty())
}

/// Giữ 4 ký tự đầu và 4 ký tự cuối, phần giữa thay bằng dấu ba chấm.
///
/// Giá trị quá ngắn thì che hết bằng một chuỗi có độ dài cố định, để chính độ dài
/// cũng không bị tiết lộ.
fn mask(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 8 {
        return "………".to_string();
    }
    let head: String = chars[..4].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}…{tail}")
}

#[cfg(test)]
mod tests {
    use super::mask;

    #[test]
    fn mask_keeps_only_the_edges() {
        assert_eq!(mask("abcdefghijklmnop"), "abcd…mnop");
    }

    #[test]
    fn short_values_are_fully_masked_without_leaking_length() {
        assert_eq!(mask("short"), "………");
        assert_eq!(mask("abcdefgh"), "………");
        assert_eq!(mask(""), "………");
    }
}
