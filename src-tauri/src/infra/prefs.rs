//! 界面偏好 KV（工单 22）：桌宠形象选择等前端持久化项。
//! 与带版本语义的 settings 表分开——偏好无生效时机、无历史，读写即所得。

use rusqlite::{params, Connection};

/// 读一条偏好；无此键返回 None（调用方回落默认值）。
pub fn get(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT value FROM app_prefs WHERE key = ?1", params![key], |r| {
        r.get(0)
    })
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        e => Err(e),
    })
}

/// 写一条偏好（覆盖式）。
pub fn set(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO app_prefs (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = ?2",
        params![key, value],
    )?;
    Ok(())
}
