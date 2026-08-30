//! 进度汇报领域（工单 07；ADR-0002 进度=耗时完成度、ADR-0009 ProgressLog 单一事实源；
//! 术语 CurrentTask / SwitchCurrentTask / ProgressGranularity / PercentAdjustControl / SubGoalUndo）。
//! 每次汇报向 progress_log 追加一条事件（任务、时刻、增量分钟、来源），撤销与下调修正
//! 以负增量补偿——账本只追加；任务进度、今日完成量全部实时派生，不固化汇总。
//! 事件归属日按"工作窗口开始日"口径（跨午夜窗口的凌晨段归前一日）。

use chrono::{DateTime, Duration, Local, NaiveDate, Timelike};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::clock::Clock;
use crate::domain::allocation::AllocationService;
use crate::domain::ledger::{minutes_by_day, LedgerService};
use crate::domain::lifecycle::LifecycleService;
use crate::domain::plans::{
    db_err, load_subgoals, notfound_or_db, PlanError, PlanService, PlanStatus, Priority,
    SubGoalView, TaskStatus,
};
use crate::domain::settings::{SettingsService, TimeWindow};

/// 汇报来源（ProgressLog.source 列；二态往返口径同其他枚举）。
#[derive(Debug, Clone, Copy)]
pub enum ProgressSource {
    /// 按序勾选子目标（+ 分钟）
    SubGoal,
    /// 撤销已完成子目标（- 分钟，补偿账）
    SubGoalUndo,
    /// 无子目标任务百分比增量汇报（+ 分钟）
    Percent,
    /// 任务详情"修正总进度"（直接设定当前值，增量可正可负）
    Correction,
}

impl ProgressSource {
    fn as_db(self) -> &'static str {
        match self {
            ProgressSource::SubGoal => "SubGoal",
            ProgressSource::SubGoalUndo => "SubGoalUndo",
            ProgressSource::Percent => "Percent",
            ProgressSource::Correction => "Correction",
        }
    }
}

/// 小看板一次装配的完整视图。
#[derive(Debug, Serialize)]
pub struct MiniBoardView {
    /// 当前任务（None = 空态：未指定 / 失效回空；Completed = "任务完成"停留态）
    pub current: Option<CurrentTaskView>,
    /// 今日完成量（分钟，事件按工作窗口开始日归属后求和）
    pub today_minutes: f64,
    /// 当日实际目标（分钟，f64）：基准 + 工时账户结转（工单 10，LedgerService 派生）
    pub target_minutes: f64,
    /// 基准 = 每日工作时间（分钟）：与 target 的差额即结转，微型条透明标注用
    pub base_minutes: u32,
    /// 今日是否工作日（false = 休息日加班态：无目标义务，推进按超额并入账户）
    pub workday: bool,
    /// 「更换任务」候选：今日推进列表分组（当前任务同计划排最前，组内按顺序）
    pub pickers: Vec<PickerGroup>,
}

/// 当前任务的展示视图（ CONTEXT CurrentTask）。
#[derive(Debug, Serialize)]
pub struct CurrentTaskView {
    /// 任务 id（汇报动作的目标）
    pub task_id: i64,
    /// 所属计划 id（更换候选"同计划排前"的锚点）
    pub plan_id: i64,
    /// 所属计划名（头部展示）
    pub plan_name: String,
    /// 任务名（主标题）
    pub task_name: String,
    /// 任务状态（Completed = "任务完成"停留态）
    pub status: TaskStatus,
    /// 是否有子目标（决定小看板走勾选还是百分比控件）
    pub has_subgoals: bool,
    /// 派生进度百分比（0–100）
    pub percent: f64,
    /// 已完成分钟（耗时口径，ADR-0002）
    pub completed_minutes: f64,
    /// 总分钟
    pub total_minutes: u32,
    /// 全部子目标（按填写顺序；前端渲染已完成可撤销行 + 当前待完成行）
    pub subgoals: Vec<SubGoalView>,
}

