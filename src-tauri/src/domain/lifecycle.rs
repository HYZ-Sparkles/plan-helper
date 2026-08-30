//! 计划生命周期领域（ADR-0001 单向瀑布）：状态机转换（未开始→进行中；进行中↔已暂停；
//! 进行中→已完成〔手动确认〕；进行中/已暂停→已放弃）、任务进度 100% 自动完成、
//! 终态计划"复制并新建"（唯一重启路径）。
//! 抢占不变式（ADR-0006，工单 12）接入各入口：任何时刻进行中的计划必然同等级——
//! 开始/继续/复制受"更高等级进行中"约束，成功则自动暂停低等级（逐层恢复在清空侧）。

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::clock::Clock;
use crate::domain::plans::{
    db_err, load_plan_state, notfound_or_db, PauseReason, PlanError, PlanService, PlanStatus,
    Priority, TaskStatus,
};

/// 生命周期操作的波及视图（AutoPauseFeedback 数据源）：start/resume/copy 使一张计划
/// 进入进行中时，被自动暂停的进行中低等级计划。
#[derive(Debug, PartialEq, Serialize)]
pub struct LifecycleOutcome {
    pub paused: Vec<PreemptedPlan>,
}

/// "复制并新建"的结果：新计划 id + 复制即进行中所触发的自动暂停（语义同 start）。
#[derive(Debug, PartialEq, Serialize)]
pub struct CopyAsNewOutcome {
    pub new_plan_id: i64,
    pub paused: Vec<PreemptedPlan>,
}

/// 被自动暂停的一张计划（id 供跳转、name 供反馈文案）。
#[derive(Debug, PartialEq, Serialize)]
pub struct PreemptedPlan {
    pub id: i64,
    pub name: String,
}

/// 生命周期读写服务。
pub struct LifecycleService;

impl LifecycleService {
    /// 未开始 → 进行中（PreemptionInvariant）：存在进行中的更高等级计划时拒绝
    /// （PreemptedByHigher）；成功则自动暂停所有进行中的低等级计划（原因记自动抢占）
    /// 并随结果返回，供前端三层反馈。
    pub fn start(conn: &Connection, plan_id: i64) -> Result<LifecycleOutcome, PlanError> {
        let tx = conn.unchecked_transaction().map_err(db_err)?;
        let priority = plan_priority(&tx, plan_id)?;
        ensure_no_higher_active(&tx, priority)?;
        transition(&tx, plan_id, &[PlanStatus::NotStarted], PlanStatus::Active, None)?;
        let paused = preempt_lower_tiers(&tx, priority)?;
        tx.commit().map_err(db_err)?;
        Ok(LifecycleOutcome { paused })
    }

