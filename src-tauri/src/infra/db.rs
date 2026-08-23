//! SQLite 基建：连接与建表。首版 schema 从零建立（spec Out of Scope 排除迁移）。

use rusqlite::Connection;
use std::path::Path;

/// 打开（必要时创建）应用数据库并建好全部表。
pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    init(&conn)?;
    Ok(conn)
}

/// 内存库（集成测试用）。
pub fn open_in_memory() -> rusqlite::Result<Connection> {
    let conn = Connection::open_in_memory()?;
    init(&conn)?;
    Ok(conn)
}

fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.execute_batch(SCHEMA)
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS settings (
    id                 INTEGER PRIMARY KEY CHECK (id = 1),
    daily_minutes      INTEGER NOT NULL,           -- 每日工作时间（分钟）
    workdays           TEXT    NOT NULL,           -- JSON [1..7]，周一=1
    time_windows       TEXT    NOT NULL,           -- JSON [{start_minute,end_minute}]
    smoothing_workdays INTEGER NOT NULL,           -- 均分窗口（工作日数）
    updated_at         TEXT    NOT NULL DEFAULT ''-- 最近一次保存（RFC3339）
);
";
