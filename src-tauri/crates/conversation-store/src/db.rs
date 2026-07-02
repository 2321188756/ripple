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
    let manager = SqliteConnectionManager::file(db_path);
    let pool = Pool::builder()
        .max_size(8)
        .build(manager)
        .map_err(|e| crate::StoreError::Pool(e.to_string()))?;

    // 初始化连接：PRAGMA + 迁移
    {
        let conn = pool.get().map_err(|e| crate::StoreError::Pool(e.to_string()))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;",
        )
        .map_err(|e| crate::StoreError::Database(e.to_string()))?;

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
        let conn = pool.get().map_err(|e| crate::StoreError::Pool(e.to_string()))?;
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
