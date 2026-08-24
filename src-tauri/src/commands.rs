//! Tauri command 层：薄代理，只做 State 解包与转发（spec 接缝决策）。
//! 业务逻辑全部在 domain，command 不做判断。

use tauri::State;

use crate::app_state::AppState;
use crate::domain::app_state::{app_state_view, AppStateView};
use crate::domain::plans::{PlanDraft, PlanError, PlanService, PlanView};

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
    new: PlanDraft,
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

/// 单个计划详情（工单 03：详情页读取）。
#[tauri::command]
pub fn get_plan(state: State<'_, AppState>, plan_id: i64) -> Result<PlanView, PlanError> {
    let conn = state.db.lock().unwrap();
    PlanService::get(&conn, plan_id)
}

/// 整计划编辑（CreationUI 编辑态保存）。
#[tauri::command]
pub fn update_plan(
    state: State<'_, AppState>,
    plan_id: i64,
    draft: PlanDraft,
) -> Result<(), PlanError> {
    let conn = state.db.lock().unwrap();
    PlanService::update(&conn, state.clock.as_ref(), plan_id, &draft)
}

/// 软删除任务进归档（确认弹窗在 UI，这里是权威删除通道）。
#[tauri::command]
pub fn delete_task(state: State<'_, AppState>, task_id: i64) -> Result<(), PlanError> {
    let conn = state.db.lock().unwrap();
    PlanService::delete_task(&conn, state.clock.as_ref(), task_id)
}
