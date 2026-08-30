//! 今日总结接缝测试（工单 11）：触发时刻（最晚工作窗口结束那一刻，时钟注入）、
//! 只弹一次、次日补登"昨日总结"、跨午夜窗口归属窗口开始日、非工作日无触发点、
//! 三层布局装配（计划→任务→推进内容）、被抢占暂停照常展示并计入总量、
//! 当日零推进不展示、更高优先级未开始提示、目标含结转（工单 10 口径）、next_fire 排程。

use chrono::{Local, TimeZone};

use plan_helper_lib::domain::lifecycle::LifecycleService;
use plan_helper_lib::domain::plans::{
    PauseReason, PlanDraft, PlanService, PlanStatus, Priority, SubGoalDraft, TaskDraft,
};
use plan_helper_lib::domain::progress::ProgressService;
use plan_helper_lib::domain::settings::{DateOverride, SettingsService, TimeWindow};
use plan_helper_lib::domain::summary::{parse_date, SummaryService};
use plan_helper_lib::infra::db;

mod common;
use common::{at, d, force_pause_reason, force_plan_status, force_settings, plain_task};

const MON: &str = "2026-08-24"; // 周一
const TUE: &str = "2026-08-25";

/// 种子时间窗口：配置"自从有设置以来一直如此"（工单 13 起 `save` 的延时字段
/// 自下一个工作日生效，验收种子必须绕开延时——force_settings 直改存储）。
fn seed_windows(conn: &rusqlite::Connection, windows: &[(u16, u16)]) {
    let mut s = SettingsService::load(conn).unwrap();
    s.time_windows = windows
        .iter()
        .map(|&(a, b)| TimeWindow { start_minute: a, end_minute: b })
        .collect();
    force_settings(conn, &s);
}

/// 计划草稿（名称 + 优先级 + 任务集）
fn draft(name: &str, priority: Priority, tasks: Vec<TaskDraft>) -> PlanDraft {
    PlanDraft { name: name.into(), summary: String::new(), detail: String::new(), priority, due_date: None, tasks }
}

/// 三个子目标（30×3 = 90 分）的任务——"推进内容 = 完成列表 / 进行中"的种子
fn subgoal_task(name: &str) -> TaskDraft {
    let mut t = plain_task(name, 0);
    t.has_subgoals = true;
    t.estimated_minutes = None;
    t.subgoals = vec![
        SubGoalDraft { id: None, name: "一".into(), estimated_minutes: 30 },
        SubGoalDraft { id: None, name: "二".into(), estimated_minutes: 30 },
        SubGoalDraft { id: None, name: "三".into(), estimated_minutes: 30 },
    ];
    t
}

/// 建一个进行中计划（单任务，创建于上周五避免创建序干扰），返回 (计划 id, 任务 id)
fn active_plan(
    conn: &rusqlite::Connection,
    name: &str,
    priority: Priority,
    task: TaskDraft,
) -> (i64, i64) {
    let plan_id = PlanService::create(conn, &at(2026, 8, 21, 9, 0), &draft(name, priority, vec![task])).unwrap();
    LifecycleService::start(conn, plan_id).unwrap();
    let task_id = PlanService::get(conn, plan_id).unwrap().tasks[0].id;
    (plan_id, task_id)
}

#[test]
fn fires_at_latest_window_end_and_only_once() {
    // 测试情况：两段窗口 09:00–12:00、13:00–18:00（周一）。第一段结束（12:00）与最晚段
    //           结束前一刻（17:59）查询；18:00 整查询；登记弹出后 19:00 再查。
    // 正确结果：12:00 与 17:59 都不触发（触发点只认**最晚**窗口的结束）；18:00 整触发
    //           （端点左闭右开的"外"侧）今日总结；登记后不再触发（只弹一次）。
    let conn = db::open_in_memory().unwrap();
    seed_windows(&conn, &[(540, 720), (780, 1080)]);
    let mon = parse_date(MON).unwrap();
    assert_eq!(SummaryService::due(&conn, &at(2026, 8, 24, 12, 0)).unwrap(), None);
    assert_eq!(SummaryService::due(&conn, &at(2026, 8, 24, 17, 59)).unwrap(), None);
    assert_eq!(SummaryService::due(&conn, &at(2026, 8, 24, 18, 0)).unwrap(), Some(mon));
    SummaryService::mark_shown(&conn, &at(2026, 8, 24, 18, 0), mon).unwrap();
    assert_eq!(SummaryService::due(&conn, &at(2026, 8, 24, 19, 0)).unwrap(), None);
}

