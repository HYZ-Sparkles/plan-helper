//! 接缝集成测试（spec Testing Decisions：全部自动化测试打在 Rust 领域服务层）。
//! 直接以 SQLite + 注入时钟驱动领域服务，不经过 Tauri command / 窗口。

use chrono::{Local, TimeZone};

use plan_helper_lib::clock::FixedClock;
use plan_helper_lib::domain::app_state::app_state_view;
use plan_helper_lib::domain::settings::{Settings, SettingsService, TimeWindow};
use plan_helper_lib::infra::db;

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
    };
    SettingsService::save(&conn, &clock, &custom).unwrap();
    assert_eq!(SettingsService::load(&conn).unwrap(), custom);
    assert_eq!(SettingsService::updated_at(&conn).unwrap(), Some(clock.0));
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
    SettingsService::load(&conn).unwrap();
    SettingsService::save(
        &conn,
        &fixed_clock(2026, 8, 20, 12, 0),
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
    )
    .unwrap();

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
