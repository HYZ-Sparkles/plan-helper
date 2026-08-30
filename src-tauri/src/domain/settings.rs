//! 设置领域：每日工作时间、每周工作日、日内时间窗口、日期例外、均分窗口。
//! FirstRun 默认（CONTEXT「首次运行」）：5h/天、周一至五、均分窗口 7 个工作日，
//! 三者同时是全局兜底值。时间窗口默认给一整段常规工作时段（09:00–18:00）。
//!
//! 生效时机（工单 13，CONTEXT SettingsEffectiveTime）：
//! - **每日工作时间 / 每周工作日 / 时间窗口**的变更自**下一个工作日**生效——写入
//!   `settings_versions` 版本历史（effective_from = 按保存时的配置"今天之后的第一个
//!   工作日"），某日期的配置 = effective_from <= 该日期的最新版本，当日与历史维持
//!   原值（当日目标、达标判定、总结不受影响）。
//! - **均分窗口**立即生效（只影响未来的分摊计算）——只存最新值，不进版本。
//! - **日期例外**（DateOverride，双向覆盖周循环）按日期**立即生效**、全局覆盖——
//!   例外天然锚定具体日期（提前标注、将来生效，story 46/48），不进版本历史。
//!
//! "今日是否工作" = 周循环 + 日期例外合并（`Settings::is_workday_on` 纯判定，
//! 由 `SettingsCalendar::for_date` 给出该日的周循环配置后合并）。

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, Timelike};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::clock::Clock;
use crate::domain::plans::{db_err, PlanError};

/// 版本历史里"自从有设置以来一直如此"的种子生效日（load 兜底种子用，
/// 早于任何真实日期 → 未保存过时全部日期解析到该版本）。测试种子 force_settings 共用。
pub const SENTINEL_EFFECTIVE: &str = "1900-01-01";

/// "找下一个工作日"的查找视野（天）：触发排程与生效日计算共用；
/// 视野内无工作日（如全部工作日被取消）按退化配置兜底。
pub(crate) const DAY_HORIZON: i64 = 366;

/// 日内一段时间窗口，端点为当日分钟数（0 = 00:00）。
/// 跨午夜窗口以 end < start 表达，如 20:00–01:00 = (1200, 60)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeWindow {
    pub start_minute: u16,
    pub end_minute: u16,
}

/// 一条日期例外（DateOverride）：把某日期覆盖为工作日 / 休息日。
/// date 用 NaiveDate（chrono serde = "YYYY-MM-DD" 字符串，与前端 string 类型直通；
/// 非法日期在 command 边界即被拒，不会落成永不命中的死行）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DateOverride {
    /// 例外标注的日期（只能是将来：提前标注、将来生效，story 46/48）
    pub date: NaiveDate,
    /// true = 这天工作（调休补班）/ false = 这天不工作（假期）
    pub working: bool,
}

/// 设置快照。`load` 给出**最新保存值**（设置页展示；均分窗口即生效值）；
/// 按日期解析走 `SettingsCalendar::for_date`（三个延时字段按版本历史取）。
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
    /// 日期例外（双向覆盖周循环；全局生效，随每份快照带出）
    #[serde(default)]
    pub date_overrides: Vec<DateOverride>,
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
            date_overrides: Vec::new(),
        }
    }
}

impl Settings {
    /// 某日期是否工作日（纯判定）：日期例外命中优先，否则按周循环。
    /// 工时域按日回放共用（ledger 的均分窗口枚举）。
    pub fn is_workday_on(&self, date: NaiveDate) -> bool {
        if let Some(o) = self.date_overrides.iter().find(|o| o.date == date) {
            return o.working;
        }
        // chrono 周一 = 1 .. 周日 = 7，与 workdays 口径一致
        self.workdays.contains(&(date.weekday().number_from_monday() as u8))
    }
}

/// 版本历史一行（三个延时字段的快照 + 生效日）。
#[derive(Debug, Clone)]
struct SettingsVersion {
    daily_minutes: u32,
    workdays: Vec<u8>,
    time_windows: Vec<TimeWindow>,
    effective_from: NaiveDate,
}