/// 更换任务候选的一个计划分组。
#[derive(Debug, Serialize)]
pub struct PickerGroup {
    /// 计划 id
    pub plan_id: i64,
    /// 计划名（分组标题）
    pub plan_name: String,
    /// 计划优先级（分组标题标签）
    pub priority: Priority,
    /// 组内候选（按任务 position = 依赖归一化后的顺序）
    pub tasks: Vec<PickerTask>,
}

/// 分组内一条候选任务。
#[derive(Debug, Serialize)]
pub struct PickerTask {
    /// 任务 id
    pub id: i64,
    /// 任务名
    pub name: String,
}

/// 汇报读写服务。
pub struct ProgressService;

impl ProgressService {
    /// 装配小看板视图：当前任务（失效即 None）+ 今日完成量 + 调整后目标（工单 10 含结转）
    /// + 工作日标记（休息日加班态）+ 更换候选。
    pub fn board(conn: &Connection, clock: &dyn Clock) -> Result<MiniBoardView, PlanError> {
        let current = Self::current_view(conn, clock)?;
        let target = LedgerService::day_target(conn, clock)?;
        Ok(MiniBoardView {
            pickers: Self::picker_groups(conn, clock, current.as_ref().map(|c| c.plan_id))?,
            today_minutes: Self::day_minutes(conn, clock)?,
            target_minutes: target.target_minutes,
            base_minutes: target.base_minutes,
            workday: SettingsService::is_workday(conn, clock).map_err(db_err)?,
            current,
        })
    }

    /// 指定当前任务（SwitchCurrentTask）：必须落在今日推进列表内，且任务可推进
    /// （计划进行中、未完成）。覆盖式写入单行表。
    pub fn set_current_task(conn: &Connection, clock: &dyn Clock, task_id: i64) -> Result<(), PlanError> {
        if !AllocationService::stored_selection(conn, clock)?.contains(&task_id) {
            return Err(PlanError::TaskNotInToday { task_id });
        }
        let ctx = load_task_context(conn, task_id)?;
        if ctx.plan_status != PlanStatus::Active {
            return Err(PlanError::PlanStatusInvalid { from: ctx.plan_status });
        }
        if ctx.status == TaskStatus::Completed {
            return Err(PlanError::ProgressLocked);
        }
        conn.execute(
            "INSERT INTO current_task (id, task_id) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET task_id = ?1",
            params![task_id],
        )
        .map_err(db_err)?;
        Ok(())
    }

    /// 按序完成一个子目标：必须是最早的未完成项（不可跳序），落 + 分钟账，
    /// 任务到 100% 自动转已完成。返回任务最新状态。
    pub fn complete_subgoal(
        conn: &Connection,
        clock: &dyn Clock,
        subgoal_id: i64,
    ) -> Result<TaskStatus, PlanError> {
        let sg = load_subgoal_context(conn, subgoal_id)?;
        let ctx = load_task_context(conn, sg.task_id)?;
        check_reportable(ctx.plan_status, ctx.status)?;
        if sg.completed
            || exists(
                conn,
                "SELECT 1 FROM subgoals WHERE task_id = ?1 AND completed_at IS NULL AND position < ?2",
                sg.task_id,
                sg.position,
            )?
        {
            return Err(PlanError::SubGoalOutOfOrder);
        }
        let now = clock.now().to_rfc3339();
        let tx = conn.unchecked_transaction().map_err(db_err)?;
        tx.execute(
            "UPDATE subgoals SET completed_at = ?1 WHERE id = ?2",
            params![now, subgoal_id],
        )
        .map_err(db_err)?;
        append_log(&tx, sg.task_id, &now, sg.estimated_minutes as f64, ProgressSource::SubGoal)?;
        let status = settle_task(&tx, sg.task_id, ctx.status)?;
        tx.commit().map_err(db_err)?;
        Ok(status)
    }

