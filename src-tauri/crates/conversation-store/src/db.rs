//! SQLite 数据库初始化和连接池管理。

use std::path::Path;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

use crate::migration;
use crate::StoreResult;

/// 连接池类型
pub type DbPool = Pool<SqliteConnectionManager>;

/// 初始化数据库文件：创建连接池、设置 PRAGMA、执行迁移。
pub fn init_db(db_path: &Path) -> StoreResult<DbPool> {
    // PRAGMA（尤其 foreign_keys）是 per-connection 的，必须通过 with_init 对池中
    // 每条连接都设置，否则只有首个连接 FK 开启，其余连接的 ON DELETE CASCADE 静默失效，
    // 删对话会留下 messages 孤儿行。
    let manager = SqliteConnectionManager::file(db_path).with_init(|c| {
        c.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;",
        )
    });
    let pool = Pool::builder()
        .max_size(8)
        .build(manager)
        .map_err(|e| crate::StoreError::Pool(e.to_string()))?;

    // 迁移只需在任一连接上执行一次
    {
        let conn = pool
            .get()
            .map_err(|e| crate::StoreError::Pool(e.to_string()))?;
        migration::run_migrations(&conn)?;
    }

    Ok(pool)
}

/// 创建内存 SQLite（测试用）
#[cfg(test)]
pub fn init_memory_db() -> StoreResult<DbPool> {
    let manager = SqliteConnectionManager::memory();
    let pool = Pool::builder()
        .max_size(2)
        .build(manager)
        .map_err(|e| crate::StoreError::Pool(e.to_string()))?;

    {
        let conn = pool
            .get()
            .map_err(|e| crate::StoreError::Pool(e.to_string()))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;",
        )
        .map_err(|e| crate::StoreError::Database(e.to_string()))?;

        migration::run_migrations(&conn)?;
    }

    Ok(pool)
}