/// 设置的**按日解析器**（SettingsEffectiveTime 的查询形态）：一次性装载版本历史
/// 与例外，`for_date` O(版本数) 解析——ledger 的逐日回放 / summary 的逐日触发
/// / is_work_time 共用，避免每天一次查询。
#[derive(Debug, Clone)]
pub struct SettingsCalendar {
    /// 最新保存值（均分窗口的生效来源；设置页展示同 `SettingsService::load`）
    pub(crate) latest: Settings,
    /// 版本历史，按 effective_from 升序（同日多版按写入序，后者胜）
    versions: Vec<SettingsVersion>,
}

impl SettingsCalendar {
    /// 指定日期的生效配置：三个延时字段取 effective_from <= date 的最新版本
    /// （无版本时回退默认——load 已保证至少有种子版本，此处仅防御），均分窗口
    /// 与日期例外取全局最新值。
    pub fn for_date(&self, date: NaiveDate) -> Settings {
        self.versions
            .iter()
            .rfind(|v| v.effective_from <= date)
            .map(|v| Settings {
                daily_minutes: v.daily_minutes,
                workdays: v.workdays.clone(),
                time_windows: v.time_windows.clone(),
                smoothing_workdays: self.latest.smoothing_workdays,
                date_overrides: self.latest.date_overrides.clone(),
            })
            .unwrap_or_else(|| {
                // load 已保证至少有种子版本，这里仅防御（无版本 = 默认值 + 全局最新值）
                Settings {
                    smoothing_workdays: self.latest.smoothing_workdays,
                    date_overrides: self.latest.date_overrides.clone(),
                    ..Settings::default()
                }
            })
    }
}

/// 设置读写服务。`load` 保证 FirstRun 默认值落库（INSERT OR IGNORE 幂等），
/// 并保证版本历史至少有一条种子版本（旧库升级/新库初始化各一次）。
pub struct SettingsService;

impl SettingsService {
    /// 今日是否工作日（按日解析的"现在"快捷方式）。
    /// 工时域共用判定（allocation 的面板 workday 标记 / should_auto_open）。
    pub fn is_workday(conn: &Connection, clock: &dyn Clock) -> rusqlite::Result<bool> {
        let today = clock.now().date_naive();
        Ok(Self::calendar(conn)?.for_date(today).is_workday_on(today))
    }

    /// 指定日期是否工作日（按该日期的生效配置解析——周循环 + 日期例外合并）。
    pub fn is_workday_on(conn: &Connection, date: NaiveDate) -> rusqlite::Result<bool> {
        Ok(Self::calendar(conn)?.for_date(date).is_workday_on(date))
    }

    /// 现在是否处于工作时间：工作日 && 当前时刻落在某段时间窗口内。
    /// 桌宠初始模式判定（启动序列完成后据此进工作/休息模式）与 13 的窗口触发共用。
    /// 窗口端点左闭右开（结束那一刻已不在窗内）。跨午夜窗口（end < start）按
    /// story 54 归属窗口开始日：今晚段（t >= start）按**今天**的配置判定，
    /// 凌晨段（t < end）按**昨天**的配置判定（窗口开始日拥有整段窗口）。
    pub fn is_work_time(conn: &Connection, clock: &dyn Clock) -> rusqlite::Result<bool> {
        let cal = Self::calendar(conn)?;
        let now = clock.now();
        let t = (now.time().hour() * 60 + now.time().minute()) as u16;
        let today = now.date_naive();
        let s = cal.for_date(today);
        // 今天的窗口：常规段或跨午夜的今晚段（常规段还须 t < end）
        let in_today = s.is_workday_on(today)
            && s.time_windows
                .iter()
                .any(|w| t >= w.start_minute && (t < w.end_minute || w.end_minute <= w.start_minute));
        if in_today {
            return Ok(true);
        }
        // 凌晨段：昨天配置里的跨午夜窗口尾巴（t < end）
        let s_prev = cal.for_date(today - Duration::days(1));
        Ok(s_prev.is_workday_on(today - Duration::days(1))
            && s_prev
                .time_windows
                .iter()
                .any(|w| w.end_minute <= w.start_minute && t < w.end_minute))
    }

