//! 工时账户结算接缝测试（工单 10；ADR-0007 对称均分、ADR-0009 单一事实源）：
//! 调整后目标全部从 ProgressLog + 设置实时派生——±10% 容差带以**调整后目标**为基数、
//! 对称抵扣（未达标上调 / 实际超额下调，选择层面超额不计）、滚动叠加、递归再均分、
//! 均分窗口以工作日为单位（非工作日不计入窗口、推进按超额入账）、历史修正后全量重算。
//! 日期脚手架：2026-08-21 周五 / 22 周六 / 23 周日 / 24 周一（默认周一至五工作）。

use plan_helper_lib::domain::allocation::AllocationService;
use plan_helper_lib::domain::ledger::LedgerService;
use plan_helper_lib::domain::lifecycle::LifecycleService;
use plan_helper_lib::domain::plans::{PlanDraft, PlanService, Priority, SubGoalDraft};
use plan_helper_lib::domain::progress::ProgressService;
use plan_helper_lib::domain::settings::{DateOverride, Settings};
use plan_helper_lib::infra::db;

mod common;
use common::{at, d, force_settings, plain_task};

/// 建进行中计划 + 一个 6000 分钟无子目标任务（百分比 ×60 = 分钟：5% = 300min = 5h，
/// 任意日完成量都能用一位小数百分比精确构造），返回任务 id。
fn seeded(conn: &rusqlite::Connection) -> i64 {
    let plan_id = PlanService::create(
        conn,
        &at(2026, 8, 20, 9, 0),
        &PlanDraft {
            name: "P".into(),
            summary: String::new(),
            detail: String::new(),
            priority: Priority::Medium,
            due_date: None,
            tasks: vec![plain_task("大河任务", 6000)],
        },
    )
    .unwrap();
    LifecycleService::start(conn, plan_id).unwrap();
    PlanService::get(conn, plan_id).unwrap().tasks[0].id
}

/// day_target 断言：base 恒为 FirstRun 默认 300，target ≈ want（f64  epsilon 比较）。
fn assert_target(conn: &rusqlite::Connection, y: i32, mo: u32, d: u32, want: f64, why: &str) {
    let t = LedgerService::day_target(conn, &at(y, mo, d, 12, 0)).unwrap();
    assert_eq!(t.base_minutes, 300, "基准 = 设置的每日工作时间");
    assert!(
        (t.target_minutes - want).abs() < 1e-6,
        "{why}：{} ≈ {want}",
        t.target_minutes
    );
}

#[test]
fn empty_history_target_is_base() {
    // 测试情况：全新库、ProgressLog 一条事件都没有（账户无任何历史）。
    // 正确结果：任意"今天"的调整后目标 = 基准 5h、结转为 0；大面板与小看板视图
    //           同步携带 base_minutes / target_minutes（f64）与小看板 workday 标记。
    let conn = db::open_in_memory().unwrap();
    let _task = seeded(&conn);
    assert_target(&conn, 2026, 8, 26, 300.0, "无历史 = 基准");
    let ab = AllocationService::board(&conn, &at(2026, 8, 26, 9, 0)).unwrap();
    assert!((ab.target_minutes - 300.0).abs() < 1e-9 && ab.base_minutes == 300);
    let mb = ProgressService::board(&conn, &at(2026, 8, 26, 9, 0)).unwrap();
    assert!((mb.target_minutes - 300.0).abs() < 1e-9 && mb.base_minutes == 300 && mb.workday);
}

#[test]
fn deficit_spreads_over_seven_workdays() {
    // 测试情况：周一实际推进 48min（0.8h），远低于 5h 目标（容差带外），
    //           差额按 7 工作日窗口均分（ADR-0007 的"目标 5.6h"示例）；
    //           之后每个工作日推 312min（在 336 的容差带内，不产生新差额——
    //           否则空闲日的递归欠债会叠上来，那是 respread 测试的职责）。
    // 正确结果：后续第 1..7 个工作日（周二 8/25 … 周三 9/2）目标各 +36min = 336（5.6h）；
    //           第 8 个工作日（周四 9/3）窗口耗尽回到基准；周六非工作日本身无目标义务。
    let conn = db::open_in_memory().unwrap();
    let task = seeded(&conn);
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 0), task, 0.8).unwrap();
    for (y, mo, d) in [
        (2026, 8, 25),
        (2026, 8, 26),
        (2026, 8, 27),
        (2026, 8, 28),
        (2026, 8, 31),
        (2026, 9, 1),
        (2026, 9, 2),
    ] {
        assert_target(&conn, y, mo, d, 336.0, "窗口内 7 个工作日均 +252/7");
        ProgressService::report_percent(&conn, &at(y, mo, d, 10, 0), task, 5.2).unwrap(); // 312min 带内中和
    }
    assert_target(&conn, 2026, 9, 3, 300.0, "第 8 个工作日窗口耗尽");
    assert_target(&conn, 2026, 8, 29, 300.0, "周六无目标义务（target = base 仅展示口径）");
}