#[test]
fn backfills_yesterday_on_next_open() {
    // 测试情况：周一最晚窗口结束已过而未登记（当天没开应用的等价场景），查询周一
    //           20:00 与周二上午 10:00（周二窗口尚未结束）；补登后再查；再查周三上午
    //           （更早的上周五不补登）。
    // 正确结果：周一 20:00 弹"今日总结"（is_today）；次日首开补登"昨日总结"
    //           （due = 周一、is_yesterday）；登记后不再弹；只回看今天与昨天两天。
    let conn = db::open_in_memory().unwrap();
    let mon = parse_date(MON).unwrap();
    assert_eq!(SummaryService::due(&conn, &at(2026, 8, 24, 20, 0)).unwrap(), Some(mon));
    assert_eq!(SummaryService::due(&conn, &at(2026, 8, 25, 10, 0)).unwrap(), Some(mon));
    let v = SummaryService::summary(&conn, &at(2026, 8, 25, 10, 0), mon).unwrap();
    assert!(!v.is_today && v.is_yesterday, "补登口径：归属日 ≠ 今天");
    SummaryService::mark_shown(&conn, &at(2026, 8, 25, 10, 0), mon).unwrap();
    assert_eq!(SummaryService::due(&conn, &at(2026, 8, 25, 10, 0)).unwrap(), None);
    // 周二从未登记 → 周三首开补登"昨日总结"（补登的基线始终是昨天，与具体星期无关）
    assert_eq!(SummaryService::due(&conn, &at(2026, 8, 26, 10, 0)).unwrap(), Some(parse_date(TUE).unwrap()));
    SummaryService::mark_shown(&conn, &at(2026, 8, 26, 10, 0), parse_date(TUE).unwrap()).unwrap();
    assert_eq!(SummaryService::due(&conn, &at(2026, 8, 26, 10, 0)).unwrap(), None);
}

#[test]
fn cross_midnight_window_fires_next_day_and_belongs_to_start_day() {
    // 测试情况：窗口 20:00–01:00（跨午夜，周一为窗口开始日）。周一 23:00 报 +30 分、
    //           周二 00:30 报 +30 分；在周一 23:00、周二 00:59、周二 01:00 分别查触发。
    // 正确结果：触发时刻 = **次日** 01:00（最晚窗口的结束在日历上的下一天）；总结归属
    //           **窗口开始日**（周一）——凌晨段的推进计入周一、周二总结为 0（story 54）。
    let conn = db::open_in_memory().unwrap();
    seed_windows(&conn, &[(1200, 60)]);
    let (_, task) = active_plan(&conn, "夜间班", Priority::Medium, plain_task("值夜", 120));
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 23, 0), task, 25.0).unwrap();
    ProgressService::report_percent(&conn, &at(2026, 8, 25, 0, 30), task, 25.0).unwrap();
    assert_eq!(SummaryService::due(&conn, &at(2026, 8, 24, 23, 0)).unwrap(), None);
    assert_eq!(SummaryService::due(&conn, &at(2026, 8, 25, 0, 59)).unwrap(), None);
    assert_eq!(
        SummaryService::due(&conn, &at(2026, 8, 25, 1, 0)).unwrap(),
        Some(parse_date(MON).unwrap())
    );
    let mon = SummaryService::summary(&conn, &at(2026, 8, 25, 1, 0), parse_date(MON).unwrap()).unwrap();
    assert_eq!(mon.total_minutes, 60.0, "两笔都归属窗口开始日（周一）");
    let tue = SummaryService::summary(&conn, &at(2026, 8, 25, 12, 0), parse_date(TUE).unwrap()).unwrap();
    assert_eq!(tue.total_minutes, 0.0, "凌晨段不撕裂到周二");
}

