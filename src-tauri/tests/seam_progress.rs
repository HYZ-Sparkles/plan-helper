//! 进度汇报接缝测试（工单 07）：增量汇报与 ProgressLog 派生、百分比颗粒度与溢出、
//! 子目标按序推进与自动完成、撤销重算（前缀不变式）、修正总进度落账、
//! 当前任务生命周期（今日列表归属 / 失效回空 / 同计划排前）、跨午夜归属。

use plan_helper_lib::domain::allocation::AllocationService;
use plan_helper_lib::domain::lifecycle::LifecycleService;
use plan_helper_lib::domain::plans::{
    PlanDraft, PlanError, PlanService, PlanStatus, Priority, SubGoalDraft, TaskStatus,
};
use plan_helper_lib::domain::progress::ProgressService;
use plan_helper_lib::domain::settings::{Settings, SettingsService, TimeWindow};
use plan_helper_lib::infra::db;

mod common;
use common::{at, force_settings, plain_task};


/// 三个子目标（60/30/30 = 120 分）的任务草稿——按序勾选路径的种子
fn subgoal_task(name: &str) -> plan_helper_lib::domain::plans::TaskDraft {
    let mut t = plain_task(name, 0);
    t.has_subgoals = true;
    t.estimated_minutes = None;
    t.subgoals = vec![
        SubGoalDraft { id: None, name: "一".into(), estimated_minutes: 60 },
        SubGoalDraft { id: None, name: "二".into(), estimated_minutes: 30 },
        SubGoalDraft { id: None, name: "三".into(), estimated_minutes: 30 },
    ];
    t
}

/// 计划草稿（Medium）+ 指定任务集
fn draft_of_tasks(name: &str, tasks: Vec<plan_helper_lib::domain::plans::TaskDraft>) -> PlanDraft {
    PlanDraft {
        name: name.into(),
        summary: String::new(),
        detail: String::new(),
        priority: Priority::Medium,
        due_date: None,
        tasks,
    }
}

/// 建"进行中计划 + 今日分配 + 当前任务"的最小闭环，返回 (计划 id, 任务 id)。
fn seeded(conn: &rusqlite::Connection, task: plan_helper_lib::domain::plans::TaskDraft) -> (i64, i64) {
    let plan_id = PlanService::create(conn, &at(2026, 8, 23, 9, 0), &draft_of_tasks("P", vec![task])).unwrap();
    LifecycleService::start(conn, plan_id).unwrap();
    let task_id = PlanService::get(conn, plan_id).unwrap().tasks[0].id;
    AllocationService::commit(conn, &at(2026, 8, 24, 8, 0), &[task_id]).unwrap();
    ProgressService::set_current_task(conn, &at(2026, 8, 24, 8, 30), task_id).unwrap();
    (plan_id, task_id)
}

/// 派生进度断言用：当前任务的百分比
fn current_percent(conn: &rusqlite::Connection) -> f64 {
    ProgressService::board(conn, &at(2026, 8, 24, 12, 0))
        .unwrap()
        .current
        .unwrap()
        .percent
}

/// 今日完成量（分钟）
fn today_minutes(conn: &rusqlite::Connection) -> f64 {
    ProgressService::board(conn, &at(2026, 8, 24, 12, 0)).unwrap().today_minutes
}

