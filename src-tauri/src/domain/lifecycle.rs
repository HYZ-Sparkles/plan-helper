//! 计划生命周期领域（ADR-0001 单向瀑布）：状态机转换（未开始→进行中；进行中↔已暂停；
//! 进行中→已完成〔手动确认〕；进行中/已暂停→已放弃）、任务进度 100% 自动完成、
//! 终态计划"复制并新建"（唯一重启路径）。
//! 抢占不变式（开始/恢复时对更高等级进行中计划的自动暂停）在工单 12 接入本服务各入口。

use rusqlite::{params, Connection};

use crate::clock::Clock;
use crate::domain::plans::{
    db_err, load_plan_state, PauseReason, PlanError, PlanService, PlanStatus, TaskStatus,
};

/// 生命周期读写服务。
pub struct LifecycleService;

impl LifecycleService {
    /// 未开始 → 进行中。
    pub fn start(conn: &Connection, plan_id: i64) -> Result<(), PlanError> {
        transition(conn, plan_id, &[PlanStatus::NotStarted], PlanStatus::Active, None)
    }

    /// 进行中 → 已暂停（手动暂停，原因记"用户主动"）。
    pub fn pause(conn: &Connection, plan_id: i64) -> Result<(), PlanError> {
        transition(
            conn,
            plan_id,
            &[PlanStatus::Active],
            PlanStatus::Paused,
            Some(PauseReason::UserInitiated),
        )
    }

    /// 已暂停 → 进行中（清空暂停原因——下次暂停会重新记录）。
    pub fn resume(conn: &Connection, plan_id: i64) -> Result<(), PlanError> {
        transition(conn, plan_id, &[PlanStatus::Paused], PlanStatus::Active, None)
    }

    /// 进行中 → 已完成。PlanCompletionConfirm：任务自动完成、计划手动确认，
    /// 且要求全部任务已完成（含至少 1 个任务——任务被删空的计划只能放弃）。
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
        transition(conn, plan_id, &[PlanStatus::Active], PlanStatus::Completed, None)
    }

    /// 进行中/已暂停 → 已放弃（终态；进度随计划留存归档可查，不物理删除）。
    pub fn abort(conn: &Connection, plan_id: i64) -> Result<(), PlanError> {
        transition(
            conn,
            plan_id,
            &[PlanStatus::Active, PlanStatus::Paused],
            PlanStatus::Abandoned,
            None,
        )
    }

    /// 任务进度到 100% 自动转已完成（CONTEXT TaskStatus；幂等，已是已完成则原样返回）。
    /// 汇报路径在工单 07 接线（子目标勾选/百分比汇报后调用）；无子目标任务的分子
    /// 自 ProgressLog 派生（07 接线前恒 0，不会被空数据误判完成）。
    pub fn sync_task_completion(conn: &Connection, task_id: i64) -> Result<TaskStatus, PlanError> {
        let progress = PlanService::task_progress(conn, task_id)?; // 不存在/已删归 NotFound
        if progress.percent() < 100.0 {
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
    /// 返回新计划 id。
    pub fn copy_as_new(conn: &Connection, clock: &dyn Clock, plan_id: i64) -> Result<i64, PlanError> {
        let source = PlanService::get(conn, plan_id)?; // 不存在归 NotFound
        let (from, _) = load_plan_state(conn, plan_id)?;
        if !matches!(from, PlanStatus::Completed | PlanStatus::Abandoned) {
            return Err(PlanError::PlanNotTerminal { from });
        }
        let now = clock.now().to_rfc3339();
        let tx = conn.unchecked_transaction().map_err(db_err)?;
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
        tx.commit().map_err(db_err)?;
        Ok(new_plan_id)
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
