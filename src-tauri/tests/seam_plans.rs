//! 计划领域接缝测试：创建校验规则、持久化、PlanOrdering 默认排序、整计划编辑不变量、
//! 软删除归档、重开数据库不丢（工单 02/03）。

use chrono::{Local, TimeZone};

use plan_helper_lib::clock::FixedClock;
use plan_helper_lib::domain::plans::{
    PlanDraft, PlanError, PlanService, PlanStatus, Priority, TaskDraft, TaskStatus,
};
use plan_helper_lib::infra::db;

/// 固定时钟（本地时区）
fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> FixedClock {
    FixedClock(Local.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap())
}

/// 任务草稿快捷构造（无 id = 新任务）
fn task(name: &str, minutes: u32) -> TaskDraft {
    TaskDraft {
        id: None,
        name: name.into(),
        summary: String::new(),
        detail: String::new(),
        has_subgoals: false,
        estimated_minutes: Some(minutes),
    }
}

/// 两任务的无子目标计划样例（第二个任务简述留空、未填详细内容）
fn sample_plan() -> PlanDraft {
    let mut a = task("背完所有 6 级词汇", 6000);
    a.summary = "词汇书两轮".into();
    let mut b = task("完成英语真题", 1200);
    b.summary = String::new();
    PlanDraft {
        name: "备考英语 6 级".into(),
        summary: String::new(),
        detail: "先词汇后真题".into(),
        priority: Priority::Medium,
        due_date: Some("2026-12-20".into()),
        tasks: vec![a, b],
    }
}

/// 从库中视图构造编辑草稿（UI 编辑态装载的镜像：任务带 id）
fn draft_of(p: &plan_helper_lib::domain::plans::PlanView) -> PlanDraft {
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
            })
            .collect(),
    }
}

/// 直接落库任务状态（汇报路径工单 07 才接通；测试用 SQL 制造已完成任务这一前置状态）
fn force_task_status(conn: &rusqlite::Connection, task_id: i64, status: TaskStatus) {
    conn.execute("UPDATE tasks SET status = ?1 WHERE id = ?2", rusqlite::params![status.as_db(), task_id])
        .unwrap();
}

/// 直接落库计划状态（状态机转换工单 05 才接通；测试用 SQL 制造"已开始"这一前置状态）
fn force_plan_status(conn: &rusqlite::Connection, plan_id: i64, status: PlanStatus) {
    conn.execute("UPDATE plans SET status = ?1 WHERE id = ?2", rusqlite::params![status.as_db(), plan_id])
        .unwrap();
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
    // 测试情况：任务勾了"需要子目标"但没有子目标（工单 04 提供录入路径前的形态）。
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
    let base = |name: &str, priority: Priority| PlanDraft {
        name: name.into(),
        summary: String::new(),
        detail: String::new(),
        priority,
        due_date: None,
        tasks: vec![task("t", 60)],
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

/* ---- 工单 03：详情读取、整计划编辑、软删除 ---- */

#[test]
fn get_returns_single_plan_or_notfound() {
    // 测试情况：创建计划后按 id 读取详情；再读一个不存在的 id。
    // 正确结果：详情含嵌套任务且字段一致；不存在的 id 返回 NotFound。
    let conn = db::open_in_memory().unwrap();
    let id = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &sample_plan()).unwrap();
    let p = PlanService::get(&conn, id).unwrap();
    assert_eq!(p.name, "备考英语 6 级");
    assert_eq!(p.tasks.len(), 2);
    assert_eq!(p.tasks[1].name, "完成英语真题");
    assert_eq!(
        PlanService::get(&conn, id + 100).unwrap_err(),
        PlanError::NotFound
    );
}

#[test]
fn update_persists_plan_and_task_field_changes() {
    // 测试情况：未开始计划整体编辑——改计划名/简述(清空回退名)/详细内容/优先级/截止日期，
    //          并修改第 1 个任务名称与耗时、第 2 个任务简述。
    // 正确结果：get 读回全部变更生效；任务 id 不变（按 id 更新而非删了重建）；新任务不产生。
    let conn = db::open_in_memory().unwrap();
    let id = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &sample_plan()).unwrap();
    let mut draft = draft_of(&PlanService::get(&conn, id).unwrap());
    draft.name = "备考 6 级（修订）".into();
    draft.summary = String::new(); // 清空 → 回退名称
    draft.detail = "先真题后词汇".into();
    draft.priority = Priority::High; // 未开始计划允许调整优先级
    draft.due_date = Some("2026-12-31".into());
    draft.tasks[0].name = "背完核心词汇".into();
    draft.tasks[0].estimated_minutes = Some(3000);
    draft.tasks[1].summary = "近 10 年真题".into();
    PlanService::update(&conn, &at(2026, 8, 24, 9, 0), id, &draft).unwrap();

    let p = PlanService::get(&conn, id).unwrap();
    assert_eq!(p.name, "备考 6 级（修订）");
    assert_eq!(p.summary, "备考 6 级（修订）"); // 简述清空 → 回退新名称
    assert_eq!(p.detail, "先真题后词汇");
    assert_eq!(p.priority, Priority::High);
    assert_eq!(p.due_date.as_deref(), Some("2026-12-31"));
    assert_eq!(p.tasks.len(), 2);
    assert_eq!(p.tasks[0].name, "背完核心词汇");
    assert_eq!(p.tasks[0].estimated_minutes, Some(3000));
    assert_eq!(p.tasks[1].summary, "近 10 年真题");
    assert_eq!(p.tasks[0].id, draft.tasks[0].id.unwrap()); // id 稳定
}