#[test]
fn percent_reports_derive_from_log_and_auto_complete() {
    // 测试情况：120 分钟无子目标任务，先报 +10% 再报 +27.5%（2026-08-24 修订后
    //           允许任意一位小数步进），最后补到 100% 后继续报。
    // 正确结果：每次汇报都是增量落账（progress_log 两条 +12/+33）；
    //           派生进度 10% → 37.5%（= 日志求和 / 总耗时）；首次汇报任务转进行中；
    //           到 100% 自动转已完成并锁定（再报 ProgressLocked）；今日完成量 = 求和。
    let conn = db::open_in_memory().unwrap();
    let (_, task_id) = seeded(&conn, plain_task("背单词", 120));

    let s1 = ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 0), task_id, 10.0).unwrap();
    assert_eq!(s1, TaskStatus::Active, "首次推进：未开始 → 进行中");
    let s2 = ProgressService::report_percent(&conn, &at(2026, 8, 24, 11, 0), task_id, 27.5).unwrap();
    assert_eq!(s2, TaskStatus::Active);
    assert!((current_percent(&conn) - 37.5).abs() < 1e-9, "37.5% = (12+33)/120");
    assert!((today_minutes(&conn) - 45.0).abs() < 1e-9, "今日完成量 = 日志求和");
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM progress_log", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 2, "每次汇报各追加一条事件");

    let s3 = ProgressService::report_percent(&conn, &at(2026, 8, 24, 13, 0), task_id, 62.5).unwrap();
    assert_eq!(s3, TaskStatus::Completed, "累计 100% 自动完成");
    assert_eq!(
        ProgressService::report_percent(&conn, &at(2026, 8, 24, 14, 0), task_id, 5.0),
        Err(PlanError::ProgressLocked),
        "已完成任务锁定，不可再汇报"
    );
    // 小看板停留态：当前任务仍展示，状态已完成（不自动切换）
    let cur = ProgressService::board(&conn, &at(2026, 8, 24, 15, 0)).unwrap().current.unwrap();
    assert_eq!((cur.status, cur.percent as u32), (TaskStatus::Completed, 100));
}

#[test]
fn percent_granularity_and_overflow_rejected() {
    // 测试情况：120 分钟任务依次尝试 0.05%（两位小数）/ 100.5%（越界）/ 0 与 -5
    //           （非正数），报到 99.5% 后再 +10%（溢出）与 +0.5%（恰好补满）。
    // 正确结果：两位小数、越界、非正数全部 PercentInvalid 且不落账；
    //           超过 100% 的增量 PercentOverflow；恰好到 100% 的增量放行。
    let conn = db::open_in_memory().unwrap();
    let (_, task_id) = seeded(&conn, plain_task("阅读", 120));

    for bad in [0.05, 100.5, 0.0, -5.0] {
        assert_eq!(
            ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 0), task_id, bad),
            Err(PlanError::PercentInvalid),
            "{bad}% 非法（正数 / 一位小数 / ≤100 至少一条不满足）"
        );
    }
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM progress_log", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 0, "被拒的汇报不落账");

    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 30), task_id, 99.0).unwrap();
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 40), task_id, 0.5).unwrap();
    assert!((current_percent(&conn) - 99.5).abs() < 1e-9);
    assert_eq!(
        ProgressService::report_percent(&conn, &at(2026, 8, 24, 11, 0), task_id, 10.0),
        Err(PlanError::PercentOverflow),
        "99.5% + 10% 越过 100%"
    );
    assert_eq!(
        ProgressService::report_percent(&conn, &at(2026, 8, 24, 11, 30), task_id, 0.5),
        Ok(TaskStatus::Completed),
        "99.5% + 0.5% 恰好补满，自动完成"
    );
}

#[test]
/// 派生 percent 一位小数 round（工单 07 验收补述 + CONTEXT ProgressGranularity）：
/// 600 分钟任务反复 +0.1% 汇报，累计到 43.1%；期望 `current_percent()` 为干净的 43.1
/// （而非 43.0999999...）；同步断言 ProgressLog 求和保留原始精度（ADR-0009 单一事实源），
/// round 只发生在派生层，存储层不动。
fn progress_percent_rounds_to_one_decimal() {
    let conn = db::open_in_memory().unwrap();
    let (_, task_id) = seeded(&conn, plain_task("代码", 600));

    // 累计到 43.1%：一次 +40% + 三次 +1% + 一次 +0.1% = 43.1%
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 0), task_id, 40.0).unwrap();
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 5), task_id, 1.0).unwrap();
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 10), task_id, 1.0).unwrap();
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 15), task_id, 1.0).unwrap();
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 20), task_id, 0.1).unwrap();

    let p = current_percent(&conn);
    assert!((p - 43.1).abs() < 1e-9, "派生 percent 一位小数 round, got {p}");
    // 存储层 ProgressLog 求和保持原始精度（240 + 6.000000000000001...），不因 round 失真
    let sum: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(delta_minutes), 0.0) FROM progress_log",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        (sum - 258.6).abs() < 0.01,
        "ProgressLog sum 接近 258.6 分钟（原始精度）, got {sum}"
    );
}

