//! 抢占不变式接缝测试（工单 12；ADR-0006，术语 PreemptionInvariant / PauseReason /
//! AutoPauseFeedback）：开始约束、自动暂停、逐层恢复、手动暂停永不自动恢复、
//! 再抢占、抢占链场景、大面板过滤前提。所有场景走服务层真实命令（不用 SQL 造状态），
//! 保证测试的就是用户会触发的路径。

use plan_helper_lib::domain::allocation::AllocationService;
use plan_helper_lib::domain::lifecycle::LifecycleService;
use plan_helper_lib::domain::plans::{
    PauseReason, PlanDraft, PlanError, PlanService, PlanStatus, Priority, TaskStatus,
};
use plan_helper_lib::infra::db;

mod common;
use common::{at, force_task_status, plain_task};

/// 双任务计划草稿（名称区分等级，断言 PreemptedByHigher 的 plan_name 用）
fn draft(name: &str, priority: Priority) -> PlanDraft {
    PlanDraft {
        name: name.into(),
        summary: String::new(),
        detail: String::new(),
        priority,
        due_date: None,
        tasks: vec![plain_task("A", 60), plain_task("B", 30)],
    }
}

/// 计划当前状态（断言用）
fn status_of(conn: &rusqlite::Connection, plan_id: i64) -> PlanStatus {
    conn.query_row("SELECT status FROM plans WHERE id = ?1", rusqlite::params![plan_id], |r| {
        Ok(PlanStatus::from_db(&r.get::<_, String>(0)?))
    })
    .unwrap()
}

/// 计划当前暂停原因（断言用）
fn reason_of(conn: &rusqlite::Connection, plan_id: i64) -> Option<PauseReason> {
    conn.query_row(
        "SELECT pause_reason FROM plans WHERE id = ?1",
        rusqlite::params![plan_id],
        |r| r.get::<_, Option<String>>(0).map(|s| s.map(|s| PauseReason::from_db(&s))),
    )
    .unwrap()
}

/// 推到"已完成"终态（开始 → 任务全完成 → 手动确认），copy 场景的源计划种子
fn make_terminal(conn: &rusqlite::Connection, plan_id: i64) {
    LifecycleService::start(conn, plan_id).unwrap();
    for t in PlanService::get(conn, plan_id).unwrap().tasks {
        force_task_status(conn, t.id, TaskStatus::Completed);
    }
    LifecycleService::complete(conn, plan_id).unwrap();
}

#[test]
fn start_blocked_while_higher_tier_active() {
    // 测试情况：High 进行中时 start Low / resume 已暂停的 Low；High 暂停后再 start Low；
    //           同等级并行开始。
    // 正确结果：更高等级进行中时低等级一律拒绝 PreemptedByHigher（带阻挡计划名）且状态
    //           不变；高等级清空（这里用手动暂停）后低等级可开；同等级开始不受限。
    let conn = db::open_in_memory().unwrap();
    let high = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &draft("高山", Priority::High)).unwrap();
    let low = PlanService::create(&conn, &at(2026, 8, 23, 9, 1), &draft("洼地", Priority::Low)).unwrap();
    LifecycleService::start(&conn, high).unwrap();

    assert_eq!(
        LifecycleService::start(&conn, low),
        Err(PlanError::PreemptedByHigher { plan_name: "高山".into() }),
        "存在进行中的更高等级：低等级开始被拒并带出阻挡计划名"
    );
    assert_eq!(status_of(&conn, low), PlanStatus::NotStarted, "拒绝后状态不变");

    // 恢复路径同理：制造出 Paused 状态后，更高等级进行中时 resume 也被拒
    LifecycleService::pause(&conn, high).unwrap();
    LifecycleService::start(&conn, low).unwrap();
    LifecycleService::pause(&conn, low).unwrap();
    LifecycleService::resume(&conn, high).unwrap();
    assert_eq!(
        LifecycleService::resume(&conn, low),
        Err(PlanError::PreemptedByHigher { plan_name: "高山".into() }),
        "更高等级恢复进行中：低等级继续被拒"
    );
    assert_eq!(status_of(&conn, low), PlanStatus::Paused);

    // 高等级再清空（手动暂停）后：低等级可继续；同等级并行不受限
    LifecycleService::pause(&conn, high).unwrap();
    LifecycleService::resume(&conn, low).unwrap();
    let low2 = PlanService::create(&conn, &at(2026, 8, 23, 9, 2), &draft("另一洼地", Priority::Low)).unwrap();
    LifecycleService::start(&conn, low2).unwrap();
    assert_eq!(status_of(&conn, low2), PlanStatus::Active, "同等级并行开始不受限");
}

