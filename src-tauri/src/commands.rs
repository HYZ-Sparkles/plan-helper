//! Tauri command 层：薄代理，只做 State 解包与转发（spec 接缝决策）。
//! 业务逻辑全部在 domain，command 不做判断。

use tauri::State;

use crate::app_state::AppState;
use crate::domain::app_state::{app_state_view, AppStateView};
use crate::domain::plans::{NewPlan, PlanError, PlanService, PlanView};

/// 示例接缝命令：UI → command → 领域服务 → 返回。
#[tauri::command]
pub fn get_app_state(state: State<'_, AppState>) -> Result<AppStateView, String> {
    let conn = state.db.lock().unwrap();
    app_state_view(&conn, state.clock.as_ref()).map_err(|e| e.to_string())
}

/// 创建计划（含任务），返回新计划 id；校验失败以 PlanError 结构化返回。
#[tauri::command]
pub fn create_plan(
    state: State<'_, AppState>,
    new: NewPlan,
) -> Result<i64, PlanError> {
    let conn = state.db.lock().unwrap();
    PlanService::create(&conn, state.clock.as_ref(), &new)
}

/// 计划列表（PlanOrdering 默认排序，含嵌套任务）。
#[tauri::command]
pub fn list_plans(state: State<'_, AppState>) -> Result<Vec<PlanView>, PlanError> {
    let conn = state.db.lock().unwrap();
    PlanService::list(&conn)
}
