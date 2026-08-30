//! 计划领域：计划（Plan）与任务（Task）的创建、读取、编辑与软删除。
//! 状态机转换（工单 05）、子目标（工单 04）、依赖（工单 04）在此模型上扩展。

use chrono::{DateTime, Local};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

use crate::clock::Clock;
use crate::domain::deps::DependencyService;

/// 优先级枚举（CONTEXT「优先级」）。派生 Ord 后 Low < Medium < High，排序即声明序。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Medium,
    High,
}

impl Priority {
    /// SQLite TEXT 列往返（不引入 sql-enum 依赖，两行 match 足够）
    pub fn as_db(self) -> &'static str {
        match self {
            Priority::Low => "Low",
            Priority::Medium => "Medium",
            Priority::High => "High",
        }
    }
    pub fn from_db(s: &str) -> Priority {
        match s {
            "High" => Priority::High,
            "Low" => Priority::Low,
            _ => Priority::Medium,
        }
    }
}

/// 计划状态（ADR-0001 单向瀑布；转换规则在工单 05 实现，此处只承载取值）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanStatus {
    NotStarted,
    Active,
    Paused,
    Completed,
    Abandoned,
}

impl PlanStatus {
    /// SQLite TEXT 列往返，口径同 Priority::as_db
    pub fn as_db(self) -> &'static str {
        match self {
            PlanStatus::NotStarted => "NotStarted",
            PlanStatus::Active => "Active",
            PlanStatus::Paused => "Paused",
            PlanStatus::Completed => "Completed",
            PlanStatus::Abandoned => "Abandoned",
        }
    }
    pub fn from_db(s: &str) -> PlanStatus {
        match s {
            "Active" => PlanStatus::Active,
            "Paused" => PlanStatus::Paused,
            "Completed" => PlanStatus::Completed,
            "Abandoned" => PlanStatus::Abandoned,
            _ => PlanStatus::NotStarted,
        }
    }
}

/// 暂停原因（CONTEXT PauseReason 二值）：手动暂停记 UserInitiated；
/// AutoPreempted 由工单 12 的抢占逻辑写入。TEXT 列往返口径同 PlanStatus。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PauseReason {
    UserInitiated,
    AutoPreempted,
}

impl PauseReason {
    pub fn as_db(self) -> &'static str {
        match self {
            PauseReason::UserInitiated => "UserInitiated",
            PauseReason::AutoPreempted => "AutoPreempted",
        }
    }
    pub fn from_db(s: &str) -> PauseReason {
        match s {
            "AutoPreempted" => PauseReason::AutoPreempted,
            _ => PauseReason::UserInitiated,
        }
    }
}

/// 任务状态（CONTEXT：进度到 100% 自动转已完成，工单 07 接线）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    NotStarted,
    Active,
    Completed,
}

impl TaskStatus {
    /// SQLite TEXT 列往返，口径同 Priority::as_db
    pub fn as_db(self) -> &'static str {
        match self {
            TaskStatus::NotStarted => "NotStarted",
            TaskStatus::Active => "Active",
            TaskStatus::Completed => "Completed",
        }
    }
    pub fn from_db(s: &str) -> TaskStatus {
        match s {
            "Active" => TaskStatus::Active,
            "Completed" => TaskStatus::Completed,
            _ => TaskStatus::NotStarted,
        }
    }
}

/// 计划草稿：创建与编辑共用的表单形态（CreationUI 双模式的服务层镜像）。
/// 任务 id 为 None 表示新任务——创建时全部视为新任务，编辑时按 id 更新、无 id 插入。
#[derive(Debug, Clone, Deserialize)]
pub struct PlanDraft {
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub detail: String,
    pub priority: Priority,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub tasks: Vec<TaskDraft>,
}

/// 任务草稿。无子目标任务必填预计耗时；有子目标时预计总耗时 = 子目标求和（effective_minutes）。
#[derive(Debug, Clone, Deserialize)]
pub struct TaskDraft {
    #[serde(default)]
    pub id: Option<i64>,
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub has_subgoals: bool,
    #[serde(default)]
    pub estimated_minutes: Option<u32>,
    /// 子目标草稿（精简输入行）。has_subgoals = false 时被忽略（UI 取消勾选确认后清空）
    #[serde(default)]
    pub subgoals: Vec<SubGoalDraft>,
    /// 前置任务在草稿 tasks 数组中的下标（同一草稿内相对引用，创建/编辑同一语义；
    /// 服务层解析为真实 id 建边——创建表单新任务无 id，用下标才能表达依赖）
    #[serde(default)]
    pub depends_on: Vec<usize>,
}