    /// 撤销一个已完成子目标（SubGoalUndo）：必须是最晚的已完成项（保持"已完成是前缀"
    /// 不变量），落 - 分钟补偿账，进度按日志实时重算。已完成任务锁定不可撤销。
    pub fn undo_subgoal(
        conn: &Connection,
        clock: &dyn Clock,
        subgoal_id: i64,
    ) -> Result<(), PlanError> {
        let sg = load_subgoal_context(conn, subgoal_id)?;
        let ctx = load_task_context(conn, sg.task_id)?;
        check_reportable(ctx.plan_status, ctx.status)?;
        if !sg.completed {
            return Err(PlanError::SubGoalNotCompleted);
        }
        if exists(
            conn,
            "SELECT 1 FROM subgoals WHERE task_id = ?1 AND completed_at IS NOT NULL AND position > ?2",
            sg.task_id,
            sg.position,
        )? {
            return Err(PlanError::SubGoalOutOfOrder);
        }
        let now = clock.now().to_rfc3339();
        let tx = conn.unchecked_transaction().map_err(db_err)?;
        tx.execute(
            "UPDATE subgoals SET completed_at = NULL WHERE id = ?1",
            params![subgoal_id],
        )
        .map_err(db_err)?;
        append_log(
            &tx,
            sg.task_id,
            &now,
            -(sg.estimated_minutes as f64),
            ProgressSource::SubGoalUndo,
        )?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// 无子目标任务增量汇报 +percent%（任意正数，最小 0.1%、最多一位小数，
    /// 上限 100%；2026-08-24 验收修订：原"5% 整数倍"颗粒度砍掉——庞大而简单的
    /// 任务需要细粒度推进）。累计不超 100%，落 + 分钟账，到 100% 自动完成。
    /// 返回任务最新状态。
    pub fn report_percent(
        conn: &Connection,
        clock: &dyn Clock,
        task_id: i64,
        percent: f64,
    ) -> Result<TaskStatus, PlanError> {
        if !valid_percent(percent, 0.1) {
            return Err(PlanError::PercentInvalid);
        }
        let ctx = load_task_context(conn, task_id)?;
        check_percent_task(&ctx)?;
        check_reportable(ctx.plan_status, ctx.status)?;
        let delta = percent as f64 * ctx.total_minutes as f64 / 100.0;
        let current = PlanService::task_progress(conn, task_id)?.completed_minutes;
        if current + delta > ctx.total_minutes as f64 + 1e-6 {
            return Err(PlanError::PercentOverflow);
        }
        let now = clock.now().to_rfc3339();
        let tx = conn.unchecked_transaction().map_err(db_err)?;
        append_log(&tx, task_id, &now, delta, ProgressSource::Percent)?;
        let status = settle_task(&tx, task_id, ctx.status)?;
        tx.commit().map_err(db_err)?;
        Ok(status)
    }

    /// 修正总进度（任务详情"改口"通道）：直接设定当前值（任意正数 0–100、最多
    /// 一位小数——2026-08-24 验收修订，同汇报颗粒度放开），差额以一条事件落账
    /// （可正可负）。仅无子目标任务；已完成任务锁定。
    /// 不设计划状态门槛——修正是对历史的对账，暂停计划的进度同样可纠偏。
    pub fn correct_total(
        conn: &Connection,
        clock: &dyn Clock,
        task_id: i64,
        percent: f64,
    ) -> Result<TaskStatus, PlanError> {
        if !valid_percent(percent, 0.0) {
            return Err(PlanError::PercentInvalid);
        }
        let ctx = load_task_context(conn, task_id)?;
        check_percent_task(&ctx)?;
        if ctx.status == TaskStatus::Completed {
            return Err(PlanError::ProgressLocked);
        }
        let target_value = round_delta_minutes(percent * ctx.total_minutes as f64 / 100.0);
        let current = PlanService::task_progress(conn, task_id)?.completed_minutes;
        let now = clock.now().to_rfc3339();
        let tx = conn.unchecked_transaction().map_err(db_err)?;
        // 差额也走边界 round：target 与 current 都是 f64，差额浮点尾巴同源污染。
        append_log(&tx, task_id, &now, round_delta_minutes(target_value - current), ProgressSource::Correction)?;
        // 归零修正不产生"首次推进"语义，未开始任务保持未开始
        let status = if percent > 0.0 {
            settle_task(&tx, task_id, ctx.status)?
        } else {
            ctx.status
        };
        tx.commit().map_err(db_err)?;
        Ok(status)
    }

    /// 当前任务视图：单行表里的 id 通过三重校验（存在未删 ∧ 计划进行中 ∧ 在今日
    /// 推进列表内）才展示；任务已完成仍返回（"✓ 任务完成"停留态，不自动切换）。
    fn current_view(conn: &Connection, clock: &dyn Clock) -> Result<Option<CurrentTaskView>, PlanError> {
        let task_id: Option<i64> = conn
            .query_row("SELECT task_id FROM current_task WHERE id = 1", [], |row| row.get(0))
            .optional()
            .map_err(db_err)?;
        let Some(task_id) = task_id else {
            return Ok(None);
        };
        let row = conn
            .query_row(
                "SELECT t.id, t.name, t.has_subgoals, t.status,
                        p.id, p.name, p.status
                 FROM tasks t JOIN plans p ON p.id = t.plan_id
                 WHERE t.id = ?1 AND t.deleted_at IS NULL",
                params![task_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)? != 0,
                        TaskStatus::from_db(&row.get::<_, String>(3)?),
                        row.get::<_, i64>(4)?,
                        row.get::<_, String>(5)?,
                        PlanStatus::from_db(&row.get::<_, String>(6)?),
                    ))
                },
            )
            .map_err(notfound_or_db)
            .ok();
        let Some((id, name, has_subgoals, status, plan_id, plan_name, plan_status)) = row
        else {
            return Ok(None); // 任务被删除：回空态
        };
        if plan_status != PlanStatus::Active
            || !AllocationService::stored_selection(conn, clock)?.contains(&id)
        {
            return Ok(None); // 计划暂停/放弃或已移出今日列表：回空态
        }
        let progress = PlanService::task_progress(conn, id)?;
        // 边界 round：UI 展示不暴露浮点尾巴（“4.3100000000000005 / 10h”），percent 一位
        // 小数与 ProgressGranularity 对齐，completed_minutes round 到 1e-9 消多次累加残留；
        // 领域 `TaskProgress::percent()` 仍保持原始精度供“分子不变分母变”语义使用。
        Ok(Some(CurrentTaskView {
            task_id: id,
            plan_id,
            plan_name,
            task_name: name,
            status,
            has_subgoals,
            percent: round_to_one_decimal(progress.percent()),
            completed_minutes: round_delta_minutes(progress.completed_minutes),
            total_minutes: progress.total_minutes,
            subgoals: if has_subgoals { load_subgoals(conn, id)? } else { Vec::new() },
        }))
    }

    /// 「更换任务」候选分组：今日推进列表 ∩ 仍可推进（计划进行中、未完成、未删除），
    /// 复用 PlanService::list 的 PlanOrdering；当前任务所属计划的分组提到最前。
    fn picker_groups(
        conn: &Connection,
        clock: &dyn Clock,
        first_plan: Option<i64>,
    ) -> Result<Vec<PickerGroup>, PlanError> {
        let selection = AllocationService::stored_selection(conn, clock)?;
        if selection.is_empty() {
            return Ok(Vec::new());
        }
        let mut groups = Vec::new();
        for plan in PlanService::list(conn)?.into_iter().filter(|p| p.status == PlanStatus::Active) {
            let tasks: Vec<PickerTask> = plan
                .tasks
                .iter()
                .filter(|t| t.status != TaskStatus::Completed && selection.contains(&t.id))
                .map(|t| PickerTask { id: t.id, name: t.name.clone() })
                .collect();
            if !tasks.is_empty() {
                groups.push(PickerGroup {
                    plan_id: plan.id,
                    plan_name: plan.name,
                    priority: plan.priority,
                    tasks,
                });
            }
        }
        if let Some(first) = first_plan {
            if let Some(pos) = groups.iter().position(|g| g.plan_id == first) {
                let g = groups.remove(pos);
                groups.insert(0, g);
            }
        }
        Ok(groups)
    }

    /// 某个工作日的完成量（分钟）：ledger 的按日聚合（`minutes_by_day`）取"今天"一档。
    /// 日志量级是每日几十条，实时派生不建汇总表（ADR-0009）。
    fn day_minutes(conn: &Connection, clock: &dyn Clock) -> Result<f64, PlanError> {
        let windows = SettingsService::load(conn).map_err(db_err)?.time_windows;
        Ok(minutes_by_day(conn, &windows)?
            .get(&clock.now().date_naive())
            .copied()
            .unwrap_or(0.0))
    }
}

