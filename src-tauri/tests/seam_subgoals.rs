//! 工单 04 接缝测试：子目标（精简输入行 → 落库/编辑/锁定/清空）、任务依赖
//!（同计划/成环/解除/等待判定）、进度按已完成分钟数自动缩放。


use plan_helper_lib::domain::deps::DependencyService;
use plan_helper_lib::domain::plans::{
    PlanDraft, PlanError, PlanService, Priority, SubGoalDraft, TaskDraft, TaskStatus,
};
use plan_helper_lib::infra::db;

mod common;
use common::{at, draft_of, force_subgoal_completed, force_task_status, plain_task};

/// 子目标草稿快捷构造（无 id = 新行）
fn sub(name: &str, minutes: u32) -> SubGoalDraft {
    SubGoalDraft {
        id: None,
        name: name.into(),
        estimated_minutes: minutes,
    }
}

/// 有子目标的任务草稿快捷构造
fn subgoal_task(name: &str, subs: Vec<SubGoalDraft>) -> TaskDraft {
    TaskDraft {
        id: None,
        name: name.into(),
        summary: String::new(),
        detail: String::new(),
        has_subgoals: true,
        estimated_minutes: None,
        subgoals: subs,
        depends_on: Vec::new(),
    }
}

/// 单任务计划草稿外壳
fn plan_of(tasks: Vec<TaskDraft>) -> PlanDraft {
    PlanDraft {
        name: "备考英语 6 级".into(),
        summary: String::new(),
        detail: String::new(),
        priority: Priority::Medium,
        due_date: None,
        tasks,
    }
}

/// 库中依赖边计数（断言边真删/真建用）
fn edge_count(conn: &rusqlite::Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM task_dependencies", [], |row| row.get(0))
        .unwrap()
}

/* ---- 子目标：创建与校验 ---- */

#[test]
fn create_persists_subgoals_in_fill_order() {
    // 测试情况：创建一个勾了子目标的任务，带两行子目标（60/40 分钟）。
    // 正确结果：任务 estimated_minutes = 子目标之和（100）；子目标按填写顺序落库
    //          （position 0,1）、均为未完成；依赖视图前置为空。
    let conn = db::open_in_memory().unwrap();
    PlanService::create(
        &conn,
        &at(2026, 8, 24, 9, 0),
        &plan_of(vec![subgoal_task(
            "背完所有 6 级词汇",
            vec![sub("词单元 1–10", 60), sub("词单元 11–20", 40)],
        )]),
    )
    .unwrap();
    let p = PlanService::list(&conn).unwrap().remove(0);
    assert!(p.tasks[0].has_subgoals);
    assert_eq!(p.tasks[0].estimated_minutes, Some(100));
    assert_eq!(p.tasks[0].subgoals.len(), 2);
    assert_eq!((p.tasks[0].subgoals[0].name.as_str(), p.tasks[0].subgoals[0].position), ("词单元 1–10", 0));
    assert_eq!((p.tasks[0].subgoals[1].name.as_str(), p.tasks[0].subgoals[1].position), ("词单元 11–20", 1));
    assert!(!p.tasks[0].subgoals[0].completed);
    assert!(p.tasks[0].prerequisite_ids.is_empty());
}

#[test]
fn create_rejects_subgoal_missing_name_or_duration() {
    // 测试情况 A：子目标内容为空白；情况 B：子目标预计耗时为 0。
    // 正确结果：都拒绝为 SubGoalInvalid，subgoal_index 指向违规行。
    let conn = db::open_in_memory().unwrap();
    let mut plan = plan_of(vec![subgoal_task(
        "t",
        vec![sub("第一行", 60), sub("  ", 30)],
    )]);
    assert_eq!(
        PlanService::create(&conn, &at(2026, 8, 24, 9, 0), &plan),
        Err(PlanError::SubGoalInvalid { task_index: 0, subgoal_index: 1 })
    );
    plan.tasks[0].subgoals[1] = sub("第二行", 0);
    assert_eq!(
        PlanService::create(&conn, &at(2026, 8, 24, 9, 0), &plan),
        Err(PlanError::SubGoalInvalid { task_index: 0, subgoal_index: 1 })
    );
}

