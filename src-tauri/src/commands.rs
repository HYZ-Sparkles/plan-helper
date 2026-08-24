//! Tauri command 层：薄代理，只做 State 解包与转发（spec 接缝决策）。
//! 业务逻辑全部在 domain，command 不做判断。

use tauri::State;

use crate::app_state::AppState;
use crate::domain::allocation::{AllocationBoardView, AllocationService};
use crate::domain::app_state::{app_state_view, AppStateView};
use crate::domain::lifecycle::LifecycleService;
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

/* ---- 计划生命周期（工单 05）：薄代理，状态机在 domain::lifecycle ---- */

/// 未开始 → 进行中。
#[tauri::command]
pub fn start_plan(state: State<'_, AppState>, plan_id: i64) -> Result<(), PlanError> {
    let conn = state.db.lock().unwrap();
    LifecycleService::start(&conn, plan_id)
}

/// 进行中 → 已暂停（手动，原因记"用户主动"）。
#[tauri::command]
pub fn pause_plan(state: State<'_, AppState>, plan_id: i64) -> Result<(), PlanError> {
    let conn = state.db.lock().unwrap();
    LifecycleService::pause(&conn, plan_id)
}

/// 已暂停 → 进行中。
#[tauri::command]
pub fn resume_plan(state: State<'_, AppState>, plan_id: i64) -> Result<(), PlanError> {
    let conn = state.db.lock().unwrap();
    LifecycleService::resume(&conn, plan_id)
}

/// 进行中 → 已完成（手动确认；全部任务已完成为前置，服务层校验）。
#[tauri::command]
pub fn complete_plan(state: State<'_, AppState>, plan_id: i64) -> Result<(), PlanError> {
    let conn = state.db.lock().unwrap();
    LifecycleService::complete(&conn, plan_id)
}

/// 进行中/已暂停 → 已放弃（二级确认在 UI）。
#[tauri::command]
pub fn abort_plan(state: State<'_, AppState>, plan_id: i64) -> Result<(), PlanError> {
    let conn = state.db.lock().unwrap();
    LifecycleService::abort(&conn, plan_id)
}

/// 终态计划复制并新建（进度归零、名称加"- 副本"、直接进行中），返回新计划 id。
#[tauri::command]
pub fn copy_plan_as_new(state: State<'_, AppState>, plan_id: i64) -> Result<i64, PlanError> {
    let conn = state.db.lock().unwrap();
    LifecycleService::copy_as_new(&conn, state.clock.as_ref(), plan_id)
}

/* ---- 今日分配（工单 06）：薄代理，装配与校验在 domain::allocation ---- */

/// 大面板视图：候选分组（依赖过滤）+ 今日回显 + 当日目标。
#[tauri::command]
pub fn get_allocation_board(state: State<'_, AppState>) -> Result<AllocationBoardView, PlanError> {
    let conn = state.db.lock().unwrap();
    AllocationService::board(&conn, state.clock.as_ref())
}

/// 提交当日分配（覆盖重选）；选中集必须全部当前可选，服务层兜底校验。
#[tauri::command]
pub fn commit_today_allocation(
    state: State<'_, AppState>,
    task_ids: Vec<i64>,
) -> Result<(), PlanError> {
    let conn = state.db.lock().unwrap();
    AllocationService::commit(&conn, state.clock.as_ref(), &task_ids)
}