/// 汇报路径的任务上下文（校验与落账共用一次查询）。
struct TaskContext {
    plan_status: PlanStatus,
    status: TaskStatus,
    has_subgoals: bool,
    total_minutes: u32,
}

/// 加载任务上下文；任务不存在或已软删除归 NotFound。
fn load_task_context(conn: &Connection, task_id: i64) -> Result<TaskContext, PlanError> {
    conn.query_row(
        "SELECT p.status, t.status, t.has_subgoals, t.estimated_minutes
         FROM tasks t JOIN plans p ON p.id = t.plan_id
         WHERE t.id = ?1 AND t.deleted_at IS NULL",
        params![task_id],
        |row| {
            Ok(TaskContext {
                plan_status: PlanStatus::from_db(&row.get::<_, String>(0)?),
                status: TaskStatus::from_db(&row.get::<_, String>(1)?),
                has_subgoals: row.get::<_, i64>(2)? != 0,
                total_minutes: row.get::<_, Option<u32>>(3)?.unwrap_or(0),
            })
        },
    )
    .map_err(notfound_or_db)
}

/// 子目标行上下文：完成/撤销路径共用。
struct SubGoalContext {
    task_id: i64,
    position: i32,
    estimated_minutes: u32,
    completed: bool,
}

fn load_subgoal_context(conn: &Connection, subgoal_id: i64) -> Result<SubGoalContext, PlanError> {
    conn.query_row(
        "SELECT task_id, position, estimated_minutes, completed_at IS NOT NULL
         FROM subgoals WHERE id = ?1",
        params![subgoal_id],
        |row| {
            Ok(SubGoalContext {
                task_id: row.get(0)?,
                position: row.get(1)?,
                estimated_minutes: row.get(2)?,
                completed: row.get::<_, i64>(3)? != 0,
            })
        },
    )
    .map_err(notfound_or_db)
}

