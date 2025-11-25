use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use anyhow::Result;
use std::path::Path;
use std::fs;

pub struct Db {
    pool: SqlitePool
}

impl Db {
    pub async fn init(db_path: &str) -> Result<Self> {
        if let Some(parent) = Path::new(db_path).parent() { if !parent.as_os_str().is_empty() { let _ = fs::create_dir_all(parent); } }
        let exists = Path::new(db_path).exists();
        let url = format!("sqlite://{}?mode=rwc", db_path);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&url).await?;
        if !exists { println!("[DB] Creating new database at {db_path}"); }
        Self::run_migrations(&pool).await?;
        Ok(Self { pool })
    }

    async fn run_migrations(pool: &SqlitePool) -> Result<()> {
        let sql = include_str!("../../migrations/001_init.sql");
        for statement in sql.split(';') {
            let stmt = statement.trim();
            if !stmt.is_empty() {
                sqlx::query(stmt).execute(pool).await?;
            }
        }
        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool { &self.pool }
}