#[test]
fn subgoals_complete_in_order_and_auto_complete() {
    // 测试情况：三个子目标（60/30/30），先勾第二个、重复勾第一个、百分比汇报此任务，
    //           再按序全勾后继续勾。
    // 正确结果：乱序与重复勾选 SubGoalOutOfOrder；百分比通道 NotPercentTask；
    //           按序勾选进度 50% → 75% → 100% 自动完成并锁定。
    let conn = db::open_in_memory().unwrap();
    let (plan_id, task_id) = seeded(&conn, subgoal_task("精读教材"));
    let sg: Vec<i64> = PlanService::get(&conn, plan_id).unwrap().tasks[0]
        .subgoals
        .iter()
        .map(|s| s.id)
        .collect();
    let (s1, s2, s3) = (sg[0], sg[1], sg[2]);

    assert_eq!(
        ProgressService::complete_subgoal(&conn, &at(2026, 8, 24, 10, 0), s2),
        Err(PlanError::SubGoalOutOfOrder),
        "跳过第一个直接勾第二个被拒"
    );
    let st = ProgressService::complete_subgoal(&conn, &at(2026, 8, 24, 10, 5), s1).unwrap();
    assert_eq!(st, TaskStatus::Active);
    assert!((current_percent(&conn) - 50.0).abs() < 1e-9, "60/120");
    assert_eq!(
        ProgressService::complete_subgoal(&conn, &at(2026, 8, 24, 10, 10), s1),
        Err(PlanError::SubGoalOutOfOrder),
        "重复勾选已完成子目标被拒"
    );
    assert_eq!(
        ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 15), task_id, 5.0),
        Err(PlanError::NotPercentTask),
        "有子目标任务不走百分比通道"
    );

    ProgressService::complete_subgoal(&conn, &at(2026, 8, 24, 10, 20), s2).unwrap();
    assert!((current_percent(&conn) - 75.0).abs() < 1e-9);
    let st = ProgressService::complete_subgoal(&conn, &at(2026, 8, 24, 10, 25), s3).unwrap();
    assert_eq!(st, TaskStatus::Completed, "全勾自动完成");
    assert_eq!(
        ProgressService::complete_subgoal(&conn, &at(2026, 8, 24, 10, 30), s2),
        Err(PlanError::ProgressLocked),
        "已完成任务子目标锁定"
    );
}

#[test]
fn subgoal_undo_recalculates_and_keeps_prefix() {
    // 测试情况：勾完前两个子目标（75%）后：撤销第一个（非末尾）、撤销未完成的第三个、
    //           撤销第二个（末尾）、重复撤销、全勾后撤销。
    // 正确结果：非末尾撤销 SubGoalOutOfOrder（保住"已完成是前缀"不变式）；
    //           撤销未完成 SubGoalNotCompleted；末尾撤销成功且今日完成量实时重算
    //           （+60 +30 -30 = 60）；已完成任务撤销 ProgressLocked。
    let conn = db::open_in_memory().unwrap();
    let (plan_id, _) = seeded(&conn, subgoal_task("刷题"));
    let task = &PlanService::get(&conn, plan_id).unwrap().tasks[0];
    let (s1, s2, s3) = (task.subgoals[0].id, task.subgoals[1].id, task.subgoals[2].id);

    ProgressService::complete_subgoal(&conn, &at(2026, 8, 24, 10, 0), s1).unwrap();
    ProgressService::complete_subgoal(&conn, &at(2026, 8, 24, 10, 5), s2).unwrap();
    assert!((current_percent(&conn) - 75.0).abs() < 1e-9);

    assert_eq!(
        ProgressService::undo_subgoal(&conn, &at(2026, 8, 24, 10, 10), s1),
        Err(PlanError::SubGoalOutOfOrder),
        "第二个还完成着，撤销第一个会破坏前缀"
    );
    assert_eq!(
        ProgressService::undo_subgoal(&conn, &at(2026, 8, 24, 10, 15), s3),
        Err(PlanError::SubGoalNotCompleted),
        "撤销未完成子目标无意义"
    );

    ProgressService::undo_subgoal(&conn, &at(2026, 8, 24, 10, 20), s2).unwrap();
    assert!((current_percent(&conn) - 50.0).abs() < 1e-9, "进度按日志实时重算");
    assert!((today_minutes(&conn) - 60.0).abs() < 1e-9, "60 + 30 - 30 = 60（补偿账）");
    assert_eq!(
        ProgressService::undo_subgoal(&conn, &at(2026, 8, 24, 10, 25), s2),
        Err(PlanError::SubGoalNotCompleted),
        "重复撤销被拒"
    );

    // 全勾成已完成后撤销被锁定
    ProgressService::complete_subgoal(&conn, &at(2026, 8, 24, 10, 30), s2).unwrap();
    ProgressService::complete_subgoal(&conn, &at(2026, 8, 24, 10, 35), s3).unwrap();
    assert_eq!(
        ProgressService::undo_subgoal(&conn, &at(2026, 8, 24, 10, 40), s3),
        Err(PlanError::ProgressLocked),
        "任务已自动完成，勾选状态锁定不可撤销"
    );
}