/// 汇报门槛：计划必须进行中（暂停/放弃/终态的进度是归档历史），任务必须未完成（锁定）。
fn check_reportable(plan_status: PlanStatus, task_status: TaskStatus) -> Result<(), PlanError> {
    if plan_status != PlanStatus::Active {
        return Err(PlanError::PlanStatusInvalid { from: plan_status });
    }
    if task_status == TaskStatus::Completed {
        return Err(PlanError::ProgressLocked);
    }
    Ok(())
}

/// 百分比通道（汇报/修正）只适用于无子目标任务。
fn check_percent_task(ctx: &TaskContext) -> Result<(), PlanError> {
    if ctx.has_subgoals {
        return Err(PlanError::NotPercentTask);
    }
    Ok(())
}

/// 百分比数值合法性（CONTEXT ProgressGranularity，2026-08-24 验收修订）：
/// ≥ min（汇报 0.1、修正 0）、≤ 100、最多一位小数（×10 后为整数的容差判定）。
fn valid_percent(p: f64, min: f64) -> bool {
    p >= min && p <= 100.0 && ((p * 10.0) - (p * 10.0).round()).abs() < 1e-9
}

/// delta 分钟边界 round：消 IEEE 754 浮点尾巴（percent × total / 100 不可精确表示），
/// round 到 1e-9（f64 安全精度），防止多次 0.1% 累加污染 ProgressLog 求和与 UI 展示。
/// 复用点：`report_percent` / `correct_total` 写日志前、CurrentTaskView.completed_minutes 装配、
/// ledger 调整后目标跨边界输出；测试 seam_progress::progress_percent_rounds_to_one_decimal。
pub(crate) fn round_delta_minutes(v: f64) -> f64 {
    (v * 1e9).round() / 1e9
}