/* ---- 子目标：编辑（改字段/追加/删除/锁定/清空） ---- */

#[test]
fn update_edits_appends_and_deletes_uncompleted_subgoals() {
    // 测试情况：三行子目标的任务做一次编辑——改第 1 行耗时、追加第 4 行、删除第 2 行（从草稿消失）。
    // 正确结果：改与删生效；新行 position 追加在最后（顺序递进）；estimated_minutes 重归一为剩余之和。
    let conn = db::open_in_memory().unwrap();
    let id = PlanService::create(
        &conn,
        &at(2026, 8, 24, 9, 0),
        &plan_of(vec![subgoal_task(
            "t",
            vec![sub("A", 60), sub("B", 40), sub("C", 20)],
        )]),
    )
    .unwrap();
    let mut draft = draft_of(&PlanService::get(&conn, id).unwrap());
    draft.tasks[0].subgoals[0].estimated_minutes = 90; // 改 A 耗时
    let b_id = draft.tasks[0].subgoals[1].id; // 删 B（从草稿消失）
    draft.tasks[0].subgoals.remove(1);
    draft.tasks[0].subgoals.push(sub("D", 30)); // 追加 D
    PlanService::update(&conn, &at(2026, 8, 25, 9, 0), id, &draft).unwrap();

    let p = PlanService::get(&conn, id).unwrap();
    let subs = &p.tasks[0].subgoals;
    assert_eq!(subs.len(), 3); // A、C、D
    assert_eq!((subs[0].name.as_str(), subs[0].estimated_minutes, subs[0].position), ("A", 90, 0));
    assert_eq!((subs[1].name.as_str(), subs[1].position), ("C", 1));
    assert_eq!((subs[2].name.as_str(), subs[2].position), ("D", 2)); // 新行沉到最后
    assert_eq!(p.tasks[0].estimated_minutes, Some(140)); // 90+20+30 重归一
    let b_alive: i64 = conn
        .query_row("SELECT COUNT(*) FROM subgoals WHERE id = ?1", rusqlite::params![b_id], |r| r.get(0))
        .unwrap();
    assert_eq!(b_alive, 0); // B 物理删除（软删除只针对任务，子目标不归档）
}

#[test]
fn update_keeps_completed_subgoals_first_in_mix() {
    // 测试情况：A(已完成)、B、C 三行；编辑草稿把未完成行提交为 C、B 的顺序并追加 D。
    // 正确结果：归一化后已完成 A 恒为前缀（position 0），未完成按草稿序 C(1)、B(2)、D(3)
    //          排在其后——"未完成必然排在已完成之后"（spec 故事 6）。
    let conn = db::open_in_memory().unwrap();
    let id = PlanService::create(
        &conn,
        &at(2026, 8, 24, 9, 0),
        &plan_of(vec![subgoal_task("t", vec![sub("A", 60), sub("B", 40), sub("C", 20)])]),
    )
    .unwrap();
    let view = PlanService::get(&conn, id).unwrap();
    force_subgoal_completed(&conn, view.tasks[0].subgoals[0].id);

    let mut draft = draft_of(&PlanService::get(&conn, id).unwrap());
    let c = draft.tasks[0].subgoals.remove(2);
    let b = draft.tasks[0].subgoals.remove(1);
    draft.tasks[0].subgoals.insert(1, c); // 未完成提交序改为 C、B
    draft.tasks[0].subgoals.insert(2, b);
    draft.tasks[0].subgoals.push(sub("D", 30));
    PlanService::update(&conn, &at(2026, 8, 25, 9, 0), id, &draft).unwrap();

    let subs = &PlanService::get(&conn, id).unwrap().tasks[0].subgoals;
    let order: Vec<(String, i32)> = subs.iter().map(|s| (s.name.clone(), s.position)).collect();
    assert_eq!(order, vec![("A".into(), 0), ("C".into(), 1), ("B".into(), 2), ("D".into(), 3)]);
}