    /// 下一个工作窗口**开始**的时刻（AutoOpenMainBoard 的"工作窗口开始时"触发，
    /// 工单 13）：从现在向前找第一个"某工作日上尚未开始的一段窗口的开始时刻"。
    /// 多段窗口取当日最早的未来段（早段已开始不掩掉晚段）；跨午夜窗口的开始在
    /// 当日晚间（凌晨尾巴不算新开始）；None = 未配置窗口或一年内没有工作日（永不触发）。
    pub fn next_window_start(
        conn: &Connection,
        clock: &dyn Clock,
    ) -> Result<Option<DateTime<Local>>, PlanError> {
        let cal = Self::calendar(conn).map_err(db_err)?;
        let now = clock.now();
        for offset in 0..=DAY_HORIZON {
            let day = now.date_naive() + Duration::days(offset);
            let s = cal.for_date(day);
            if !s.is_workday_on(day) || s.time_windows.is_empty() {
                continue;
            }
            let midnight = midnight_of(day);
            let upcoming = s
                .time_windows
                .iter()
                .map(|w| midnight + Duration::minutes(i64::from(w.start_minute)))
                .filter(|m| *m > now)
                .min();
            if let Some(start) = upcoming {
                return Ok(Some(start));
            }
        }
        Ok(None)
    }

    /// 读取当前设置（最新保存值）；库中无行时先写入默认值，版本历史为空时
    /// 以当前行值落一条"自从有设置以来"的种子版本（新库初始化与旧库升级共用）。
    pub fn load(conn: &Connection) -> rusqlite::Result<Settings> {
        let d = Settings::default();
        conn.execute(
            "INSERT OR IGNORE INTO settings (id, daily_minutes, workdays, time_windows, smoothing_workdays)
             VALUES (1, ?1, ?2, ?3, ?4)",
            params![d.daily_minutes, json(&d.workdays), json(&d.time_windows), d.smoothing_workdays],
        )?;
        let seeded: i64 = conn.query_row("SELECT COUNT(*) FROM settings_versions", [], |r| r.get(0))?;
        if seeded == 0 {
            // 种子版本取当前行值：未保存过 = 默认值"一直如此"；旧开发库升级 = 已有值"一直如此"
            let (daily, workdays, windows): (u32, String, String) = conn.query_row(
                "SELECT daily_minutes, workdays, time_windows FROM settings WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            conn.execute(
                "INSERT INTO settings_versions (daily_minutes, workdays, time_windows, effective_from, created_at)
                 VALUES (?1, ?2, ?3, ?4, '')",
                params![daily, workdays, windows, SENTINEL_EFFECTIVE],
            )?;
        }
        let date_overrides = load_overrides(conn)?;
        conn.query_row(
            "SELECT daily_minutes, workdays, time_windows, smoothing_workdays FROM settings WHERE id = 1",
            [],
            |row| {
                Ok(Settings {
                    daily_minutes: row.get(0)?,
                    workdays: serde_json::from_str(&row.get::<_, String>(1)?).unwrap(),
                    time_windows: serde_json::from_str(&row.get::<_, String>(2)?).unwrap(),
                    smoothing_workdays: row.get(3)?,
                    date_overrides: date_overrides.clone(),
                })
            },
        )
    }