/// 派生百分比边界 round：四舍五入到一位小数（与 CONTEXT ProgressGranularity 0.1% 颗粒度对齐），
/// 消 IEEE 754 浮点尾巴暴露到 UI（如 “43.0999999...”）；领域 `TaskProgress::percent()`
/// 仍保持原始精度供“分子不变分母变”语义使用，本函数仅作用于跨边界输出。
/// 复用点：CurrentTaskView.percent 装配、summary 视图的 SummaryTask.percent（工单 11）；
/// 测试 seam_progress::progress_percent_rounds_to_one_decimal。
pub(crate) fn round_to_one_decimal(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// 汇报落账后的收尾：首次推进把未开始转进行中，再按派生进度自动完成（幂等）。
fn settle_task(conn: &Connection, task_id: i64, from: TaskStatus) -> Result<TaskStatus, PlanError> {
    if from == TaskStatus::NotStarted {
        conn.execute(
            "UPDATE tasks SET status = ?1 WHERE id = ?2",
            params![TaskStatus::Active.as_db(), task_id],
        )
        .map_err(db_err)?;
    }
    LifecycleService::sync_task_completion(conn, task_id)
}

/// 向 progress_log 追加一条事件（可在事务连接上调用）。
fn append_log(
    conn: &Connection,
    task_id: i64,
    at: &str,
    delta_minutes: f64,
    source: ProgressSource,
) -> Result<(), PlanError> {
    conn.execute(
        "INSERT INTO progress_log (task_id, at, delta_minutes, source) VALUES (?1, ?2, ?3, ?4)",
        params![task_id, at, delta_minutes, source.as_db()],
    )
    .map_err(db_err)?;
    Ok(())
}

/// 参数化 EXISTS 探测（子目标前缀不变式的两个方向共用）。
fn exists(
    conn: &Connection,
    sql: &str,
    a: i64,
    b: i32,
) -> Result<bool, PlanError> {
    conn.query_row(sql, params![a, b], |_| Ok(()))
        .optional()
        .map(|row| row.is_some())
        .map_err(db_err)
}

/// 事件时刻归属的工作窗口开始日（ADR-0009 跨午夜口径）：找到包含该时刻的窗口，
/// 跨午夜窗口的 [00:00, end) 段归前一日；不在任何窗口内 → 归属自身日期
/// （非工作日 / 窗口外的推进照常记账，CONTEXT WorkingHours）。
/// ledger 的按日聚合（`minutes_by_day`）共用同一归属口径。
pub(crate) fn attributed_date(at: DateTime<Local>, windows: &[TimeWindow]) -> NaiveDate {
    let t = (at.hour() * 60 + at.minute()) as u16;
    for w in windows {
        if w.start_minute <= w.end_minute {
            if t >= w.start_minute && t < w.end_minute {
                return at.date_naive();
            }
        } else if t >= w.start_minute {
            return at.date_naive();
        } else if t < w.end_minute {
            return (at - Duration::days(1)).date_naive();
        }
    }
    at.date_naive()
}
