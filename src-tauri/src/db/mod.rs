use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

/// Migration được nhúng vào binary lúc biên dịch và chạy qua `sqlx::migrate!`, nên
/// mỗi file chỉ chạy một lần và được ghi lại trong bảng `_sqlx_migrations`.
///
/// Bản trước tự viết migration runner: nó chỉ nạp `001_init.sql`, tách chuỗi theo
/// dấu `;` rồi chạy lại toàn bộ mỗi lần khởi động. Hệ quả là `002_add_payment_fields.sql`
/// không bao giờ chạy, và bất kỳ migration nào có `;` bên trong (trigger, view) sẽ bị
/// cắt thành các câu lệnh vô nghĩa.
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

pub struct Db {
    pool: SqlitePool,
}

impl Db {
    pub async fn init(db_path: &str) -> Result<Self> {
        if let Some(parent) = Path::new(db_path).parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }

        // Dùng SqliteConnectOptions thay vì tự ghép chuỗi `sqlite://{path}`: đường dẫn
        // trên Windows chứa dấu `\` và có thể chứa khoảng trắng, ghép tay là sai URL.
        let options = SqliteConnectOptions::from_str(&format!("sqlite://{db_path}"))
            .unwrap_or_else(|_| SqliteConnectOptions::new())
            .filename(db_path)
            .create_if_missing(true)
            // WAL cho phép đọc song song với ghi — cần thiết vì scheduler ghi trong
            // khi UI đang đọc.
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            // Không có timeout thì mọi tranh chấp ghi trả về SQLITE_BUSY ngay lập tức.
            .busy_timeout(Duration::from_secs(5))
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(10))
            .connect_with(options)
            .await?;

        MIGRATOR.run(&pool).await?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}
