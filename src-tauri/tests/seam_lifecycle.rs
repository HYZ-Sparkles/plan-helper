//! 计划生命周期接缝测试（工单 05）：状态机全转换矩阵、暂停原因、任务 100% 自动完成、
//! 完成计划前置校验、复制并新建归零、手动排序持久化与优先级。


use plan_helper_lib::domain::lifecycle::LifecycleService;
use plan_helper_lib::domain::plans::{
    PauseReason, PlanDraft, PlanError, PlanService, PlanStatus, Priority, TaskDraft, TaskStatus,
};
use plan_helper_lib::infra::db;

mod common;
use common::{at, draft_of, force_plan_status, force_task_status, plain_task};

/// 两任务计划样例
fn sample(name: &str) -> PlanDraft {
    PlanDraft {
        name: name.into(),
        summary: String::new(),
        detail: "计划详情".into(),
        priority: Priority::Medium,
        due_date: Some("2026-12-20".into()),
        tasks: vec![plain_task("A", 60), plain_task("B", 30)],
    }
}

/// 直接勾完任务的全部子目标（按任务种子；单个子目标的种子见 common::force_subgoal_completed）
fn force_subgoals_completed(conn: &rusqlite::Connection, task_id: i64) {
    conn.execute(
        "UPDATE subgoals SET completed_at = '2026-08-24T10:00:00+08:00' WHERE task_id = ?1",
        rusqlite::params![task_id],
    )
    .unwrap();
}

/// 计划当前状态（断言用）
fn status_of(conn: &rusqlite::Connection, plan_id: i64) -> PlanStatus {
    conn.query_row("SELECT status FROM plans WHERE id = ?1", rusqlite::params![plan_id], |r| {
        Ok(PlanStatus::from_db(&r.get::<_, String>(0)?))
    })
    .unwrap()
}

#[test]
fn state_machine_allows_only_waterfall_transitions() {
    // 测试情况：五个初始状态 × 四个无前置的操作（start/pause/resume/abort）全矩阵调用；
    //          complete 因需要任务全部完成，单独在下方覆盖。
    // 正确结果：只有 未开始→start、进行中→pause/abort、已暂停→resume/abort 合法并落库目标状态；
    //          其余一律拒绝 PlanStatusInvalid{from}（含终态重启、未开始放弃、已暂停暂停等），
    //          且拒绝后状态不变。
    let legal = |op: &str, from: PlanStatus| {
        matches!(
            (op, from),
            ("start", PlanStatus::NotStarted)
                | ("pause", PlanStatus::Active)
                | ("abort", PlanStatus::Active)
                | ("resume", PlanStatus::Paused)
                | ("abort", PlanStatus::Paused)
        )
    };
    let to_of = |op: &str| match op {
        "start" | "resume" => PlanStatus::Active,
        "pause" => PlanStatus::Paused,
        _ => PlanStatus::Abandoned,
    };
    for from in [
        PlanStatus::NotStarted,
        PlanStatus::Active,
        PlanStatus::Paused,
        PlanStatus::Completed,
        PlanStatus::Abandoned,
    ] {
        for op in ["start", "pause", "resume", "abort"] {
            let conn = db::open_in_memory().unwrap();
            let id = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &sample("矩阵")).unwrap();
            force_plan_status(&conn, id, from);
            let result = match op {
                "start" => LifecycleService::start(&conn, id),
                "pause" => LifecycleService::pause(&conn, id),
                "resume" => LifecycleService::resume(&conn, id),
                _ => LifecycleService::abort(&conn, id),
            };
            if legal(op, from) {
                result.unwrap();
                assert_eq!(status_of(&conn, id), to_of(op), "{op} from {from:?}");
            } else {
                assert_eq!(result.unwrap_err(), PlanError::PlanStatusInvalid { from }, "{op} from {from:?}");
                assert_eq!(status_of(&conn, id), from, "拒绝后状态不变：{op} from {from:?}");
            }
        }
    }
}

