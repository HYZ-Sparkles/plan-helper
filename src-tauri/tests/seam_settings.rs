//! 接缝集成测试（spec Testing Decisions：全部自动化测试打在 Rust 领域服务层）。
//! 直接以 SQLite + 注入时钟驱动领域服务，不经过 Tauri command / 窗口。

use chrono::{Local, TimeZone};

use plan_helper_lib::clock::FixedClock;
use plan_helper_lib::domain::app_state::app_state_view;
use plan_helper_lib::domain::settings::{DateOverride, Settings, SettingsService, TimeWindow};
use plan_helper_lib::infra::db;

mod common;
use common::{d, force_settings};

/// 造一个固定时钟（本地时区某日的某时刻）
fn fixed_clock(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> FixedClock {
    FixedClock(Local.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap())
}

#[test]
fn first_run_loads_and_persists_defaults() {
    // 测试情况：全新数据库第一次读取设置（FirstRun 路径）。
    // 正确结果：返回 CONTEXT「首次运行」默认值——每日 5h（300 分钟）、
    //          工作日周一至五、均分窗口 7 个工作日；且默认值已落库——
    //          第二次读取得到同一份（INSERT OR IGNORE 不重复插入、不覆盖）。
    let conn = db::open_in_memory().unwrap();
    let first = SettingsService::load(&conn).unwrap();
    assert_eq!(first.daily_minutes, 300);
    assert_eq!(first.workdays, vec![1, 2, 3, 4, 5]);
    assert_eq!(first.smoothing_workdays, 7);
    assert_eq!(SettingsService::load(&conn).unwrap(), first);
}

#[test]
fn save_overwrites_and_records_injected_clock() {
    // 测试情况：保存一份自定义设置后回读；updated_at 应取注入时钟而非系统时间。
    // 正确结果：回读值与保存值完全一致（含多段窗口与跨午夜窗口的序列化往返）；
    //          updated_at 等于 FixedClock 注入的时刻。
    let conn = db::open_in_memory().unwrap();
    SettingsService::load(&conn).unwrap();
    let clock = fixed_clock(2026, 8, 23, 20, 30);
    let custom = Settings {
        daily_minutes: 240,
        workdays: vec![2, 4, 6],
        time_windows: vec![
            TimeWindow {
                start_minute: 10 * 60,
                end_minute: 11 * 60 + 30,
            },
            TimeWindow {
                start_minute: 20 * 60, // 20:00–01:00 跨午夜，end < start
                end_minute: 60,
            },
        ],
        smoothing_workdays: 14,
        date_overrides: Vec::new(),
    };
    SettingsService::save(&conn, &clock, &custom).unwrap();
    assert_eq!(SettingsService::load(&conn).unwrap(), custom);
    assert_eq!(SettingsService::updated_at(&conn).unwrap(), Some(clock.0));
    // 2026-08-23 是周日：默认配置的下一个工作日 = 周一 08-24（返回值供设置页
    // "自 X 起生效"提示）
    assert_eq!(
        SettingsService::save(&conn, &clock, &custom).unwrap(),
        chrono::NaiveDate::from_ymd_opt(2026, 8, 24).unwrap()
    );
}

#[test]
fn app_state_view_composes_settings_with_injected_now() {
    // 测试情况：示例接缝命令背后的领域组装——快照 = 设置 + 服务端当前时间。
    // 正确结果：server_now 精确等于注入时钟的时间（证明时钟注入贯通），
    //          settings 为 FirstRun 默认值。
    let conn = db::open_in_memory().unwrap();
    let clock = fixed_clock(2026, 8, 23, 9, 0);
    let view = app_state_view(&conn, &clock).unwrap();
    assert_eq!(view.server_now, clock.0);
    assert_eq!(view.settings, Settings::default());
}

#[test]
fn is_work_time_requires_workday_and_window() {
    // 测试情况：默认设置（工作日周一至五、窗口 09:00–18:00），用注入时钟在
    //           窗口内 / 窗口前 / 窗口后 / 休息日各取一个时刻判定。
    // 正确结果：工作日窗口内 true；窗口外（08:00、18:00 整点 = 右开端点）false；
    //           休息日（周六）即便窗口内时刻也 false。
    let conn = db::open_in_memory().unwrap();
    SettingsService::load(&conn).unwrap();
    assert!(
        SettingsService::is_work_time(&conn, &fixed_clock(2026, 8, 24, 10, 0)).unwrap(),
        "周一 10:00 在窗"
    );
    assert!(
        !SettingsService::is_work_time(&conn, &fixed_clock(2026, 8, 24, 8, 0)).unwrap(),
        "周一 08:00 窗口前"
    );
    assert!(
        !SettingsService::is_work_time(&conn, &fixed_clock(2026, 8, 24, 18, 0)).unwrap(),
        "周一 18:00 整 = 窗口结束那一刻"
    );
    assert!(
        !SettingsService::is_work_time(&conn, &fixed_clock(2026, 8, 29, 10, 0)).unwrap(),
        "周六 10:00 休息日"
    );
}

#[test]
fn is_work_time_covers_multi_window_and_cross_midnight() {
    // 测试情况：自定义设置（工作日周二/周四、窗口 10:00–11:30 与跨午夜 20:00–01:00），
    //           在两段窗口内、窗间空档、跨午夜今晚段与凌晨段分别判定。
    // 正确结果：两段窗口内 true；窗间空档 false；跨午夜今晚段（周四 23:00）true；
    //           凌晨段归属窗口开始日——周五 00:30（昨天周四是工作日）true、
    //           周五 01:30（已出窗）false；周日 00:30（昨天周六非工作日）false。
    let conn = db::open_in_memory().unwrap();
    force_settings(
        &conn,
        &Settings {
            time_windows: vec![
                TimeWindow {
                    start_minute: 10 * 60,
                    end_minute: 11 * 60 + 30,
                },
                TimeWindow {
                    start_minute: 20 * 60, // 20:00–01:00 跨午夜，end < start
                    end_minute: 60,
                },
            ],
            workdays: vec![2, 4], // 周二、周四
            ..Settings::default()
        },
    );

    assert!(
        SettingsService::is_work_time(&conn, &fixed_clock(2026, 8, 25, 11, 0)).unwrap(),
        "周二 11:00 第一段在窗"
    );
    assert!(
        !SettingsService::is_work_time(&conn, &fixed_clock(2026, 8, 25, 15, 0)).unwrap(),
        "周二 15:00 窗间空档"
    );
    assert!(
        SettingsService::is_work_time(&conn, &fixed_clock(2026, 8, 27, 23, 0)).unwrap(),
        "周四 23:00 跨午夜今晚段"
    );
    assert!(
        SettingsService::is_work_time(&conn, &fixed_clock(2026, 8, 28, 0, 30)).unwrap(),
        "周五 00:30 凌晨段，窗口开始日（周四）是工作日"
    );
    assert!(
        !SettingsService::is_work_time(&conn, &fixed_clock(2026, 8, 28, 1, 30)).unwrap(),
        "周五 01:30 已出窗"
    );
    assert!(
        !SettingsService::is_work_time(&conn, &fixed_clock(2026, 8, 30, 0, 30)).unwrap(),
        "周日 00:30 昨天周六非工作日，凌晨段不算"
    );
}

#[test]
fn delayed_fields_take_effect_next_workday() {
    // 测试情况：周一 12:00 保存改动后的每日工作时间（延时字段），
    //           随后按日解析周一（当日）/ 周二（下一个工作日）/ 下周一的配置。
    // 正确结果：save 返回周二 08-25（生效日）；周一维持原值 300（当日目标、达标
    //           判定、总结不受影响），周二起读到 400——SettingsEffectiveTime。
    let conn = db::open_in_memory().unwrap();
    let clock = fixed_clock(2026, 8, 24, 12, 0); // 周一
    let mut s = SettingsService::load(&conn).unwrap(); // FirstRun 落默认行
    s.daily_minutes = 400;
    let effective = SettingsService::save(&conn, &clock, &s).unwrap();
    assert_eq!(effective, chrono::NaiveDate::from_ymd_opt(2026, 8, 25).unwrap());
    let cal = SettingsService::calendar(&conn).unwrap();
    assert_eq!(cal.for_date(chrono::NaiveDate::from_ymd_opt(2026, 8, 24).unwrap()).daily_minutes, 300, "当日维持原值");
    assert_eq!(cal.for_date(chrono::NaiveDate::from_ymd_opt(2026, 8, 25).unwrap()).daily_minutes, 400, "下一个工作日生效");
    assert_eq!(cal.for_date(chrono::NaiveDate::from_ymd_opt(2026, 8, 31).unwrap()).daily_minutes, 400, "以后每个工作日都是新值");
    assert!(SettingsService::is_workday_on(&conn, chrono::NaiveDate::from_ymd_opt(2026, 8, 24).unwrap()).unwrap(), "当日工作日属性不因保存改变");
}

#[test]
fn immediate_fields_take_effect_today() {
    // 测试情况：周一保存只改均分窗口（7→14），再保存只增删日期例外。
    // 正确结果：两次 save 都返回周一（今日即生效，无延时版本追加）；按日解析周一
    //           立即读到新均分窗口与例外，版本历史始终只有种子一条。
    let conn = db::open_in_memory().unwrap();
    let clock = fixed_clock(2026, 8, 24, 12, 0); // 周一
    let mut s = SettingsService::load(&conn).unwrap(); // FirstRun 落默认行
    s.smoothing_workdays = 14;
    assert_eq!(SettingsService::save(&conn, &clock, &s).unwrap(), chrono::NaiveDate::from_ymd_opt(2026, 8, 24).unwrap());
    assert_eq!(
        SettingsService::calendar(&conn).unwrap().for_date(chrono::NaiveDate::from_ymd_opt(2026, 8, 24).unwrap()).smoothing_workdays,
        14,
        "均分窗口立即生效"
    );
    s.date_overrides = vec![DateOverride { date: d(2026, 8, 29), working: true }];
    assert_eq!(SettingsService::save(&conn, &clock, &s).unwrap(), chrono::NaiveDate::from_ymd_opt(2026, 8, 24).unwrap());
    let versions: i64 = conn.query_row("SELECT COUNT(*) FROM settings_versions", [], |r| r.get(0)).unwrap();
    assert_eq!(versions, 1, "均分窗口与例外不追加延时版本");
    let cal = SettingsService::calendar(&conn).unwrap();
    assert!(
        cal.for_date(chrono::NaiveDate::from_ymd_opt(2026, 8, 29).unwrap()).is_workday_on(chrono::NaiveDate::from_ymd_opt(2026, 8, 29).unwrap()),
        "例外立即生效：周六被覆盖为工作日"
    );
}

#[test]
fn date_overrides_override_weekly_loop_both_ways() {
    // 测试情况：默认周一至五，双向例外——周六 08-29 标"这天工作"、周二 08-25 标
    //           "这天不工作"。
    // 正确结果：例外命中日按例外判定（周六 true、周二 false），未命中日维持周循环
    //           （周三 true、周日 false）；load 回读例外完整往返。
    let conn = db::open_in_memory().unwrap();
    let clock = fixed_clock(2026, 8, 24, 12, 0);
    let mut s = SettingsService::load(&conn).unwrap(); // FirstRun 落默认行
    s.date_overrides = vec![
        DateOverride { date: d(2026, 8, 29), working: true },  // 周六：调休补班
        DateOverride { date: d(2026, 8, 25), working: false }, // 周二：请假
    ];
    SettingsService::save(&conn, &clock, &s).unwrap();
    let wd = |y, m, d| SettingsService::is_workday_on(&conn, chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap()).unwrap();
    assert!(wd(2026, 8, 29), "例外：周六（原休息）→ 工作");
    assert!(!wd(2026, 8, 25), "例外：周二（原工作）→ 休息");
    assert!(wd(2026, 8, 26), "未命中：周三维持周循环工作日");
    assert!(!wd(2026, 8, 30), "未命中：周日维持周循环休息日");
    assert_eq!(SettingsService::load(&conn).unwrap().date_overrides.len(), 2, "load 回读例外");
}

#[test]
fn invalid_settings_rejected_and_untouched() {
    // 测试情况：依次提交三份非法设置——窗口起止相同（会把跨午夜判定变成
    //           "start 之后永远在窗"）、均分窗口 0、每日工作时间 0。
    // 正确结果：全部被拒（InvalidSettings），且库里设置保持默认值（校验先于变更）。
    let conn = db::open_in_memory().unwrap();
    let clock = fixed_clock(2026, 8, 24, 12, 0);
    let base = SettingsService::load(&conn).unwrap(); // FirstRun 落默认行
    let mut bad_window = base.clone();
    bad_window.time_windows = vec![TimeWindow { start_minute: 600, end_minute: 600 }];
    let mut bad_smoothing = base.clone();
    bad_smoothing.smoothing_workdays = 0;
    let mut bad_daily = base.clone();
    bad_daily.daily_minutes = 0;
    for (name, s) in [("窗口起止相同", &bad_window), ("均分窗口 0", &bad_smoothing), ("每日 0 分钟", &bad_daily)] {
        let err = SettingsService::save(&conn, &clock, s).unwrap_err();
        assert!(matches!(err, plan_helper_lib::domain::plans::PlanError::InvalidSettings { .. }), "{name} 应被拒");
    }
    assert_eq!(SettingsService::load(&conn).unwrap(), base, "被拒的保存不落库");
}

#[test]
fn date_override_must_be_in_the_future() {
    // 测试情况（工单 13 审查补验）：把日期例外标在"今天"或过去。
    // 正确结果：save 被拒（InvalidSettings，提示只能标注将来）——日期例外是
    //           "提前标注、将来生效"（story 46/48），不提供对当日与历史的追溯改写。
    let conn = db::open_in_memory().unwrap();
    let clock = fixed_clock(2026, 8, 24, 12, 0); // 周一
    let mut s = SettingsService::load(&conn).unwrap(); // FirstRun 落默认行
    s.date_overrides = vec![
        DateOverride { date: d(2026, 8, 24), working: false }, // 今天
        DateOverride { date: d(2026, 8, 23), working: true },  // 昨天
    ];
    let err = SettingsService::save(&conn, &clock, &s).unwrap_err();
    assert!(matches!(err, plan_helper_lib::domain::plans::PlanError::InvalidSettings { .. }));
}

#[test]
fn next_window_start_finds_first_upcoming_window() {
    // 测试情况：工作日周一至五、窗口 10:00–11:30 与 20:00–22:00，在一天的不同
    //           时刻、周末、以及例外覆盖下查询下一个窗口开始时刻。
    // 正确结果：窗前 → 当日 10:00；第一段窗内 → 当日 20:00；第二段窗内 → 次日
    //           10:00；周五晚间 → 跳过周末落周一；周一被例外标休 → 落周二；
    //           未配置窗口 → None（永不触发）。
    let conn = db::open_in_memory().unwrap();
    let windows = vec![
        TimeWindow { start_minute: 10 * 60, end_minute: 11 * 60 + 30 },
        TimeWindow { start_minute: 20 * 60, end_minute: 22 * 60 },
    ];
    force_settings(&conn, &Settings { time_windows: windows.clone(), ..Settings::default() });
    let next = |y, mo, d, h, mi| SettingsService::next_window_start(&conn, &fixed_clock(y, mo, d, h, mi)).unwrap().map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string());
    assert_eq!(next(2026, 8, 24, 9, 0).unwrap(), "2026-08-24 10:00:00", "窗前 → 当日第一段开始");
    assert_eq!(next(2026, 8, 24, 10, 30).unwrap(), "2026-08-24 20:00:00", "第一段窗内 → 当日第二段开始");
    assert_eq!(next(2026, 8, 24, 21, 0).unwrap(), "2026-08-25 10:00:00", "第二段窗内 → 次日第一段开始");
    assert_eq!(next(2026, 8, 28, 21, 0).unwrap(), "2026-08-31 10:00:00", "周五晚间 → 跳过周末落周一");
    force_settings(&conn, &Settings {
        time_windows: windows,
        date_overrides: vec![DateOverride { date: d(2026, 8, 31), working: false }],
        ..Settings::default()
    });
    assert_eq!(next(2026, 8, 28, 21, 0).unwrap(), "2026-09-01 10:00:00", "周一被例外标休 → 落周二");
    force_settings(&conn, &Settings { time_windows: vec![], ..Settings::default() });
    assert_eq!(next(2026, 8, 24, 9, 0), None, "未配置窗口 → 永不触发");
}