/// 子目标草稿：内容 + 预计耗时（CONTEXT「精简输入行」两项皆必填）。id 为 None = 新子目标。
#[derive(Debug, Clone, Deserialize)]
pub struct SubGoalDraft {
    #[serde(default)]
    pub id: Option<i64>,
    pub name: String,
    pub estimated_minutes: u32,
}

/// 计划领域的错误（command 边界序列化给前端展示）。
/// 统一内部标记形态：`{"kind":"变体名","payload":{...}}`，前端文案见 src/lib/labels.ts。
#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum PlanError {
    /// 计划或任务名称为空
    EmptyName { task_index: Option<usize> },
    /// 计划至少 1 个任务（spec 用户故事 8）
    NoTasks,
    /// 无子目标任务必填预计耗时
    TaskNeedsDuration { index: usize },
    /// 勾了子目标的任务至少 1 个子目标（工单 04 提供录入前必然拒绝）
    SubGoalsRequired { index: usize },
    /// 子目标行内容或预计耗时缺失/非法（两项皆必填）
    SubGoalInvalid { task_index: usize, subgoal_index: usize },
    /// 已完成子目标锁定：被修改、或从提交集中消失（含取消勾选时的整体清空）
    SubGoalLocked { task_index: usize },
    /// 前置引用下标越界（自指由环检测拒绝）
    DependencyInvalid { task_index: usize },
    /// 依赖边跨计划（仅同计划内允许）
    DependencyCrossPlan,
    /// 依赖成环（task_index = 完成环的那条边所属草稿任务下标；link 直连时为 None）
    DependencyCycle { task_index: Option<usize> },
    /// 计划开始后优先级不可调整（spec 用户故事 12）
    PriorityLocked,
    /// 计划状态不允许该操作（ADR-0001 单向瀑布外的转换；from = 当前状态）
    PlanStatusInvalid { from: PlanStatus },
    /// "复制并新建"仅终态计划可用（from = 当前状态）
    PlanNotTerminal { from: PlanStatus },
    /// 完成计划要求所有任务已完成（PlanCompletionConfirm：任务自动、计划手动）
    TasksNotCompleted,
    /// 任务不可选入今日分配（前置未完成 / 计划不在进行中 / 已完成）
    TaskNotAllocatable { task_id: i64 },
    /// 任务不在今日推进列表（指定当前任务的后端兜底，工单 07）
    TaskNotInToday { task_id: i64 },
    /// 子目标乱序：未完成前面的子目标就推进后面 / 撤销的不是最后一个已完成（前缀不变式）
    SubGoalOutOfOrder,
    /// 撤销目标尚未完成
    SubGoalNotCompleted,
    /// 已完成任务进度锁定：不可汇报 / 撤销 / 修正（CONTEXT TaskStatus）
    ProgressLocked,
    /// 百分比非法（汇报 0.1–100、修正 0–100；正数、最多一位小数——
    /// 2026-08-24 验收修订，原"5% 倍数"颗粒度砍掉）
    PercentInvalid,
    /// 累计汇报将超过 100%（ProgressGranularity 增量上限）
    PercentOverflow,
    /// 百分比汇报 / 修正只适用于无子目标任务（有子目标走按序勾选与撤销）
    NotPercentTask,
    /// 日期参数非法（须 YYYY-MM-DD；工单 11 总结的 command 边界解析）
    InvalidDate,
    /// 已完成任务字段锁定，提交内容与库中不一致
    TaskLocked { index: usize },
    /// 提交任务集与库中现存任务不一致（缺失或未知 id）——删除必须显式走 delete_task
    TaskSetMismatch,
    /// 目标计划/任务不存在
    NotFound,
    /// 存储故障兜底（校验错误之外的罕见情况）
    Storage(String),
}

/// 列表/详情的输出视图：计划带嵌套任务。
#[derive(Debug, Serialize)]
pub struct PlanView {
    pub id: i64,
    pub name: String,
    pub summary: String,
    pub detail: String,
    pub priority: Priority,
    pub due_date: Option<String>,
    pub status: PlanStatus,
    /// 非 NULL = 暂停原因（用户主动 / 自动抢占），详情页展示用
    pub pause_reason: Option<PauseReason>,
    pub created_at: DateTime<Local>,
    pub tasks: Vec<TaskView>,
}

