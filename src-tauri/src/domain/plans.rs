//! 计划领域：计划（Plan）与任务（Task）的创建、读取、编辑与软删除。
//! 状态机转换（工单 05）、子目标（工单 04）、依赖（工单 04）在此模型上扩展。

use chrono::{DateTime, Local};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

use crate::clock::Clock;

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

/// 任务草稿。无子目标任务必填预计耗时；有子目标时预计总耗时由子目标求和（工单 04）。
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
    /// 计划开始后优先级不可调整（spec 用户故事 12）
    PriorityLocked,
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
                return Err(PlanError::SubGoalsRequired { index: i });
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
        for (pos, t) in new.tasks.iter().enumerate() {
            insert_task(&tx, plan_id, t, pos as i32, &now)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(plan_id)
    }

    /// 整计划编辑（CreationUI 编辑态）：计划字段覆盖更新，任务按 id 更新、无 id 追加。
    /// 不变量（README 增删改规则 + spec 用户故事 9–12）：
    /// 优先级开始后锁定；已完成任务内容锁定；任务集与库一致（删除显式走 delete_task）；
    /// 落库顺序 = 未完成按提交序在前、已完成按库内序沉底。
    pub fn update(
        conn: &Connection,
        clock: &dyn Clock,
        plan_id: i64,
        draft: &PlanDraft,
    ) -> Result<(), PlanError> {
        draft.validate()?;
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
        let mut ordered: Vec<&TaskDraft> = draft.tasks.iter().filter(|t| !is_completed(t)).collect();
        // 已完成不可拖：顺序以库内 position 为准，而非提交序
        for id in &completed {
            if let Some(t) = draft.tasks.iter().find(|t| t.id == Some(*id)) {
                ordered.push(t);
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
        for (pos, t) in ordered.iter().enumerate() {
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
                }
                None => insert_task(&tx, plan_id, t, pos as i32, &now)?,
            }
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// 单个计划详情（含任务，按 position 排序）；不存在返回 NotFound。
    pub fn get(conn: &Connection, plan_id: i64) -> Result<PlanView, PlanError> {
        let mut plan = conn
            .query_row(
                "SELECT id, name, summary, detail, priority, due_date, status, created_at
                 FROM plans WHERE id = ?1",
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

    /// 软删除任务进归档（deleted_at 落删除时刻）；确认弹窗在 UI，这里是权威通道。
    pub fn delete_task(conn: &Connection, clock: &dyn Clock, task_id: i64) -> Result<(), PlanError> {
        let n = conn
            .execute(
                "UPDATE tasks SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
                params![clock.now().to_rfc3339(), task_id],
            )
            .map_err(db_err)?;
        if n == 0 {
            return Err(PlanError::NotFound); // 不存在或已删除，一律 NotFound（幂等）
        }
        Ok(())
    }

    /// 全部未软删除的计划（含任务），按 PlanOrdering 默认规则：
    /// Priority 降序 → CreatedAt 倒序 → id 倒序（同秒创建时新的在前）。
    pub fn list(conn: &Connection) -> Result<Vec<PlanView>, PlanError> {
        let mut plans = conn
            .prepare("SELECT id, name, summary, detail, priority, due_date, status, created_at FROM plans")
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
        plans.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| b.created_at.cmp(&a.created_at))
                .then_with(|| b.id.cmp(&a.id))
        });
        Ok(plans)
    }
}

/// 插入一张任务卡（create 与 update 追加共用；新任务状态恒为未开始）。
fn insert_task(
    conn: &Connection,
    plan_id: i64,
    t: &TaskDraft,
    position: i32,
    now: &str,
) -> Result<(), PlanError> {
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
    Ok(())
}

/// 无子目标任务的落库耗时；有子目标时由子目标求和（工单 04），当前恒 None。
fn effective_minutes(t: &TaskDraft) -> Option<u32> {
    if t.has_subgoals {
        None
    } else {
        t.estimated_minutes
    }
}

/// 加载未软删除任务（plan_filter 为 None 取全部计划），返回 (plan_id, TaskView)，按 position 排序。
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
    conn.prepare(sql)
        .map_err(db_err)?
        .query_map(binds.as_slice(), task_row)
        .map_err(db_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_err)
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
        tasks: Vec::new(),
    })
}

/// tasks 表行 → (所属 plan_id, TaskView)
fn task_row(row: &Row) -> rusqlite::Result<(i64, TaskView)> {
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
        },
    ))
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

/// 单行查询无结果归 NotFound，其余 rusqlite 错误归存储兜底
fn notfound_or_db(e: rusqlite::Error) -> PlanError {
    match e {
        rusqlite::Error::QueryReturnedNoRows => PlanError::NotFound,
        e => db_err(e),
    }
}

/// rusqlite 错误归入领域错误的兜底分支（校验错误之外的存储故障）
fn db_err(e: rusqlite::Error) -> PlanError {
    PlanError::Storage(format!("{e}"))
}
