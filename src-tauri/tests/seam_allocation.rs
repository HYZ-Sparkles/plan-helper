//! 今日分配接缝测试（工单 06）：候选分组的依赖过滤、当日分配存取与覆盖重选、
//! 提交校验（不可选任务拒绝）、自动打开判定三条件、隔日分配失效。

use plan_helper_lib::domain::allocation::AllocationService;
use plan_helper_lib::domain::lifecycle::LifecycleService;
use plan_helper_lib::domain::plans::{
    PlanDraft, PlanError, PlanService, Priority, SubGoalDraft, TaskStatus,
};
use plan_helper_lib::infra::db;

mod common;
use common::{at, force_task_status, plain_task};

/// 三任务计划草稿：A（60 分）、B 依赖 A（30 分）、SG 带两个子目标（60+30 分）
fn sample(name: &str) -> PlanDraft {
    let mut b = plain_task("B", 30);
    b.depends_on = vec![0];
    let mut sg = plain_task("SG", 30);
    sg.has_subgoals = true;
    sg.estimated_minutes = None;
    sg.subgoals = vec![
        SubGoalDraft { id: None, name: "读第一章".into(), estimated_minutes: 60 },
        SubGoalDraft { id: None, name: "读第二章".into(), estimated_minutes: 30 },
    ];
    PlanDraft {
        name: name.into(),
        summary: String::new(),
        detail: String::new(),
        priority: Priority::Medium,
        due_date: None,
        tasks: vec![plain_task("A", 60), b, sg],
    }
}

#[test]
fn board_groups_active_plans_selectable_only() {
    // 测试情况：高、中两个进行中计划 + 一个未开始的同内容计划；进行中计划内含
    //           依赖边 B→A（A 未完成）、一个已完成任务（force 状态）。
    // 正确结果：分组只含进行中计划且高优先级在前（PlanOrdering）；未开始计划不出现；
    //           被阻塞的 B 不进候选（只展示可选任务），A 完成后 B 实时回到列表；
    //           已完成任务不进候选；有子目标任务预计耗时 = 子目标求和（90）。
    let conn = db::open_in_memory().unwrap();
    let mut high = sample("高优先计划");
    high.priority = Priority::High;
    let p_high = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &high).unwrap();
    let p_mid = PlanService::create(&conn, &at(2026, 8, 22, 9, 0), &sample("中优先计划")).unwrap();
    PlanService::create(&conn, &at(2026, 8, 21, 9, 0), &sample("未开始计划")).unwrap();
    LifecycleService::start(&conn, p_high).unwrap();
    LifecycleService::start(&conn, p_mid).unwrap();

    let v = AllocationService::board(&conn, &at(2026, 8, 24, 9, 0)).unwrap();
    assert_eq!(v.date, "2026-08-24");
    assert_eq!(v.target_minutes, 300, "FirstRun 默认基准 5h");
    assert_eq!(v.selected_task_ids, Vec::<i64>::new(), "尚未分配");
    assert_eq!(v.groups.len(), 2, "未开始计划不进候选");
    assert_eq!(v.groups[0].plan_name, "高优先计划", "Priority 降序");
    assert_eq!(v.groups[0].priority, Priority::High);

    let names = |g: usize| v.groups[g].tasks.iter().map(|t| t.name.as_str()).collect::<Vec<_>>();
    assert_eq!(names(1), vec!["A", "SG"], "被阻塞的 B 不进候选，其余按 position 排");
    let sg = v.groups[1].tasks.iter().find(|t| t.name == "SG").unwrap();
    assert_eq!(sg.estimated_minutes, 90, "子目标任务耗时 = 子目标求和");
    let a_id = v.groups[1].tasks.iter().find(|t| t.name == "A").unwrap().id;

    force_task_status(&conn, a_id, TaskStatus::Completed);
    let v2 = AllocationService::board(&conn, &at(2026, 8, 24, 9, 0)).unwrap();
    let names2 = v2.groups[1].tasks.iter().map(|t| t.name.as_str()).collect::<Vec<_>>();
    assert_eq!(names2, vec!["B", "SG"], "A 完成后不进候选，B 实时解锁进入");
}

#[test]
fn commit_persists_overwrites_and_board_roundtrips() {
    // 测试情况：提交一组选中任务后读面板，再提交另一组（覆盖重选）后读面板。
    // 正确结果：回显集与最后一次提交一致（整行覆盖，不是追加）；
    //           含被阻塞任务的提交被拒且不落库。
    let conn = db::open_in_memory().unwrap();
    let p = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &sample("分配")).unwrap();
    LifecycleService::start(&conn, p).unwrap();
    let v = AllocationService::board(&conn, &at(2026, 8, 24, 9, 0)).unwrap();
    let ids: Vec<i64> = v.groups[0].tasks.iter().map(|t| t.id).collect();
    let (a, sg) = (ids[0], ids[1]); // 候选 = A、SG（B 被阻塞不展示）
    let b: i64 = conn
        .query_row("SELECT id FROM tasks WHERE plan_id = ?1 AND name = 'B'", rusqlite::params![p], |r| r.get(0))
        .unwrap();

    AllocationService::commit(&conn, &at(2026, 8, 24, 10, 0), &[a, sg]).unwrap();
    let v = AllocationService::board(&conn, &at(2026, 8, 24, 12, 0)).unwrap();
    assert_eq!(v.selected_task_ids, vec![a, sg], "当日任意时刻读都回显今日提交");

    AllocationService::commit(&conn, &at(2026, 8, 24, 14, 0), &[sg]).unwrap();
    let v = AllocationService::board(&conn, &at(2026, 8, 24, 15, 0)).unwrap();
    assert_eq!(v.selected_task_ids, vec![sg], "再次提交整行覆盖（重开重选）");

    // B 前置 A 未完成（面板上不可见）：直接提交被服务层拒绝，之前的分配保持不变
    assert_eq!(
        AllocationService::commit(&conn, &at(2026, 8, 24, 16, 0), &[b]),
        Err(PlanError::TaskNotAllocatable { task_id: b })
    );
    let v = AllocationService::board(&conn, &at(2026, 8, 24, 17, 0)).unwrap();
    assert_eq!(v.selected_task_ids, vec![sg], "被拒的提交不污染已有分配");
}

