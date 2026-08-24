//! 应用入口装配：模块声明、数据库初始化、状态托管、command 注册。
//! 领域（domain）与基建（infra）声明为 pub，供 `tests/` 集成测试直接驱动接缝。

pub mod app_state;
pub mod clock;
pub mod commands;
pub mod domain;
pub mod infra;

use app_state::AppState;
use clock::SystemClock;
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
            app.manage(AppState {
                db: Mutex::new(conn),
                clock: Box::new(SystemClock),
            });
            position_pet(app)?;
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
            commands::copy_plan_as_new
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