/// 任务视图（软删除的任务不出现在列表）
#[derive(Debug, Serialize)]
pub struct TaskView {
    pub id: i64,
    pub name: String,
    pub summary: String,
    pub detail: String,
    pub has_subgoals: bool,
    pub estimated_minutes: Option<u32>,
    pub position: i32,
    pub status: TaskStatus,
    pub subgoals: Vec<SubGoalView>,
    /// 前置任务 id（同计划内，DependencyEditor 所见即所存）
    pub prerequisite_ids: Vec<i64>,
    /// 派生进度百分比（0–100，ADR-0002）：有子目标 = 已完成子目标耗时占比；
    /// 无子目标 = ProgressLog 增量分钟占预计总耗时比（工单 07 接线）
    pub progress_percent: f64,
}

/// 子目标视图（completed 由 completed_at 派生，按 position 排序 = 填写顺序）
#[derive(Debug, Clone, Serialize)]
pub struct SubGoalView {
    pub id: i64,
    pub name: String,
    pub estimated_minutes: u32,
    pub position: i32,
    pub completed: bool,
}

/// 任务派生进度（ADR-0002 耗时完成度）：分子 = 已完成分钟。
/// 有子目标 = 已完成子目标耗时之和；无子目标 = ProgressLog 增量之和（百分比 × 总耗时落账）。
#[derive(Debug, PartialEq)]
pub struct TaskProgress {
    pub completed_minutes: f64,
    pub total_minutes: u32,
}

impl TaskProgress {
    /// 百分比（0–100）；总耗时为 0 视为 0（validate 已保证 > 0，纯防御）。
    /// 保持原始 f64 精度——这是领域语义值（分子不变分母变缩放后 60/140 = 42.857142857142854
    /// 必须如实保留，UI 展示层再 round）；存储 / 派生边界 round 在 `report_percent` / `correct_total`
    /// 写日志前完成，前端 `hoursFromMinutes` / `taskProgress` 显示层也兜底 round。
    pub fn percent(&self) -> f64 {
        if self.total_minutes == 0 {
            0.0
        } else {
            self.completed_minutes * 100.0 / self.total_minutes as f64
        }
    }

    /// 是否到 100%（工单 07 自动完成判定）。浮点累计有舍入误差，用 epsilon 容差比较。
    pub fn is_complete(&self) -> bool {
        self.total_minutes > 0 && self.completed_minutes + 1e-6 >= self.total_minutes as f64
    }
}

impl PlanDraft {
    /// 保存前的领域校验，创建与更新共用（spec 用户故事 8：不留空壳）。
    /// UI 按段先校验，这里是权威兜底。
    fn validate(&self) -> Result<(), PlanError> {
        if self.name.trim().is_empty() {
            return Err(PlanError::EmptyName { task_index: None });
        }
        if self.tasks.is_empty() {
            return Err(PlanError::NoTasks);
        }
        for (i, t) in self.tasks.iter().enumerate() {
            if t.name.trim().is_empty() {
                return Err(PlanError::EmptyName { task_index: Some(i) });
            }
            if !t.has_subgoals && t.estimated_minutes.unwrap_or(0) == 0 {
                return Err(PlanError::TaskNeedsDuration { index: i });
            }
            if t.has_subgoals {
                if t.subgoals.is_empty() {
                    return Err(PlanError::SubGoalsRequired { index: i });
                }
                for (j, s) in t.subgoals.iter().enumerate() {
                    if s.name.trim().is_empty() || s.estimated_minutes == 0 {
                        return Err(PlanError::SubGoalInvalid { task_index: i, subgoal_index: j });
                    }
                }
            }
        }
        Ok(())
    }
}

/// 计划读写服务。
pub struct PlanService;

