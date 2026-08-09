use anyhow::Result;
use chrono::Utc;
use serde_json::Value;
use sqlx::Row;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::api::c2c_api_client::{ApiError, C2CApiClient};
use crate::orders::repo::OrderRepo;

const PAGE_SIZE: u32 = 20;
const MAX_ATTEMPTS: u32 = 3;
/// Nghỉ giữa hai trang để không đụng giới hạn tần suất của Binance.
const PAGE_DELAY: Duration = Duration::from_millis(100);
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

pub struct SyncEngine<'a> {
    pub client: &'a C2CApiClient,
    pub repo: &'a OrderRepo,
}

impl<'a> SyncEngine<'a> {
    pub fn new(client: &'a C2CApiClient, repo: &'a OrderRepo) -> Self {
        Self { client, repo }
    }

    /// Gọi API, chỉ thử lại với những lỗi có khả năng tự khỏi.
    ///
    /// Bản trước thử lại mọi lỗi ba lần, kể cả sai chữ ký — vừa vô ích vừa làm mỗi
    /// trang mất thêm 3 giây khi credentials sai.
    async fn fetch_page(
        &self,
        trade_type: &str,
        start: i64,
        end: i64,
        page: u32,
    ) -> Result<Value, ApiError> {
        let mut delay = Duration::from_secs(1);

        for attempt in 1..=MAX_ATTEMPTS {
            match self
                .client
                .list_user_order_history(trade_type, start, end, page, PAGE_SIZE)
                .await
            {
                Ok(res) => return Ok(res),
                Err(err) => {
                    if !err.kind.is_retryable() || attempt == MAX_ATTEMPTS {
                        return Err(err);
                    }

                    // Lệch giờ thì đồng bộ lại đồng hồ trước khi thử lại, nếu không
                    // lần sau cũng sẽ lệch y như vậy.
                    if err.kind == crate::api::c2c_api_client::ApiFailure::ClockSkew {
                        if let Err(e) = self.client.sync_time().await {
                            warn!(error = %e, "không đồng bộ lại được giờ");
                        }
                    }

                    warn!(
                        attempt,
                        max = MAX_ATTEMPTS,
                        delay_ms = delay.as_millis(),
                        error = %err,
                        "thử lại lời gọi API"
                    );
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                }
            }
        }

        unreachable!("vòng lặp luôn trả về ở lần thử cuối")
    }

    fn extract_data_array(res: &Value) -> Vec<Value> {
        res.get("data")
            .and_then(|d| {
                d.get("data")
                    .and_then(|x| x.as_array())
                    .cloned()
                    .or_else(|| d.as_array().cloned())
            })
            .unwrap_or_default()
    }

    fn extract_total(res: &Value) -> Option<i64> {
        res.get("data")
            .and_then(|d| d.get("total").and_then(|x| x.as_i64()))
            .or_else(|| res.get("total").and_then(|x| x.as_i64()))
    }

    /// Đồng bộ một khoảng thời gian, trả về số lệnh mới hoặc có thay đổi.
    async fn sync_chunk(&self, trade_type: &str, start: i64, end: i64) -> Result<u64> {
        let mut page = 1;
        let mut fetched = 0i64;
        let mut changed = 0u64;

        loop {
            let res = self
                .fetch_page(trade_type, start, end, page)
                .await
                .map_err(anyhow::Error::new)?;

            let data = Self::extract_data_array(&res);
            if data.is_empty() {
                break;
            }

            changed += self.repo.upsert_many(&data, end).await?;

            let page_size = data.len() as i64;
            fetched += page_size;
            debug!(trade_type, page, page_size, "đã đồng bộ một trang");

            match Self::extract_total(&res) {
                Some(total) if fetched >= total => break,
                None if page_size < PAGE_SIZE as i64 => break,
                _ => {}
            }

            page += 1;
            tokio::time::sleep(PAGE_DELAY).await;
        }

        Ok(changed)
    }

    /// Đồng bộ lại toàn bộ lịch sử trong `days` ngày gần nhất.
    ///
    /// Bản trước chạy `DELETE FROM orders` **trước khi** gọi API. Nếu mạng lỗi ở
    /// giữa, dữ liệu cũ đã mất mà dữ liệu mới chưa về. Ở đây ghi trước, rồi mới cắt
    /// phần nằm ngoài cửa sổ, nên không có thời điểm nào bảng bị trống.
    pub async fn force_initial_sync(&self, days: i64) -> Result<u64> {
        let days = days.clamp(1, 365);
        let chunk_days = 7;
        let now = Utc::now().timestamp_millis();
        let mut changed = 0u64;

        for trade in ["BUY", "SELL"] {
            let mut current_days = 0;
            while current_days < days {
                let end_time = now - (current_days * DAY_MS);
                let chunk_end = (current_days + chunk_days).min(days);
                let start_time = now - (chunk_end * DAY_MS);

                changed += self.sync_chunk(trade, start_time, end_time).await?;
                current_days = chunk_end;
            }

            // Đặt mốc đồng bộ tới hiện tại để incremental_sync không lấy lại từ đầu.
            let oldest = now - (days * DAY_MS);
            self.set_sync_window(trade, oldest, now).await?;
        }

        let removed = self.repo.delete_older_than(now - (days * DAY_MS)).await?;
        info!(days, changed, removed, "hoàn tất đồng bộ lại toàn bộ");
        Ok(changed)
    }

    /// Lấy các lệnh phát sinh từ lần đồng bộ trước tới nay.
    pub async fn incremental_sync(&self) -> Result<u64> {
        let now = Utc::now().timestamp_millis();
        let mut changed = 0u64;

        for trade in ["BUY", "SELL"] {
            let last = self
                .get_sync_window(trade)
                .await?
                .unwrap_or(now - 7 * DAY_MS);

            changed += self.sync_chunk(trade, last, now).await?;
            self.set_sync_window(trade, last, now).await?;
        }

        Ok(changed)
    }

    /// Làm mới các lệnh trong 24 giờ gần nhất để bắt thay đổi trạng thái.
    pub async fn active_poll(&self) -> Result<u64> {
        let now = Utc::now().timestamp_millis();
        let start = now - DAY_MS;
        let mut changed = 0u64;

        for trade in ["BUY", "SELL"] {
            changed += self.sync_chunk(trade, start, now).await?;
        }

        Ok(changed)
    }

    async fn get_sync_window(&self, trade: &str) -> Result<Option<i64>> {
        let row = sqlx::query("SELECT last_end_timestamp FROM sync_state WHERE id = ?1")
            .bind(format!("{trade}_WIN"))
            .fetch_optional(self.repo.pool())
            .await?;
        Ok(row.and_then(|r| r.get::<Option<i64>, _>("last_end_timestamp")))
    }

    async fn set_sync_window(&self, trade: &str, start: i64, end: i64) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        sqlx::query(
            r#"
            INSERT INTO sync_state (id, last_start_timestamp, last_end_timestamp, last_complete_ts)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(id) DO UPDATE SET
                last_start_timestamp = excluded.last_start_timestamp,
                last_end_timestamp   = excluded.last_end_timestamp,
                last_complete_ts     = excluded.last_complete_ts
            "#,
        )
        .bind(format!("{trade}_WIN"))
        .bind(start)
        .bind(end)
        .bind(now)
        .execute(self.repo.pool())
        .await?;
        Ok(())
    }
}
