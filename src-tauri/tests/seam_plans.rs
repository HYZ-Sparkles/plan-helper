//! 计划领域接缝测试：创建校验规则、持久化、PlanOrdering 默认排序、重开数据库不丢（工单 02）。

use chrono::{Local, TimeZone};

use plan_helper_lib::clock::FixedClock;
use plan_helper_lib::domain::plans::{
    NewPlan, NewTask, PlanError, PlanService, PlanStatus, Priority, TaskStatus,
};
use plan_helper_lib::infra::db;

/// 固定时钟（本地时区）
fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> FixedClock {
    FixedClock(Local.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap())
}

/// 两任务的无子目标计划样例（第二个任务简述留空、未填详细内容）
fn sample_plan() -> NewPlan {
    NewPlan {
        name: "备考英语 6 级".into(),
        summary: String::new(),
        detail: "先词汇后真题".into(),
        priority: Priority::Medium,
        due_date: Some("2026-12-20".into()),
        tasks: vec![
            NewTask {
                name: "背完所有 6 级词汇".into(),
                summary: "词汇书两轮".into(),
                detail: String::new(),
                has_subgoals: false,
                estimated_minutes: Some(6000),
            },
            NewTask {
                name: "完成英语真题".into(),
                summary: String::new(),
                detail: String::new(),
                has_subgoals: false,
                estimated_minutes: Some(1200),
            },
        ],
    }
}

#[test]
fn create_persists_no_subgoal_plan() {
    // 测试情况：创建含两个无子目标任务的计划（计划简述留空、优先级 Medium、含截止日期）。
    // 正确结果：列表读回时字段一致——计划简述回退为计划名称、任务简述回退为任务名称、
    //          任务顺序 = 提交顺序（position 0,1）、计划状态 未开始、任务状态 未开始、
    //          预计耗时以分钟原样落库。
    let conn = db::open_in_memory().unwrap();
    PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &sample_plan()).unwrap();
    let plans = PlanService::list(&conn).unwrap();
    assert_eq!(plans.len(), 1);
    let p = &plans[0];
    assert_eq!(p.name, "备考英语 6 级");
    assert_eq!(p.summary, "备考英语 6 级"); // 简述留空 → 名称
    assert_eq!(p.detail, "先词汇后真题");
    assert_eq!(p.priority, Priority::Medium);
    assert_eq!(p.due_date.as_deref(), Some("2026-12-20"));
    assert_eq!(p.status, PlanStatus::NotStarted);
    assert_eq!(p.tasks.len(), 2);
    assert_eq!(p.tasks[0].name, "背完所有 6 级词汇");
    assert_eq!(p.tasks[0].summary, "词汇书两轮");
    assert_eq!(p.tasks[0].estimated_minutes, Some(6000));
    assert_eq!(p.tasks[0].position, 0);
    assert_eq!(p.tasks[0].status, TaskStatus::NotStarted);
    assert_eq!(p.tasks[1].summary, "完成英语真题"); // 任务简述留空 → 名称
    assert_eq!(p.tasks[1].position, 1);
}

#[test]
fn create_rejects_plan_without_tasks() {
    // 测试情况：保存没有任何任务的计划（spec 用户故事 8：不留空壳）。
    // 正确结果：被拒绝，错误为 NoTasks；且列表仍为空（事务未写入任何计划）。
    let conn = db::open_in_memory().unwrap();
    let mut plan = sample_plan();
    plan.tasks.clear();
    assert_eq!(
        PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &plan),
        Err(PlanError::NoTasks)
    );
    assert_eq!(PlanService::list(&conn).unwrap().len(), 0);
}

#[test]
fn create_rejects_task_missing_duration_or_name() {
    // 测试情况 A：无子目标任务未填预计耗时；情况 B：任务名称为空。
    // 正确结果：A 拒绝为 TaskNeedsDuration（index 指向第 2 个任务）；B 拒绝为 EmptyName。
    let conn = db::open_in_memory().unwrap();
    let mut plan = sample_plan();
    plan.tasks[1].estimated_minutes = None;
    assert_eq!(
        PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &plan),
        Err(PlanError::TaskNeedsDuration { index: 1 })
    );
    plan.tasks[1].name = "  ".into();
    assert_eq!(
        PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &plan).unwrap_err(),
        PlanError::EmptyName { task_index: Some(1) }
    );
}

#[test]
fn create_rejects_subgoal_task_without_subgoals() {
    // 测试情况：任务勾了"需要子目标"但没有子目标（工单 03 提供录入路径前的形态）。
    // 正确结果：拒绝为 SubGoalsRequired（spec 用户故事 8：勾了子目标至少 1 个子目标）。
    let conn = db::open_in_memory().unwrap();
    let mut plan = sample_plan();
    plan.tasks[0].has_subgoals = true;
    assert_eq!(
        PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &plan),
        Err(PlanError::SubGoalsRequired { index: 0 })
    );
}

#[test]
fn list_orders_priority_desc_then_created_desc() {
    // 测试情况：不同时刻创建三个不同优先级的计划，再晚些创建第二个 Medium 计划。
    // 正确结果：顺序为 Priority 降序（High → Medium → Low），同优先级内创建时间倒序
    //          （晚创建的 Medium 在前）——PlanOrdering 默认规则。
    let conn = db::open_in_memory().unwrap();
    let base = |name: &str, priority: Priority| NewPlan {
        name: name.into(),
        summary: String::new(),
        detail: String::new(),
        priority,
        due_date: None,
        tasks: vec![NewTask {
            name: "t".into(),
            summary: String::new(),
            detail: String::new(),
            has_subgoals: false,
            estimated_minutes: Some(60),
        }],
    };
    PlanService::create(&conn, &at(2026, 8, 20, 9, 0), &base("H", Priority::High)).unwrap();
    PlanService::create(&conn, &at(2026, 8, 21, 9, 0), &base("L", Priority::Low)).unwrap();
    PlanService::create(&conn, &at(2026, 8, 22, 9, 0), &base("M1", Priority::Medium)).unwrap();
    PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &base("M2", Priority::Medium)).unwrap();
    let names: Vec<String> = PlanService::list(&conn)
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, vec!["H", "M2", "M1", "L"]);
}

#[test]
fn plans_survive_database_reopen() {
    // 测试情况：文件库中创建计划后关闭连接（模拟应用退出），重新打开再读取。
    // 正确结果：计划与任务完整读回——重启应用数据不丢。
    let path = std::env::temp_dir().join(format!("plan-helper-seam-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        let conn = db::open(&path).unwrap();
        PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &sample_plan()).unwrap();
    }
    let conn = db::open(&path).unwrap();
    let plans = PlanService::list(&conn).unwrap();
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].tasks.len(), 2);
    let _ = std::fs::remove_file(&path);
}