    /// 进行中 → 已暂停（手动暂停，原因记"用户主动"）。
    /// 高等级就此清空（PreemptionInvariant 恢复条件之一）→ 逐层恢复检查。
    pub fn pause(conn: &Connection, plan_id: i64) -> Result<(), PlanError> {
        let tx = conn.unchecked_transaction().map_err(db_err)?;
        transition(
            &tx,
            plan_id,
            &[PlanStatus::Active],
            PlanStatus::Paused,
            Some(PauseReason::UserInitiated),
        )?;
        auto_resume_top_tier(&tx)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// 已暂停 → 进行中（清空暂停原因——下次暂停会重新记录）。
    /// 与 start 同一套不变式：更高等级进行中时拒绝；成功则抢占低等级
    /// （恢复的高等级再遇面即再抢占，用户故事 19）。
    pub fn resume(conn: &Connection, plan_id: i64) -> Result<LifecycleOutcome, PlanError> {
        let tx = conn.unchecked_transaction().map_err(db_err)?;
        let priority = plan_priority(&tx, plan_id)?;
        ensure_no_higher_active(&tx, priority)?;
        transition(&tx, plan_id, &[PlanStatus::Paused], PlanStatus::Active, None)?;
        let paused = preempt_lower_tiers(&tx, priority)?;
        tx.commit().map_err(db_err)?;
        Ok(LifecycleOutcome { paused })
    }

    /// 进行中 → 已完成。PlanCompletionConfirm：任务自动完成、计划手动确认，
    /// 且要求全部任务已完成（含至少 1 个任务——任务被删空的计划只能放弃）。
    /// 高等级就此清空 → 逐层恢复检查。
    pub fn complete(conn: &Connection, plan_id: i64) -> Result<(), PlanError> {
        load_plan_state(conn, plan_id)?; // 不存在先归 NotFound（在任务计数判定之前）
        let unfinished: i64 = conn
            .query_row(
                "SELECT (SELECT COUNT(*) FROM tasks WHERE plan_id = ?1 AND deleted_at IS NULL AND status != ?2),
                        (SELECT COUNT(*) FROM tasks WHERE plan_id = ?1 AND deleted_at IS NULL)",
                params![plan_id, TaskStatus::Completed.as_db()],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .map(|(unfinished, total)| if total == 0 { 1 } else { unfinished }) // 删空视同未完成
            .map_err(db_err)?;
        if unfinished > 0 {
            return Err(PlanError::TasksNotCompleted);
        }
        let tx = conn.unchecked_transaction().map_err(db_err)?;
        transition(&tx, plan_id, &[PlanStatus::Active], PlanStatus::Completed, None)?;
        auto_resume_top_tier(&tx)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// 进行中/已暂停 → 已放弃（终态；进度随计划留存归档可查，不物理删除）。
    /// 放弃进行中的高等级同样是清空 → 逐层恢复检查。
    pub fn abort(conn: &Connection, plan_id: i64) -> Result<(), PlanError> {
        let tx = conn.unchecked_transaction().map_err(db_err)?;
        transition(
            &tx,
            plan_id,
            &[PlanStatus::Active, PlanStatus::Paused],
            PlanStatus::Abandoned,
            None,
        )?;
        auto_resume_top_tier(&tx)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// 任务进度到 100% 自动转已完成（CONTEXT TaskStatus；幂等，已是已完成则原样返回）。
    /// 汇报路径在工单 07 接线（子目标勾选/百分比汇报后调用）；无子目标任务的分子
    /// 自 ProgressLog 派生（07 接线前恒 0，不会被空数据误判完成）。
    pub fn sync_task_completion(conn: &Connection, task_id: i64) -> Result<TaskStatus, PlanError> {
        let progress = PlanService::task_progress(conn, task_id)?; // 不存在/已删归 NotFound
        if !progress.is_complete() {
            return conn
                .query_row(
                    "SELECT status FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
                    params![task_id],
                    |row| Ok(TaskStatus::from_db(&row.get::<_, String>(0)?)),
                )
                .map_err(db_err);
        }
        conn.execute(
            "UPDATE tasks SET status = ?1 WHERE id = ?2",
            params![TaskStatus::Completed.as_db(), task_id],
        )
        .map_err(db_err)?;
        Ok(TaskStatus::Completed)
    }

    /// 复制并新建（仅终态计划，CopyAsNewPlan）：复制名称（加"- 副本"）/详细内容/简述/优先级/
    /// 截止日期/任务与子目标，进度与完成状态归零，新计划直接进入进行中；旧计划保持终态。
    /// 复制即开始 = 同一套抢占不变式：更高等级进行中时拒绝，成功则抢占低等级。
    /// 返回新计划 id 与自动暂停清单。
    pub fn copy_as_new(
        conn: &Connection,
        clock: &dyn Clock,
        plan_id: i64,
    ) -> Result<CopyAsNewOutcome, PlanError> {
        let source = PlanService::get(conn, plan_id)?; // 不存在归 NotFound
        let (from, _) = load_plan_state(conn, plan_id)?;
        if !matches!(from, PlanStatus::Completed | PlanStatus::Abandoned) {
            return Err(PlanError::PlanNotTerminal { from });
        }
        let now = clock.now().to_rfc3339();
        let tx = conn.unchecked_transaction().map_err(db_err)?;
        ensure_no_higher_active(&tx, source.priority)?;
        tx.execute(
            "INSERT INTO plans (name, summary, detail, priority, due_date, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                format!("{} - 副本", source.name),
                source.summary,
                source.detail,
                source.priority.as_db(),
                source.due_date,
                PlanStatus::Active.as_db(), // 一键复制 = 复制模板 + 立即开始
                now,
            ],
        )
        .map_err(db_err)?;
        let new_plan_id = tx.last_insert_rowid();
        // 旧任务 id → 新任务 id（依赖边按下标映射复制；源计划的边在建边时已过环校验，原样安全）
        let mut new_id_of: Vec<i64> = Vec::with_capacity(source.tasks.len());
        for t in &source.tasks {
            tx.execute(
                "INSERT INTO tasks (plan_id, name, summary, detail, has_subgoals, estimated_minutes,
                                     position, status, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    new_plan_id,
                    t.name,
                    t.summary,
                    t.detail,
                    t.has_subgoals,
                    t.estimated_minutes,
                    t.position,
                    TaskStatus::NotStarted.as_db(), // 进度归零
                    now,
                ],
            )
            .map_err(db_err)?;
            let new_task_id = tx.last_insert_rowid();
            for s in &t.subgoals {
                // 子目标完成状态归零：completed_at 留 NULL
                tx.execute(
                    "INSERT INTO subgoals (task_id, name, estimated_minutes, position, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![new_task_id, s.name, s.estimated_minutes, s.position, now],
                )
                .map_err(db_err)?;
            }
            new_id_of.push(new_task_id);
        }
        for (i, t) in source.tasks.iter().enumerate() {
            for &pred in &t.prerequisite_ids {
                let pred_index = source.tasks.iter().position(|p| p.id == pred);
                if let Some(pi) = pred_index {
                    tx.execute(
                        "INSERT OR IGNORE INTO task_dependencies (predecessor_id, successor_id)
                         VALUES (?1, ?2)",
                        params![new_id_of[pi], new_id_of[i]],
                    )
                    .map_err(db_err)?;
                }
            }
        }
        let paused = preempt_lower_tiers(&tx, source.priority)?;
        tx.commit().map_err(db_err)?;
        Ok(CopyAsNewOutcome { new_plan_id, paused })
    }
}

/// 单条状态转换的权威通道：当前状态不在 allowed_from 内即拒绝（PlanStatusInvalid，含终态重启）；
/// pause_reason 仅暂停时写入，其余转换一律清空（不留陈旧原因）。
fn transition(
    conn: &Connection,
    plan_id: i64,
    allowed_from: &[PlanStatus],
    to: PlanStatus,
    pause_reason: Option<PauseReason>,
) -> Result<(), PlanError> {
    let (from, _) = load_plan_state(conn, plan_id)?;
    if !allowed_from.contains(&from) {
        return Err(PlanError::PlanStatusInvalid { from });
    }
    conn.execute(
        "UPDATE plans SET status = ?1, pause_reason = ?2 WHERE id = ?3",
        params![to.as_db(), pause_reason.map(|r| r.as_db()), plan_id],
    )
    .map_err(db_err)?;
    Ok(())
}

/// 计划的优先级（不存在归 NotFound）：start/resume/copy 的抢占判定入参。
fn plan_priority(conn: &Connection, plan_id: i64) -> Result<Priority, PlanError> {
    conn.query_row(
        "SELECT priority FROM plans WHERE id = ?1",
        params![plan_id],
        |row| Ok(Priority::from_db(&row.get::<_, String>(0)?)),
    )
    .map_err(notfound_or_db)
}

/// 开始约束（PreemptionInvariant）：存在进行中的更高等级计划时拒绝开始/继续/复制——
/// 想开低等级计划，先完成或手动暂停当前高等级。错误带出阻挡计划名（tooltip/文案用）。
/// 同等级并行不受限（CONTEXT Parallelism）。优先级比较在 Rust 侧做——priority 是
/// TEXT 列，SQL 字典序（High < Low < Medium）与语义序（Low < Medium < High）不同。
fn ensure_no_higher_active(conn: &Connection, priority: Priority) -> Result<(), PlanError> {
    let actives: Vec<(String, Priority)> = conn
        .prepare("SELECT name, priority FROM plans WHERE status = ?1")
        .map_err(db_err)?
        .query_map(params![PlanStatus::Active.as_db()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                Priority::from_db(&row.get::<_, String>(1)?),
            ))
        })
        .map_err(db_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_err)?;
    match actives.into_iter().find(|(_, p)| *p > priority) {
        Some((name, _)) => Err(PlanError::PreemptedByHigher { plan_name: name }),
        None => Ok(()),
    }
}