#[test]
fn tolerance_band_uses_adjusted_target() {
    // 测试情况（三段独立库）：
    //  a) 周一 270min = 0.9×300（恰在容差下沿）→ 达标不均分；
    //  b) 周一 240min（4h，带外）→ 差额 -60min 均分，周二上调 60/7；
    //  c) 周五先推满 300min 锚定历史 → 周一无任何事件（整日缺口入账）→ 周二目标 300+300/7，
    //     周二推 300min：300 < 0.9×(300+300/7) → 容差以**调整后目标**为基数仍判未达标，
    //     新差额继续向后均分。
    // 正确结果：a) 周二 300；b) 周二 300+60/7；c) 周三 = 300 + 300/7 + (300/7)/7。
    //           （c 的周一同时覆盖"没开应用的工作日照样欠整日差额——债跟着走"）
    let conn = db::open_in_memory().unwrap();
    let t = seeded(&conn);
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 0), t, 4.5).unwrap();
    assert_target(&conn, 2026, 8, 25, 300.0, "4.5h 恰在容差下沿 = 达标不均分");

    let conn = db::open_in_memory().unwrap();
    let t = seeded(&conn);
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 0), t, 4.0).unwrap();
    assert_target(&conn, 2026, 8, 25, 300.0 + 60.0 / 7.0, "4h 带外 → 周二 +60/7");

    let conn = db::open_in_memory().unwrap();
    let t = seeded(&conn);
    ProgressService::report_percent(&conn, &at(2026, 8, 21, 10, 0), t, 5.0).unwrap(); // 周五锚定，恰好达标
    assert_target(&conn, 2026, 8, 25, 300.0 + 300.0 / 7.0, "周一整日未推进 → 周二 300+300/7");
    ProgressService::report_percent(&conn, &at(2026, 8, 25, 10, 0), t, 5.0).unwrap(); // 周二 300min
    assert_target(
        &conn,
        2026,
        8,
        26,
        300.0 + 300.0 / 7.0 + 300.0 / 7.0 / 7.0,
        "容差按调整后目标判定，新差额递归再均分",
    );
}

#[test]
fn surplus_symmetric_band() {
    // 测试情况（两段独立库）：a) 周一推 384min（6.4h，超出 +10% 带上沿 330）；
    //           b) 周一推 324min（5.4h，带内）。周二用 300min（带内）中和，不引入新差额。
    // 正确结果：a) 超额对称抵扣：后续 7 个工作日各下调 84/7 = 12min（周二、周三 = 288 = 4.8h）；
    //           b) 带内不抵扣（容差带对称豁免"多干一点"与"少干一点"），周二仍 300。
    let conn = db::open_in_memory().unwrap();
    let t = seeded(&conn);
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 0), t, 6.4).unwrap();
    assert_target(&conn, 2026, 8, 25, 288.0, "实际超额 84min → 周二 -84/7");
    ProgressService::report_percent(&conn, &at(2026, 8, 25, 10, 0), t, 5.0).unwrap(); // 288 的带内中和
    assert_target(&conn, 2026, 8, 26, 288.0, "窗口内工作日持续对称下调");

    let conn = db::open_in_memory().unwrap();
    let t = seeded(&conn);
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 0), t, 5.4).unwrap();
    assert_target(&conn, 2026, 8, 25, 300.0, "带内超额不抵扣");
}

#[test]
fn rolling_sum_of_multiple_deficits() {
    // 测试情况：周一推 48min（缺口 252）、周二在已被上调的目标（336）下仍只推 48min
    //           （缺口 288）——两个历史差额并存。
    // 正确结果：周三目标 = 基准 + Σ 各差额均分到今天的份额 = 300 + 252/7 + 288/7（滚动叠加）。
    let conn = db::open_in_memory().unwrap();
    let t = seeded(&conn);
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 0), t, 0.8).unwrap();
    ProgressService::report_percent(&conn, &at(2026, 8, 25, 10, 0), t, 0.8).unwrap();
    assert_target(
        &conn,
        2026,
        8,
        26,
        300.0 + 252.0 / 7.0 + 288.0 / 7.0,
        "多日差额滚动叠加到同一日",
    );
}

