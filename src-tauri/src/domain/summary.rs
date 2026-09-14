//! 今日总结领域（工单 11；ADR-0009 单一事实源；术语 DailySummaryTrigger / DailySummaryLayout）。
//! 总结内容完全从 ProgressLog **实时派生**——弹出后新进展不重弹、控制面板可随时调出看
//! 最新版、次日补登的"昨日总结"同理由此免费获得（不固化任何总结表）。
//!
//! 触发口径（CONTEXT DailySummaryTrigger）：
//! - **最晚**一个工作时间窗口结束那一刻自动弹出（多段窗口取最晚段；跨午夜窗口的结束
//!   落在次日，触发时刻同样按"D 零点 + end 分钟"计算）。
//! - 当天没开应用（触发时刻已过而未登记）→ 次日第一次打开时补登，标题"昨日总结"。
//!   只回看**今天与昨天**两天：更早的日子不再补登——总结的意义是"对这一天的账"，
//!   隔夜的旧账翻出来只添噪声。
//! - **只弹一次**：弹出即登记（summary_shown 单表），此后新进展不重弹。
//! - 事件归属日沿用"工作窗口开始日"口径（ADR-0009，`progress::attributed_date`）。

use std::collections::HashMap;

use chrono::{DateTime, Duration, Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::clock::Clock;
use crate::domain::ledger::{day_met, LedgerService};
use crate::domain::plans::{
    db_err, PlanError, PlanService, PlanStatus, PauseReason, Priority, SubGoalView,
};
use crate::domain::progress::{attributed_date_on, round_delta_minutes, round_to_one_decimal};
use crate::domain::settings::{midnight_of, DAY_HORIZON, Settings, SettingsService};

/// 净推进有效阈值（分钟）：低于它视为当日零推进（不展示）。写入端已 round 到 1e-9 网格，
/// 求和残留至多 1e-6 量级；合法最小推进（0.1% × 任意耗时）远高于它。
const MINUTES_EPS: f64 = 1e-6;

/// 总结中一个推进过的任务行（DailySummaryLayout 第二层）。
#[derive(Debug, Serialize)]
pub struct SummaryTask {
    pub task_id: i64,
    pub name: String,
    /// 派生总进度百分比（0–100，一位小数）——展示口径，非当日增量
    pub percent: f64,
    /// 当日净推进分钟（撤销/下调修正后的净额）
    pub minutes: f64,
    pub has_subgoals: bool,
    /// 子目标快照（按填写顺序）：前端渲染"✓ 完成列表 / 进行中"（实时重算的当前态）
    pub subgoals: Vec<SubGoalView>,
}

/// 总结中一个推进过的计划的 section（DailySummaryLayout 第一层）。
#[derive(Debug, Serialize)]
pub struct SummaryPlan {
    pub plan_id: i64,
    pub plan_name: String,
    pub priority: Priority,
    /// 已被抢占暂停标注（Paused + AutoPreempted）：照常展示、推进计入总量与达标判定
    pub preempted: bool,
    /// 当日净推进总耗时（含已删除任务的推进——干了的活就是干了，ADR-0009）
    pub minutes: f64,
    /// 当日有推进的任务行（保持任务 position 顺序）
    pub tasks: Vec<SummaryTask>,
}

/// 指定日期总结的一次装配（DailySummaryLayout）。
#[derive(Debug, Serialize)]
pub struct DailySummaryView {
    /// 总结归属日（YYYY-MM-DD）——跨午夜窗口内的事件归属窗口开始日
    pub date: String,
    /// 归属日 = 今天（标题"今日总结"）
    pub is_today: bool,
    /// 归属日 = 昨天（标题"昨日总结"，补登）
    pub is_yesterday: bool,
    /// 归属日是否工作日（false = 休息日加班态：无目标义务，推进按超额并入账户）
    pub workday: bool,
    /// 完成总量（分钟，当日全部事件净额——与工时账户的达标判定同口径）
    pub total_minutes: f64,
    /// 目标（分钟，f64）：归属日的调整后目标（含结转，`LedgerService::day_target_on`）
    pub target_minutes: f64,
    /// 基准 = 每日工作时间（分钟）：与 target 的差额即结转，carryLabel 标注用
    pub base_minutes: u32,
    /// 当日是否达标（±10% 容差带下沿，基数 = 调整后目标；休息日加班态恒 false——
    /// 无目标义务）：工单 20 failed 显示的判定源，前端不再自推容差
    pub met_target: bool,
    /// 有更高优先级计划未开始提示：存在 NotStarted 计划，优先级**严格高于**今日推进过
    /// 的计划中的最高优先级（今天什么都没推时，任何 NotStarted 计划都算）
    pub higher_priority_hint: bool,
    /// 推进过的计划（保持 PlanOrdering：优先级降序 + 创建倒序）；当日零推进的不展示
    pub plans: Vec<SummaryPlan>,
}

/// 触发状态查询结果：一个 command 喂三处（启动补登检查 / 前端定时器 / 控制面板调出入口）。
#[derive(Debug, Serialize)]
pub struct DailySummaryStatus {
    /// 该弹而未弹的总结日期（YYYY-MM-DD）——前端据此立即弹出
    pub due: Option<String>,
    /// 最近一次已弹出的总结日期（控制面板"随时调出"的默认日期）
    pub last_shown: Option<String>,
    /// 下一次自动触发时刻（最晚窗口结束；None = 未配置窗口，永不自动触发）
    pub next_fire_at: Option<DateTime<Local>>,
}

/// 今日总结服务：触发判定（due / next_fire / mark_shown）+ 视图装配（summary）。
pub struct SummaryService;

impl SummaryService {
    /// DailySummaryTrigger 判定：现在是否到了"该弹而未弹"的总结时刻。
    /// 只看今天与昨天：今天的最晚窗口结束已过而未登记 → 补弹"今日总结"（当天没开着
    /// 应用的场景）；否则昨天已过而未登记 → 补登"昨日总结"（次日首开）。
    /// 逐日按该日期的生效配置取窗口（工单 13：设置变更不追溯改写当日/历史触发点）。
    pub fn due(conn: &Connection, clock: &dyn Clock) -> Result<Option<NaiveDate>, PlanError> {
        let cal = SettingsService::calendar(conn).map_err(db_err)?;
        let now = clock.now();
        for offset in [0, 1] {
            let day = now.date_naive() - Duration::days(offset);
            if let Some(fire) = latest_window_end(&cal.for_date(day), day) {
                if fire <= now && !Self::is_shown(conn, day)? {
                    return Ok(Some(day));
                }
            }
        }
        Ok(None)
    }

    /// 下一次自动触发的时刻：从今天向前找第一个"最晚窗口结束 > 现在"的工作日。
    /// 前端定时器据此排程（触发后重查重排，设置保存后经 settings:changed 事件重排）。
    pub fn next_fire(
        conn: &Connection,
        clock: &dyn Clock,
    ) -> Result<Option<DateTime<Local>>, PlanError> {
        let cal = SettingsService::calendar(conn).map_err(db_err)?;
        let now = clock.now();
        let mut day = now.date_naive();
        for _ in 0..DAY_HORIZON {
            if let Some(fire) = latest_window_end(&cal.for_date(day), day) {
                if fire > now {
                    return Ok(Some(fire));
                }
            }
            day += Duration::days(1);
        }
        Ok(None)
    }

    /// 状态一次查全（due + last_shown + next_fire_at）。
    pub fn status(conn: &Connection, clock: &dyn Clock) -> Result<DailySummaryStatus, PlanError> {
        Ok(DailySummaryStatus {
            due: Self::due(conn, clock)?.map(|d| d.to_string()),
            last_shown: last_shown(conn)?,
            next_fire_at: Self::next_fire(conn, clock)?,
        })
    }

    /// 登记某日的总结已弹出（只弹一次的账；幂等——控制面板补看待弹总结后同样登记）。
    pub fn mark_shown(
        conn: &Connection,
        clock: &dyn Clock,
        date: NaiveDate,
    ) -> Result<(), PlanError> {
        conn.execute(
            "INSERT INTO summary_shown (date, shown_at) VALUES (?1, ?2)
             ON CONFLICT(date) DO UPDATE SET shown_at = excluded.shown_at",
            params![date.to_string(), clock.now().to_rfc3339()],
        )
        .map_err(db_err)?;
        Ok(())
    }

    /// 装配指定日期的总结视图。每次调用都从 ProgressLog 重算（ADR-0009）：
    /// 弹出后继续推进、控制面板调出、次日补登，读到的都是最新账。
    pub fn summary(
        conn: &Connection,
        clock: &dyn Clock,
        date: NaiveDate,
    ) -> Result<DailySummaryView, PlanError> {
        let cal = SettingsService::calendar(conn).map_err(db_err)?;
        // 当日净推进按任务聚合（JOIN 不过滤 deleted_at：已删任务的推进仍是真实工时，
        // 计入计划与总量，只是渲染不出任务行——归档不是抹账）
        let mut by_task: HashMap<i64, f64> = HashMap::new();
        let mut plan_of_task: HashMap<i64, i64> = HashMap::new();
        let mut stmt = conn
            .prepare(
                "SELECT l.task_id, l.at, l.delta_minutes, t.plan_id
                 FROM progress_log l JOIN tasks t ON t.id = l.task_id",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, f64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(db_err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db_err)?;
        for (task_id, at, delta, plan_id) in rows {
            let Ok(at) = DateTime::parse_from_rfc3339(&at) else {
                continue;
            };
            if attributed_date_on(at.with_timezone(&Local), &cal) != date {
                continue;
            }
            *by_task.entry(task_id).or_default() += delta;
            plan_of_task.insert(task_id, plan_id);
        }
        // 计划级净额（含未展示的负净额任务——撤销重算后计划总量要如实）
        let mut by_plan: HashMap<i64, f64> = HashMap::new();
        for (task_id, delta) in &by_task {
            *by_plan.entry(plan_of_task[task_id]).or_default() += delta;
        }
        // PlanService::list 保持 PlanOrdering（优先级降序 + 创建倒序）→ 总结顺序一致
        let all_plans = PlanService::list(conn)?;
        let plans: Vec<SummaryPlan> = all_plans
            .iter()
            .filter(|p| by_plan.get(&p.id).copied().unwrap_or(0.0) > MINUTES_EPS)
            .map(|p| SummaryPlan {
                plan_id: p.id,
                plan_name: p.name.clone(),
                priority: p.priority,
                preempted: p.status == PlanStatus::Paused
                    && p.pause_reason == Some(PauseReason::AutoPreempted),
                minutes: round_delta_minutes(by_plan[&p.id]),
                tasks: p
                    .tasks
                    .iter()
                    .filter_map(|t| {
                        let minutes = by_task.get(&t.id).copied().unwrap_or(0.0);
                        (minutes > MINUTES_EPS).then(|| SummaryTask {
                            task_id: t.id,
                            name: t.name.clone(),
                            percent: round_to_one_decimal(t.progress_percent),
                            minutes: round_delta_minutes(minutes),
                            has_subgoals: t.has_subgoals,
                            subgoals: t.subgoals.clone(),
                        })
                    })
                    .collect(),
            })
            .collect();
        let total = round_delta_minutes(by_task.values().sum());
        // 有更高优先级计划未开始：NotStarted 且优先级严格高于今日推进过的最高优先级
        let max_worked = plans.iter().map(|p| p.priority).max();
        let higher_priority_hint = all_plans.iter().any(|p| {
            p.status == PlanStatus::NotStarted
                && max_worked.map_or(true, |m| p.priority > m)
        });
        let target = LedgerService::day_target_on(conn, date)?;
        let today = clock.now().date_naive();
        // 归属日按其生效配置解析（周循环 + 日期例外；工单 13）；休息日加班态无目标
        // 义务——met_target 恒 false（与 workday 同源判定）
        let workday = cal.for_date(date).is_workday_on(date);
        Ok(DailySummaryView {
            date: date.to_string(),
            is_today: date == today,
            is_yesterday: date == today - Duration::days(1),
            workday,
            total_minutes: total,
            target_minutes: target.target_minutes,
            base_minutes: target.base_minutes,
            met_target: workday && day_met(total, target.target_minutes),
            higher_priority_hint,
            plans,
        })
    }

    /// 某日是否已登记弹出（只弹一次的判据）。
    fn is_shown(conn: &Connection, date: NaiveDate) -> Result<bool, PlanError> {
        let hit: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM summary_shown WHERE date = ?1",
                params![date.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_err)?;
        Ok(hit.is_some())
    }
}

/// YYYY-MM-DD 边界解析（command 边界的类型转换，错误走 PlanError 通道）。
pub fn parse_date(s: &str) -> Result<NaiveDate, PlanError> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| PlanError::InvalidDate)
}