#[test]
fn correction_sets_absolute_value_and_logs_delta() {
    // 测试情况：120 分钟任务报 10% 后修正到 70%、再修正到 20%、修正到 0；
    //           对有子目标任务修正、非法值、已完成任务修正、修正到 100%。
    // 正确结果：每次修正落一条 Correction 事件，增量 = 目标值 - 当前值（可正可负）；
    //           派生进度等于设定值；有子目标 NotPercentTask；非法值 PercentInvalid；
    //           已完成 ProgressLocked；修正到 100% 自动完成。
    let conn = db::open_in_memory().unwrap();
    let (_, task_id) = seeded(&conn, plain_task("写作", 120));
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 10, 0), task_id, 10.0).unwrap();

    ProgressService::correct_total(&conn, &at(2026, 8, 24, 11, 0), task_id, 62.5).unwrap();
    assert!((current_percent(&conn) - 62.5).abs() < 1e-9, "一位小数设定值（2026-08-24 修订）");
    ProgressService::correct_total(&conn, &at(2026, 8, 24, 12, 0), task_id, 20.0).unwrap();
    assert!((current_percent(&conn) - 20.0).abs() < 1e-9, "下调修正：负增量落账");
    ProgressService::correct_total(&conn, &at(2026, 8, 24, 13, 0), task_id, 0.0).unwrap();
    assert!((current_percent(&conn) - 0.0).abs() < 1e-9, "归零修正");
    assert!((today_minutes(&conn) - 0.0).abs() < 1e-9, "对账后今日净完成量 = 0");
    let sources: Vec<String> = conn
        .prepare("SELECT source FROM progress_log ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(sources, vec!["Percent", "Correction", "Correction", "Correction"]);

    assert_eq!(
        ProgressService::correct_total(&conn, &at(2026, 8, 24, 14, 0), task_id, 12.34),
        Err(PlanError::PercentInvalid),
        "12.34% 超过一位小数"
    );
    let sg_plan = PlanService::create(
        &conn,
        &at(2026, 8, 24, 9, 0),
        &draft_of_tasks("SG计划", vec![subgoal_task("带子目标")]),
    )
    .unwrap();
    let sg_task = PlanService::get(&conn, sg_plan).unwrap().tasks[0].id;
    assert_eq!(
        ProgressService::correct_total(&conn, &at(2026, 8, 24, 14, 0), sg_task, 50.0),
        Err(PlanError::NotPercentTask),
        "有子目标任务修正走撤销通道，不走百分比"
    );

    ProgressService::correct_total(&conn, &at(2026, 8, 24, 15, 0), task_id, 100.0).unwrap();
    assert_eq!(
        ProgressService::correct_total(&conn, &at(2026, 8, 24, 16, 0), task_id, 50.0),
        Err(PlanError::ProgressLocked),
        "修正到 100% 自动完成后锁定"
    );
}