#[test]
fn deficit_day_respreads_recursively() {
    // 测试情况：周五推满 300min 锚定历史（达标不产生差额）→ 周一、周二整日未推进：
    //           周一欠 300，周二在抬高的目标（300+300/7）下又欠 300+300/7。
    // 正确结果：周三目标 = 300 + 300/7（周一的债）+ (300+300/7)/7（周二的债）——
    //           均分目标日未达标，其新差额继续向后均分（递归，欠的债跟着走）。
    let conn = db::open_in_memory().unwrap();
    let t = seeded(&conn);
    ProgressService::report_percent(&conn, &at(2026, 8, 21, 10, 0), t, 5.0).unwrap();
    assert_target(
        &conn,
        2026,
        8,
        26,
        300.0 + 300.0 / 7.0 + (300.0 + 300.0 / 7.0) / 7.0,
        "递归再均分",
    );
}

#[test]
fn weekend_never_consumes_window_but_surplus_enters() {
    // 测试情况：周五推 30min（缺口 270）、周六（非工作日）推 120min；随后每个工作日
    //           推 300min（在 321.43 的容差带内，不引入新差额）。
    // 正确结果：周六不消费均分窗口——两笔差额都落到周五之后的**工作日**序列
    //           （周一 8/24 … 周二 9/1，共 7 个），第 7 个工作日仍在窗内（若是自然日窗口
    //           早已耗尽）；每个窗内工作日 = 300 + 270/7 - 120/7（净 150/7）；
    //           周六自身无目标义务；第 8 个工作日（周三 9/2）窗口耗尽回到基准。
    let conn = db::open_in_memory().unwrap();
    let t = seeded(&conn);
    ProgressService::report_percent(&conn, &at(2026, 8, 21, 10, 0), t, 0.5).unwrap();
    ProgressService::report_percent(&conn, &at(2026, 8, 22, 11, 0), t, 2.0).unwrap();
    assert_target(&conn, 2026, 8, 22, 300.0, "周六当日无目标义务");
    for (y, mo, d) in [
        (2026, 8, 24),
        (2026, 8, 25),
        (2026, 8, 26),
        (2026, 8, 27),
        (2026, 8, 28),
        (2026, 8, 31),
        (2026, 9, 1),
    ] {
        assert_target(&conn, y, mo, d, 300.0 + 150.0 / 7.0, "缺口上调 + 加班超额入账，跨周末逐工作日落账");
        ProgressService::report_percent(&conn, &at(y, mo, d, 10, 0), t, 5.0).unwrap(); // 带内中和
    }
    assert_target(&conn, 2026, 9, 2, 300.0, "第 8 个工作日窗口耗尽");
}

#[test]
fn correction_recomputes_future_targets() {
    // 测试情况：a) 周一报 180min 后"修正总进度"到 300min（恰好达标；事件时刻仍在周一，
    //           归属周一）；
    //           b) 子目标任务周一勾掉一个 60min 子目标后撤销（补偿账同样归属周一）。
    // 正确结果：修正/撤销前周二目标已上调；改口后受影响日期差额与未来目标**实时重算**——
    //           a) 周一回到带内 → 周二回到基准 300；b) 周一净量归零 → 周二 = 300+300/7。
    //           （不存在日结冻结，日志是唯一真相，ADR-0009）
    let conn = db::open_in_memory().unwrap();
    let t = seeded(&conn);
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 0), t, 3.0).unwrap();
    assert_target(&conn, 2026, 8, 25, 300.0 + 120.0 / 7.0, "修正前：缺口 120 已上调周二");
    ProgressService::correct_total(&conn, &at(2026, 8, 24, 20, 0), t, 5.0).unwrap();
    assert_target(&conn, 2026, 8, 25, 300.0, "修正到 300min 达标 → 周二实时回到基准");

    let conn = db::open_in_memory().unwrap();
    let plan_id = PlanService::create(
        &conn,
        &at(2026, 8, 20, 9, 0),
        &PlanDraft {
            name: "Q".into(),
            summary: String::new(),
            detail: String::new(),
            priority: Priority::Medium,
            due_date: None,
            tasks: vec![{
                let mut task = plain_task("清单任务", 0);
                task.has_subgoals = true;
                task.estimated_minutes = None;
                task.subgoals = (1..=3)
                    .map(|i| SubGoalDraft { id: None, name: format!("第{i}项"), estimated_minutes: 60 })
                    .collect();
                task
            }],
        },
    )
    .unwrap();
    LifecycleService::start(&conn, plan_id).unwrap();
    let view = PlanService::get(&conn, plan_id).unwrap();
    let sg = view.tasks[0].subgoals[0].id;
    ProgressService::complete_subgoal(&conn, &at(2026, 8, 24, 10, 0), sg).unwrap();
    assert_target(&conn, 2026, 8, 25, 300.0 + 240.0 / 7.0, "撤销前：缺口 240 已上调周二");
    ProgressService::undo_subgoal(&conn, &at(2026, 8, 24, 20, 0), sg).unwrap();
    assert_target(&conn, 2026, 8, 25, 300.0 + 300.0 / 7.0, "撤销后净量归零 → 周二实时重算为整日缺口");
}