/// 自动暂停（PreemptionInvariant）：所有进行中的低等级计划暂停（原因记自动抢占），
/// 未开始的计划不受影响；返回被暂停的计划供 AutoPauseFeedback。
fn preempt_lower_tiers(conn: &Connection, priority: Priority) -> Result<Vec<PreemptedPlan>, PlanError> {
    let lower: Vec<PreemptedPlan> = conn
        .prepare("SELECT id, name, priority FROM plans WHERE status = ?1")
        .map_err(db_err)?
        .query_map(params![PlanStatus::Active.as_db()], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                Priority::from_db(&row.get::<_, String>(2)?),
            ))
        })
        .map_err(db_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_err)?
        .into_iter()
        .filter(|(_, _, p)| *p < priority)
        .map(|(id, name, _)| PreemptedPlan { id, name })
        .collect();
    for p in &lower {
        conn.execute(
            "UPDATE plans SET status = ?1, pause_reason = ?2 WHERE id = ?3",
            params![PlanStatus::Paused.as_db(), PauseReason::AutoPreempted.as_db(), p.id],
        )
        .map_err(db_err)?;
    }
    Ok(lower)
}

/// 逐层恢复（PreemptionInvariant）：当不存在进行中的更高等级计划时（完成、放弃、
/// 手动暂停高等级之后），恢复等级最高的"自动抢占"组——整组恢复，且只恢复暂停原因
/// 为自动抢占的计划（用户主动暂停的永不自动恢复）；更低的被抢占组等这组再清空才轮到，
/// 所以单次调用只恢复一组——恢复后它自身就是更低等级组面前的"进行中更高等级"。
fn auto_resume_top_tier(conn: &Connection) -> Result<(), PlanError> {
    let plans: Vec<(i64, PlanStatus, Priority, Option<PauseReason>)> = conn
        .prepare("SELECT id, status, priority, pause_reason FROM plans")
        .map_err(db_err)?
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                PlanStatus::from_db(&row.get::<_, String>(1)?),
                Priority::from_db(&row.get::<_, String>(2)?),
                row.get::<_, Option<String>>(3)?.map(|s| PauseReason::from_db(&s)),
            ))
        })
        .map_err(db_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_err)?;
    let active_top = plans
        .iter()
        .filter(|(_, s, _, _)| *s == PlanStatus::Active)
        .map(|(_, _, p, _)| *p)
        .max();
    let preempted: Vec<(i64, Priority)> = plans
        .iter()
        .filter(|(_, s, _, r)| {
            *s == PlanStatus::Paused && *r == Some(PauseReason::AutoPreempted)
        })
        .map(|(id, _, p, _)| (*id, *p))
        .collect();
    let Some(top) = preempted.iter().map(|(_, p)| *p).max() else {
        return Ok(()); // 没有被抢占组，无需恢复
    };
    // 恢复条件：不存在进行中的更高等级（active_top 严格低于 top，或没有任何进行中）；
    // 有进行中且 ≥ top（含数据异常下的同等级并存）则不恢复
    if active_top.is_some_and(|a| a >= top) {
        return Ok(());
    }
    for &(id, p) in &preempted {
        if p != top {
            continue; // 只恢复最高组（整组）；更低的组等这组清空
        }
        conn.execute(
            "UPDATE plans SET status = ?1, pause_reason = NULL WHERE id = ?2",
            params![PlanStatus::Active.as_db(), id],
        )
        .map_err(db_err)?;
    }
    Ok(())
}