#[test]
fn is_work_time_uses_effective_config_per_day() {
    // 测试情况：默认窗口 09:00–18:00；周一 12:00 保存新窗口 08:00–09:00
    //           （延时字段 → 周二生效），分别在周一与周二判定 is_work_time。
    // 正确结果：周一 12:00 仍按旧窗口判 true（当日不受影响）；周二 12:00 按新
    //           窗口判 false、周二 08:30 按新窗口判 true。
    let conn = db::open_in_memory().unwrap();
    let clock = fixed_clock(2026, 8, 24, 12, 0);
    let mut s = SettingsService::load(&conn).unwrap(); // FirstRun 落默认行
    s.time_windows = vec![TimeWindow { start_minute: 8 * 60, end_minute: 9 * 60 }];
    SettingsService::save(&conn, &clock, &s).unwrap();
    assert!(SettingsService::is_work_time(&conn, &fixed_clock(2026, 8, 24, 12, 0)).unwrap(), "周一按旧窗口（09:00–18:00）判定");
    assert!(!SettingsService::is_work_time(&conn, &fixed_clock(2026, 8, 25, 12, 0)).unwrap(), "周二 12:00 按新窗口（08:00–09:00）判窗外");
    assert!(SettingsService::is_work_time(&conn, &fixed_clock(2026, 8, 25, 8, 30)).unwrap(), "周二 08:30 按新窗口判窗内");
}

