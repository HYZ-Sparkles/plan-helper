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
    conn.execute_batch(SCHEMA)?;
    backfill_columns(conn)
}

/// 工单 05 给 plans 补 pause_reason 列：CREATE TABLE IF NOT EXISTS 不会给已存在的旧库加列，
/// 幂等 ALTER 兜底开发期数据（spec 排除正式迁移，这里只是加列回填）。
fn backfill_columns(conn: &Connection) -> rusqlite::Result<()> {
    let has_pause_reason: bool = conn
        .prepare("SELECT COUNT(*) FROM pragma_table_info('plans') WHERE name = 'pause_reason'")?
        .query_row([], |row| row.get::<_, i64>(0))
        .map(|n| n > 0)?;
    if !has_pause_reason {
        conn.execute("ALTER TABLE plans ADD COLUMN pause_reason TEXT", [])?;
    }
    Ok(())
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

CREATE TABLE IF NOT EXISTS plans (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    name         TEXT NOT NULL,
    summary      TEXT NOT NULL,                    -- 留空落库时已回退为名称
    detail       TEXT NOT NULL DEFAULT '',
    priority     TEXT NOT NULL DEFAULT 'Medium',   -- Low | Medium | High
    due_date     TEXT,                             -- 仅展示，YYYY-MM-DD
    status       TEXT NOT NULL DEFAULT 'NotStarted',
    created_at   TEXT NOT NULL,                    -- RFC3339
    pause_reason TEXT                              -- 非 NULL = 暂停原因：UserInitiated | AutoPreempted
);

CREATE TABLE IF NOT EXISTS tasks (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    plan_id           INTEGER NOT NULL REFERENCES plans(id),
    name              TEXT NOT NULL,
    summary           TEXT NOT NULL,
    detail            TEXT NOT NULL DEFAULT '',
    has_subgoals      INTEGER NOT NULL DEFAULT 0,  -- 0/1
    estimated_minutes INTEGER,                     -- 无子目标任务必填；有子目标=子目标和
    position          INTEGER NOT NULL,            -- 计划内顺序
    status            TEXT NOT NULL DEFAULT 'NotStarted',
    deleted_at        TEXT,                        -- 软删除（非 NULL = 已归档）
    created_at        TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS subgoals (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id           INTEGER NOT NULL REFERENCES tasks(id),
    name              TEXT NOT NULL,
    estimated_minutes INTEGER NOT NULL,            -- 必填（CONTEXT 精简输入行）
    position          INTEGER NOT NULL,            -- 任务内顺序 = 填写顺序，不可拖拽
    completed_at      TEXT,                        -- 非 NULL = 已完成（记录完成时刻，07 接线）
    created_at        TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS task_dependencies (
    predecessor_id INTEGER NOT NULL REFERENCES tasks(id), -- 前置任务（A）
    successor_id   INTEGER NOT NULL REFERENCES tasks(id), -- 后继任务（B，依赖 A）
    PRIMARY KEY (predecessor_id, successor_id)
);

CREATE TABLE IF NOT EXISTS today_allocations (
    id       INTEGER PRIMARY KEY CHECK (id = 1), -- 单行：最近一次提交的分配（覆盖重选即整行替换）
    date     TEXT NOT NULL,                      -- 分配归属的工作日 YYYY-MM-DD（本地日期）
    task_ids TEXT NOT NULL                       -- JSON [taskId,...]（今日选中的任务集）
);

CREATE TABLE IF NOT EXISTS progress_log (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id       INTEGER NOT NULL REFERENCES tasks(id), -- 汇报所属任务
    at            TEXT NOT NULL,                -- RFC3339 事件时刻
    delta_minutes REAL NOT NULL,                -- 增量分钟（撤销 / 下调修正为负，追加式账本）
    source        TEXT NOT NULL                 -- SubGoal | SubGoalUndo | Percent | Correction
);

CREATE TABLE IF NOT EXISTS current_task (
    id      INTEGER PRIMARY KEY CHECK (id = 1), -- 单行：用户指定的「此刻正在做」（工单 07）
    task_id INTEGER NOT NULL REFERENCES tasks(id)
);

CREATE TABLE IF NOT EXISTS summary_shown (
    date     TEXT PRIMARY KEY,            -- 总结归属日 YYYY-MM-DD（只弹一次的登记，工单 11）
    shown_at TEXT NOT NULL                -- 弹出时刻（RFC3339）
);
";
