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
use infra::db;
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = db::open(&dir.join("plan-helper.db"))?;
            // 重启时已有有效当前任务则直接亮出小看板（无则保持隐藏，等大面板确认分配）
            if ProgressService::board(&conn, &SystemClock)
                .map(|v| v.current.is_some())
                .unwrap_or(false)
            {
                if let Some(mini) = app.get_webview_window("mini-board") {
                    mini.show()?;
                }
            }
            app.manage(AppState {
                db: Mutex::new(conn),
                clock: Box::new(SystemClock),
            });
            position_pet(app)?;
            position_mini_board(app)?;
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
            commands::correct_total_progress
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 把桌宠摆到主显示器工作区右下角（后续拖拽/记忆位置由桌宠工单接管）。
fn position_pet(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let pet = app.get_webview_window("pet");
    let monitor = pet
        .as_ref()
        .and_then(|w| w.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    if let (Some(pet), Some(monitor)) = (pet, monitor) {
        let area = monitor.work_area();
        let margin = 40;
        let size = pet.outer_size()?;
        let pos = tauri::PhysicalPosition::new(
            area.position.x + area.size.width as i32 - size.width as i32 - margin,
            area.position.y + area.size.height as i32 - size.height as i32 - margin,
        );
        pet.set_position(pos)?;
    }
    Ok(())
}

/// 小看板摆在桌宠正上方（共享坐标系的初始位；拖拽跟随由工单 09 接管，
/// 模式显隐由工单 08 接管）。上下留 12px 间距，点击互不遮挡。
fn position_mini_board(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let (pet, mini) = (app.get_webview_window("pet"), app.get_webview_window("mini-board"));
    if let (Some(pet), Some(mini)) = (pet, mini) {
        let pet_pos = pet.outer_position()?;
        let size = mini.outer_size()?;
        mini.set_position(tauri::PhysicalPosition::new(
            pet_pos.x,
            pet_pos.y - size.height as i32 - 12,
        ))?;
    }
    Ok(())
}