#[test]
fn current_task_requires_today_list_and_follows_validity() {
    // 测试情况：两个进行中计划（抢占不变式保证同等级，工单 12）各一个任务，全部选入今日；
    //           指定未分配任务为当前任务、指定当前任务（创建较早的计划）、暂停其计划、
    //           恢复、重交分配剔除该任务、隔日再读。
    // 正确结果：不在今日列表 TaskNotInToday；指定成功后视图展示；
    //           计划暂停 → 小看板回空态，恢复 → 回到当前任务；
    //           被移出今日列表 → 回空态；隔日分配失效 → 回空态、指定也被拒；
    //           「更换任务」候选当前任务同计划分组排最前（默认序创建倒序为
    //           [高计划, 中计划]，当前任务的中计划被提到最前才算通过）。
    let conn = db::open_in_memory().unwrap();
    let p_high = PlanService::create(&conn, &at(2026, 8, 23, 9, 0), &draft_of_tasks("高计划", vec![plain_task("高任务", 60)])).unwrap();
    let p_mid = PlanService::create(&conn, &at(2026, 8, 22, 9, 0), &draft_of_tasks("中计划", vec![plain_task("中任务", 90)])).unwrap();
    LifecycleService::start(&conn, p_high).unwrap();
    LifecycleService::start(&conn, p_mid).unwrap();
    let t_high = PlanService::get(&conn, p_high).unwrap().tasks[0].id;
    let t_mid = PlanService::get(&conn, p_mid).unwrap().tasks[0].id;

    // 未分配先指定：拒绝（今日推进列表为空）
    assert_eq!(
        ProgressService::set_current_task(&conn, &at(2026, 8, 24, 8, 0), t_mid),
        Err(PlanError::TaskNotInToday { task_id: t_mid })
    );
    AllocationService::commit(&conn, &at(2026, 8, 24, 8, 30), &[t_high, t_mid]).unwrap();
    ProgressService::set_current_task(&conn, &at(2026, 8, 24, 9, 0), t_mid).unwrap();

    let v = ProgressService::board(&conn, &at(2026, 8, 24, 10, 0)).unwrap();
    let cur = v.current.as_ref().unwrap();
    assert_eq!((cur.task_id, cur.plan_name.as_str(), cur.task_name.as_str()), (t_mid, "中计划", "中任务"));
    assert_eq!(v.target_minutes, 300.0, "无账户历史 = 目标为基准（工单 10 起含结转派生，f64）");
    assert_eq!(
        v.pickers.iter().map(|g| g.plan_name.as_str()).collect::<Vec<_>>(),
        vec!["中计划", "高计划"],
        "更换候选：当前任务同计划排最前，其余按 PlanOrdering"
    );

    LifecycleService::pause(&conn, p_mid).unwrap();
    assert!(
        ProgressService::board(&conn, &at(2026, 8, 24, 11, 0)).unwrap().current.is_none(),
        "计划暂停：小看板回空态"
    );
    assert_eq!(
        ProgressService::set_current_task(&conn, &at(2026, 8, 24, 11, 30), t_mid),
        Err(PlanError::PlanStatusInvalid { from: PlanStatus::Paused }),
        "暂停计划的任务不可指定为当前任务"
    );
    LifecycleService::resume(&conn, p_mid).unwrap();
    assert!(
        ProgressService::board(&conn, &at(2026, 8, 24, 12, 0)).unwrap().current.is_some(),
        "恢复进行中：当前任务回来（存储未清）"
    );

    AllocationService::commit(&conn, &at(2026, 8, 24, 13, 0), &[t_high]).unwrap();
    assert!(
        ProgressService::board(&conn, &at(2026, 8, 24, 13, 30)).unwrap().current.is_none(),
        "当前任务被移出今日推进列表：回空态"
    );

    let next_day = ProgressService::board(&conn, &at(2026, 8, 25, 9, 0)).unwrap();
    assert!(next_day.current.is_none(), "隔日分配失效：回空态");
    assert_eq!(
        ProgressService::set_current_task(&conn, &at(2026, 8, 25, 9, 30), t_high),
        Err(PlanError::TaskNotInToday { task_id: t_high })
    );
}