#[test]
fn update_rejects_priority_change_after_start() {
    // 测试情况：计划已进入进行中（spec 用户故事 12"计划开始后优先级不允许调整"），
    //          编辑草稿改了优先级。
    // 正确结果：拒绝为 PriorityLocked；保持原优先级提交则成功。
    let conn = db::open_in_memory().unwrap();
    let id = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &sample_plan()).unwrap();
    force_plan_status(&conn, id, PlanStatus::Active);
    let mut draft = draft_of(&PlanService::get(&conn, id).unwrap());
    draft.priority = Priority::High;
    assert_eq!(
        PlanService::update(&conn, &at(2026, 8, 24, 9, 0), id, &draft),
        Err(PlanError::PriorityLocked)
    );
    draft.priority = Priority::Medium;
    PlanService::update(&conn, &at(2026, 8, 24, 9, 0), id, &draft).unwrap();
}

#[test]
fn update_rejects_completed_task_edit() {
    // 测试情况：第 2 个任务已完成的计划，编辑草稿改了该任务的名称。
    // 正确结果：拒绝为 TaskLocked（index 指向提交序第 2 个任务）；
    //          已完成任务字段原样提交（改其它未完成任务）则成功。
    let conn = db::open_in_memory().unwrap();
    let id = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &sample_plan()).unwrap();
    let before = PlanService::get(&conn, id).unwrap();
    force_task_status(&conn, before.tasks[1].id, TaskStatus::Completed);
    let mut draft = draft_of(&PlanService::get(&conn, id).unwrap());
    draft.tasks[1].name = "想改已完成任务".into();
    assert_eq!(
        PlanService::update(&conn, &at(2026, 8, 24, 9, 0), id, &draft),
        Err(PlanError::TaskLocked { index: 1 })
    );
    draft.tasks[1].name = "完成英语真题".into(); // 恢复原值
    draft.tasks[0].name = "背完核心词汇".into(); // 只改未完成任务
    PlanService::update(&conn, &at(2026, 8, 24, 9, 0), id, &draft).unwrap();
}

#[test]
fn update_rejects_task_set_mismatch() {
    // 测试情况 A：编辑草稿漏掉库中现存的一个任务（想绕过删除确认直接丢任务）；
    //             情况 B：草稿携带库中不存在的任务 id。
    // 正确结果：都拒绝为 TaskSetMismatch——删除必须显式走 delete_task 通道。
    let conn = db::open_in_memory().unwrap();
    let id = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &sample_plan()).unwrap();
    let mut drop_last = draft_of(&PlanService::get(&conn, id).unwrap());
    drop_last.tasks.pop();
    assert_eq!(
        PlanService::update(&conn, &at(2026, 8, 24, 9, 0), id, &drop_last),
        Err(PlanError::TaskSetMismatch)
    );
    let mut unknown = draft_of(&PlanService::get(&conn, id).unwrap());
    unknown.tasks[1].id = Some(9999);
    assert_eq!(
        PlanService::update(&conn, &at(2026, 8, 24, 9, 0), id, &unknown),
        Err(PlanError::TaskSetMismatch)
    );
}

