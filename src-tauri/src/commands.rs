//! Tauri command 层：薄代理，只做 State 解包与转发（spec 接缝决策）。
//! 业务逻辑全部在 domain，command 不做判断。

use tauri::State;

use crate::app_state::AppState;
use crate::domain::app_state::{app_state_view, AppStateView};

/// 示例接缝命令：UI → command → 领域服务 → 返回。
#[tauri::command]
pub fn get_app_state(state: State<'_, AppState>) -> Result<AppStateView, String> {
    let conn = state.db.lock().unwrap();
    app_state_view(&conn, state.clock.as_ref()).map_err(|e| e.to_string())
}