#[test]
fn update_rejects_subgoal_id_from_other_task() {
    // 测试情况：两个任务各有子目标；编辑草稿把任务 2 的一行子目标 id 挂到任务 1 名下提交。
    // 正确结果：拒绝为 SubGoalInvalid（id 归属预检）——伪造/跨任务引用不落库、不改他行。
    let conn = db::open_in_memory().unwrap();
    let id = PlanService::create(
        &conn,
        &at(2026, 8, 24, 9, 0),
        &plan_of(vec![
            subgoal_task("t1", vec![sub("A", 60)]),
            subgoal_task("t2", vec![sub("B", 40)]),
        ]),
    )
    .unwrap();
    let view = PlanService::get(&conn, id).unwrap();
    let foreign = view.tasks[1].subgoals[0].id;
    let mut draft = draft_of(&view);
    draft
        .tasks[0]
        .subgoals
        .push(SubGoalDraft { id: Some(foreign), name: "冒名行".into(), estimated_minutes: 10 });
    assert_eq!(
        PlanService::update(&conn, &at(2026, 8, 25, 9, 0), id, &draft),
        Err(PlanError::SubGoalInvalid { task_index: 0, subgoal_index: 1 })
    );
    // 库中他行原样未动
    assert_eq!(PlanService::get(&conn, id).unwrap().tasks[1].subgoals[0].name, "B");
}

#[test]
fn update_rejects_modifying_or_dropping_completed_subgoal() {
    // 测试情况 A：第 1 行子目标已完成，编辑草稿改其名称；情况 B：草稿把它整行去掉（想删）。
    // 正确结果：都拒绝为 SubGoalLocked（spec 故事 10：已完成锁定不可改）；
    //          原样提交并只改未完成行则成功。
    let conn = db::open_in_memory().unwrap();
    let id = PlanService::create(
        &conn,
        &at(2026, 8, 24, 9, 0),
        &plan_of(vec![subgoal_task("t", vec![sub("A", 60), sub("B", 40)])]),
    )
    .unwrap();
    let view = PlanService::get(&conn, id).unwrap();
    force_subgoal_completed(&conn, view.tasks[0].subgoals[0].id);

    let mut rename = draft_of(&PlanService::get(&conn, id).unwrap());
    rename.tasks[0].subgoals[0].name = "想改已完成".into();
    assert_eq!(
        PlanService::update(&conn, &at(2026, 8, 25, 9, 0), id, &rename),
        Err(PlanError::SubGoalLocked { task_index: 0 })
    );

    let mut drop_it = draft_of(&PlanService::get(&conn, id).unwrap());
    drop_it.tasks[0].subgoals.remove(0);
    assert_eq!(
        PlanService::update(&conn, &at(2026, 8, 25, 9, 0), id, &drop_it),
        Err(PlanError::SubGoalLocked { task_index: 0 })
    );

    let mut legit = draft_of(&PlanService::get(&conn, id).unwrap());
    legit.tasks[0].subgoals[1].name = "B2".into(); // 只改未完成行
    PlanService::update(&conn, &at(2026, 8, 25, 9, 0), id, &legit).unwrap();
    assert_eq!(PlanService::get(&conn, id).unwrap().tasks[0].subgoals[1].name, "B2");
}

#[test]
fn update_unchecking_subgoals_clears_all_or_rejects_when_any_completed() {
    // 测试情况 A：全部未完成时取消勾选（草稿 has_subgoals=false、子目标清空、改填手填耗时）。
    //            情况 B：存在已完成子目标时同样取消勾选。
    // 正确结果：A 清空成功（子目标全删、任务改为手填耗时）；B 拒绝为 SubGoalLocked
    //          （已完成子目标的进度不允许随清空消失）。
    let conn = db::open_in_memory().unwrap();
    let id = PlanService::create(
        &conn,
        &at(2026, 8, 24, 9, 0),
        &plan_of(vec![subgoal_task("t", vec![sub("A", 60), sub("B", 40)])]),
    )
    .unwrap();

    let view = PlanService::get(&conn, id).unwrap();
    force_subgoal_completed(&conn, view.tasks[0].subgoals[0].id);
    let mut uncheck = draft_of(&PlanService::get(&conn, id).unwrap());
    uncheck.tasks[0].has_subgoals = false;
    uncheck.tasks[0].subgoals.clear();
    uncheck.tasks[0].estimated_minutes = Some(90);
    assert_eq!(
        PlanService::update(&conn, &at(2026, 8, 25, 9, 0), id, &uncheck),
        Err(PlanError::SubGoalLocked { task_index: 0 })
    );

    // 全部未完成（撤销上面种子的完成态）→ 清空成功
    conn.execute("UPDATE subgoals SET completed_at = NULL", []).unwrap();
    PlanService::update(&conn, &at(2026, 8, 25, 9, 0), id, &uncheck).unwrap();
    let p = PlanService::get(&conn, id).unwrap();
    assert!(!p.tasks[0].has_subgoals);
    assert!(p.tasks[0].subgoals.is_empty());
    assert_eq!(p.tasks[0].estimated_minutes, Some(90));
}