impl PlanService {
    /// 创建计划与全部任务（同一事务）；简述留空回退为名称。返回新计划 id。
    pub fn create(conn: &Connection, clock: &dyn Clock, new: &PlanDraft) -> Result<i64, PlanError> {
        new.validate()?;
        check_dep_indices(&new.tasks)?;
        let now = clock.now().to_rfc3339();
        let tx = conn.unchecked_transaction().map_err(db_err)?;
        tx.execute(
            "INSERT INTO plans (name, summary, detail, priority, due_date, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                new.name.trim(),
                default_text(&new.summary, &new.name),
                new.detail,
                new.priority.as_db(),
                new.due_date,
                PlanStatus::NotStarted.as_db(),
                now,
            ],
        )
        .map_err(db_err)?;
        let plan_id = tx.last_insert_rowid();
        let mut ids: Vec<i64> = Vec::with_capacity(new.tasks.len());
        for (pos, t) in new.tasks.iter().enumerate() {
            ids.push(insert_task(&tx, plan_id, t, pos as i32, &now)?);
        }
        link_draft_deps(&tx, &new.tasks, &ids)?;
        tx.commit().map_err(db_err)?;
        Ok(plan_id)
    }

    /// 整计划编辑（CreationUI 编辑态）：计划字段覆盖更新，任务按 id 更新、无 id 追加。
    /// 不变量（README 增删改规则 + spec 用户故事 9–12）：
    /// 优先级开始后锁定；已完成任务内容锁定；任务集与库一致（删除显式走 delete_task）；
    /// 落库顺序 = 未完成按提交序在前、已完成按库内序沉底。
    /// 子目标全量替换（update_subgoals）；依赖边全量替换（DependencyEditor 所见即所存）。
    pub fn update(
        conn: &Connection,
        clock: &dyn Clock,
        plan_id: i64,
        draft: &PlanDraft,
    ) -> Result<(), PlanError> {
        draft.validate()?;
        check_dep_indices(&draft.tasks)?;
        let (db_priority, status) = conn
            .query_row(
                "SELECT priority, status FROM plans WHERE id = ?1",
                params![plan_id],
                |row| {
                    Ok((
                        Priority::from_db(&row.get::<_, String>(0)?),
                        PlanStatus::from_db(&row.get::<_, String>(1)?),
                    ))
                },
            )
            .map_err(notfound_or_db)?;
        if status != PlanStatus::NotStarted && draft.priority != db_priority {
            return Err(PlanError::PriorityLocked);
        }
        let existing = load_tasks(conn, Some(plan_id))?;
        check_task_set(&existing, draft)?;
        check_completed_locked(&existing, draft)?;

        // 位置归一化：未完成（含新任务）按提交相对序，已完成按库内序沉底
        let completed: Vec<i64> = existing
            .iter()
            .filter(|(_, t)| t.status == TaskStatus::Completed)
            .map(|(_, t)| t.id)
            .collect();
        let is_completed = |t: &TaskDraft| t.id.is_some_and(|id| completed.contains(&id));
        let mut ordered: Vec<(usize, &TaskDraft)> = draft
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| !is_completed(t))
            .collect();
        // 已完成不可拖：顺序以库内 position 为准，而非提交序
        for id in &completed {
            if let Some((di, t)) = draft.tasks.iter().enumerate().find(|(_, t)| t.id == Some(*id)) {
                ordered.push((di, t));
            }
        }

        let now = clock.now().to_rfc3339();
        let tx = conn.unchecked_transaction().map_err(db_err)?;
        tx.execute(
            "UPDATE plans SET name = ?1, summary = ?2, detail = ?3, priority = ?4, due_date = ?5
             WHERE id = ?6",
            params![
                draft.name.trim(),
                default_text(&draft.summary, &draft.name),
                draft.detail,
                draft.priority.as_db(),
                draft.due_date,
                plan_id,
            ],
        )
        .map_err(db_err)?;
        // 依赖解析用：草稿下标 → 实际任务 id（新任务用插入返回的 rowid）
        let mut id_of: Vec<i64> = vec![0; draft.tasks.len()];
        for (pos, (di, t)) in ordered.iter().enumerate() {
            match t.id {
                Some(id) => {
                    tx.execute(
                        "UPDATE tasks SET name = ?1, summary = ?2, detail = ?3, has_subgoals = ?4,
                         estimated_minutes = ?5, position = ?6 WHERE id = ?7",
                        params![
                            t.name.trim(),
                            default_text(&t.summary, &t.name),
                            t.detail,
                            t.has_subgoals,
                            effective_minutes(t),
                            pos as i32,
                            id,
                        ],
                    )
                    .map_err(db_err)?;
                    update_subgoals(&tx, id, t, *di, &now)?;
                    id_of[*di] = id;
                }
                None => id_of[*di] = insert_task(&tx, plan_id, t, pos as i32, &now)?,
            }
        }
        // 依赖边全量替换：先清本计划旧边（不变量保证边只在计划内，按后继覆盖即可），再按草稿重建
        tx.execute(
            "DELETE FROM task_dependencies
             WHERE successor_id IN (SELECT id FROM tasks WHERE plan_id = ?1)",
            params![plan_id],
        )
        .map_err(db_err)?;
        link_draft_deps(&tx, &draft.tasks, &id_of)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// 单个计划详情（含任务，按 position 排序）；不存在返回 NotFound。
    pub fn get(conn: &Connection, plan_id: i64) -> Result<PlanView, PlanError> {
        let mut plan = conn
            .query_row(
                &format!("SELECT {PLAN_COLS} FROM plans WHERE id = ?1"),
                params![plan_id],
                plan_row,
            )
            .map_err(notfound_or_db)?;
        plan.tasks = load_tasks(conn, Some(plan_id))?
            .into_iter()
            .map(|(_, task)| task)
            .collect();
        Ok(plan)
    }

    /// 软删除任务进归档（deleted_at 落删除时刻）并自动解除其依赖边（后继解锁）；
    /// 确认弹窗在 UI，这里是权威通道。
    pub fn delete_task(conn: &Connection, clock: &dyn Clock, task_id: i64) -> Result<(), PlanError> {
        let tx = conn.unchecked_transaction().map_err(db_err)?;
        let n = tx
            .execute(
                "UPDATE tasks SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
                params![clock.now().to_rfc3339(), task_id],
            )
            .map_err(db_err)?;
        if n == 0 {
            return Err(PlanError::NotFound); // 不存在或已删除，一律 NotFound（幂等）
        }
        DependencyService::detach_task(&tx, task_id)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// 全部未软删除的计划（含任务），按 PlanOrdering 默认规则（唯一排序，
    /// 2026-08-24 决策砍掉手动覆盖）：Priority 降序 → CreatedAt 倒序 → id 倒序
    /// （同秒创建时新的在前）。
    pub fn list(conn: &Connection) -> Result<Vec<PlanView>, PlanError> {
        let mut plans = conn
            .prepare(&format!("SELECT {PLAN_COLS} FROM plans"))
            .map_err(db_err)?
            .query_map([], plan_row)
            .map_err(db_err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db_err)?;
        for (plan_id, task) in load_tasks(conn, None)? {
            plans
                .iter_mut()
                .find(|p| p.id == plan_id)
                .map(|p| p.tasks.push(task));
        }
        plans.sort_by(default_order_cmp);
        Ok(plans)
    }

    /// 任务派生进度（ADR-0002）：改耗时/增删未完成子目标后自动缩放——
    /// 分子（已完成分钟）不变、分母（总量）随编辑变化。
    /// 有子目标 = 已完成子目标耗时之和；无子目标 = ProgressLog 增量之和（工单 07 接线）。
    pub fn task_progress(conn: &Connection, task_id: i64) -> Result<TaskProgress, PlanError> {
        let (has_subgoals, subgoal_done, log_sum, total): (bool, f64, f64, Option<u32>) = conn
            .query_row(
                "SELECT tasks.has_subgoals,
                        (SELECT COALESCE(SUM(estimated_minutes), 0) FROM subgoals
                         WHERE task_id = tasks.id AND completed_at IS NOT NULL),
                        (SELECT COALESCE(SUM(delta_minutes), 0.0) FROM progress_log
                         WHERE task_id = tasks.id),
                        tasks.estimated_minutes
                 FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
                params![task_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)? != 0,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                    ))
                },
            )
            .map_err(notfound_or_db)?;
        Ok(TaskProgress {
            completed_minutes: if has_subgoals { subgoal_done } else { log_sum },
            total_minutes: total.unwrap_or(0),
        })
    }
}

