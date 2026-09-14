//! 应用入口装配：模块声明、数据库初始化、状态托管、command 注册。
//! 领域（domain）与基建（infra）声明为 pub，供 `tests/` 集成测试直接驱动接缝。

pub mod app_state;
pub mod clock;
pub mod commands;
pub mod domain;
pub mod infra;

use app_state::AppState;
use clock::SystemClock;
use domain::progress::ProgressService;
use domain::settings::SettingsService;
use infra::db;
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};

/// 托盘退出 → 桌宠告别事件的通道名（与 src/lib/pet/menu.ts 的 TRAY_EXIT_EVENT 对应，
/// 跨端字符串各写一份、注释互指）
const TRAY_EXIT_EVENT: &str = "pet:tray-exit";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = db::open(&dir.join("plan-helper.db"))?;
            // 重启时亮出小看板的条件：有有效当前任务**且此刻是工作时间**（= 初始工作
            // 模式）。休息时段启动进休息模式、小看板保持隐藏（CONTEXT 休息即不工作），
            // 切回工作模式由前端按同一规则恢复显示（2026-08-29 用户反馈）
            let show_board = ProgressService::board(&conn, &SystemClock)
                .map(|v| v.current.is_some())
                .unwrap_or(false)
                && SettingsService::is_work_time(&conn, &SystemClock).unwrap_or(false);
            if show_board {
                if let Some(mini) = app.get_webview_window("mini-board") {
                    mini.show()?;
                }
            }
            app.manage(AppState {
                db: Mutex::new(conn),
                clock: Box::new(SystemClock),
            });
            position_pet_and_board(app)?;
            setup_tray(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_state,
            commands::create_plan,
            commands::list_plans,
            commands::get_plan,
            commands::update_plan,
            commands::delete_task,
            commands::start_plan,
            commands::pause_plan,
            commands::resume_plan,
            commands::complete_plan,
            commands::abort_plan,
            commands::copy_plan_as_new,
            commands::get_allocation_board,
            commands::commit_today_allocation,
            commands::get_mini_board,
            commands::set_current_task,
            commands::complete_subgoal,
            commands::undo_subgoal,
            commands::report_percent,
            commands::correct_total_progress,
            commands::should_auto_open_main_board,
            commands::is_work_time,
            commands::save_settings,
            commands::get_next_window_start,
            commands::get_daily_summary,
            commands::get_daily_summary_status,
            commands::mark_daily_summary_shown,
            commands::exit_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 把「桌宠 + 小看板」组合体锚到主显示器工作区右下角：小看板在下方、右对齐 40px
/// 边距，桌宠居其正上方（水平居中、12px 间距），两者一起完整落在屏幕内
/// （2026-08-24 验收要求：小看板在桌宠下方且不出屏）。拖拽跟随/记忆位置由工单 09
/// 接管，模式显隐由工单 08 接管。
fn position_pet_and_board(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let (pet, mini) = (
        app.get_webview_window("pet"),
        app.get_webview_window("mini-board"),
    );
    let monitor = pet
        .as_ref()
        .and_then(|w| w.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    if let (Some(pet), Some(mini), Some(monitor)) = (pet, mini, monitor) {
        let area = monitor.work_area();
        let margin = 40;
        let gap = 12;
        let pet_size = pet.outer_size()?;
        let mini_size = mini.outer_size()?;
        let mini_x = area.position.x + area.size.width as i32 - mini_size.width as i32 - margin;
        let mini_y = area.position.y + area.size.height as i32 - mini_size.height as i32 - margin;
        mini.set_position(tauri::PhysicalPosition::new(mini_x, mini_y))?;
        pet.set_position(tauri::PhysicalPosition::new(
            mini_x + (mini_size.width as i32 - pet_size.width as i32) / 2,
            mini_y - gap - pet_size.height as i32,
        ))?;
    }
    Ok(())
}

/// 系统托盘（工单 14，SystemTray）：左键单击开控制面板；右键菜单「打开控制面板」「退出」。
/// 退出不在这里退进程——只向桌宠窗口发告别事件，桌宠窗口淡出后（工单 17）由前端
/// 走 exit_app 统一退出，与桌宠菜单「再见」共用同一通道（两条退出路径等价性由此保证）。
fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "打开控制面板", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let mut tray = TrayIconBuilder::with_id("tray")
        .tooltip("任务帮手")
        .menu(&menu)
        .show_menu_on_left_click(false) // 左键让给「打开控制面板」，菜单只归右键
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => reveal_control_panel(app),
            "quit" => request_exit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // 左键抬起才算单击（按住拖出菜单区域不误触）
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                reveal_control_panel(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// 控制面板亮到前台。unminimize → show → set_focus 与前端 revealWindow（src/lib/windows.ts）
/// 同序：Windows 上 show 对最小化窗口无效，必须先 unminimize（2026-08-30 弹窗失效教训）。
fn reveal_control_panel(app: &tauri::AppHandle) {
    if let Some(panel) = app.get_webview_window("control-panel") {
        let _ = panel.unminimize();
        let _ = panel.show();
        let _ = panel.set_focus();
    }
}

/// 托盘退出：向桌宠窗口发告别事件（桌宠窗口淡出后由前端调 exit_app 关闭全部窗口并退进程）
fn request_exit(app: &tauri::AppHandle) {
    let _ = app.emit_to("pet", TRAY_EXIT_EVENT, ());
}