    /// 按日解析器（一次性装载；ledger 回放 / summary 触发 / is_work_time 共用）。
    pub fn calendar(conn: &Connection) -> rusqlite::Result<SettingsCalendar> {
        let latest = Self::load(conn)?;
        let mut stmt = conn.prepare(
            "SELECT daily_minutes, workdays, time_windows, effective_from
             FROM settings_versions ORDER BY effective_from ASC, id ASC",
        )?;
        let versions = stmt
            .query_map([], |row| {
                Ok(SettingsVersion {
                    daily_minutes: row.get(0)?,
                    workdays: serde_json::from_str(&row.get::<_, String>(1)?).unwrap(),
                    time_windows: serde_json::from_str(&row.get::<_, String>(2)?).unwrap(),
                    effective_from: NaiveDate::parse_from_str(&row.get::<_, String>(3)?, "%Y-%m-%d")
                        .unwrap(),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(SettingsCalendar { latest, versions })
    }

    /// 覆盖保存整份设置（updated_at 取注入时钟），返回三个延时字段的生效日
    /// （设置页"自 X 起生效"提示用；无延时变更 = 今天即生效）。
    /// 均分窗口与日期例外随保存立即生效；延时字段相对**今天生效的配置**有变化时
    /// 追加版本，effective_from = 按保存前配置"今天之后的第一个工作日"（CONTEXT
    /// SettingsEffectiveTime——当日维持原值；按保存前配置找，保证"下一个工作日"
    /// 与用户当前所历的日历一致）。
    pub fn save(
        conn: &Connection,
        clock: &dyn Clock,
        s: &Settings,
    ) -> Result<NaiveDate, PlanError> {
        let today = clock.now().date_naive();
        validate(s, today)?;
        // 时间窗口归一化（保存时合并重叠/首尾相接的段——所见即所存的是合并结果）
        let windows = merge_time_windows(&s.time_windows)?;
        let cal = Self::calendar(conn).map_err(db_err)?;
        let current = cal.for_date(today);
        let delayed_changed = s.daily_minutes != current.daily_minutes
            || s.workdays != current.workdays
            || s.time_windows != current.time_windows;
        let effective_from = if delayed_changed {
            (1..=DAY_HORIZON)
                .map(|i| today + Duration::days(i))
                .find(|&d| current.is_workday_on(d))
                .unwrap_or(today + Duration::days(1)) // 配置退化成"永无工作日"：明天生效兜底
        } else {
            today
        };
        // 三条写（设置行 / 版本 / 例外替换）一个事务：中途失败不留下半份保存
        let tx = conn.unchecked_transaction().map_err(db_err)?;
        tx.execute(
            "UPDATE settings SET daily_minutes = ?1, workdays = ?2, time_windows = ?3,
             smoothing_workdays = ?4, updated_at = ?5 WHERE id = 1",
            params![
                s.daily_minutes,
                json(&s.workdays),
                json(&windows),
                s.smoothing_workdays,
                clock.now().to_rfc3339(),
            ],
        )
        .map_err(db_err)?;
        if delayed_changed {
            tx.execute(
                "INSERT INTO settings_versions (daily_minutes, workdays, time_windows, effective_from, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    s.daily_minutes,
                    json(&s.workdays),
                    json(&windows),
                    effective_from.to_string(),
                    clock.now().to_rfc3339(),
                ],
            )
            .map_err(db_err)?;
        }
        replace_overrides(&tx, &s.date_overrides).map_err(db_err)?;
        tx.commit().map_err(db_err)?;
        Ok(effective_from)
    }

    /// 设置最近一次保存时刻（尚未保存过时为 None）。
    pub fn updated_at(conn: &Connection) -> rusqlite::Result<Option<DateTime<Local>>> {
        let raw: String =
            conn.query_row("SELECT updated_at FROM settings WHERE id = 1", [], |row| {
                row.get(0)
            })?;
        Ok(if raw.is_empty() {
            None
        } else {
            Some(
                DateTime::parse_from_rfc3339(&raw)
                    .unwrap()
                    .with_timezone(&Local),
            )
        })
    }
}

/// 保存前的领域校验（权威在服务层，UI 只做即时反馈）：
/// 时间窗口起止不得相等（end == start 会让 is_work_time 的跨午夜分支把窗口
/// 变成"start 之后永远在窗"）、均分窗口至少 1 个工作日、每日工作时间至少 1 分钟、
/// 日期例外只能标注将来（提前标注、将来生效——不提供对当日与历史的追溯改写，
/// story 46"而不是事后清账"）。
fn validate(s: &Settings, today: NaiveDate) -> Result<(), PlanError> {
    if let Some(w) = s.time_windows.iter().find(|w| w.start_minute == w.end_minute) {
        return Err(PlanError::InvalidSettings {
            reason: format!(
                "时间窗口 {}:{} 起止相同",
                w.start_minute / 60,
                w.start_minute % 60
            ),
        });
    }
    if s.smoothing_workdays < 1 {
        return Err(PlanError::InvalidSettings {
            reason: "均分窗口至少 1 个工作日".into(),
        });
    }
    if s.daily_minutes < 1 {
        return Err(PlanError::InvalidSettings {
            reason: "每日工作时间至少 1 分钟".into(),
        });
    }
    if let Some(o) = s.date_overrides.iter().find(|o| o.date <= today) {
        return Err(PlanError::InvalidSettings {
            reason: format!("日期例外 {} 只能标注将来的日期", o.date),
        });
    }
    Ok(())
}

/// 时间窗口归一化：重叠或**首尾相接**（端点左闭右开语义下的连续段，2026-08-30 用户
/// 决策）合并为一段，按 start 升序返回。跨午夜窗口先拆成 [0,1440) 内的线性段参与
/// 合并、再按"穿过午夜"重新组装；并集覆盖全天时返回 Err——TimeWindow 无法表达
/// start==end 的全天窗口（用户决策：拒绝保存并提示，不改领域语义）。
pub(crate) fn merge_time_windows(windows: &[TimeWindow]) -> Result<Vec<TimeWindow>, PlanError> {
    // 1) 展开为线性段：常规窗口一段；跨午夜拆"当晚 + 凌晨"两段（end == 0 的凌晨段为空，丢弃）
    let mut segs: Vec<(u32, u32)> = Vec::new();
    for w in windows {
        let (s, e) = (u32::from(w.start_minute), u32::from(w.end_minute));
        if s < e {
            segs.push((s, e));
        } else if s > e {
            segs.push((s, 1440));
            if e > 0 {
                segs.push((0, e));
            }
        }
        // s == e 不会到这里：validate 先按行拒绝（即使漏进来了，下方也会以全天覆盖拒绝）
    }
    // 2) 线性合并：排序后相邻相接（next.start <= cur.end）即并
    segs.sort_unstable();
    let mut merged: Vec<(u32, u32)> = Vec::new();
    for &(s, e) in &segs {
        match merged.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => merged.push((s, e)),
        }
    }
    if merged.len() == 1 && merged[0] == (0, 1440) {
        return Err(PlanError::InvalidSettings {
            reason: "时间窗口并集不能覆盖全天".into(),
        });
    }
    // 3) 组装回窗口：线性合并不会连接"末段到午夜 + 首段从零点"的跨午夜相接，
    //    先按段还原（尾段 1440 = 次日零点 → end 记 0），再补一次首尾穿午夜合并
    let mut out: Vec<TimeWindow> = merged
        .iter()
        .map(|&(s, e)| TimeWindow {
            start_minute: s as u16,
            end_minute: (e % 1440) as u16,
        })
        .collect();
    if out.len() >= 2 {
        let last = out.last().unwrap();
        let first = out.first().unwrap();
        if last.end_minute == 0 && first.start_minute == 0 {
            if first.end_minute >= last.start_minute {
                // 首尾拼起来覆盖全天（含恰好首尾相接的整圈）
                return Err(PlanError::InvalidSettings {
                    reason: "时间窗口并集不能覆盖全天".into(),
                });
            }
            let joined = TimeWindow {
                start_minute: last.start_minute,
                end_minute: first.end_minute,
            };
            out.pop();
            out[0] = joined;
        }
    }
    out.sort_by_key(|w| w.start_minute);
    Ok(out)
}

/// 某日零点（本地时区）。窗口时刻的拼装共用（next_window_start / summary 触发）。
pub(crate) fn midnight_of(day: NaiveDate) -> DateTime<Local> {
    day.and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(Local)
        .unwrap()
}

/// 日期例外全量读取（按日期升序；date 列为自写的 YYYY-MM-DD，解析失败即库损坏）。
fn load_overrides(conn: &Connection) -> rusqlite::Result<Vec<DateOverride>> {
    let mut stmt = conn.prepare("SELECT date, working FROM date_overrides ORDER BY date ASC")?;
    let rows = stmt
        .query_map([], |row| {
            Ok(DateOverride {
                date: NaiveDate::parse_from_str(&row.get::<_, String>(0)?, "%Y-%m-%d").unwrap(),
                working: row.get::<_, i64>(1)? != 0,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// 日期例外全量替换（保存以提交快照为准——所见即所存，同依赖边的替换口径）。
/// 事务内执行（save 传 Transaction，内部 Deref 到 Connection）。
fn replace_overrides(conn: &Connection, overrides: &[DateOverride]) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM date_overrides", [])?;
    for o in overrides {
        conn.execute(
            "INSERT OR IGNORE INTO date_overrides (date, working) VALUES (?1, ?2)",
            params![o.date.to_string(), i64::from(o.working)],
        )?;
    }
    Ok(())
}

/// JSON 列序列化（Vec<u8> / Vec<TimeWindow> 都是 infallible，直接 unwrap）
fn json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap()
}