/// 插入一张任务卡（create 与 update 追加共用；新任务状态恒为未开始），连带子目标，返回新任务 id。
fn insert_task(
    conn: &Connection,
    plan_id: i64,
    t: &TaskDraft,
    position: i32,
    now: &str,
) -> Result<i64, PlanError> {
    conn.execute(
        "INSERT INTO tasks (plan_id, name, summary, detail, has_subgoals, estimated_minutes, position, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            plan_id,
            t.name.trim(),
            default_text(&t.summary, &t.name),
            t.detail,
            t.has_subgoals,
            effective_minutes(t),
            position,
            TaskStatus::NotStarted.as_db(),
            now,
        ],
    )
    .map_err(db_err)?;
    let task_id = conn.last_insert_rowid();
    if t.has_subgoals {
        for (pos, s) in t.subgoals.iter().enumerate() {
            insert_subgoal(conn, task_id, s, pos as i32, now)?;
        }
    }
    Ok(task_id)
}

/// 插入一行子目标（insert_task 与 update_subgoals 追加共用；新行恒为未完成）。
fn insert_subgoal(
    conn: &Connection,
    task_id: i64,
    s: &SubGoalDraft,
    position: i32,
    now: &str,
) -> Result<(), PlanError> {
    conn.execute(
        "INSERT INTO subgoals (task_id, name, estimated_minutes, position, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![task_id, s.name.trim(), s.estimated_minutes, position, now],
    )
    .map_err(db_err)?;
    Ok(())
}

