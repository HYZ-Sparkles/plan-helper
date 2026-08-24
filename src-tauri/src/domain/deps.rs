//! 任务依赖领域（ADR-0008 硬依赖边）：建边校验（同计划、无环）、删任务解除边、
//! 以及供大面板（工单 06）消费的"前置未完成列表 / 依赖是否就绪"判定。
//! 边没有状态转换，阻塞判定全部实时派生（不存任何冗余列）。

use std::collections::HashSet;

use rusqlite::{params, Connection};

use crate::domain::plans::{db_err, task_row, PlanError, TaskStatus, TaskView};

/// 依赖读写服务。
pub struct DependencyService;

impl DependencyService {
    /// 建立一条前置边 predecessor → successor（可在事务连接上调用）。
    /// 校验：两端存在且未软删除、同一计划（跨计划拒绝）、不自指、加入后不成环。
    pub fn link(conn: &Connection, predecessor_id: i64, successor_id: i64) -> Result<(), PlanError> {
        if predecessor_id == successor_id {
            return Err(PlanError::DependencyCycle { task_index: None }); // 自指 = 长度 1 的环
        }
        let (pred_plan, succ_plan): (Option<i64>, Option<i64>) = conn
            .query_row(
                "SELECT (SELECT plan_id FROM tasks WHERE id = ?1 AND deleted_at IS NULL),
                        (SELECT plan_id FROM tasks WHERE id = ?2 AND deleted_at IS NULL)",
                params![predecessor_id, successor_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(db_err)?;
        match (pred_plan, succ_plan) {
            (None, _) | (_, None) => return Err(PlanError::NotFound),
            (Some(p), Some(s)) if p != s => return Err(PlanError::DependencyCrossPlan),
            _ => {}
        }
        // 加边 P→S 成环 ⟺ 已存在沿后继方向的路径 S ⇝ P
        if reaches(conn, successor_id, predecessor_id)? {
            return Err(PlanError::DependencyCycle { task_index: None });
        }
        conn.execute(
            "INSERT OR IGNORE INTO task_dependencies (predecessor_id, successor_id) VALUES (?1, ?2)",
            params![predecessor_id, successor_id],
        )
        .map_err(db_err)?;
        Ok(())
    }

    /// 删除任务时解除其全部依赖边（双向：作为前置与作为后继），后继任务随之解锁。
    pub fn detach_task(conn: &Connection, task_id: i64) -> Result<(), PlanError> {
        conn.execute(
            "DELETE FROM task_dependencies WHERE predecessor_id = ?1 OR successor_id = ?1",
            params![task_id],
        )
        .map_err(db_err)?;
        Ok(())
    }

    /// 前置任务中尚未完成的列表（大面板"等待：任务A"置灰提示的数据源）。
    pub fn waiting_on(conn: &Connection, task_id: i64) -> Result<Vec<TaskView>, PlanError> {
        conn.prepare(
            "SELECT t.id, t.plan_id, t.name, t.summary, t.detail, t.has_subgoals,
                    t.estimated_minutes, t.position, t.status
             FROM task_dependencies d JOIN tasks t ON t.id = d.predecessor_id
             WHERE d.successor_id = ?1 AND t.deleted_at IS NULL AND t.status != ?2
             ORDER BY t.position",
        )
        .map_err(db_err)?
        .query_map(params![task_id, TaskStatus::Completed.as_db()], task_row)
        .map_err(db_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map(|rows| rows.into_iter().map(|(_, t)| t).collect())
        .map_err(db_err)
    }

    /// 依赖就绪判定（工单 06"任务当前是否可选入今日分配"的依赖侧）：前置全部已完成才可选。
    pub fn is_unblocked(conn: &Connection, task_id: i64) -> Result<bool, PlanError> {
        Ok(Self::waiting_on(conn, task_id)?.is_empty())
    }
}

/// 沿后继方向（predecessor → successor）判定 from 是否可达 to；迭代栈遍历，深图不递归。
fn reaches(conn: &Connection, from: i64, to: i64) -> Result<bool, PlanError> {
    let mut stmt = conn
        .prepare("SELECT successor_id FROM task_dependencies WHERE predecessor_id = ?1")
        .map_err(db_err)?;
    let mut stack = vec![from];
    let mut seen = HashSet::new();
    while let Some(cur) = stack.pop() {
        if cur == to {
            return Ok(true);
        }
        if !seen.insert(cur) {
            continue;
        }
        let successors = stmt
            .query_map(params![cur], |row| row.get::<_, i64>(0))
            .map_err(db_err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db_err)?;
        stack.extend(successors);
    }
    Ok(false)
}
