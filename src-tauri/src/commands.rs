//! Tauri command 层：薄代理，只做 State 解包与转发（spec 接缝决策）。
//! 业务逻辑全部在 domain，command 不做判断。

use tauri::State;

use crate::app_state::AppState;
use crate::domain::allocation::{AllocationBoardView, AllocationService};
use crate::domain::app_state::{app_state_view, AppStateView};
use crate::domain::lifecycle::LifecycleService;
use crate::domain::plans::{PlanDraft, PlanError, PlanService, PlanView};
use crate::domain::progress::{MiniBoardView, ProgressService};

/// 示例接缝命令：UI → command → 领域服务 → 返回。
#[tauri::command]
pub fn get_app_state(state: State<'_, AppState>) -> Result<AppStateView, String> {
    let conn = state.db.lock().unwrap();
    app_state_view(&conn, state.clock.as_ref()).map_err(|e| e.to_string())
}

/// 创建计划（含任务），返回新计划 id；校验失败以 PlanError 结构化返回。
#[tauri::command]
pub fn create_plan(state: State<'_, AppState>, new: PlanDraft) -> Result<i64, PlanError> {
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

/* ---- 进度汇报（工单 07）：薄代理，账本与派生在 domain::progress ---- */

/// 小看板视图：当前任务 + 今日完成量 + 当日目标 + 更换候选。
#[tauri::command]
pub fn get_mini_board(state: State<'_, AppState>) -> Result<MiniBoardView, PlanError> {
    let conn = state.db.lock().unwrap();
    ProgressService::board(&conn, state.clock.as_ref())
}

/// 指定当前任务（须在今日推进列表内且可推进）。
#[tauri::command]
pub fn set_current_task(state: State<'_, AppState>, task_id: i64) -> Result<(), PlanError> {
    let conn = state.db.lock().unwrap();
    ProgressService::set_current_task(&conn, state.clock.as_ref(), task_id)
}

/// 按序完成一个子目标（乱序拒绝；到 100% 自动完成）。
#[tauri::command]
pub fn complete_subgoal(state: State<'_, AppState>, subgoal_id: i64) -> Result<(), PlanError> {
    let conn = state.db.lock().unwrap();
    ProgressService::complete_subgoal(&conn, state.clock.as_ref(), subgoal_id).map(|_| ())
}

/// 撤销最后一个已完成的子目标（补偿账，进度实时重算）。
#[tauri::command]
pub fn undo_subgoal(state: State<'_, AppState>, subgoal_id: i64) -> Result<(), PlanError> {
    let conn = state.db.lock().unwrap();
    ProgressService::undo_subgoal(&conn, state.clock.as_ref(), subgoal_id)
}

/// 无子目标任务增量汇报 +percent%（任意正数，最小 0.1%、最多一位小数，累计不超 100%）。
#[tauri::command]
pub fn report_percent(
    state: State<'_, AppState>,
    task_id: i64,
    percent: f64,
) -> Result<(), PlanError> {
    let conn = state.db.lock().unwrap();
    ProgressService::report_percent(&conn, state.clock.as_ref(), task_id, percent).map(|_| ())
}

/// 修正总进度（直接设定当前值，差额以事件落账；仅无子目标任务）。
#[tauri::command]
pub fn correct_total_progress(
    state: State<'_, AppState>,
    task_id: i64,
    percent: f64,
) -> Result<(), PlanError> {
    let conn = state.db.lock().unwrap();
    ProgressService::correct_total(&conn, state.clock.as_ref(), task_id, percent).map(|_| ())
}

/* ---- 桌宠（工单 08）：薄代理，窗口/流程接线，无领域逻辑 ---- */

/// AutoOpenMainBoard 判定（06 预留的触发接线：启动序列完成后 / 手动切入工作模式时检测）。
#[tauri::command]
pub fn should_auto_open_main_board(
    state: State<'_, AppState>,
    work_mode: bool,
) -> Result<bool, PlanError> {
    let conn = state.db.lock().unwrap();
    AllocationService::should_auto_open(&conn, state.clock.as_ref(), work_mode)
}

/// 再见：跳箱动画播完后退出整个应用（关闭全部窗口；托盘退出（工单 14）走同一通道）。
#[tauri::command]
pub fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}