#[test]
fn higher_start_auto_pauses_lower_only() {
    // 测试情况：两个 Low 进行中 + 一个 Medium 未开始，随后开始 Medium。
    // 正确结果：outcome.paused 恰为两张进行中的 Low（含 id 与名称）；它们落库为
    //           Paused + AutoPreempted；未开始的 Medium 不受影响。
    let conn = db::open_in_memory().unwrap();
    let l1 = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &draft("后台一", Priority::Low)).unwrap();
    let l2 = PlanService::create(&conn, &at(2026, 8, 23, 9, 1), &draft("后台二", Priority::Low)).unwrap();
    let m1 = PlanService::create(&conn, &at(2026, 8, 23, 9, 2), &draft("前排", Priority::Medium)).unwrap();
    LifecycleService::start(&conn, l1).unwrap();
    LifecycleService::start(&conn, l2).unwrap();

    let outcome = LifecycleService::start(&conn, m1).unwrap();
    let mut paused_names: Vec<&str> = outcome.paused.iter().map(|p| p.name.as_str()).collect();
    paused_names.sort_unstable();
    assert_eq!(paused_names, vec!["后台一", "后台二"], "波及清单只含进行中的低等级计划");
    for id in [l1, l2] {
        assert_eq!(status_of(&conn, id), PlanStatus::Paused);
        assert_eq!(reason_of(&conn, id), Some(PauseReason::AutoPreempted));
    }
    assert_eq!(status_of(&conn, m1), PlanStatus::Active, "未开始的计划不受自动暂停影响");
    assert_eq!(reason_of(&conn, m1), None);
}

#[test]
fn preemption_chain_recovers_layer_by_layer() {
    // 测试情况（工单 12 抢占链场景）：Low 开始 → Medium 开始（抢 Low）→ High 开始
    //           （抢 Medium，Low 仍暂停）→ High 完成 → 恢复 Medium 组 → Medium 放弃
    //           → 恢复 Low 组。
    // 正确结果：每次恢复只轮到等级最高的"自动抢占"组（整组），更低组等这组清空；
    //           恢复后 pause_reason 清空。
    let conn = db::open_in_memory().unwrap();
    let low = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &draft("底", Priority::Low)).unwrap();
    let mid = PlanService::create(&conn, &at(2026, 8, 23, 9, 1), &draft("腰", Priority::Medium)).unwrap();
    let high = PlanService::create(&conn, &at(2026, 8, 23, 9, 2), &draft("顶", Priority::High)).unwrap();

    LifecycleService::start(&conn, low).unwrap();
    LifecycleService::start(&conn, mid).unwrap(); // 抢占 Low
    assert_eq!(reason_of(&conn, low), Some(PauseReason::AutoPreempted));
    LifecycleService::start(&conn, high).unwrap(); // 抢占 Medium
    assert_eq!(reason_of(&conn, mid), Some(PauseReason::AutoPreempted));
    assert_eq!(status_of(&conn, low), PlanStatus::Paused, "Low 仍暂停——恢复逐层进行");

    // High 完成 → 恢复 Medium 组；Low 不动
    for t in PlanService::get(&conn, high).unwrap().tasks {
        force_task_status(&conn, t.id, TaskStatus::Completed);
    }
    LifecycleService::complete(&conn, high).unwrap();
    assert_eq!(status_of(&conn, mid), PlanStatus::Active, "High 清空 → Medium 组整组恢复");
    assert_eq!(reason_of(&conn, mid), None, "恢复后暂停原因清空");
    assert_eq!(status_of(&conn, low), PlanStatus::Paused, "Medium 组未清空，Low 继续等");

    // Medium 放弃 → 轮到 Low 组恢复
    LifecycleService::abort(&conn, mid).unwrap();
    assert_eq!(status_of(&conn, low), PlanStatus::Active, "Medium 组清空 → Low 组恢复");
    assert_eq!(reason_of(&conn, low), None);
}

