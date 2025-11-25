use anyhow::{Result, anyhow};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use reqwest::Client;
use chrono::Utc;
use serde_json::Value;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub struct C2CApiClient {
    api_key: String,
    api_secret: String,
    http: Client,
    base: String,
    time_offset: std::sync::Arc<std::sync::Mutex<i64>>, // Offset in milliseconds
}

impl C2CApiClient {
    pub fn new(api_key: String, api_secret: String) -> Self {
        Self { 
            api_key, 
            api_secret, 
            http: Client::new(), 
            base: "https://api.binance.com".into(),
            time_offset: std::sync::Arc::new(std::sync::Mutex::new(0)),
        }
    }

    /// Sync time with Binance server to calculate offset
    pub async fn sync_time(&self) -> Result<()> {
        println!("[API] Syncing time with Binance server...");
        
        let local_time = Utc::now().timestamp_millis();
        
        // Get Binance server time
        let url = format!("{}/api/v3/time", self.base);
        let res = self.http.get(&url).send().await?;
        let json: Value = res.json().await?;
        
        let server_time = json["serverTime"]
            .as_i64()
            .ok_or_else(|| anyhow!("Failed to parse server time"))?;
        
        // Calculate offset: server_time - local_time
        let offset = server_time - local_time;
        
        *self.time_offset.lock().unwrap() = offset;
        
        println!("[API] Time synced! Offset: {}ms ({}s)", offset, offset as f64 / 1000.0);
        
        Ok(())
    }

    /// Get current timestamp adjusted with server offset
    fn get_timestamp(&self) -> i64 {
        let local_time = Utc::now().timestamp_millis();
        let offset = *self.time_offset.lock().unwrap();
        local_time + offset
    }

    pub async fn list_user_order_history(&self, trade_type: &str, start_ts: i64, end_ts: i64, page: u32, rows: u32) -> Result<Value> {
        let timestamp = self.get_timestamp(); // Use adjusted timestamp
        let recv_window = 60000; // 60s skew tolerance (increased from 5s)
        let query = format!(
            "tradeType={}&startTimestamp={}&endTimestamp={}&page={}&rows={}&timestamp={}&recvWindow={}",
            trade_type, start_ts, end_ts, page, rows, timestamp, recv_window
        );
        let signature = self.sign(&query)?;
        let url = format!("{}/sapi/v1/c2c/orderMatch/listUserOrderHistory?{}&signature={}", self.base, query, signature);
        let res = self.http.get(&url).header("X-MBX-APIKEY", &self.api_key).send().await?;
        let text = res.text().await?;
        let json: Value = serde_json::from_str(&text).map_err(|e| anyhow!("JSON parse error: {e} body={text}"))?;
        if json.get("code").and_then(|x| x.as_str()) == Some("000000") { Ok(json) } else { Err(anyhow!("API error: {text}")) }
    }

    fn sign(&self, query: &str) -> Result<String> {
        let mut mac = HmacSha256::new_from_slice(self.api_secret.as_bytes())?;
        mac.update(query.as_bytes());
        Ok(hex::encode(mac.finalize().into_bytes()))
    }
}
