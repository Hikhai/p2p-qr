use anyhow::Result;
use serde::Serialize;
use serde_json::Value;
use sqlx::{Row, SqlitePool};
use std::sync::Arc;
use tracing::warn;

use super::payment_repo::IN_PROGRESS_STATUS;
use super::stage_map::StageMap;

#[derive(Debug, Serialize)]
pub struct OrderRow {
    pub order_number: String,
    pub trade_type: String,
    pub fiat: String,
    pub asset: String,
    pub amount_asset: String,
    pub total_fiat: String,
    pub price: String,
    pub create_time_ms: i64,
    pub status_code: i64,
    pub status_label: String,
    pub buyer_nickname: String,
    pub seller_nickname: String,
    pub has_payment_detail: bool,
    pub last_api_sync_ts: i64,
}

/// Một lệnh đã được bóc ra từ JSON của Binance.
///
/// Tách khỏi phần ghi DB để phần suy luận trạng thái có thể kiểm thử được — đây là
/// chỗ dễ sai nhất vì Binance trả `orderStatus` lúc là số, lúc là chuỗi.
#[derive(Debug, PartialEq)]
pub struct ParsedOrder {
    pub order_number: String,
    pub trade_type: String,
    pub asset: String,
    pub fiat: String,
    pub amount_asset: String,
    pub total_fiat: String,
    pub price: String,
    pub status_code: i64,
    pub create_time_ms: i64,
    pub buyer_nickname: String,
    pub seller_nickname: String,
}

/// Mã trạng thái dùng khi không suy ra được từ phản hồi.
pub const STATUS_UNKNOWN: i64 = -1;

fn str_field<'a>(order: &'a Value, keys: &[&str]) -> &'a str {
    for key in keys {
        if let Some(v) = order.get(*key).and_then(|x| x.as_str()) {
            if !v.trim().is_empty() {
                return v.trim();
            }
        }
    }
    ""
}

fn status_from_value(value: &Value) -> Option<i64> {
    if let Some(num) = value.as_i64() {
        return Some(num);
    }
    if let Some(num) = value.as_u64() {
        return i64::try_from(num).ok();
    }
    let s = value.as_str()?.trim();
    if let Ok(num) = s.parse::<i64>() {
        return Some(num);
    }
    // Binance C2C history trả `orderStatus` dạng enum chữ, không phải số.
    // Thiếu BUYER_PAYED/DISTRIBUTING/IN_APPEAL khiến lệnh rơi về -1 ("Không rõ"),
    // UI ẩn QR và scheduler tắt poll 15s.
    match s.to_ascii_uppercase().as_str() {
        "PENDING" | "TRADING" => Some(1),
        "BUYER_PAYED" | "PAID" => Some(2),
        "DISTRIBUTING" | "VERIFYING" => Some(3),
        "COMPLETED" => Some(4),
        "IN_APPEAL" | "APPEALING" => Some(5),
        "CANCELLED" | "TIMEOUT" => Some(6),
        "CANCELLED_BY_SYSTEM" => Some(7),
        _ => None,
    }
}

pub fn parse_order(order: &Value) -> Option<ParsedOrder> {
    let order_number = str_field(order, &["orderNumber"]);
    if order_number.is_empty() {
        return None;
    }

    let status_code = order
        .get("orderStatus")
        .or_else(|| order.get("status"))
        .or_else(|| order.get("order_status"))
        .and_then(status_from_value)
        .unwrap_or(STATUS_UNKNOWN);

    let create_time_ms = order
        .get("createTime")
        .and_then(|x| x.as_i64().or_else(|| x.as_str()?.parse::<i64>().ok()))
        .unwrap_or(0);

    Some(ParsedOrder {
        order_number: order_number.to_string(),
        trade_type: str_field(order, &["tradeType"]).to_string(),
        asset: str_field(order, &["asset"]).to_string(),
        fiat: str_field(order, &["fiat"]).to_string(),
        amount_asset: str_field(order, &["amount"]).to_string(),
        total_fiat: str_field(order, &["totalPrice"]).to_string(),
        price: str_field(order, &["price", "unitPrice"]).to_string(),
        status_code,
        create_time_ms,
        buyer_nickname: str_field(order, &["buyerNickname", "buyerNickName"]).to_string(),
        seller_nickname: str_field(
            order,
            &[
                "sellerNickname",
                "sellerNickName",
                "makerNickname",
                "sellerCompanyAccountName",
                "counterPartNickName",
            ],
        )
        .to_string(),
    })
}

pub struct OrderRepo {
    pool: SqlitePool,
    stage_map: Arc<StageMap>,
}

const SELECT_COLUMNS: &str = "order_number, trade_type, fiat, asset, amount_asset, total_fiat, \
     price, create_time_ms, order_status_code, buyer_nickname, seller_nickname, \
     has_payment_detail, last_api_sync_ts";