#[test]
fn manual_pause_never_auto_resumes() {
    // 测试情况：Medium1/Medium2 并行进行中；Medium2 手动暂停（用户主动）；High 开始
    //           抢占 Medium1；High 完成触发逐层恢复。
    // 正确结果：只恢复"自动抢占"的 Medium1；用户主动暂停的 Medium2 保持已暂停，
    //           只能手动恢复（用户故事 18）。
    let conn = db::open_in_memory().unwrap();
    let m1 = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &draft("自动组", Priority::Medium)).unwrap();
    let m2 = PlanService::create(&conn, &at(2026, 8, 23, 9, 1), &draft("手动组", Priority::Medium)).unwrap();
    let high = PlanService::create(&conn, &at(2026, 8, 23, 9, 2), &draft("高压", Priority::High)).unwrap();
    LifecycleService::start(&conn, m1).unwrap();
    LifecycleService::start(&conn, m2).unwrap();
    LifecycleService::pause(&conn, m2).unwrap();
    assert_eq!(reason_of(&conn, m2), Some(PauseReason::UserInitiated));
    LifecycleService::start(&conn, high).unwrap();

    for t in PlanService::get(&conn, high).unwrap().tasks {
        force_task_status(&conn, t.id, TaskStatus::Completed);
    }
    LifecycleService::complete(&conn, high).unwrap();
    assert_eq!(status_of(&conn, m1), PlanStatus::Active, "自动抢占组整组恢复");
    assert_eq!(status_of(&conn, m2), PlanStatus::Paused, "用户主动暂停的计划永不自动恢复");
    assert_eq!(reason_of(&conn, m2), Some(PauseReason::UserInitiated));
}

#[test]
fn recovery_fires_on_pause_abort_and_resume_repreempts() {
    // 测试情况：High 进行中抢占 Medium 后——(a) 手动暂停 High（恢复条件含手动暂停）；
    //           (b) 恢复 High（Medium 被再次抢占，用户故事 19）；(c) 放弃 High。
    // 正确结果：(a) Medium 立即恢复；(b) Medium 再次被自动抢占且原因仍记自动抢占；
    //           (c) Medium 再次恢复。
    let conn = db::open_in_memory().unwrap();
    let high = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &draft("高压电", Priority::High)).unwrap();
    let mid = PlanService::create(&conn, &at(2026, 8, 23, 9, 1), &draft("中场", Priority::Medium)).unwrap();
    LifecycleService::start(&conn, mid).unwrap();
    LifecycleService::start(&conn, high).unwrap();
    assert_eq!(reason_of(&conn, mid), Some(PauseReason::AutoPreempted));

    LifecycleService::pause(&conn, high).unwrap();
    assert_eq!(status_of(&conn, mid), PlanStatus::Active, "手动暂停也算清空，触发恢复");

    LifecycleService::resume(&conn, high).unwrap();
    assert_eq!(reason_of(&conn, mid), Some(PauseReason::AutoPreempted), "再遇更高等级开始/恢复 → 再次抢占");

    LifecycleService::abort(&conn, high).unwrap();
    assert_eq!(status_of(&conn, mid), PlanStatus::Active, "放弃同样触发恢复");
    assert_eq!(reason_of(&conn, mid), None);
}