/// 工作日 D 的触发时刻：当日**最晚**工作窗口的结束时刻（多段窗口取 max）。跨午夜窗口
/// （end < start，如 20:00–01:00）的结束落在**次日**，偏移 = 1440 + end 分钟；非工作日或
/// 未配置窗口 → None（无触发点，永不自动弹出）。settings 为该日期的生效配置（调用方
/// 按日解析后传入，工单 13）。
fn latest_window_end(settings: &Settings, date: NaiveDate) -> Option<DateTime<Local>> {
    if !settings.is_workday_on(date) || settings.time_windows.is_empty() {
        return None;
    }
    let midnight = midnight_of(date);
    settings
        .time_windows
        .iter()
        .map(|w| {
            let offset = if w.end_minute < w.start_minute {
                1440 + i64::from(w.end_minute) // 跨午夜：结束在次日 end 分钟处
            } else {
                i64::from(w.end_minute)
            };
            midnight + Duration::minutes(offset)
        })
        .max()
}

/// 最近一次已弹出的总结日期（无任何登记 → None）。
fn last_shown(conn: &Connection) -> Result<Option<String>, PlanError> {
    conn.query_row(
        "SELECT date FROM summary_shown ORDER BY date DESC LIMIT 1",
        [],
        |row| row.get(0),
    )
    .optional()
    .map_err(db_err)
}