impl OrderRepo {
    pub fn new(pool: SqlitePool, stage_map: Arc<StageMap>) -> Self {
        Self { pool, stage_map }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Ghi một trang kết quả API trong đúng một transaction.
    ///
    /// Bản trước gọi `INSERT` riêng lẻ cho từng lệnh, mỗi lần là một transaction
    /// ngầm và một lần fsync. Với 500 lệnh đó là 500 lần ghi đĩa; gộp lại còn một.
    ///
    /// Trả về số lệnh thực sự mới hoặc có dữ liệu thay đổi. Nhờ điều kiện `WHERE`
    /// trong nhánh `DO UPDATE`, lần đồng bộ không mang thay đổi nào sẽ trả về 0 và
    /// scheduler bỏ qua việc bắn event, thay vì buộc UI tải lại danh sách mỗi 15 giây.
    pub async fn upsert_many(&self, orders: &[Value], now: i64) -> Result<u64> {
        if orders.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut changed = 0u64;

        for raw in orders {
            let Some(order) = parse_order(raw) else { continue };

            if order.status_code == STATUS_UNKNOWN {
                // Ghi lại mã lệnh thôi, không ghi cả payload: payload chứa nickname,
                // số tiền và có thể cả thông tin tài khoản.
                warn!(
                    order_number = %order.order_number,
                    raw_status = ?raw.get("orderStatus").or_else(|| raw.get("status")),
                    "không suy ra được trạng thái lệnh từ phản hồi API"
                );
            }

            let result = sqlx::query(
                r#"
                INSERT INTO orders (
                    order_number, trade_type, asset, fiat, price, amount_asset, total_fiat,
                    order_status_code, create_time_ms, update_time_ms,
                    buyer_nickname, seller_nickname, last_api_sync_ts, source_flags
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 1)
                ON CONFLICT(order_number) DO UPDATE SET
                    trade_type        = excluded.trade_type,
                    asset             = excluded.asset,
                    fiat              = excluded.fiat,
                    price             = excluded.price,
                    amount_asset      = excluded.amount_asset,
                    total_fiat        = excluded.total_fiat,
                    order_status_code = excluded.order_status_code,
                    update_time_ms    = excluded.update_time_ms,
                    buyer_nickname    = excluded.buyer_nickname,
                    seller_nickname   = excluded.seller_nickname,
                    last_api_sync_ts  = excluded.last_api_sync_ts,
                    source_flags      = orders.source_flags | 1
                WHERE orders.order_status_code IS NOT excluded.order_status_code
                   OR orders.total_fiat        IS NOT excluded.total_fiat
                   OR orders.amount_asset      IS NOT excluded.amount_asset
                   OR orders.price             IS NOT excluded.price
                   OR orders.buyer_nickname    IS NOT excluded.buyer_nickname
                   OR orders.seller_nickname   IS NOT excluded.seller_nickname
                "#,
            )
            .bind(&order.order_number)
            .bind(&order.trade_type)
            .bind(&order.asset)
            .bind(&order.fiat)
            .bind(&order.price)
            .bind(&order.amount_asset)
            .bind(&order.total_fiat)
            .bind(order.status_code)
            .bind(order.create_time_ms)
            .bind(now)
            .bind(&order.buyer_nickname)
            .bind(&order.seller_nickname)
            .bind(now)
            .execute(&mut *tx)
            .await?;

            changed += result.rows_affected();
        }

        tx.commit().await?;
        Ok(changed)
    }

    pub async fn list_orders(&self, limit: i64) -> Result<Vec<OrderRow>> {
        let sql = format!("SELECT {SELECT_COLUMNS} FROM orders ORDER BY create_time_ms DESC LIMIT ?1");
        // limit <= 0 nghĩa là "không giới hạn"; SQLite hiểu LIMIT -1 đúng như vậy.
        let effective_limit = if limit > 0 { limit } else { -1 };

        let rows = sqlx::query(&sql)
            .bind(effective_limit)
            .fetch_all(&self.pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(|r| {
                let code: i64 = r.get("order_status_code");
                OrderRow {
                    order_number: r.get("order_number"),
                    trade_type: r.get("trade_type"),
                    fiat: r.get("fiat"),
                    asset: r.get("asset"),
                    amount_asset: r.get("amount_asset"),
                    total_fiat: r.get("total_fiat"),
                    price: r.get("price"),
                    create_time_ms: r.get("create_time_ms"),
                    status_code: code,
                    status_label: self.stage_map.label(code),
                    buyer_nickname: r.get("buyer_nickname"),
                    seller_nickname: r.get("seller_nickname"),
                    has_payment_detail: r.get::<i64, _>("has_payment_detail") == 1,
                    last_api_sync_ts: r.get("last_api_sync_ts"),
                }
            })
            .collect())
    }

    /// Tổng tiền VND của một lệnh, dùng làm số tiền trên QR.
    ///
    /// Đáng tin hơn con số extension bóc từ giao diện web, vì đây là giá trị API trả về.
    pub async fn total_fiat_vnd(&self, order_number: &str) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT total_fiat FROM orders WHERE order_number = ?1 AND fiat = 'VND' LIMIT 1",
        )
        .bind(order_number)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.and_then(|r| r.get::<Option<String>, _>("total_fiat")))
    }

    /// Số lệnh đang xử lý — scheduler dùng để quyết định có cần poll nhanh hay không.
    pub async fn count_in_progress(&self) -> Result<i64> {
        let row = sqlx::query(
            "SELECT COUNT(*) AS count FROM orders WHERE order_status_code IN (?1, ?2, ?3)",
        )
        .bind(IN_PROGRESS_STATUS[0])
        .bind(IN_PROGRESS_STATUS[1])
        .bind(IN_PROGRESS_STATUS[2])
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get("count"))
    }

    /// Tạo lệnh BUY tạm khi extension capture sớm hơn sync API.
    ///
    /// `source_flags` bit 2 đánh dấu nguồn extension. Sync API sau đó sẽ ghi đè
    /// nickname/số tiền/trạng thái thật qua `ON CONFLICT DO UPDATE`.
    pub async fn ensure_buy_placeholder(
        &self,
        order_number: &str,
        total_fiat: Option<&str>,
        now_ms: i64,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            INSERT INTO orders (
                order_number, trade_type, asset, fiat, price, amount_asset, total_fiat,
                order_status_code, create_time_ms, update_time_ms,
                buyer_nickname, seller_nickname, last_api_sync_ts, source_flags
            )
            VALUES (?1, 'BUY', '', 'VND', '', '', ?2, 1, ?3, ?3, '', '', 0, 2)
            ON CONFLICT(order_number) DO NOTHING
            "#,
        )
        .bind(order_number)
        .bind(total_fiat.unwrap_or(""))
        .bind(now_ms)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Xoá các lệnh nằm ngoài cửa sổ thời gian người dùng chọn.
    pub async fn delete_older_than(&self, cutoff_ms: i64) -> Result<u64> {
        let result = sqlx::query("DELETE FROM orders WHERE create_time_ms < ?1")
            .bind(cutoff_ms)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Xoá toàn bộ dữ liệu nghiệp vụ trong một transaction.
    pub async fn clear_all(&self) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM order_payment_detail")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM orders").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM sync_state")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_numeric_and_string_status() {
        let numeric = json!({"orderNumber": "1", "orderStatus": 4});
        assert_eq!(parse_order(&numeric).unwrap().status_code, 4);

        let numeric_string = json!({"orderNumber": "1", "orderStatus": "4"});
        assert_eq!(parse_order(&numeric_string).unwrap().status_code, 4);

        for (label, code) in [
            ("PENDING", 1),
            ("TRADING", 1),
            ("BUYER_PAYED", 2),
            ("buyer_payed", 2),
            ("PAID", 2),
            ("DISTRIBUTING", 3),
            ("VERIFYING", 3),
            ("COMPLETED", 4),
            ("IN_APPEAL", 5),
            ("APPEALING", 5),
            ("CANCELLED", 6),
            ("TIMEOUT", 6),
            ("CANCELLED_BY_SYSTEM", 7),
        ] {
            let v = json!({"orderNumber": "1", "orderStatus": label});
            assert_eq!(parse_order(&v).unwrap().status_code, code, "sai với {label}");
        }
    }

    #[test]
    fn unknown_status_falls_back_to_sentinel() {
        let v = json!({"orderNumber": "1", "orderStatus": "SOMETHING_NEW"});
        assert_eq!(parse_order(&v).unwrap().status_code, STATUS_UNKNOWN);

        let missing = json!({"orderNumber": "1"});
        assert_eq!(parse_order(&missing).unwrap().status_code, STATUS_UNKNOWN);
    }

    #[test]
    fn order_without_number_is_skipped() {
        assert!(parse_order(&json!({"orderStatus": 4})).is_none());
        assert!(parse_order(&json!({"orderNumber": ""})).is_none());
        assert!(parse_order(&json!({"orderNumber": "   "})).is_none());
    }

    #[test]
    fn falls_back_through_nickname_aliases() {
        let v = json!({
            "orderNumber": "1",
            "buyerNickName": "buyer",
            "counterPartNickName": "seller",
        });
        let parsed = parse_order(&v).unwrap();
        assert_eq!(parsed.buyer_nickname, "buyer");
        assert_eq!(parsed.seller_nickname, "seller");
    }

    #[test]
    fn create_time_accepts_string_or_number() {
        let as_num = json!({"orderNumber": "1", "createTime": 1700000000000i64});
        assert_eq!(parse_order(&as_num).unwrap().create_time_ms, 1700000000000);

        let as_str = json!({"orderNumber": "1", "createTime": "1700000000000"});
        assert_eq!(parse_order(&as_str).unwrap().create_time_ms, 1700000000000);

        let missing = json!({"orderNumber": "1"});
        assert_eq!(parse_order(&missing).unwrap().create_time_ms, 0);
    }
}
