//! 计划领域：计划（Plan）与任务（Task）的创建、读取与排序。
//! 状态机转换（工单 05）、子目标（工单 03）、依赖（工单 04）在此模型上扩展。

use chrono::{DateTime, Local};
use rusqlite::{params, Connection};
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

/// 创建计划的输入（UI 表单直传）。简述/detail 允许缺省。
#[derive(Debug, Deserialize)]
pub struct NewPlan {
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub detail: String,
    pub priority: Priority,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub tasks: Vec<NewTask>,
}

/// 创建任务的输入。无子目标任务必填预计耗时；预计总耗时在有子目标时由子目标求和（工单 03）。
#[derive(Debug, Deserialize)]
pub struct NewTask {
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

/// 计划创建的领域错误（command 边界序列化给前端展示）。
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
    /// 勾了子目标的任务至少 1 个子目标（工单 03 提供录入前必然拒绝）
    SubGoalsRequired { index: usize },
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

impl NewPlan {
    /// 保存前的领域校验（spec 用户故事 8：不留空壳）。UI 按段先校验，这里是权威兜底。
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
    pub fn create(conn: &Connection, clock: &dyn Clock, new: &NewPlan) -> Result<i64, PlanError> {
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
            tx.execute(
                "INSERT INTO tasks (plan_id, name, summary, detail, has_subgoals, estimated_minutes, position, status, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    plan_id,
                    t.name.trim(),
                    default_text(&t.summary, &t.name),
                    t.detail,
                    t.has_subgoals,
                    if t.has_subgoals { None } else { t.estimated_minutes },
                    pos as i32,
                    TaskStatus::NotStarted.as_db(),
                    now,
                ],
            )
            .map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(plan_id)
    }

    /// 全部未软删除的计划（含任务），按 PlanOrdering 默认规则：
    /// Priority 降序 → CreatedAt 倒序 → id 倒序（同秒创建时新的在前）。
    pub fn list(conn: &Connection) -> Result<Vec<PlanView>, PlanError> {
        let mut stmt = conn.prepare(
            "SELECT id, name, summary, detail, priority, due_date, status, created_at
             FROM plans ORDER BY id",
        )
        .map_err(db_err)?;
        let mut plans = stmt
            .query_map([], |row| {
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
            })
            .map_err(db_err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db_err)?;

        let mut task_stmt = conn.prepare(
            "SELECT id, plan_id, name, summary, detail, has_subgoals, estimated_minutes, position, status
             FROM tasks WHERE deleted_at IS NULL ORDER BY plan_id, position",
        )
        .map_err(db_err)?;
        let tasks = task_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(1)?,
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
            })
            .map_err(db_err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db_err)?;
        for (plan_id, task) in tasks {
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

/// 简述留空（或全空白）时回退为名称——计划与任务共用（README 创建规则）
fn default_text<'a>(summary: &'a str, name: &'a str) -> &'a str {
    let trimmed = summary.trim();
    if trimmed.is_empty() {
        name.trim()
    } else {
        trimmed
    }
}

/// rusqlite 错误归入领域错误的兜底分支（校验错误之外的存储故障）
fn db_err(e: rusqlite::Error) -> PlanError {
    PlanError::Storage(format!("{e}"))
}