#[test]
fn lifecycle_operations_on_missing_plan_are_notfound() {
    // 测试情况：对不存在的计划 id 调用生命周期操作。
    // 正确结果：全部返回 NotFound（含 copy_as_new 与 complete）。
    let conn = db::open_in_memory().unwrap();
    assert_eq!(LifecycleService::start(&conn, 999), Err(PlanError::NotFound));
    assert_eq!(LifecycleService::pause(&conn, 999), Err(PlanError::NotFound));
    assert_eq!(LifecycleService::resume(&conn, 999), Err(PlanError::NotFound));
    assert_eq!(LifecycleService::complete(&conn, 999), Err(PlanError::NotFound));
    assert_eq!(LifecycleService::abort(&conn, 999), Err(PlanError::NotFound));
    assert_eq!(
        LifecycleService::copy_as_new(&conn, &at(2026, 8, 24, 9, 0), 999),
        Err(PlanError::NotFound)
    );
}

#[test]
fn manual_pause_records_reason_and_resume_clears_it() {
    // 测试情况：进行中计划手动暂停，再恢复。
    // 正确结果：暂停后 pause_reason = UserInitiated；恢复后回到进行中且 pause_reason 清空。
    let conn = db::open_in_memory().unwrap();
    let id = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &sample("暂停原因")).unwrap();
    LifecycleService::start(&conn, id).unwrap();
    LifecycleService::pause(&conn, id).unwrap();
    let p = PlanService::get(&conn, id).unwrap();
    assert_eq!(p.status, PlanStatus::Paused);
    assert_eq!(p.pause_reason, Some(PauseReason::UserInitiated));
    LifecycleService::resume(&conn, id).unwrap();
    let p = PlanService::get(&conn, id).unwrap();
    assert_eq!(p.status, PlanStatus::Active);
    assert_eq!(p.pause_reason, None);
}

#[test]
fn complete_requires_all_tasks_completed_then_is_terminal() {
    // 测试情况 A：进行中计划仍有未完成任务时点完成；情况 B：任务被删空后点完成；
    //             情况 C：全部任务完成后点完成；情况 D：完成后再点任何生命周期操作。
    // 正确结果：A/B 拒绝 TasksNotCompleted 且状态停留进行中；C 成功转已完成；
    //          D 全部拒绝 PlanStatusInvalid（终态不可重启，含 complete 自身）。
    let conn = db::open_in_memory().unwrap();
    let id = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &sample("完成计划")).unwrap();
    LifecycleService::start(&conn, id).unwrap();
    assert_eq!(LifecycleService::complete(&conn, id), Err(PlanError::TasksNotCompleted));

    let tasks = PlanService::get(&conn, id).unwrap().tasks;
    for t in &tasks {
        force_task_status(&conn, t.id, TaskStatus::Completed);
    }
    LifecycleService::complete(&conn, id).unwrap();
    assert_eq!(status_of(&conn, id), PlanStatus::Completed);

    for result in [
        LifecycleService::start(&conn, id),
        LifecycleService::pause(&conn, id),
        LifecycleService::resume(&conn, id),
        LifecycleService::complete(&conn, id),
        LifecycleService::abort(&conn, id),
    ] {
        assert_eq!(result.unwrap_err(), PlanError::PlanStatusInvalid { from: PlanStatus::Completed });
    }

    let emptied = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &sample("删空")).unwrap();
    LifecycleService::start(&conn, emptied).unwrap();
    for t in PlanService::get(&conn, emptied).unwrap().tasks {
        PlanService::delete_task(&conn, &at(2026, 8, 24, 9, 0), t.id).unwrap();
    }
    assert_eq!(LifecycleService::complete(&conn, emptied), Err(PlanError::TasksNotCompleted));
}

