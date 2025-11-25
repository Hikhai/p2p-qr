use anyhow::Result;
use chrono::Utc;
use crate::api::c2c_api_client::C2CApiClient;
use crate::orders::repo::OrderRepo;
use sqlx::Row;
use serde_json::Value;

pub struct SyncEngine<'a> {
    pub client: &'a C2CApiClient,
    pub repo: &'a OrderRepo
}

impl<'a> SyncEngine<'a> {
    pub fn new(client: &'a C2CApiClient, repo: &'a OrderRepo) -> Self { Self { client, repo } }

    /// Fetch with retry and exponential backoff
    async fn fetch_with_retry(
        &self,
        trade_type: &str,
        start: i64,
        end: i64,
        page: u32,
        rows: u32,
        max_retries: u32,
    ) -> Result<Value> {
        let mut retries = 0;
        let mut delay = 1000; // 1s

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
                }
            }
        }
    }

    /// Extract data array from API response
    fn extract_data_array(res: &Value) -> Vec<Value> {
        res.get("data")
            .and_then(|d| {
                if let Some(nested) = d.get("data").and_then(|x| x.as_array()).cloned() {
                    Some(nested)
                } else {
                    d.as_array().cloned()
                }
            })
            .unwrap_or_default()
    }

    /// Extract total count from API response
    fn extract_total(res: &Value) -> Option<i64> {
        res.get("data")
            .and_then(|d| d.get("total").and_then(|x| x.as_i64()))
            .or_else(|| res.get("total").and_then(|x| x.as_i64()))
    }

    /// Sync a specific time chunk
    async fn sync_chunk(&self, trade_type: &str, start: i64, end: i64) -> Result<()> {
        let rows: u32 = 20; // Smaller page size for stability
        let mut page = 1;
        let mut fetched = 0i64;

        loop {
            let res = self.fetch_with_retry(trade_type, start, end, page, rows, 3).await?;

            let data_vec = Self::extract_data_array(&res);
            let total_opt = Self::extract_total(&res);

            if data_vec.is_empty() {
                println!("[SYNC] No more data at page {}", page);
                break;
            }

            for order in &data_vec {
                self.repo.upsert_from_api(order, end).await?;
            }

            let page_size = data_vec.len() as i64;
            fetched += page_size;

            println!("[SYNC] {} page {}: fetched {} items (total: {})", 
                trade_type, page, page_size, 
                total_opt.map(|t| t.to_string()).unwrap_or_else(|| "unknown".to_string())
            );

            // Stop conditions
            if let Some(total) = total_opt {
                if fetched >= total {
                    println!("[SYNC] Reached total count: {}/{}", fetched, total);
                    break;
                }
            } else if page_size < (rows as i64) {
                println!("[SYNC] Last page detected (partial page: {})", page_size);
                break;
            }

            page += 1;

            // Rate limit protection: 100ms between requests
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        println!("[SYNC] Completed {} chunk: {} orders total", trade_type, fetched);
        Ok(())
    }

    /// Force initial sync with chunked approach
    pub async fn force_initial_sync(&self, days: i64) -> Result<()> {
        let chunk_days = 7; // Sync 7 days at a time for stability
        let now = Utc::now().timestamp_millis();

        // ✅ Clear ALL old data first before syncing fresh data
        // This ensures we only keep exactly what user requested (X days)
        println!("[SYNC] Clearing all old orders before fresh sync...");
        sqlx::query("DELETE FROM orders")
            .execute(self.repo.pool())
            .await?;
        println!("[SYNC] Old orders cleared. Starting fresh sync for {} days...", days);

        for trade in ["BUY", "SELL"] {
            println!("[SYNC] Starting force_initial_sync for {} ({} days)", trade, days);
            let mut current_days = 0;

            while current_days < days {
                let end_time = now - (current_days * 24 * 60 * 60 * 1000);
                let chunk_end = (current_days + chunk_days).min(days);
                let start_time = now - (chunk_end * 24 * 60 * 60 * 1000);

                println!("[SYNC] Syncing {} from {} to {} days ago", trade, current_days, chunk_end);

                self.sync_chunk(trade, start_time, end_time).await?;

                current_days = chunk_end;
            }

            // ✅ IMPORTANT: Set sync window to "now" after force sync
            // This prevents incremental_sync from re-syncing the same data
            let oldest_time = now - (days * 24 * 60 * 60 * 1000);
            self.set_sync_window(trade, oldest_time, now, now).await?;

            println!("[SYNC] Completed force_initial_sync for {}", trade);
        }

        Ok(())
    }

    /// Incremental sync: fetch orders newer than last sync timestamp
    pub async fn incremental_sync(&self) -> Result<()> {
        let now = Utc::now().timestamp_millis();

        for trade in ["BUY", "SELL"] {
            // Get last sync time, default to 7 days ago
            let last = self.get_sync_window(trade).await?
                .unwrap_or(now - 7 * 24 * 60 * 60 * 1000);

            println!("[INCREMENTAL] Syncing {} from {} to {}", trade, last, now);

            self.sync_chunk(trade, last, now).await?;

            // CRITICAL: Always update to "now" after successful sync
            // This ensures we don't re-sync the same window next time
            self.set_sync_window(trade, last, now, now).await?;
        }

        Ok(())
    }

    /// Active poll: refresh orders in last 24 hours (catches in-progress and recent status changes)
    pub async fn active_poll(&self) -> Result<()> {
        let now = Utc::now().timestamp_millis();

        for trade in ["BUY", "SELL"] {
            // Strategy: Only query last 24 hours for active orders
            // This avoids querying too much old data
            let start = now - 24 * 60 * 60 * 1000;

            println!("[ACTIVE_POLL] Checking {} orders in last 24h", trade);

            self.sync_chunk(trade, start, now).await?;
        }

        Ok(())
    }

    async fn get_sync_window(&self, trade: &str) -> Result<Option<i64>> {
        let row = sqlx::query("SELECT last_end_timestamp FROM sync_state WHERE id = ?1")
            .bind(format!("{}_WIN", trade))
            .fetch_optional(self.repo.pool()).await?;
        Ok(row.and_then(|r| r.get::<Option<i64>,_>("last_end_timestamp")))
    }

    async fn set_sync_window(&self, trade: &str, start: i64, end: i64, newest: i64) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        sqlx::query(r#"INSERT INTO sync_state(id, last_start_timestamp, last_end_timestamp, last_complete_ts) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(id) DO UPDATE SET last_start_timestamp=excluded.last_start_timestamp, last_end_timestamp=excluded.last_end_timestamp, last_complete_ts=excluded.last_complete_ts"#)
            .bind(format!("{}_WIN", trade)).bind(start).bind(newest.max(end)).bind(now)
            .execute(self.repo.pool()).await?;
        Ok(())
    }
}
