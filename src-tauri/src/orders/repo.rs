use anyhow::Result;
use sqlx::{SqlitePool, Row};
use serde::Serialize;
use std::sync::Arc;

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
    pub last_api_sync_ts: i64
}

pub struct OrderRepo { pool: SqlitePool, stage_map: Arc<StageMap> }
impl OrderRepo { pub fn new(pool: SqlitePool, stage_map: Arc<StageMap>) -> Self { Self { pool, stage_map } } }

impl OrderRepo { pub fn pool(&self) -> &SqlitePool { &self.pool } }

impl OrderRepo {
    pub async fn upsert_from_api(&self, order: &serde_json::Value, now: i64) -> Result<()> {
        let order_number = order.get("orderNumber").and_then(|x| x.as_str()).unwrap_or("");
        if order_number.is_empty() { return Ok(()); }
        let trade_type = order.get("tradeType").and_then(|x| x.as_str()).unwrap_or("");
        let asset = order.get("asset").and_then(|x| x.as_str()).unwrap_or("");
        let fiat = order.get("fiat").and_then(|x| x.as_str()).unwrap_or("");
        let amount_asset = order.get("amount").and_then(|x| x.as_str()).unwrap_or("");
        let total_fiat = order.get("totalPrice").and_then(|x| x.as_str()).unwrap_or("");
        let price = order.get("price").and_then(|x| x.as_str())
            .or_else(|| order.get("unitPrice").and_then(|x| x.as_str()))
            .unwrap_or("");
        let status_code = order.get("orderStatus")
            .or_else(|| order.get("status"))
            .or_else(|| order.get("order_status"))
            .and_then(|x| {
                // Nếu là số, dùng trực tiếp
                if let Some(num) = x.as_i64() {
                    return Some(num);
                }
                // Nếu là string, parse về số hoặc map string status
                if let Some(s) = x.as_str() {
                    // Thử parse string thành số trước
                    if let Ok(num) = s.parse::<i64>() {
                        return Some(num);
                    }
                    // Map string status về code (theo đúng Binance P2P)
                    match s {
                        "PENDING" => Some(1),                    // Đang chờ thanh toán
                        "TRADING" => Some(1),                    // Đang giao dịch (tương tự PENDING)
                        "PAID" => Some(2),                       // Đã thanh toán
                        "VERIFYING" => Some(3),                  // Đang xác minh
                        "COMPLETED" => Some(4),                  // Đã hoàn thành
                        "CANCELLED" => Some(5),                  // Đã hủy
                        "CANCELLED_BY_SYSTEM" => Some(6),        // Đã hủy bởi hệ thống
                        "TIMEOUT" => Some(5),                    // Timeout -> hủy
                        "APPEALING" => Some(3),                  // Đang khiếu nại -> xác minh
                        _ => {
                            eprintln!("[PARSE] Unknown orderStatus string: '{}' for order", s);
                            Some(-1)
                        }
                    }
                } else {
                    Some(-1)
                }
            })
            .unwrap_or(-1);
        let create_time = order.get("createTime")
            .and_then(|x| x.as_i64().or_else(|| x.as_str().and_then(|s| s.parse::<i64>().ok())))
            .unwrap_or(0);
        let buyer_nick = order.get("buyerNickname").and_then(|x| x.as_str())
            .or_else(|| order.get("buyerNickName").and_then(|x| x.as_str()))
            .unwrap_or("").trim();
        let seller_nick = order.get("sellerNickname").and_then(|x| x.as_str())
            .or_else(|| order.get("sellerNickName").and_then(|x| x.as_str()))
            .or_else(|| order.get("makerNickname").and_then(|x| x.as_str()))
            .or_else(|| order.get("sellerCompanyAccountName").and_then(|x| x.as_str()))
            .or_else(|| order.get("counterPartNickName").and_then(|x| x.as_str()))
            .unwrap_or("").trim();
        if status_code == -1 || seller_nick.is_empty() {
            let keys = order.as_object().map(|m| m.keys().cloned().collect::<Vec<_>>()).unwrap_or_default();
            let os = order.get("orderStatus").cloned().unwrap_or(serde_json::Value::Null);
            eprintln!("[PARSE] order {} anomalies: status_code={}, seller='{}' orderStatus={:?} keys={:?}", order_number, status_code, seller_nick, os, keys);
        }
        sqlx::query(r#"INSERT INTO orders (order_number, trade_type, asset, fiat, price, amount_asset, total_fiat, order_status_code, create_time_ms, update_time_ms, buyer_nickname, seller_nickname, last_api_sync_ts, source_flags) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,1) ON CONFLICT(order_number) DO UPDATE SET trade_type=excluded.trade_type, asset=excluded.asset, fiat=excluded.fiat, price=excluded.price, amount_asset=excluded.amount_asset, total_fiat=excluded.total_fiat, order_status_code=excluded.order_status_code, update_time_ms=excluded.update_time_ms, buyer_nickname=excluded.buyer_nickname, seller_nickname=excluded.seller_nickname, last_api_sync_ts=excluded.last_api_sync_ts, source_flags = orders.source_flags | 1"#)
            .bind(order_number).bind(trade_type).bind(asset).bind(fiat).bind(price).bind(amount_asset).bind(total_fiat).bind(status_code).bind(create_time).bind(now).bind(buyer_nick).bind(seller_nick).bind(now)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn list_orders(&self, limit: i64) -> Result<Vec<OrderRow>> {
        let rows = if limit > 0 {
            sqlx::query(r#"SELECT order_number, trade_type, fiat, asset, amount_asset, total_fiat, price, create_time_ms, order_status_code, buyer_nickname, seller_nickname, has_payment_detail, last_api_sync_ts FROM orders ORDER BY create_time_ms DESC LIMIT ?"#)
                .bind(limit)
                .fetch_all(&self.pool)
                .await?
        } else {
            sqlx::query(r#"SELECT order_number, trade_type, fiat, asset, amount_asset, total_fiat, price, create_time_ms, order_status_code, buyer_nickname, seller_nickname, has_payment_detail, last_api_sync_ts FROM orders ORDER BY create_time_ms DESC"#)
                .fetch_all(&self.pool)
                .await?
        };
        let mut out = Vec::new();
        for r in rows {
            let code: i64 = r.get("order_status_code");
            out.push(OrderRow {
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
                has_payment_detail: r.get::<i64,_>("has_payment_detail") == 1,
                last_api_sync_ts: r.get("last_api_sync_ts")
            });
        }
        Ok(out)
    }
}

