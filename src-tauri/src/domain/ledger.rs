//! 工时账户结算领域（工单 10；ADR-0007 对称均分、ADR-0009 单一事实源；
//! 术语 WorkHourLedger / DailyCompletionTolerance / LoadSmoothing）。
//! 每个工作日的当日实际目标 = 基准 + Σ(历史差额均分到今天的份额)，全部从
//! ProgressLog + 设置**实时派生**、不固化任何汇总——历史修正后未来目标自动重算。
//!
//! 规则口径（CONTEXT 工时账户 + ADR-0007）：
//! - **±10% 达标容差带以调整后目标为基数**，带内差额豁免不均分；带对称作用于两个方向
//!   （差几分钟的未达标不算失败，多干几分钟的超额也不入账）。
//! - 带外差额 = 目标 - 实际（正 = 欠债上调后续，负 = 超额下调后续，对称抵扣）。
//! - **只有实际完成的差额进入账户**：结算只读 ProgressLog，选择层面（今日分配）的超额不计。
//! - 均分窗口以**工作日**为单位：非工作日不消费窗口、不接收结转，其推进全额按超额并入。
//! - 账户历史锚定在**最早一条进度事件**的归属日：此前的日子无从谈起，此后的工作日
//!   整日未推进也欠全额（没开应用不豁免——欠的债跟着走，ADR-0007 否决窗口顺延）。

use std::collections::{BTreeMap, VecDeque};

use chrono::{DateTime, Duration, Local, NaiveDate};
use rusqlite::Connection;
use serde::Serialize;

use crate::clock::Clock;
use crate::domain::plans::{db_err, PlanError};
use crate::domain::progress::{attributed_date, round_delta_minutes};
use crate::domain::settings::{SettingsService, TimeWindow};

/// ±10% 达标容差（DailyCompletionTolerance），带内差额豁免；浮点边界比较容差 1e-9。
const TOLERANCE: f64 = 0.10;
const EPS: f64 = 1e-9;

/// 某日的目标口径：基准 + 结转（大面板状态条 / 小看板微型条 / 11 的今日总结共用）。
#[derive(Debug, Serialize)]
pub struct DayTarget {
    /// 基准 = 设置的每日工作时间（分钟）
    pub base_minutes: u32,
    /// 调整后目标（分钟，f64：结转份额是除法结果）。非工作日无目标义务，恒等于基准。
    pub target_minutes: f64,
}

/// 工时账户结算服务。
pub struct LedgerService;

impl LedgerService {
    /// 求注入时钟"今天"的调整后目标：从账户锚定日起逐日回放到昨天（今天的差额
    /// 要到今天结束才发生），落在今天窗口位上的历史份额求和。任何历史修正后
    /// 再次调用即得到重算值——日志是唯一真相（ADR-0009）。
    pub fn day_target(conn: &Connection, clock: &dyn Clock) -> Result<DayTarget, PlanError> {
        let settings = SettingsService::load(conn).map_err(db_err)?;
        let base = settings.daily_minutes as f64;
        let today = clock.now().date_naive();
        let minutes = minutes_by_day(conn, &settings.time_windows)?;
        let Some(anchor) = minutes.keys().next().copied() else {
            return Ok(base_target(settings.daily_minutes)); // 无任何进度事件：账户无历史
        };
        let window = settings.smoothing_workdays.max(1) as usize;
        // 滚动窗口（定长 window）：slots[i] = 第 i+1 个未来工作日将接收的结转份额。
        // 每过一个工作日队首出队、尾部补 0；一笔差额入账时给全部 W 位各加 debt/W
        //（= 均分到后续 W 个工作日），O(天数) 完成回放。
        let mut slots: VecDeque<f64> = vec![0.0; window].into();
        let mut day = anchor;
        while day < today {
            let actual = minutes.get(&day).copied().unwrap_or(0.0);
            let debt = if settings.is_workday_on(day) {
                let landing = slots.pop_front().unwrap_or(0.0); // 工作日消费一个窗口位
                slots.push_back(0.0);
                day_debt(actual, base + landing)
            } else {
                -actual // 非工作日：无目标义务、不消费窗口；有推进全额按超额并入（负 = 抵扣）
            };
            if debt != 0.0 {
                let share = debt / window as f64;
                slots.iter_mut().for_each(|s| *s += share);
            }
            day += Duration::days(1);
        }
        // 今天：工作日接收窗口内份额；非工作日无义务（份额留给下一个工作日）
        let carry = if settings.is_workday_on(today) {
            slots.front().copied().unwrap_or(0.0)
        } else {
            0.0
        };
        Ok(DayTarget {
            base_minutes: settings.daily_minutes,
            target_minutes: round_delta_minutes(base + carry),
        })
    }
}

/// 某日的差额（正 = 欠债 → 上调后续；负 = 超额 → 下调后续）。
/// 容差带 [0.9T, 1.1T] 内豁免为 0（带基数 T 是**调整后**目标）。
fn day_debt(actual: f64, target: f64) -> f64 {
    if actual >= target * (1.0 - TOLERANCE) - EPS && actual <= target * (1.0 + TOLERANCE) + EPS {
        0.0
    } else {
        target - actual
    }
}

/// 全量扫描进度日志，按事件归属日（ADR-0009 跨午夜口径）聚合每日完成分钟数。
/// 结算回放与 progress 域的"今日完成量"共用同一聚合；日志量级每日几十条，不建汇总表。
/// 窗口由调用方传入（与调用方自身的设置读取合并，避免重复查询）。
pub(crate) fn minutes_by_day(
    conn: &Connection,
    windows: &[TimeWindow],
) -> Result<BTreeMap<NaiveDate, f64>, PlanError> {
    let mut stmt = conn
        .prepare("SELECT at, delta_minutes FROM progress_log")
        .map_err(db_err)?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?)))
        .map_err(db_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_err)?;
    let mut by_day: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    for (raw, delta) in rows {
        if let Ok(at) = DateTime::parse_from_rfc3339(&raw) {
            *by_day
                .entry(attributed_date(at.with_timezone(&Local), windows))
                .or_default() += delta;
        }
    }
    Ok(by_day)
}

/// 无历史时的目标（基准即目标，结转为 0）。
fn base_target(base_minutes: u32) -> DayTarget {
    DayTarget { base_minutes, target_minutes: base_minutes as f64 }
}
