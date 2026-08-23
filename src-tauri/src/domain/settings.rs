//! 设置领域：每日工作时间、每周工作日、日内时间窗口、均分窗口。
//! FirstRun 默认（CONTEXT「首次运行」）：5h/天、周一至五、均分窗口 7 个工作日，
//! 三者同时是全局兜底值。时间窗口默认给一整段常规工作时段（09:00–18:00），
//! 仅作为占位默认，设置页（工单 13）可改。

use chrono::{DateTime, Local};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::clock::Clock;

/// 日内一段时间窗口，端点为当日分钟数（0 = 00:00）。
/// 跨午夜窗口以 end < start 表达，如 20:00–01:00 = (1200, 60)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeWindow {
    pub start_minute: u16,
    pub end_minute: u16,
}

/// 全局设置（单行存储，id 恒为 1）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// 每日工作时间（分钟），默认 300 = 5h
    pub daily_minutes: u32,
    /// 每周工作日，周一 = 1 .. 周日 = 7，默认周一至五
    pub workdays: Vec<u8>,
    /// 日内多段时间窗口
    pub time_windows: Vec<TimeWindow>,
    /// 均分窗口（工作日数），默认 7
    pub smoothing_workdays: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            daily_minutes: 300,
            workdays: vec![1, 2, 3, 4, 5],
            time_windows: vec![TimeWindow {
                start_minute: 9 * 60,
                end_minute: 18 * 60,
            }],
            smoothing_workdays: 7,
        }
    }
}

/// 设置读写服务。`load` 保证 FirstRun 默认值落库（INSERT OR IGNORE 幂等）。
pub struct SettingsService;

impl SettingsService {
    /// 读取当前设置；库中无行时先写入默认值再返回。
    pub fn load(conn: &Connection) -> rusqlite::Result<Settings> {
        let d = Settings::default();
        conn.execute(
            "INSERT OR IGNORE INTO settings (id, daily_minutes, workdays, time_windows, smoothing_workdays)
             VALUES (1, ?1, ?2, ?3, ?4)",
            params![d.daily_minutes, json(&d.workdays), json(&d.time_windows), d.smoothing_workdays],
        )?;
        conn.query_row(
            "SELECT daily_minutes, workdays, time_windows, smoothing_workdays FROM settings WHERE id = 1",
            [],
            |row| {
                Ok(Settings {
                    daily_minutes: row.get(0)?,
                    workdays: serde_json::from_str(&row.get::<_, String>(1)?).unwrap(),
                    time_windows: serde_json::from_str(&row.get::<_, String>(2)?).unwrap(),
                    smoothing_workdays: row.get(3)?,
                })
            },
        )
    }

    /// 覆盖保存整份设置；updated_at 取注入时钟（SettingsEffectiveTime 需要）。
    pub fn save(conn: &Connection, clock: &dyn Clock, s: &Settings) -> rusqlite::Result<()> {
        conn.execute(
            "UPDATE settings SET daily_minutes = ?1, workdays = ?2, time_windows = ?3,
             smoothing_workdays = ?4, updated_at = ?5 WHERE id = 1",
            params![
                s.daily_minutes,
                json(&s.workdays),
                json(&s.time_windows),
                s.smoothing_workdays,
                clock.now().to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// 设置最近一次保存时刻（尚未保存过时为 None）。
    pub fn updated_at(conn: &Connection) -> rusqlite::Result<Option<DateTime<Local>>> {
        let raw: String = conn.query_row(
            "SELECT updated_at FROM settings WHERE id = 1",
            [],
            |row| row.get(0),
        )?;
        Ok(if raw.is_empty() {
            None
        } else {
            Some(DateTime::parse_from_rfc3339(&raw).unwrap().with_timezone(&Local))
        })
    }
}

/// JSON 列序列化（Vec<u8> / Vec<TimeWindow> 都是 infallible，直接 unwrap）
fn json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap()
}