#[test]
fn non_workday_never_triggers_and_next_fire_skips_weekend() {
    // 测试情况：默认窗口（09:00–18:00）。周五窗口结束后未登记时周六中午查询；
    //           登记周五弹出后周六再查；周五 19:00 查下次触发时刻。
    // 正确结果：周六是周五的"次日"——周五该弹未弹则周六首开补登（Some(Friday)）；
    //           登记后非工作日本身无触发点（None）；周五晚间的 next_fire 跳过周末
    //           落到下周一 18:00。
    let conn = db::open_in_memory().unwrap();
    let fri = parse_date("2026-08-28").unwrap();
    assert_eq!(SummaryService::due(&conn, &at(2026, 8, 29, 12, 0)).unwrap(), Some(fri));
    SummaryService::mark_shown(&conn, &at(2026, 8, 28, 18, 30), fri).unwrap();
    assert_eq!(SummaryService::due(&conn, &at(2026, 8, 29, 12, 0)).unwrap(), None);
    let nf = SummaryService::next_fire(&conn, &at(2026, 8, 28, 19, 0)).unwrap().unwrap();
    assert_eq!(nf, Local.with_ymd_and_hms(2026, 8, 31, 18, 0, 0).unwrap());
}

#[test]
fn next_fire_uses_latest_window_and_cross_midnight_lands_next_day() {
    // 测试情况：两段窗口查上午的 next_fire（应取最晚段）；晚间查（落次日）；跨午夜
    //           单窗口查上午（应落次日 01:00）。
    // 正确结果：多段取最晚段结束（18:00 而非 12:00）；已过的今天落到明天；跨午夜
    //           窗口的触发时刻在日历上属于次日。
    let conn = db::open_in_memory().unwrap();
    seed_windows(&conn, &[(540, 720), (780, 1080)]);
    let nf = SummaryService::next_fire(&conn, &at(2026, 8, 24, 10, 0)).unwrap().unwrap();
    assert_eq!(nf, Local.with_ymd_and_hms(2026, 8, 24, 18, 0, 0).unwrap());
    let nf = SummaryService::next_fire(&conn, &at(2026, 8, 24, 19, 0)).unwrap().unwrap();
    assert_eq!(nf, Local.with_ymd_and_hms(2026, 8, 25, 18, 0, 0).unwrap());

    let conn = db::open_in_memory().unwrap();
    seed_windows(&conn, &[(1200, 60)]);
    let nf = SummaryService::next_fire(&conn, &at(2026, 8, 24, 10, 0)).unwrap().unwrap();
    assert_eq!(nf, Local.with_ymd_and_hms(2026, 8, 25, 1, 0, 0).unwrap());
}

#[test]
fn summary_assembles_three_layers_in_priority_order() {
    // 测试情况：三个计划同库，按抢占不变式（工单 12）的真实时间线推进——Low 开始 →
    //           Medium 开始（抢停 Low，零推进）→ Medium 报 25%（30 分）→ High 开始
    //           （抢停 Medium）→ High 勾 2/3 个子目标（60 分）。装配周一的总结。
    // 正确结果：只展示当日有推进的计划且优先级降序（Low 缺席）；计划 section 带优先级
    //           与当日总耗时；任务行带派生总进度（一位小数）与当日耗时；有子目标显示
    //           快照（前两个已完成、第三个待完成）；无子目标走百分比；总览 = 90/300、
    //           无结转、无提示、is_today。
    let conn = db::open_in_memory().unwrap();
    let p_low = PlanService::create(&conn, &at(2026, 8, 21, 9, 0), &draft("低", Priority::Low, vec![plain_task("不干", 60)])).unwrap();
    let p_mid = PlanService::create(&conn, &at(2026, 8, 21, 9, 0), &draft("中", Priority::Medium, vec![plain_task("百分活", 120)])).unwrap();
    let hp = PlanService::create(&conn, &at(2026, 8, 21, 9, 0), &draft("高", Priority::High, vec![subgoal_task("勾选活")])).unwrap();
    LifecycleService::start(&conn, p_low).unwrap();
    LifecycleService::start(&conn, p_mid).unwrap(); // 抢停低（零推进 → 不展示）
    let mt = PlanService::get(&conn, p_mid).unwrap().tasks[0].id;
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 9, 30), mt, 25.0).unwrap();
    LifecycleService::start(&conn, hp).unwrap(); // 抢停中
    let sg = PlanService::get(&conn, hp).unwrap().tasks[0].subgoals.clone();
    ProgressService::complete_subgoal(&conn, &at(2026, 8, 24, 10, 0), sg[0].id).unwrap();
    ProgressService::complete_subgoal(&conn, &at(2026, 8, 24, 10, 30), sg[1].id).unwrap();

    let v = SummaryService::summary(&conn, &at(2026, 8, 24, 12, 0), parse_date(MON).unwrap()).unwrap();
    assert!(v.is_today && !v.is_yesterday);
    assert_eq!(v.plans.len(), 2, "零推进计划不展示");
    assert_eq!(v.plans[0].plan_id, hp, "PlanOrdering：优先级降序");
    assert_eq!(v.plans[0].minutes, 60.0);
    assert!(!v.plans[0].preempted);
    let t = &v.plans[0].tasks[0];
    assert_eq!(t.percent, 66.7, "60/90 派生进度 round 一位小数");
    assert_eq!(t.minutes, 60.0);
    assert!(t.has_subgoals);
    assert!(t.subgoals[0].completed && t.subgoals[1].completed && !t.subgoals[2].completed);
    let m = &v.plans[1];
    assert_eq!(m.minutes, 30.0);
    assert!(!m.tasks[0].has_subgoals, "无子目标任务走百分比进度条");
    assert_eq!(m.tasks[0].percent, 25.0);
    assert_eq!(v.total_minutes, 90.0);
    assert_eq!(v.target_minutes, 300.0, "无历史结转：目标 = 基准");
    assert_eq!(v.base_minutes, 300);
    assert!(v.workday);
    assert!(!v.higher_priority_hint, "没有更高优先级的未开始计划");
}