/// 编辑态子目标全量落库（任务行的 estimated_minutes 由外层 UPDATE 以 effective_minutes 归一）：
/// 已完成锁定（必须原样在场，否则 SubGoalLocked）；未完成可改可删（从草稿消失 = 删除）；
/// 新行按草稿序追加在已完成之后；取消勾选（has_subgoals=false）= 清空全部。
fn update_subgoals(
    conn: &Connection,
    task_id: i64,
    t: &TaskDraft,
    task_index: usize,
    now: &str,
) -> Result<(), PlanError> {
    let existing = load_subgoals(conn, task_id)?;
    if !t.has_subgoals {
        // UI 取消勾选已弹确认；已完成子目标不允许随清空消失（进度锁定）
        if existing.iter().any(|s| s.completed) {
            return Err(PlanError::SubGoalLocked { task_index });
        }
        conn.execute("DELETE FROM subgoals WHERE task_id = ?1", params![task_id])
            .map_err(db_err)?;
        return Ok(());
    }
    for s in existing.iter().filter(|s| s.completed) {
        let unchanged = t.subgoals.iter().any(|d| {
            d.id == Some(s.id) && d.name.trim() == s.name && d.estimated_minutes == s.estimated_minutes
        });
        if !unchanged {
            return Err(PlanError::SubGoalLocked { task_index });
        }
    }
    // id 归属预检：草稿携带的已知 id 必须属于本任务（跨任务/伪造 id 拒绝，防越权改行）
    for (j, d) in t.subgoals.iter().enumerate() {
        if d.id.is_some_and(|id| !existing.iter().any(|s| s.id == id)) {
            return Err(PlanError::SubGoalInvalid { task_index, subgoal_index: j });
        }
    }
    // 位置归一化：已完成按库内序在前（按序勾选保证其为前缀），未完成（含新行）按草稿序追加
    let mut ordered: Vec<&SubGoalDraft> = Vec::with_capacity(t.subgoals.len());
    for s in existing.iter().filter(|s| s.completed) {
        if let Some(d) = t.subgoals.iter().find(|d| d.id == Some(s.id)) {
            ordered.push(d);
        }
    }
    let is_done = |d: &SubGoalDraft| {
        d.id.is_some_and(|id| existing.iter().any(|s| s.id == id && s.completed))
    };
    ordered.extend(t.subgoals.iter().filter(|d| !is_done(d)));
    // 库中未完成但草稿未提交 = 单条删除（UI 已确认）
    for s in existing.iter().filter(|s| !s.completed) {
        if !t.subgoals.iter().any(|d| d.id == Some(s.id)) {
            conn.execute("DELETE FROM subgoals WHERE id = ?1", params![s.id])
                .map_err(db_err)?;
        }
    }
    for (pos, d) in ordered.iter().enumerate() {
        match d.id {
            Some(id) => {
                conn.execute(
                    "UPDATE subgoals SET name = ?1, estimated_minutes = ?2, position = ?3 WHERE id = ?4",
                    params![d.name.trim(), d.estimated_minutes, pos as i32, id],
                )
                .map_err(db_err)?;
            }
            None => insert_subgoal(conn, task_id, d, pos as i32, now)?,
        }
    }
    Ok(())
}

/// 依赖下标预检：越界在开事务/解析前拒绝（自指由环检测拒绝），避免解析时越界 panic。
fn check_dep_indices(tasks: &[TaskDraft]) -> Result<(), PlanError> {
    for (i, t) in tasks.iter().enumerate() {
        if t.depends_on.iter().any(|&pi| pi >= tasks.len()) {
            return Err(PlanError::DependencyInvalid { task_index: i });
        }
    }
    Ok(())
}

/// 草稿依赖 → 真实边：按下标取已解析的 id 逐条 link（同计划/自指/环校验在 link 内），
/// 环错误补上所属草稿任务下标供前端定位。
fn link_draft_deps(conn: &Connection, tasks: &[TaskDraft], ids: &[i64]) -> Result<(), PlanError> {
    for (i, t) in tasks.iter().enumerate() {
        for &pi in &t.depends_on {
            DependencyService::link(conn, ids[pi], ids[i]).map_err(|e| match e {
                PlanError::DependencyCycle { task_index: None } => {
                    PlanError::DependencyCycle { task_index: Some(i) }
                }
                e => e,
            })?;
        }
    }
    Ok(())
}

