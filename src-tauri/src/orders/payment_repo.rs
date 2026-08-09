//! Truy cập bảng `order_payment_detail`.
//!
//! Trước đây SQL của bảng này được viết inline ở bốn chỗ khác nhau trong `main.rs`,
//! mỗi chỗ một tập cột khác nhau. Đó là lý do sinh ra ba lỗi cùng lúc: ghi bằng
//! `INSERT OR REPLACE` trong khi bảng không có UNIQUE index để replace lên, đọc
//! không `ORDER BY` nên trả về bản ghi cũ nhất, và hai query tham chiếu cột không
//! tồn tại (`created_at`, `orders.status_code`).

use anyhow::Result;
use serde::Serialize;
use sqlx::{Row, SqlitePool};

/// Thông tin thanh toán được giữ tối đa 24 giờ.
const PURGE_TTL_MS: i64 = 24 * 60 * 60 * 1000;

/// Các mã trạng thái được coi là "đang xử lý" — chỉ những lệnh này cần giữ
/// thông tin thanh toán.
pub const IN_PROGRESS_STATUS: [i64; 3] = [1, 2, 3];

/// Dữ liệu đầu vào khi extension hoặc UI gửi thông tin thanh toán lên.
#[derive(Debug, Default, Clone)]
pub struct PaymentDetailInput {
    pub order_number: String,
    pub account_name: Option<String>,
    pub account_no: Option<String>,
    pub bank_name: Option<String>,
    pub sub_bank: Option<String>,
    pub qr_code_url: Option<String>,
    pub amount: Option<String>,
    pub transfer_content: Option<String>,
    pub suggested_transfer_content: Option<String>,
}

/// Bản ghi trả về cho UI. Tên field giữ nguyên dạng snake_case vì frontend đang
/// đọc trực tiếp các khoá này.
#[derive(Debug, Serialize)]
pub struct PaymentDetail {
    pub account_name: Option<String>,
    pub account_no: Option<String>,
    pub bank_name: Option<String>,
    pub sub_bank: Option<String>,
    pub qr_code_url: Option<String>,
    pub amount: Option<String>,
    pub transfer_content: Option<String>,
    pub suggested_transfer_content: Option<String>,
    pub captured_at: Option<i64>,
}

pub struct PaymentRepo {
    pool: SqlitePool,
}

impl PaymentRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Lệnh có tồn tại trong bảng `orders` hay không.
    ///
    /// Dùng để từ chối thông tin thanh toán gửi cho một số lệnh bất kỳ — nếu không,
    /// bất cứ ai gọi được endpoint HTTP đều bơm được bản ghi vào DB.
    pub async fn order_exists(&self, order_number: &str) -> Result<bool> {
        let row = sqlx::query("SELECT 1 AS ok FROM orders WHERE order_number = ?1 LIMIT 1")
            .bind(order_number)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }

    /// Ghi thông tin thanh toán cho một lệnh.
    ///
    /// Dùng `ON CONFLICT(order_number)` nên mỗi lệnh chỉ có đúng một dòng. `COALESCE`
    /// giữ lại giá trị cũ khi lần ghi mới không mang field đó, nhờ vậy một lời gọi
    /// thiếu field không xoá mất dữ liệu đã có.
    pub async fn upsert(&self, input: &PaymentDetailInput, now_ms: i64) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            INSERT INTO order_payment_detail (
                order_number, account_name, account_no, bank_name, sub_bank,
                qr_code_url, amount, transfer_content, suggested_transfer_content,
                captured_at, purge_after
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(order_number) DO UPDATE SET
                account_name               = COALESCE(excluded.account_name, order_payment_detail.account_name),
                account_no                 = COALESCE(excluded.account_no, order_payment_detail.account_no),
                bank_name                  = COALESCE(excluded.bank_name, order_payment_detail.bank_name),
                sub_bank                   = COALESCE(excluded.sub_bank, order_payment_detail.sub_bank),
                qr_code_url                = COALESCE(excluded.qr_code_url, order_payment_detail.qr_code_url),
                amount                     = COALESCE(excluded.amount, order_payment_detail.amount),
                transfer_content           = COALESCE(excluded.transfer_content, order_payment_detail.transfer_content),
                suggested_transfer_content = COALESCE(excluded.suggested_transfer_content, order_payment_detail.suggested_transfer_content),
                captured_at                = excluded.captured_at,
                purge_after                = excluded.purge_after
            "#,
        )
        .bind(&input.order_number)
        .bind(&input.account_name)
        .bind(&input.account_no)
        .bind(&input.bank_name)
        .bind(&input.sub_bank)
        .bind(&input.qr_code_url)
        .bind(&input.amount)
        .bind(&input.transfer_content)
        .bind(&input.suggested_transfer_content)
        .bind(now_ms)
        .bind(now_ms + PURGE_TTL_MS)
        .execute(&mut *tx)
        .await?;

        // Cột này trước đây chỉ được đọc, không ai ghi, nên luôn bằng 0. Cập nhật ở
        // đây để UI biết lệnh nào đã có thông tin thanh toán.
        sqlx::query("UPDATE orders SET has_payment_detail = 1 WHERE order_number = ?1")
            .bind(&input.order_number)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Đọc thông tin thanh toán mới nhất của một lệnh.
    pub async fn get(&self, order_number: &str) -> Result<Option<PaymentDetail>> {
        let row = sqlx::query(
            r#"
            SELECT account_name, account_no, bank_name, sub_bank, qr_code_url,
                   amount, transfer_content, suggested_transfer_content, captured_at
            FROM order_payment_detail
            WHERE order_number = ?1
            ORDER BY captured_at DESC
            LIMIT 1
            "#,
        )
        .bind(order_number)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| PaymentDetail {
            account_name: r.get("account_name"),
            account_no: r.get("account_no"),
            bank_name: r.get("bank_name"),
            sub_bank: r.get("sub_bank"),
            qr_code_url: r.get("qr_code_url"),
            amount: r.get("amount"),
            transfer_content: r.get("transfer_content"),
            suggested_transfer_content: r.get("suggested_transfer_content"),
            captured_at: r.get("captured_at"),
        }))
    }

    /// Xoá thông tin thanh toán đã hết hạn hoặc thuộc lệnh không còn đang xử lý.
    ///
    /// Bản cũ lọc theo `orders.status_code`, nhưng cột thật tên `order_status_code`,
    /// nên query luôn lỗi và dữ liệu chưa bao giờ được xoá.
    pub async fn purge_expired(&self, now_ms: i64) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM order_payment_detail
            WHERE purge_after < ?1
               OR NOT EXISTS (
                    SELECT 1 FROM orders
                    WHERE orders.order_number = order_payment_detail.order_number
                      AND orders.order_status_code IN (?2, ?3, ?4)
               )
            "#,
        )
        .bind(now_ms)
        .bind(IN_PROGRESS_STATUS[0])
        .bind(IN_PROGRESS_STATUS[1])
        .bind(IN_PROGRESS_STATUS[2])
        .execute(&self.pool)
        .await?;

        if result.rows_affected() > 0 {
            // Đồng bộ lại cờ để UI không hiển thị sai.
            sqlx::query(
                r#"
                UPDATE orders SET has_payment_detail = 0
                WHERE has_payment_detail = 1
                  AND NOT EXISTS (
                        SELECT 1 FROM order_payment_detail
                        WHERE order_payment_detail.order_number = orders.order_number
                  )
                "#,
            )
            .execute(&self.pool)
            .await?;
        }

        Ok(result.rows_affected())
    }
}
