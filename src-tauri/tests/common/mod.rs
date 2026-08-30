//! 集成测试共用种子与助手：各 seam 测试二进制以 `mod common; use common::*;` 引入
//!（Rust 集成测试每个文件是独立二进制，公共助手走这里而不是重复拷贝）。
#![allow(dead_code)] // 各二进制只用其中一部分，不因未用告警

use chrono::{Local, TimeZone};

use plan_helper_lib::clock::FixedClock;
use plan_helper_lib::domain::plans::{
    PauseReason, PlanDraft, PlanStatus, PlanView, SubGoalDraft, TaskDraft, TaskStatus,
};

/// 固定时钟（本地时区）
pub fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> FixedClock {
    FixedClock(Local.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap())
}

/// 无子目标、无依赖的普通任务草稿
pub fn plain_task(name: &str, minutes: u32) -> TaskDraft {
    TaskDraft {
        id: None,
        name: name.into(),
        summary: String::new(),
        detail: String::new(),
        has_subgoals: false,
        estimated_minutes: Some(minutes),
        subgoals: Vec::new(),
        depends_on: Vec::new(),
    }
}

/// 从库中视图构造编辑草稿（UI 编辑态装载的镜像：任务与子目标都带 id）
pub fn draft_of(p: &PlanView) -> PlanDraft {
    PlanDraft {
        name: p.name.clone(),
        summary: p.summary.clone(),
        detail: p.detail.clone(),
        priority: p.priority,
        due_date: p.due_date.clone(),
        tasks: p
            .tasks
            .iter()
            .map(|t| TaskDraft {
                id: Some(t.id),
                name: t.name.clone(),
                summary: t.summary.clone(),
                detail: t.detail.clone(),
                has_subgoals: t.has_subgoals,
                estimated_minutes: t.estimated_minutes,
                subgoals: t
                    .subgoals
                    .iter()
                    .map(|s| SubGoalDraft {
                        id: Some(s.id),
                        name: s.name.clone(),
                        estimated_minutes: s.estimated_minutes,
                    })
                    .collect(),
                depends_on: Vec::new(),
            })
            .collect(),
    }
}

/// 直接落库计划状态（状态机转换前置的种子：测试用 SQL 制造"已开始"等状态）
pub fn force_plan_status(conn: &rusqlite::Connection, plan_id: i64, status: PlanStatus) {
    conn.execute("UPDATE plans SET status = ?1 WHERE id = ?2", rusqlite::params![status.as_db(), plan_id])
        .unwrap();
}

/// 直接落库任务状态（汇报路径工单 07 接通；测试用 SQL 制造已完成任务这一前置状态）
pub fn force_task_status(conn: &rusqlite::Connection, task_id: i64, status: TaskStatus) {
    conn.execute("UPDATE tasks SET status = ?1 WHERE id = ?2", rusqlite::params![status.as_db(), task_id])
        .unwrap();
}

/// 直接落库子目标完成态（同上：制造"已完成子目标"这一前置状态）
pub fn force_subgoal_completed(conn: &rusqlite::Connection, subgoal_id: i64) {
    conn.execute(
        "UPDATE subgoals SET completed_at = '2026-08-24T10:00:00+08:00' WHERE id = ?1",
        rusqlite::params![subgoal_id],
    )
    .unwrap();
}

/// 直接落库暂停原因（工单 11 总结的"已被抢占暂停"标注 / 工单 12 抢占路径的种子；
/// None = 清空，与 resume 同效）
pub fn force_pause_reason(conn: &rusqlite::Connection, plan_id: i64, reason: Option<PauseReason>) {
    conn.execute(
        "UPDATE plans SET pause_reason = ?1 WHERE id = ?2",
        rusqlite::params![reason.map(|r| r.as_db().to_string()), plan_id],
    )
    .unwrap();
}