#[test]
fn save_merges_overlapping_and_adjacent_windows() {
    // 测试情况（2026-08-30 用户反馈）：保存时提交重叠 / 首尾相接 / 跨午夜相接的窗口。
    // 正确结果：全部合并为一段（含跨午夜拆段重组与首尾穿午夜合并），按 start 升序；
    //           不相接的窗口原样保留。
    let conn = db::open_in_memory().unwrap();
    let clock = fixed_clock(2026, 8, 24, 12, 0);
    let save_windows = |windows: &[TimeWindow]| {
        let mut s = SettingsService::load(&conn).unwrap(); // FirstRun 落默认行
        s.time_windows = windows.to_vec();
        SettingsService::save(&conn, &clock, &s).unwrap();
        SettingsService::load(&conn).unwrap().time_windows
    };
    let tw = |a: u16, b: u16| TimeWindow { start_minute: a, end_minute: b };

    // 重叠：09:00–11:30 与 11:00–12:30 → 09:00–12:30
    assert_eq!(save_windows(&[tw(600, 690), tw(660, 750)]), vec![tw(600, 750)]);
    // 首尾相接：09:00–12:00 与 12:00–18:00 → 09:00–18:00
    assert_eq!(save_windows(&[tw(540, 720), tw(720, 1080)]), vec![tw(540, 1080)]);
    // 跨午夜 + 凌晨相接：20:00–01:00 与 01:00–05:00 → 20:00–05:00（跨午夜）
    assert_eq!(save_windows(&[tw(1200, 60), tw(60, 300)]), vec![tw(1200, 300)]);
    // 显式凌晨窗被跨午夜窗吸收：20:00–00:00 与 00:00–05:00 → 20:00–05:00
    assert_eq!(save_windows(&[tw(1200, 0), tw(0, 300)]), vec![tw(1200, 300)]);
    // 不相接：保持两段，按 start 升序
    assert_eq!(save_windows(&[tw(720, 780), tw(600, 660)]), vec![tw(600, 660), tw(720, 780)]);
}

#[test]
fn save_rejects_windows_covering_full_day() {
    // 测试情况（2026-08-30 用户决策）：多段合并后并集覆盖全天（09:00–22:00 与
    //           20:00–10:00 跨午夜相接拼成整圈）。
    // 正确结果：save 被拒（InvalidSettings"时间窗口并集不能覆盖全天"）——
    //           TimeWindow 无法表达 start==end 的全天窗口，报错而非改语义。
    let conn = db::open_in_memory().unwrap();
    let clock = fixed_clock(2026, 8, 24, 12, 0);
    let mut s = SettingsService::load(&conn).unwrap();
    s.time_windows = vec![
        TimeWindow { start_minute: 540, end_minute: 1320 },
        TimeWindow { start_minute: 1200, end_minute: 600 },
    ];
    let err = SettingsService::save(&conn, &clock, &s).unwrap_err();
    assert!(matches!(err, plan_helper_lib::domain::plans::PlanError::InvalidSettings { .. }));
    // 库里保持默认窗口（校验先于变更）
    assert_eq!(SettingsService::load(&conn).unwrap().time_windows, Settings::default().time_windows);
}