#[test]
fn preempted_paused_plans_still_shown_and_counted() {
    // 测试情况：两个计划当日各有推进后分别暂停——A 记"自动抢占"、B 记"用户主动"。
    // 正确结果：两者照常出 section（干了的活就是干了），A 标注"已被抢占暂停"、B 不标；
    //           推进都计入完成总量（与工时账户的达标判定同口径）。
    let conn = db::open_in_memory().unwrap();
    let (pa, ta) = active_plan(&conn, "被抢", Priority::High, plain_task("a", 100));
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 0), ta, 30.0).unwrap();
    force_plan_status(&conn, pa, PlanStatus::Paused);
    force_pause_reason(&conn, pa, Some(PauseReason::AutoPreempted));
    let (pb, tb) = active_plan(&conn, "手停", Priority::Medium, plain_task("b", 100));
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 11, 0), tb, 20.0).unwrap();
    force_plan_status(&conn, pb, PlanStatus::Paused);
    force_pause_reason(&conn, pb, Some(PauseReason::UserInitiated));

    let v = SummaryService::summary(&conn, &at(2026, 8, 24, 19, 0), parse_date(MON).unwrap()).unwrap();
    assert_eq!(v.plans.len(), 2);
    let a = v.plans.iter().find(|p| p.plan_id == pa).unwrap();
    let b = v.plans.iter().find(|p| p.plan_id == pb).unwrap();
    assert!(a.preempted, "自动抢占标注");
    assert!(!b.preempted, "用户主动暂停不标抢占");
    assert_eq!(a.minutes, 30.0);
    assert_eq!(b.minutes, 20.0);
    assert_eq!(v.total_minutes, 50.0, "被抢占计划的推进计入完成总量");
}

#[test]
fn higher_priority_not_started_hint() {
    // 测试情况：(a) 推 Medium 时存在 High 未开始；(b) 推 High 时只有 Medium 未开始；
    //           (c) 什么都没推但存在未开始计划；(d) 推 High 时只有另一个 High 未开始。
    // 正确结果：(a)(c) 提示（"更高"以当日推进过的最高优先级为基线，什么都没推时任何
    //           未开始都算）；(b)(d) 不提示（未开始计划没有**严格**更高）。
    let conn = db::open_in_memory().unwrap();
    let (_, tm) = active_plan(&conn, "中", Priority::Medium, plain_task("m", 100));
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 0), tm, 10.0).unwrap();
    active_plan_ready_but_never_started(&conn, Priority::High);
    assert!(SummaryService::summary(&conn, &at(2026, 8, 24, 19, 0), parse_date(MON).unwrap()).unwrap().higher_priority_hint);

    let conn = db::open_in_memory().unwrap();
    let (_, th) = active_plan(&conn, "高", Priority::High, plain_task("h", 100));
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 0), th, 10.0).unwrap();
    let _ = active_plan_ready_but_never_started(&conn, Priority::Medium); // 工单 12：High 进行中时 Medium 本就不能开始，hint 场景用未开始种子
    assert!(!SummaryService::summary(&conn, &at(2026, 8, 24, 19, 0), parse_date(MON).unwrap()).unwrap().higher_priority_hint);

    let conn = db::open_in_memory().unwrap();
    let _ = active_plan_ready_but_never_started(&conn, Priority::Low);
    let v = SummaryService::summary(&conn, &at(2026, 8, 24, 19, 0), parse_date(MON).unwrap()).unwrap();
    assert!(v.higher_priority_hint, "零推进日：任何未开始计划都提示");
    assert_eq!(v.total_minutes, 0.0);

    let conn = db::open_in_memory().unwrap();
    let (_, th) = active_plan(&conn, "高", Priority::High, plain_task("h", 100));
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 0), th, 10.0).unwrap();
    let _ = active_plan_ready_but_never_started(&conn, Priority::High);
    assert!(!SummaryService::summary(&conn, &at(2026, 8, 24, 19, 0), parse_date(MON).unwrap()).unwrap().higher_priority_hint);
}