/// 任务的落库耗时：无子目标 = 手填值；有子目标 = 子目标求和（estimated_minutes 冗余存总量，
/// 列表/进度免 join；子目标变更时由同一 UPDATE 归一）。
fn effective_minutes(t: &TaskDraft) -> Option<u32> {
    if t.has_subgoals {
        Some(t.subgoals.iter().map(|s| s.estimated_minutes).sum())
    } else {
        t.estimated_minutes
    }
}

/// 加载未软删除任务（plan_filter 为 None 取全部计划），返回 (plan_id, TaskView)，按 position 排序。
/// 每张任务连带子目标与前置 id（load_tasks 是视图装配的唯一入口，列表与详情同构）。
fn load_tasks(conn: &Connection, plan_filter: Option<i64>) -> Result<Vec<(i64, TaskView)>, PlanError> {
    // Option<i64> 自身实现 ToSql（Some → 值），借 plan_filter 避免臂内临时值的生命周期问题
    let (sql, binds): (&str, Vec<&dyn rusqlite::ToSql>) = if plan_filter.is_some() {
        (
            "SELECT id, plan_id, name, summary, detail, has_subgoals, estimated_minutes, position, status
             FROM tasks WHERE deleted_at IS NULL AND plan_id = ?1 ORDER BY position",
            vec![&plan_filter],
        )
    } else {
        (
            "SELECT id, plan_id, name, summary, detail, has_subgoals, estimated_minutes, position, status
             FROM tasks WHERE deleted_at IS NULL ORDER BY plan_id, position",
            Vec::new(),
        )
    };
    let mut tasks: Vec<(i64, TaskView)> = conn
        .prepare(sql)
        .map_err(db_err)?
        .query_map(binds.as_slice(), task_row)
        .map_err(db_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_err)?;
    for (_, task) in tasks.iter_mut() {
        task.subgoals = load_subgoals(conn, task.id)?;
        task.progress_percent = PlanService::task_progress(conn, task.id)?.percent();
        let mut stmt = conn
            .prepare("SELECT predecessor_id FROM task_dependencies WHERE successor_id = ?1")
            .map_err(db_err)?;
        task.prerequisite_ids = stmt
            .query_map(params![task.id], |row| row.get::<_, i64>(0))
            .map_err(db_err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db_err)?;
    }
    Ok(tasks)
}

/// 加载任务的子目标（按 position 排序 = 填写顺序）。progress 域装配小看板视图复用。
pub(crate) fn load_subgoals(conn: &Connection, task_id: i64) -> Result<Vec<SubGoalView>, PlanError> {
    conn.prepare(
        "SELECT id, name, estimated_minutes, position, completed_at IS NOT NULL
         FROM subgoals WHERE task_id = ?1 ORDER BY position",
    )
    .map_err(db_err)?
    .query_map(params![task_id], subgoal_row)
    .map_err(db_err)?
    .collect::<rusqlite::Result<Vec<_>>>()
    .map_err(db_err)
}

/// plans 表查询列（get/list 共用，避免列清单两处漂移）
pub(crate) const PLAN_COLS: &str = "id, name, summary, detail, priority, due_date, status, created_at, pause_reason";

/// 默认排序（唯一排序）：Priority 降序 → CreatedAt 倒序 → id 倒序
fn default_order_cmp(a: &PlanView, b: &PlanView) -> std::cmp::Ordering {
    b.priority
        .cmp(&a.priority)
        .then_with(|| b.created_at.cmp(&a.created_at))
        .then_with(|| b.id.cmp(&a.id))
}

/// 计划的 (状态, 暂停原因)；生命周期服务判定转换合法性用（单行查询，不存在归 NotFound）。
pub(crate) fn load_plan_state(
    conn: &Connection,
    plan_id: i64,
) -> Result<(PlanStatus, Option<PauseReason>), PlanError> {
    conn.query_row(
        "SELECT status, pause_reason FROM plans WHERE id = ?1",
        params![plan_id],
        |row| {
            Ok((
                PlanStatus::from_db(&row.get::<_, String>(0)?),
                row.get::<_, Option<String>>(1)?.map(|s| PauseReason::from_db(&s)),
            ))
        },
    )
    .map_err(notfound_or_db)
}