#[test]
fn commit_rejects_non_selectable_tasks() {
    // 测试情况：分别提交 前置未完成的任务（面板上不可见）/ 已完成任务 /
    //           未开始计划里的任务 / 不存在 id。
    // 正确结果：全部拒绝 TaskNotAllocatable{task_id}（UI 失步的后端兜底）。
    let conn = db::open_in_memory().unwrap();
    let p_active = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &sample("进行中")).unwrap();
    let p_idle = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &sample("未开始")).unwrap();
    LifecycleService::start(&conn, p_active).unwrap();

    // 计划内任务 id（按名取，被过滤的照常可查）
    let id_of = |name: &str, plan: i64| {
        conn.query_row(
            "SELECT id FROM tasks WHERE plan_id = ?1 AND name = ?2",
            rusqlite::params![plan, name],
            |r| r.get(0),
        )
        .unwrap()
    };
    let a = id_of("A", p_active);
    let b = id_of("B", p_active); // 被阻塞（前置 A 未完成）
    let sg = id_of("SG", p_active);
    force_task_status(&conn, sg, TaskStatus::Completed); // 已完成（A 保持未完成，B 的阻塞才有意义）
    let idle_id = id_of("A", p_idle);

    let clock = at(2026, 8, 24, 9, 0);
    assert!(AllocationService::commit(&conn, &clock, &[a]).is_ok(), "可选任务正常提交");
    for bad in [b, sg, idle_id, 999] {
        assert_eq!(
            AllocationService::commit(&conn, &clock, &[bad]),
            Err(PlanError::TaskNotAllocatable { task_id: bad })
        );
    }
}

#[test]
fn should_auto_open_requires_workmode_workday_and_unallocated() {
    // 测试情况：2026-08-24 是周一（默认工作日周一至五）、2026-08-23 是周日；
    //           逐一打破 工作模式 / 工作日 / 未分配 三个条件后调用判定。
    // 正确结果：三条件齐备才 true——休息模式 false、周日 false、已分配 false、齐备 true；
    //           面板视图的 workday 标记随周循环翻转（周日 false、周一 true）。
    let conn = db::open_in_memory().unwrap();
    let p = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &sample("判定")).unwrap();
    LifecycleService::start(&conn, p).unwrap();
    let v = AllocationService::board(&conn, &at(2026, 8, 24, 8, 0)).unwrap();
    let a = v.groups[0].tasks[0].id;

    assert!(!AllocationService::should_auto_open(&conn, &at(2026, 8, 24, 8, 0), false).unwrap());
    assert!(!AllocationService::should_auto_open(&conn, &at(2026, 8, 23, 8, 0), true).unwrap(),
        "周日不在默认工作日");
    assert!(!AllocationService::board(&conn, &at(2026, 8, 23, 8, 0)).unwrap().workday,
        "周日面板视图标记非工作日（分配不加载的 UI 判据）");
    assert!(AllocationService::should_auto_open(&conn, &at(2026, 8, 24, 8, 0), true).unwrap());

    AllocationService::commit(&conn, &at(2026, 8, 24, 8, 30), &[a]).unwrap();
    assert!(!AllocationService::should_auto_open(&conn, &at(2026, 8, 24, 9, 0), true).unwrap(),
        "今日已分配不触发");
}

#[test]
fn allocation_expires_next_day() {
    // 测试情况：8/24 提交分配，8/25（周二，工作日）再读面板与判定。
    // 正确结果：隔日回显为空、自动打开判定回到 true——分配只属于它的日期，
    //           新工作日重新分配（不残留昨日选择）。
    let conn = db::open_in_memory().unwrap();
    let p = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &sample("隔日")).unwrap();
    LifecycleService::start(&conn, p).unwrap();
    let a = AllocationService::board(&conn, &at(2026, 8, 24, 8, 0)).unwrap().groups[0].tasks[0].id;
    AllocationService::commit(&conn, &at(2026, 8, 24, 8, 30), &[a]).unwrap();

    let next = AllocationService::board(&conn, &at(2026, 8, 25, 8, 0)).unwrap();
    assert_eq!(next.date, "2026-08-25");
    assert_eq!(next.selected_task_ids, Vec::<i64>::new());
    assert!(AllocationService::should_auto_open(&conn, &at(2026, 8, 25, 8, 0), true).unwrap());
}