/// 建一个**未开始**计划（active_plan 会 start，这里保持 NotStarted——hint 场景的种子）
fn active_plan_ready_but_never_started(conn: &rusqlite::Connection, priority: Priority) -> i64 {
    let plan_id = PlanService::create(
        conn,
        &at(2026, 8, 21, 9, 0),
        &draft("未开始", priority, vec![plain_task("t", 100)]),
    )
    .unwrap();
    plan_id
}

#[test]
fn summary_target_includes_carry_from_ledger() {
    // 测试情况：周一 300 分目标只推 150 分（带外缺口 150 均分到后续 7 个工作日）。
    //           时钟周二装配周一与周二的总结。
    // 正确结果：周二目标 = 300 + 150/7（含结转，与工单 10 同一派生）；周一自己的总结
    //           目标仍 = 300（回放只到归属日为止，当日缺口在当日总结里如实呈现为未达标）。
    let conn = db::open_in_memory().unwrap();
    let (_, t) = active_plan(&conn, "欠账", Priority::Medium, plain_task("慢活", 600));
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 0), t, 25.0).unwrap(); // 150 分

    let tue = SummaryService::summary(&conn, &at(2026, 8, 25, 10, 0), parse_date(TUE).unwrap()).unwrap();
    assert!((tue.target_minutes - (300.0 + 150.0 / 7.0)).abs() < 1e-6, "调整后目标含结转");
    assert_eq!(tue.base_minutes, 300);
    let mon = SummaryService::summary(&conn, &at(2026, 8, 25, 10, 0), parse_date(MON).unwrap()).unwrap();
    assert_eq!(mon.target_minutes, 300.0);
    assert_eq!(mon.total_minutes, 150.0);
}

#[test]
fn deleted_task_minutes_still_count_for_plan_and_total() {
    // 测试情况：任务当日推进 30 分后被软删除（归档）。
    // 正确结果：计划照常出 section 且分钟数含已删任务的推进（归档不是抹账，ADR-0009），
    //           只是渲染不出任务行；完成总量如实。
    let conn = db::open_in_memory().unwrap();
    let (_p, t) = active_plan(&conn, "有删", Priority::Medium, plain_task("删我", 120));
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 0), t, 25.0).unwrap();
    PlanService::delete_task(&conn, &at(2026, 8, 24, 11, 0), t).unwrap();

    let v = SummaryService::summary(&conn, &at(2026, 8, 24, 19, 0), parse_date(MON).unwrap()).unwrap();
    assert_eq!(v.plans.len(), 1);
    assert_eq!(v.plans[0].minutes, 30.0);
    assert!(v.plans[0].tasks.is_empty(), "已归档任务渲染不出任务行");
    assert_eq!(v.total_minutes, 30.0);
}

#[test]
fn status_feeds_due_last_shown_and_next_fire() {
    // 测试情况：周一、周二各自在窗口结束弹出后登记；周二 19:00 查 status。
    // 正确结果：due 无（都弹过）；last_shown = 最近登记的周二；next_fire = 周三 18:00
    //           （前端定时器据此排程下一次）。
    let conn = db::open_in_memory().unwrap();
    SummaryService::mark_shown(&conn, &at(2026, 8, 24, 18, 0), parse_date(MON).unwrap()).unwrap();
    SummaryService::mark_shown(&conn, &at(2026, 8, 25, 18, 0), parse_date(TUE).unwrap()).unwrap();
    let st = SummaryService::status(&conn, &at(2026, 8, 25, 19, 0)).unwrap();
    assert_eq!(st.due, None);
    assert_eq!(st.last_shown.as_deref(), Some(TUE));
    let nf = st.next_fire_at.unwrap();
    assert_eq!(nf, Local.with_ymd_and_hms(2026, 8, 26, 18, 0, 0).unwrap());
}