/// plans 表行 → PlanView（tasks 由调用方填充）
fn plan_row(row: &Row) -> rusqlite::Result<PlanView> {
    Ok(PlanView {
        id: row.get(0)?,
        name: row.get(1)?,
        summary: row.get(2)?,
        detail: row.get(3)?,
        priority: Priority::from_db(&row.get::<_, String>(4)?),
        due_date: row.get(5)?,
        status: PlanStatus::from_db(&row.get::<_, String>(6)?),
        created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
            .unwrap()
            .with_timezone(&Local),
        pause_reason: row
            .get::<_, Option<String>>(8)?
            .map(|s| PauseReason::from_db(&s)),
        tasks: Vec::new(),
    })
}

/// tasks 表行 → (所属 plan_id, TaskView)（subgoals/prerequisite_ids/progress_percent 由 load_tasks 装配）
pub(crate) fn task_row(row: &Row) -> rusqlite::Result<(i64, TaskView)> {
    Ok((
        row.get(1)?,
        TaskView {
            id: row.get(0)?,
            name: row.get(2)?,
            summary: row.get(3)?,
            detail: row.get(4)?,
            has_subgoals: row.get::<_, i64>(5)? != 0,
            estimated_minutes: row.get(6)?,
            position: row.get(7)?,
            status: TaskStatus::from_db(&row.get::<_, String>(8)?),
            subgoals: Vec::new(),
            prerequisite_ids: Vec::new(),
            progress_percent: 0.0,
        },
    ))
}

/// subgoals 表行 → SubGoalView（completed_at IS NOT NULL 由 SQL 算好）
fn subgoal_row(row: &Row) -> rusqlite::Result<SubGoalView> {
    Ok(SubGoalView {
        id: row.get(0)?,
        name: row.get(1)?,
        estimated_minutes: row.get(2)?,
        position: row.get(3)?,
        completed: row.get::<_, i64>(4)? != 0,
    })
}

/// 任务集一致性：提交的已知 id 集合必须与库中现存任务一致（README：删除必须显式走 delete_task）。
fn check_task_set(existing: &[(i64, TaskView)], draft: &PlanDraft) -> Result<(), PlanError> {
    let mut db_ids: Vec<i64> = existing.iter().map(|(_, t)| t.id).collect();
    let mut draft_ids: Vec<i64> = draft.tasks.iter().filter_map(|t| t.id).collect();
    db_ids.sort_unstable();
    draft_ids.sort_unstable();
    if db_ids == draft_ids {
        Ok(())
    } else {
        Err(PlanError::TaskSetMismatch)
    }
}

/// 已完成任务内容锁定：字段必须与库中逐项一致（position 除外——归一化会重排）。
fn check_completed_locked(existing: &[(i64, TaskView)], draft: &PlanDraft) -> Result<(), PlanError> {
    for (i, t) in draft.tasks.iter().enumerate() {
        if let Some(id) = t.id {
            let modified = existing.iter().any(|(_, v)| {
                v.id == id
                    && v.status == TaskStatus::Completed
                    && (v.name != t.name.trim()
                        || v.summary != default_text(&t.summary, &t.name)
                        || v.detail != t.detail
                        || v.has_subgoals != t.has_subgoals
                        || v.estimated_minutes != effective_minutes(t))
            });
            if modified {
                return Err(PlanError::TaskLocked { index: i });
            }
        }
    }
    Ok(())
}

/// 简述留空（或全空白）时回退为名称——计划与任务共用（README 创建规则）
fn default_text<'a>(summary: &'a str, name: &'a str) -> &'a str {
    let trimmed = summary.trim();
    if trimmed.is_empty() {
        name.trim()
    } else {
        trimmed
    }
}

/// 单行查询无结果归 NotFound，其余 rusqlite 错误归存储兜底。
/// pub(crate)：progress 等同 crate 领域模块复用同一兜底
pub(crate) fn notfound_or_db(e: rusqlite::Error) -> PlanError {
    match e {
        rusqlite::Error::QueryReturnedNoRows => PlanError::NotFound,
        e => db_err(e),
    }
}

/// rusqlite 错误归入领域错误的兜底分支（校验错误之外的存储故障）；
/// pub(crate)：deps 等同 crate 领域模块复用同一兜底
pub(crate) fn db_err(e: rusqlite::Error) -> PlanError {
    PlanError::Storage(format!("{e}"))
}