/* ---- 进度缩放（ADR-0002 派生公式的自然结果） ---- */

#[test]
fn progress_scales_by_completed_minutes_across_edits() {
    // 测试情况：任务带 A(60)/B(40) 两行子目标，A 已完成；随后改 B 耗时为 80、追加 C(20)、
    //          再删掉 B。
    // 正确结果：已完成分钟恒为 60（分子不变），总耗时随编辑缩放（分母变化）——
    //          100→140→160→80，派生百分比随之 60%→42.857%→37.5%→75%。
    let conn = db::open_in_memory().unwrap();
    let id = PlanService::create(
        &conn,
        &at(2026, 8, 24, 9, 0),
        &plan_of(vec![subgoal_task("t", vec![sub("A", 60), sub("B", 40)])]),
    )
    .unwrap();
    let task_id = PlanService::get(&conn, id).unwrap().tasks[0].id;
    let view = PlanService::get(&conn, id).unwrap();
    force_subgoal_completed(&conn, view.tasks[0].subgoals[0].id);

    let pr = PlanService::task_progress(&conn, task_id).unwrap();
    assert_eq!(pr, plan_helper_lib::domain::plans::TaskProgress { completed_minutes: 60, total_minutes: 100 });
    assert_eq!(pr.percent(), 60.0);

    let mut d1 = draft_of(&PlanService::get(&conn, id).unwrap());
    d1.tasks[0].subgoals[1].estimated_minutes = 80; // 改未完成行耗时
    PlanService::update(&conn, &at(2026, 8, 25, 9, 0), id, &d1).unwrap();
    let pr = PlanService::task_progress(&conn, task_id).unwrap();
    assert_eq!((pr.completed_minutes, pr.total_minutes), (60, 140));
    assert!((pr.percent() - 42.857142857142854).abs() < 1e-9);

    let mut d2 = draft_of(&PlanService::get(&conn, id).unwrap());
    d2.tasks[0].subgoals.push(sub("C", 20)); // 增未完成行
    PlanService::update(&conn, &at(2026, 8, 25, 10, 0), id, &d2).unwrap();
    let pr = PlanService::task_progress(&conn, task_id).unwrap();
    assert_eq!((pr.completed_minutes, pr.total_minutes), (60, 160));
    assert_eq!(pr.percent(), 37.5);

    let mut d3 = draft_of(&PlanService::get(&conn, id).unwrap());
    d3.tasks[0].subgoals.remove(1); // 删未完成行 B
    PlanService::update(&conn, &at(2026, 8, 25, 11, 0), id, &d3).unwrap();
    let pr = PlanService::task_progress(&conn, task_id).unwrap();
    assert_eq!((pr.completed_minutes, pr.total_minutes), (60, 80));
    assert_eq!(pr.percent(), 75.0);
}

/* ---- 依赖：创建表单（下标引用） ---- */