#[test]
fn today_minutes_attributed_by_window_start_day() {
    // 测试情况：工作窗口设为跨午夜 20:00–01:00；在 8/24 20:30 与 8/25 00:30 各报一次，
    //           8/25 10:00（窗口外）再报一次，随后在三个时点读"今日完成量"。
    // 正确结果：跨午夜窗口的凌晨段归属窗口开始日（8/25 00:30 的量算进 8/24）；
    //           8/24 深夜读 = 前两笔；8/25 凌晨读 = 0（已归 8/24）；窗口外的量归自身日期。
    let conn = db::open_in_memory().unwrap();
    let (plan_id, task_id) = seeded(&conn, plain_task("夜间任务", 120));
    let _ = plan_id;
    // 跨午夜窗口"自从有设置以来一直如此"（工单 13 起 save 的延时字段自下一个
    // 工作日生效，会把 8/24 留在默认窗口——force_settings 直改存储绕开延时）
    force_settings(
        &conn,
        &Settings {
            time_windows: vec![TimeWindow { start_minute: 20 * 60, end_minute: 60 }],
            ..Settings::default()
        },
    );

    ProgressService::report_percent(&conn, &at(2026, 8, 24, 20, 30), task_id, 10.0).unwrap();
    ProgressService::report_percent(&conn, &at(2026, 8, 25, 0, 30), task_id, 20.0).unwrap();

    let late_night = ProgressService::board(&conn, &at(2026, 8, 24, 23, 0)).unwrap().today_minutes;
    assert!((late_night - 36.0).abs() < 1e-9, "8/24 深夜读：12 + 24 都归 8/24，实际 {late_night}");
    let after_midnight = ProgressService::board(&conn, &at(2026, 8, 25, 0, 45)).unwrap().today_minutes;
    assert!((after_midnight - 0.0).abs() < 1e-9, "8/25 凌晨读：凌晨段归 8/24，今日为 0，实际 {after_midnight}");

    // 8/25 10:00 不在任何窗口内（窗口只有 20:00–01:00）：归属自身日期 8/25
    ProgressService::report_percent(&conn, &at(2026, 8, 25, 10, 0), task_id, 5.0).unwrap();
    let day2 = ProgressService::board(&conn, &at(2026, 8, 25, 12, 0)).unwrap().today_minutes;
    assert!((day2 - 6.0).abs() < 1e-9, "窗口外推进归属自身日期 8/25，实际 {day2}");
}

#[test]
fn attribution_uses_window_owner_day_after_settings_change() {
    // 测试情况（工单 13 生效时机 × 跨午夜归属）：跨午夜窗口 20:00–01:00"一直如此"；
    //           8/24 23:30 报 12min，随后 23:45 把窗口改为 09:00–12:00（延时字段 →
    //           8/25 生效），8/25 00:30 再报 24min。
    // 正确结果：00:30 事件的归属看**窗口开始日（8/24）的配置**——8/24 仍有跨午夜
    //           窗口，凌晨段归 8/24（两笔共 36min）；8/25 的"今日完成量"为 0。
    //           归属若只看事件当日（8/25 已是新窗口）就会误归 8/25。
    let conn = db::open_in_memory().unwrap();
    let (plan_id, task_id) = seeded(&conn, plain_task("夜间任务", 120));
    let _ = plan_id;
    force_settings(
        &conn,
        &Settings {
            time_windows: vec![TimeWindow { start_minute: 20 * 60, end_minute: 60 }],
            ..Settings::default()
        },
    );
    ProgressService::report_percent(&conn, &at(2026, 8, 24, 23, 30), task_id, 10.0).unwrap(); // 12min
    let mut s = SettingsService::load(&conn).unwrap();
    s.time_windows = vec![TimeWindow { start_minute: 540, end_minute: 720 }];
    SettingsService::save(&conn, &at(2026, 8, 24, 23, 45), &s).unwrap(); // 周一改 → 周二生效
    ProgressService::report_percent(&conn, &at(2026, 8, 25, 0, 30), task_id, 20.0).unwrap(); // 24min

    let monday = ProgressService::board(&conn, &at(2026, 8, 24, 23, 59)).unwrap().today_minutes;
    assert!((monday - 36.0).abs() < 1e-9, "两笔都归窗口开始日 8/24，实际 {monday}");
    let tuesday = ProgressService::board(&conn, &at(2026, 8, 25, 0, 45)).unwrap().today_minutes;
    assert!((tuesday - 0.0).abs() < 1e-9, "8/25 凌晨读：凌晨段归 8/24，今日为 0，实际 {tuesday}");
}