#[test]
fn parse_date_rejects_garbage() {
    // 测试情况：非法日期串过 command 边界解析。
    // 正确结果：拒绝（InvalidDate），合法串往返一致。
    assert!(parse_date("2026-08-24").is_ok());
    assert!(parse_date("08/24/2026").is_err());
    assert!(parse_date("").is_err());
}

#[test]
fn settings_shift_tomorrow_trigger_not_today() {
    // 测试情况（工单 13 生效时机 × 总结触发）：周一 10:00 把窗口从 09:00–18:00
    //           改为 09:00–12:00（延时字段 → 周二生效）。
    // 正确结果：周一 next_fire 仍是 18:00（当日按生效中的旧配置——改动不影响
    //           当日总结）；周一 13:00 未到触发点、18:00 整 due=周一；周二起按
    //           新配置 next_fire=12:00。
    let conn = db::open_in_memory().unwrap();
    seed_windows(&conn, &[(540, 1080)]); // 09:00–18:00，"一直如此"
    let mut s = SettingsService::load(&conn).unwrap();
    s.time_windows = vec![TimeWindow { start_minute: 540, end_minute: 720 }];
    SettingsService::save(&conn, &at(2026, 8, 24, 10, 0), &s).unwrap(); // 周一改 → 周一生效

    let fmt = |t: Option<chrono::DateTime<chrono::Local>>| {
        t.map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
    };
    assert_eq!(
        fmt(SummaryService::next_fire(&conn, &at(2026, 8, 24, 10, 0)).unwrap()),
        Some("2026-08-24 18:00:00".into()),
        "当日触发点按旧配置"
    );
    assert_eq!(SummaryService::due(&conn, &at(2026, 8, 24, 13, 0)).unwrap(), None, "周一 13:00 未到旧触发点");
    assert_eq!(SummaryService::due(&conn, &at(2026, 8, 24, 18, 0)).unwrap().map(|d| d.to_string()), Some(MON.to_string()), "周一 18:00 整该弹");
    assert_eq!(
        fmt(SummaryService::next_fire(&conn, &at(2026, 8, 25, 10, 0)).unwrap()),
        Some("2026-08-25 12:00:00".into()),
        "周二起按新配置触发"
    );
}

#[test]
fn date_overrides_shift_summary_triggers() {
    // 测试情况（工单 13 日期例外 × 总结触发）：周一 8/24 被例外标休、周六 8/29
    //           被例外标工（其余维持默认周一至五 + 09:00–18:00）。
    // 正确结果：周一 19:00 无触发点（休日不弹）；next_fire 周一 10:00 落周二
    //           18:00；周五 10:00 落补班周六 18:00；周六 18:30 该弹周六总结。
    let conn = db::open_in_memory().unwrap();
    let mut s = SettingsService::load(&conn).unwrap();
    s.date_overrides = vec![
        DateOverride { date: d(2026, 8, 24), working: false },
        DateOverride { date: d(2026, 8, 29), working: true },
    ];
    force_settings(&conn, &s);

    assert_eq!(SummaryService::due(&conn, &at(2026, 8, 24, 19, 0)).unwrap(), None, "休日无触发点");
    let fmt = |t: Option<chrono::DateTime<chrono::Local>>| {
        t.map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
    };
    assert_eq!(
        fmt(SummaryService::next_fire(&conn, &at(2026, 8, 24, 10, 0)).unwrap()),
        Some("2026-08-25 18:00:00".into()),
        "周一标休 → 触发落周二"
    );
    assert_eq!(
        fmt(SummaryService::next_fire(&conn, &at(2026, 8, 28, 18, 30)).unwrap()),
        Some("2026-08-29 18:00:00".into()),
        "周五触发过后 → 补班周六也有触发点"
    );
    assert_eq!(
        SummaryService::due(&conn, &at(2026, 8, 29, 18, 30)).unwrap().map(|d| d.to_string()),
        Some("2026-08-29".into()),
        "补班周六 18:30 该弹"
    );
}