#[test]
fn copy_as_new_respects_invariant_and_reports_preemption() {
    // 测试情况：(a) High 进行中时复制 Low 终态计划；(b) 暂停 High 后复制（无波及）；
    //           (c) Medium 进行中时复制 High 终态计划。
    // 正确结果：(a) 拒绝 PreemptedByHigher；(b) 成功且 paused 为空、新计划直接进行中；
    //           (c) 成功且复制即抢占返回 Medium 清单。
    let conn = db::open_in_memory().unwrap();
    let low_src = PlanService::create(&conn, &at(2026, 8, 20, 9, 0), &draft("旧低", Priority::Low)).unwrap();
    make_terminal(&conn, low_src);
    let high = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &draft("高山", Priority::High)).unwrap();
    LifecycleService::start(&conn, high).unwrap();

    assert_eq!(
        LifecycleService::copy_as_new(&conn, &at(2026, 8, 24, 9, 0), low_src),
        Err(PlanError::PreemptedByHigher { plan_name: "高山".into() }),
        "复制即开始：存在进行中的更高等级时复制同样被拒"
    );

    LifecycleService::pause(&conn, high).unwrap();
    let out = LifecycleService::copy_as_new(&conn, &at(2026, 8, 24, 9, 1), low_src).unwrap();
    assert_eq!(out.paused, Vec::new(), "无低等级进行中 → 无波及");
    assert_eq!(status_of(&conn, out.new_plan_id), PlanStatus::Active, "复制即进行中");

    // 复制高等级计划 → 抢占进行中的 Medium（先手动暂停低等级副本，
    // 避免它作为进行中的低等级被"复制即抢占"混进 out2.paused 断言）
    let high_src = PlanService::create(&conn, &at(2026, 8, 21, 9, 0), &draft("旧高", Priority::High)).unwrap();
    LifecycleService::pause(&conn, out.new_plan_id).unwrap();
    make_terminal(&conn, high_src);
    let mid = PlanService::create(&conn, &at(2026, 8, 23, 9, 3), &draft("中流", Priority::Medium)).unwrap();
    LifecycleService::start(&conn, mid).unwrap();
    let out2 = LifecycleService::copy_as_new(&conn, &at(2026, 8, 24, 9, 2), high_src).unwrap();
    assert_eq!(
        out2.paused.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        vec!["中流"],
        "复制即进行中所触发的抢占随结果返回"
    );
    assert_eq!(reason_of(&conn, mid), Some(PauseReason::AutoPreempted));
}

#[test]
fn board_after_preemption_only_shows_active_plans() {
    // 测试情况（工单 12 回归，06 过滤前提）：Medium 进行中并已提交当日分配；High 开始
    //           抢占 Medium；随后装配大面板。
    // 正确结果：候选分组只含进行中的 High（不变式保证同等级，无需"只显示最高等级"特判）；
    //           被抢占的 Medium 移入"暂停"分组（灰显数据源）；stale 选中集（Medium 的任务）
    //           被求交剔除。
    let conn = db::open_in_memory().unwrap();
    let mid = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &draft("中场", Priority::Medium)).unwrap();
    let high = PlanService::create(&conn, &at(2026, 8, 23, 9, 1), &draft("顶配", Priority::High)).unwrap();
    LifecycleService::start(&conn, mid).unwrap();
    let mid_tasks: Vec<i64> = PlanService::get(&conn, mid).unwrap().tasks.iter().map(|t| t.id).collect();
    AllocationService::commit(&conn, &at(2026, 8, 24, 8, 0), &mid_tasks).unwrap();

    LifecycleService::start(&conn, high).unwrap();
    let v = AllocationService::board(&conn, &at(2026, 8, 24, 9, 0)).unwrap();
    assert_eq!(
        v.groups.iter().map(|g| g.plan_name.as_str()).collect::<Vec<_>>(),
        vec!["顶配"],
        "候选只含进行中计划"
    );
    assert_eq!(
        v.paused_groups.iter().map(|g| g.plan_id).collect::<Vec<_>>(),
        vec![mid],
        "被抢占计划进入暂停分组"
    );
    assert_eq!(
        v.paused_groups[0].tasks.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
        vec!["A", "B"],
        "暂停分组展示其未完成任务（不可勾选，仅信息）"
    );
    assert!(v.selected_task_ids.is_empty(), "stale 选中集（被抢占计划的任务）被求交剔除");
    assert_eq!(status_of(&conn, mid), PlanStatus::Paused);

    // 高等级清空 → Medium 自动恢复 → 暂停分组消失、回到候选
    LifecycleService::pause(&conn, high).unwrap();
    let v2 = AllocationService::board(&conn, &at(2026, 8, 24, 9, 30)).unwrap();
    assert_eq!(
        v2.groups.iter().map(|g| g.plan_name.as_str()).collect::<Vec<_>>(),
        vec!["中场"],
        "恢复后的计划回到候选分组（High 已手动暂停，不在候选）"
    );
    assert_eq!(v2.selected_task_ids, mid_tasks, "自动恢复当日选中集原样回显（存储未清）");
    assert!(v2.paused_groups.is_empty(), "自动恢复后暂停分组清空");
}