#[test]
fn create_persists_dependencies_and_waiting_judgement_follows_completion() {
    // 测试情况：创建 A、B、C 三任务，B 前置 A（下标 0）、C 前置 A、B（下标 0、1）；
    //          随后依次把 A、B 置为已完成（SQL 种子）。
    // 正确结果：前置 id 精确落库；waiting_on(C) 先为 [A, B]，A 完成后为 [B]，B 也完成后为空
    //          ——is_unblocked 即"前置全部已完成才可选入今日分配"。
    let conn = db::open_in_memory().unwrap();
    let mut b = plain_task("B", 60);
    b.depends_on = vec![0];
    let mut c = plain_task("C", 60);
    c.depends_on = vec![0, 1];
    let id = PlanService::create(
        &conn,
        &at(2026, 8, 24, 9, 0),
        &plan_of(vec![plain_task("A", 60), b, c]),
    )
    .unwrap();
    let p = PlanService::get(&conn, id).unwrap();
    let (a_id, b_id, c_id) = (p.tasks[0].id, p.tasks[1].id, p.tasks[2].id);
    assert_eq!(p.tasks[1].prerequisite_ids, vec![a_id]);
    assert_eq!(p.tasks[2].prerequisite_ids, vec![a_id, b_id]);

    let waiting: Vec<String> = DependencyService::waiting_on(&conn, c_id)
        .unwrap()
        .into_iter()
        .map(|t| t.name)
        .collect();
    assert_eq!(waiting, vec!["A", "B"]);
    assert!(!DependencyService::is_unblocked(&conn, c_id).unwrap());

    force_task_status(&conn, a_id, TaskStatus::Completed);
    let waiting: Vec<String> = DependencyService::waiting_on(&conn, c_id)
        .unwrap()
        .into_iter()
        .map(|t| t.name)
        .collect();
    assert_eq!(waiting, vec!["B"]); // 已完成前置不再阻塞

    force_task_status(&conn, b_id, TaskStatus::Completed);
    assert_eq!(DependencyService::waiting_on(&conn, c_id).unwrap().len(), 0);
    assert!(DependencyService::is_unblocked(&conn, c_id).unwrap());
}

#[test]
fn create_rejects_cyclic_and_invalid_dependency_refs() {
    // 测试情况 A：A→B→A（A 前置 B、B 前置 A）；情况 B：任务前置自己；情况 C：前置下标越界。
    // 正确结果：A、B 拒绝为 DependencyCycle（task_index 指向完成环的那条边所属任务）；
    //          C 拒绝为 DependencyInvalid；三种情况都不落任何边。
    let conn = db::open_in_memory().unwrap();
    let mut a = plain_task("A", 60);
    a.depends_on = vec![1];
    let mut b = plain_task("B", 60);
    b.depends_on = vec![0];
    assert_eq!(
        PlanService::create(&conn, &at(2026, 8, 24, 9, 0), &plan_of(vec![a, b])),
        Err(PlanError::DependencyCycle { task_index: Some(1) })
    );

    let mut self_ref = plain_task("A", 60);
    self_ref.depends_on = vec![0];
    assert_eq!(
        PlanService::create(&conn, &at(2026, 8, 24, 9, 0), &plan_of(vec![self_ref])),
        Err(PlanError::DependencyCycle { task_index: Some(0) })
    );

    let mut out_of_range = plain_task("A", 60);
    out_of_range.depends_on = vec![3];
    assert_eq!(
        PlanService::create(&conn, &at(2026, 8, 24, 9, 0), &plan_of(vec![out_of_range])),
        Err(PlanError::DependencyInvalid { task_index: 0 })
    );
    assert_eq!(edge_count(&conn), 0);
}

/* ---- 依赖：link 直连的守卫（跨计划 / 长环） ---- */

#[test]
fn link_rejects_cross_plan_and_long_cycles() {
    // 测试情况：两个计划各两个任务。跨计划建边（甲计划的 A → 乙计划的 X）；
    //          同计划内 A→B→C→A 三步闭环。
    // 正确结果：跨计划拒绝为 DependencyCrossPlan；不存在任务拒绝为 NotFound；
    //          A→B、B→C 成功后 C→A 拒绝为环（link 层无草稿下标，task_index 为 None）；
    //          成环那条边不落库（前两条保留）。
    let conn = db::open_in_memory().unwrap();
    let jia = PlanService::create(
        &conn,
        &at(2026, 8, 24, 9, 0),
        &plan_of(vec![plain_task("A", 60), plain_task("B", 60), plain_task("C", 60)]),
    )
    .unwrap();
    let yi = PlanService::create(
        &conn,
        &at(2026, 8, 24, 9, 0),
        &plan_of(vec![plain_task("X", 60), plain_task("Y", 60)]),
    )
    .unwrap();
    let jia_tasks = PlanService::get(&conn, jia).unwrap().tasks;
    let yi_tasks = PlanService::get(&conn, yi).unwrap().tasks;
    let (a, b, c) = (jia_tasks[0].id, jia_tasks[1].id, jia_tasks[2].id);

    assert_eq!(
        DependencyService::link(&conn, a, yi_tasks[0].id),
        Err(PlanError::DependencyCrossPlan)
    );
    assert_eq!(
        DependencyService::link(&conn, a, 9999),
        Err(PlanError::NotFound)
    );

    DependencyService::link(&conn, a, b).unwrap();
    DependencyService::link(&conn, b, c).unwrap();
    assert_eq!(
        DependencyService::link(&conn, c, a),
        Err(PlanError::DependencyCycle { task_index: None })
    );
    assert_eq!(edge_count(&conn), 2);
}

