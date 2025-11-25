use anyhow::Result;
use sqlx::{SqlitePool, Row};
use chrono::Utc;
use crate::crypto::CryptoCtx;

pub struct CredentialsRepo {
    pool: SqlitePool,
    crypto: CryptoCtx
}

impl CredentialsRepo {
    pub fn new(pool: SqlitePool, crypto: CryptoCtx) -> Self { Self { pool, crypto } }

    pub async fn store(&self, label: &str, api_key: &str, api_secret: &str) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        let api_key_enc = self.crypto.encrypt(api_key.as_bytes())?;
        let api_secret_enc = self.crypto.encrypt(api_secret.as_bytes())?;
        
        // Delete existing credentials with the same label to allow updating
        sqlx::query(r#"DELETE FROM api_credentials WHERE label = ?1"#)
            .bind(label)
            .execute(&self.pool).await?;
        
        // Insert new credentials
        sqlx::query(r#"INSERT INTO api_credentials(label, api_key_enc, api_secret_enc, created_at, last_used_at) VALUES (?1, ?2, ?3, ?4, ?4)"#)
            .bind(label)
            .bind(api_key_enc)
            .bind(api_secret_enc)
            .bind(now)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn latest(&self) -> Result<Option<(String,String)>> {
        if let Some(row) = sqlx::query(r#"SELECT api_key_enc, api_secret_enc FROM api_credentials ORDER BY id DESC LIMIT 1"#)
            .fetch_optional(&self.pool).await? {
            let key_enc: Vec<u8> = row.get("api_key_enc");
            let sec_enc: Vec<u8> = row.get("api_secret_enc");
            let key = String::from_utf8(self.crypto.decrypt(&key_enc)?)?;
            let secret = String::from_utf8(self.crypto.decrypt(&sec_enc)?)?;
            Ok(Some((key, secret)))
        } else { Ok(None) }
    }
}