#[test]
fn update_reorders_unfinished_and_sinks_completed() {
    // 测试情况：三任务计划 A、B、C，B 已完成；编辑草稿把未完成任务重排为 C、A，
    //          并把已完成的 B 夹在中间提交（UI 锁拖拽，服务层兜底归一化）。
    // 正确结果：落库顺序 C(0)、A(1)、B(2)——未完成按提交序在前，已完成必然沉底且保持库内相对序。
    let conn = db::open_in_memory().unwrap();
    let base = || PlanDraft {
        name: "复习计划".into(),
        summary: String::new(),
        detail: String::new(),
        priority: Priority::Medium,
        due_date: None,
        tasks: vec![task("A", 60), task("B", 60), task("C", 60)],
    };
    let id = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &base()).unwrap();
    let loaded = PlanService::get(&conn, id).unwrap();
    force_task_status(&conn, loaded.tasks[1].id, TaskStatus::Completed);

    let mut draft = draft_of(&PlanService::get(&conn, id).unwrap());
    draft.tasks = vec![draft.tasks[2].clone(), draft.tasks[1].clone(), draft.tasks[0].clone()]; // C, B, A
    PlanService::update(&conn, &at(2026, 8, 24, 9, 0), id, &draft).unwrap();

    let p = PlanService::get(&conn, id).unwrap();
    let order: Vec<(String, i32)> = p.tasks.iter().map(|t| (t.name.clone(), t.position)).collect();
    assert_eq!(
        order,
        vec![("C".into(), 0), ("A".into(), 1), ("B".into(), 2)]
    );
}

#[test]
fn update_appends_new_task_after_unfinished() {
    // 测试情况：进行中计划 A（未完成）、B（已完成），编辑时追加新任务 N（spec 用户故事 9）。
    // 正确结果：N 落在所有未完成任务之后（position 1）、已完成任务仍沉底（B=2）；
    //          N 状态为未开始、id 为新分配。
    let conn = db::open_in_memory().unwrap();
    let base = || PlanDraft {
        name: "进行中计划".into(),
        summary: String::new(),
        detail: String::new(),
        priority: Priority::Medium,
        due_date: None,
        tasks: vec![task("A", 60), task("B", 60)],
    };
    let id = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &base()).unwrap();
    force_plan_status(&conn, id, PlanStatus::Active);
    let loaded = PlanService::get(&conn, id).unwrap();
    force_task_status(&conn, loaded.tasks[1].id, TaskStatus::Completed);

    let mut draft = draft_of(&PlanService::get(&conn, id).unwrap());
    draft.tasks.push(task("N", 90));
    PlanService::update(&conn, &at(2026, 8, 24, 9, 0), id, &draft).unwrap();

    let p = PlanService::get(&conn, id).unwrap();
    assert_eq!(p.tasks.len(), 3);
    assert_eq!((p.tasks[0].name.as_str(), p.tasks[0].position), ("A", 0));
    assert_eq!((p.tasks[1].name.as_str(), p.tasks[1].position), ("N", 1));
    assert_eq!((p.tasks[2].name.as_str(), p.tasks[2].position), ("B", 2));
    assert_eq!(p.tasks[1].status, TaskStatus::NotStarted);
    assert_ne!(p.tasks[1].id, p.tasks[0].id); // 新 id
}

#[test]
fn delete_task_soft_deletes_into_archive() {
    // 测试情况：删除计划中的一个任务（spec 用户故事 11：软删除进归档）。
    // 正确结果：视图（list/get）不再含该任务，其余任务完整；库中该行仍存在且 deleted_at 非空
    //          （归档可查而非物理删除）；重复删除或删不存在的 id 返回 NotFound。
    let conn = db::open_in_memory().unwrap();
    let id = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &sample_plan()).unwrap();
    let victim = PlanService::get(&conn, id).unwrap().tasks[0].id;
    PlanService::delete_task(&conn, &at(2026, 8, 24, 9, 0), victim).unwrap();

    let p = PlanService::get(&conn, id).unwrap();
    assert_eq!(p.tasks.len(), 1);
    assert_eq!(p.tasks[0].name, "完成英语真题");
    let archived: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE id = ?1 AND deleted_at IS NOT NULL",
            rusqlite::params![victim],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(archived, 1); // 行还在库中，只是进了归档

    assert_eq!(
        PlanService::delete_task(&conn, &at(2026, 8, 24, 9, 0), victim),
        Err(PlanError::NotFound)
    );
    assert_eq!(
        PlanService::delete_task(&conn, &at(2026, 8, 24, 9, 0), 9999),
        Err(PlanError::NotFound)
    );
}