/* ---- 依赖：编辑态替换与删除解除 ---- */

#[test]
fn update_replaces_dependency_set_and_supports_new_tasks() {
    // 测试情况：A、B、C，B 前置 A、C 前置 A、B。编辑草稿：去掉 B 的前置、
    //          C 改为只前置 B、并追加新任务 N 前置 A。
    // 正确结果：边全量替换为草稿所见（B 无前置、C→B、N→A）——DependencyEditor 所见即所存；
    //          新任务（无 id）的依赖同样按草稿下标解析成功。
    let conn = db::open_in_memory().unwrap();
    let mut b = plain_task("B", 60);
    b.depends_on = vec![0];
    let mut c = plain_task("C", 60);
    c.depends_on = vec![0, 1];
    let id = PlanService::create(
        &conn,
        &at(2026, 8, 24, 9, 0),
        &plan_of(vec![plain_task("A", 60), b, c]),
    )
    .unwrap();
    let view = PlanService::get(&conn, id).unwrap();
    let (a_id, b_id) = (view.tasks[0].id, view.tasks[1].id);

    let mut draft = draft_of(&view);
    draft.tasks[1].depends_on = vec![]; // B 解除前置
    draft.tasks[2].depends_on = vec![1]; // C 只前置 B
    let mut n = plain_task("N", 90);
    n.depends_on = vec![0]; // 新任务前置 A（草稿下标 0）
    draft.tasks.push(n);
    PlanService::update(&conn, &at(2026, 8, 25, 9, 0), id, &draft).unwrap();

    let p = PlanService::get(&conn, id).unwrap();
    assert_eq!(p.tasks[1].prerequisite_ids, Vec::<i64>::new()); // B 无前置
    assert_eq!(p.tasks[2].prerequisite_ids, vec![b_id]); // C 前置 B
    assert_eq!(p.tasks[3].prerequisite_ids, vec![a_id]); // N 前置 A
    assert_eq!(
        DependencyService::waiting_on(&conn, p.tasks[3].id)
            .unwrap()
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
        vec!["A"]
    );
}

#[test]
fn delete_task_detaches_dependencies_and_unlocks_successors() {
    // 测试情况：A→B→C 链（B 前置 A、C 前置 B），软删除中间任务 B。
    // 正确结果：B 的两条边（作为后继对 A、作为前置对 C）全部自动解除；
    //          C 的等待列表为空（后继解锁）、库中边清零。
    let conn = db::open_in_memory().unwrap();
    let mut b = plain_task("B", 60);
    b.depends_on = vec![0];
    let mut c = plain_task("C", 60);
    c.depends_on = vec![1];
    let id = PlanService::create(
        &conn,
        &at(2026, 8, 24, 9, 0),
        &plan_of(vec![plain_task("A", 60), b, c]),
    )
    .unwrap();
    let p = PlanService::get(&conn, id).unwrap();
    let b_id = p.tasks[1].id;
    let c_id = p.tasks[2].id;
    assert_eq!(edge_count(&conn), 2);

    PlanService::delete_task(&conn, &at(2026, 8, 25, 9, 0), b_id).unwrap();
    assert_eq!(edge_count(&conn), 0);
    assert_eq!(DependencyService::waiting_on(&conn, c_id).unwrap().len(), 0);
    assert!(DependencyService::is_unblocked(&conn, c_id).unwrap());
}