#[test]
fn task_auto_completes_at_100_percent_and_is_idempotent() {
    // 测试情况：有子目标任务——部分勾选后同步、全部勾选后同步、已完成后再同步；
    //          无子目标任务（ProgressLog 未接线、分子恒 0）同步。
    // 正确结果：部分勾选保持未开始；100% 自动转已完成；重复同步仍已完成不报错；
    //          无子目标任务不会被空分子误判完成。
    let conn = db::open_in_memory().unwrap();
    let mut draft = sample("自动完成");
    draft.tasks[0] = TaskDraft {
        id: None,
        name: "子目标任务".into(),
        summary: String::new(),
        detail: String::new(),
        has_subgoals: true,
        estimated_minutes: None,
        subgoals: vec![
            plan_helper_lib::domain::plans::SubGoalDraft { id: None, name: "s1".into(), estimated_minutes: 60 },
            plan_helper_lib::domain::plans::SubGoalDraft { id: None, name: "s2".into(), estimated_minutes: 30 },
        ],
        depends_on: Vec::new(),
    };
    let id = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &draft).unwrap();
    let tasks = PlanService::get(&conn, id).unwrap().tasks;
    let subgoal_task = tasks[0].id;
    let plain_task = tasks[1].id;

    // 只勾第一个子目标（60/90）——未到 100%
    conn.execute(
        "UPDATE subgoals SET completed_at = '2026-08-24T10:00:00+08:00'
         WHERE task_id = ?1 AND position = 0",
        rusqlite::params![subgoal_task],
    )
    .unwrap();
    assert_eq!(
        LifecycleService::sync_task_completion(&conn, subgoal_task).unwrap(),
        TaskStatus::NotStarted
    );

    // 全部勾完（90/90 = 100%）——自动转已完成；重复同步幂等
    force_subgoals_completed(&conn, subgoal_task);
    assert_eq!(
        LifecycleService::sync_task_completion(&conn, subgoal_task).unwrap(),
        TaskStatus::Completed
    );
    assert_eq!(
        LifecycleService::sync_task_completion(&conn, subgoal_task).unwrap(),
        TaskStatus::Completed
    );

    // 无子目标任务：ProgressLog 接线前分子恒 0，不误判
    assert_eq!(
        LifecycleService::sync_task_completion(&conn, plain_task).unwrap(),
        TaskStatus::NotStarted
    );
    assert_eq!(
        LifecycleService::sync_task_completion(&conn, 999),
        Err(PlanError::NotFound)
    );
}

#[test]
fn copy_as_new_resets_progress_and_starts_active() {
    // 测试情况：已完成的计划（任务全部完成、子目标全勾、含依赖边 B 前置 A）复制并新建；
    //          再对非终态计划复制。
    // 正确结果：新计划名称加"- 副本"、字段（简述/详细/优先级/截止日期）与任务/子目标/依赖全复制；
    //          进度归零（任务未开始、子目标 completed=false）、状态直接进行中、优先级开始后锁定；
    //          旧计划保持已完成且进度留存；非终态复制拒绝 PlanNotTerminal。
    let conn = db::open_in_memory().unwrap();
    let mut draft = sample("备考英语 6 级");
    draft.summary = "词汇书两轮".into();
    draft.tasks[0] = TaskDraft {
        id: None,
        name: "词汇".into(),
        summary: String::new(),
        detail: String::new(),
        has_subgoals: true,
        estimated_minutes: None,
        subgoals: vec![
            plan_helper_lib::domain::plans::SubGoalDraft { id: None, name: "list 1".into(), estimated_minutes: 60 },
            plan_helper_lib::domain::plans::SubGoalDraft { id: None, name: "list 2".into(), estimated_minutes: 60 },
        ],
        // B（真题，下标 1）依赖词汇
        depends_on: Vec::new(),
    };
    draft.tasks[1].depends_on = vec![0];
    let id = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &draft).unwrap();

    // 非终态拒绝
    let err = LifecycleService::copy_as_new(&conn, &at(2026, 8, 24, 9, 0), id).unwrap_err();
    assert_eq!(err, PlanError::PlanNotTerminal { from: PlanStatus::NotStarted });

    // 推到终态：开始 → 任务全完成（子目标勾完模拟真实完成路径）→ 完成计划
    LifecycleService::start(&conn, id).unwrap();
    let old_tasks = PlanService::get(&conn, id).unwrap().tasks;
    force_subgoals_completed(&conn, old_tasks[0].id);
    for t in &old_tasks {
        force_task_status(&conn, t.id, TaskStatus::Completed);
    }
    LifecycleService::complete(&conn, id).unwrap();

    let new_id = LifecycleService::copy_as_new(&conn, &at(2026, 8, 25, 9, 0), id).unwrap();
    let new_plan = PlanService::get(&conn, new_id).unwrap();
    assert_eq!(new_plan.name, "备考英语 6 级 - 副本");
    assert_eq!(new_plan.summary, "词汇书两轮");
    assert_eq!(new_plan.detail, "计划详情");
    assert_eq!(new_plan.priority, Priority::Medium);
    assert_eq!(new_plan.due_date.as_deref(), Some("2026-12-20"));
    assert_eq!(new_plan.status, PlanStatus::Active); // 直接进入进行中
    assert_eq!(new_plan.tasks.len(), 2);
    assert_eq!(new_plan.tasks[0].name, "词汇");
    assert_eq!(new_plan.tasks[0].status, TaskStatus::NotStarted); // 进度归零
    assert!(new_plan.tasks[0].subgoals.iter().all(|s| !s.completed));
    // 依赖边随复制：真题仍前置词汇
    assert_eq!(new_plan.tasks[1].prerequisite_ids, vec![new_plan.tasks[0].id]);

    // 旧计划保持终态、进度留存
    let old = PlanService::get(&conn, id).unwrap();
    assert_eq!(old.status, PlanStatus::Completed);
    assert!(old.tasks.iter().all(|t| t.status == TaskStatus::Completed));
    assert!(old.tasks[0].subgoals.iter().all(|s| s.completed));

    // 新计划已开始：优先级锁定（复制即进行中的直接后果）
    let mut edited = draft_of(&new_plan);
    edited.priority = Priority::High;
    assert_eq!(
        PlanService::update(&conn, &at(2026, 8, 25, 9, 30), new_id, &edited),
        Err(PlanError::PriorityLocked)
    );
}