#[test]
fn selection_level_surplus_not_credited() {
    // 测试情况：大面板勾选两个任务共 14h 提交分配，当日实际只推 5h（"选 14h 推 5h"）。
    // 正确结果：选择层面的超额不进入账户——当日实际 5h 恰好达标，周二目标仍是基准 300
    //           （只有实际完成的超额才抵扣，CONTEXT TodayLoadCommitment）。
    let conn = db::open_in_memory().unwrap();
    let plan_id = PlanService::create(
        &conn,
        &at(2026, 8, 20, 9, 0),
        &PlanDraft {
            name: "P".into(),
            summary: String::new(),
            detail: String::new(),
            priority: Priority::Medium,
            due_date: None,
            tasks: vec![plain_task("任务一", 360), plain_task("任务二", 480)],
        },
    )
    .unwrap();
    LifecycleService::start(&conn, plan_id).unwrap();
    let view = PlanService::get(&conn, plan_id).unwrap();
    let (t1, t2) = (view.tasks[0].id, view.tasks[1].id);
    AllocationService::commit(&conn, &at(2026, 8, 24, 8, 0), &[t1, t2]).unwrap(); // 选满 14h
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 0), t1, 50.0).unwrap(); // 实推 180
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 11, 0), t2, 25.0).unwrap(); // 实推 120
    assert_target(&conn, 2026, 8, 25, 300.0, "实推 5h 恰达标，选择层面的 14h 不抵扣");
}

#[test]
fn override_holiday_progress_counts_as_overtime() {
    // 测试情况（工单 13 日期例外 × 工单 10 结算回归）：周二 8/25 被例外标为休息
    //           （原工作日）；周一推 198min（3.3%×6000，带外欠 102）、周二休日推
    //           600min（10%×6000）。
    // 正确结果：周一欠 102 均分到后续 7 个**工作日**（周二被例外剔除，顺延到
    //           周三起）；周二的推进按超额全额入账（-600 抵扣）；周三目标 =
    //           300 + (102-600)/7 ≈ 228.857——休日干活换来了之后的轻松日。
    let conn = db::open_in_memory().unwrap();
    let task = seeded(&conn);
    force_settings(
        &conn,
        &Settings {
            date_overrides: vec![DateOverride { date: d(2026, 8, 25), working: false }],
            ..Settings::default()
        },
    );
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 0), task, 3.3).unwrap();
    ProgressService::report_percent(&conn, &at(2026, 8, 25, 11, 0), task, 10.0).unwrap();
    assert_target(&conn, 2026, 8, 25, 300.0, "休日无目标义务（基准展示，UI 隐藏目标）");
    assert_target(&conn, 2026, 8, 26, 300.0 + (102.0 - 600.0) / 7.0, "周二推进按超额入账且不占工作日窗口位");
}

#[test]
fn override_workday_adds_target_and_window_slot() {
    // 测试情况（工单 13 日期例外 × 工单 10 结算回归）：周六 8/29 被例外标为工作
    //           （调休补班）；周六推 102min（1.7%×6000）。
    // 正确结果：周六自身有目标义务（基准 300、无结转）；差额 198 均分到后续
    //           工作日 → 周一目标 ≈ 328.57；对照无例外时周六是休日、102 全额按
    //           超额抵扣 → 周一目标 ≈ 285.43。
    let with_work = db::open_in_memory().unwrap();
    let task = seeded(&with_work);
    force_settings(
        &with_work,
        &Settings {
            date_overrides: vec![DateOverride { date: d(2026, 8, 29), working: true }],
            ..Settings::default()
        },
    );
    ProgressService::report_percent(&with_work, &at(2026, 8, 29, 10, 0), task, 1.7).unwrap();
    assert_target(&with_work, 2026, 8, 29, 300.0, "补班周六自身有目标义务（无结转）");
    assert_target(&with_work, 2026, 8, 31, 300.0 + 198.0 / 7.0, "补班日计入均分窗口（周日跳过）");

    let without_work = db::open_in_memory().unwrap();
    let task2 = seeded(&without_work);
    force_settings(&without_work, &Settings::default());
    ProgressService::report_percent(&without_work, &at(2026, 8, 29, 10, 0), task2, 1.7).unwrap();
    assert_target(&without_work, 2026, 8, 31, 300.0 - 102.0 / 7.0, "对照：休日推进按超额抵扣后续目标");
}