#[test]
fn manual_order_overrides_default_and_persists() {
    // 测试情况：按优先级默认排序的三个计划（H/M/L），手动持久化倒序；
    //          之后新建计划；再重新手动排序。
    // 正确结果：手动序优先于默认序（倒序生效）；新建计划（无 override）排在手动序之后；
    //          再次手动排序覆盖旧序；重开数据库顺序不丢。
    let conn = db::open_in_memory().unwrap();
    let mut h = sample("H"); h.priority = Priority::High;
    let mut m = sample("M"); m.priority = Priority::Medium;
    let mut l = sample("L"); l.priority = Priority::Low;
    let hid = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &h).unwrap();
    let mid = PlanService::create(&conn, &at(2026, 8, 23, 9, 1), &m).unwrap();
    let lid = PlanService::create(&conn, &at(2026, 8, 23, 9, 2), &l).unwrap();
    let names = || PlanService::list(&conn).unwrap().into_iter().map(|p| p.name).collect::<Vec<_>>();
    assert_eq!(names(), vec!["H", "M", "L"]); // 默认：优先级降序

    PlanService::set_order(&conn, &[lid, mid, hid]).unwrap();
    assert_eq!(names(), vec!["L", "M", "H"]); // 手动序接管

    let mut n = sample("N"); n.priority = Priority::High;
    let nid = PlanService::create(&conn, &at(2026, 8, 24, 9, 0), &n).unwrap();
    assert_eq!(names(), vec!["L", "M", "H", "N"]); // 新计划落在手动序之后

    PlanService::set_order(&conn, &[nid, hid, mid, lid]).unwrap();
    assert_eq!(names(), vec!["N", "H", "M", "L"]); // 重新手动排序覆盖

    // 重开库（模拟重启）：手动序持久化
    let path = std::env::temp_dir().join(format!("plan-helper-order-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        let file_conn = db::open(&path).unwrap();
        PlanService::create(&file_conn, &at(2026, 8, 23, 9, 0), &h).unwrap();
        PlanService::create(&file_conn, &at(2026, 8, 23, 9, 1), &m).unwrap();
        PlanService::set_order(&file_conn, &[mid, hid]).unwrap();
    }
    let file_conn = db::open(&path).unwrap();
    assert_eq!(
        PlanService::list(&file_conn).unwrap().into_iter().map(|p| p.name).collect::<Vec<_>>(),
        vec!["M", "H"]
    );
    let _ = std::fs::remove_file(&path);
}
