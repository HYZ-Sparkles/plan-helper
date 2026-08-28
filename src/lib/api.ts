/**
 * 后端 command 的类型化封装：类型与 src-tauri 领域层的 serde 结构一一对应。
 * 新 command 先在这里登记类型与包装函数，UI 组件不直接 invoke。
 */
import { invoke } from "@tauri-apps/api/core";

/** 日内一段时间窗口（分钟数端点；跨午夜以 end < start 表达） */
export interface TimeWindow {
  start_minute: number;
  end_minute: number;
}

/** 全局设置（对应 domain::settings::Settings） */
export interface Settings {
  daily_minutes: number;
  workdays: number[]; // 周一=1 .. 周日=7
  time_windows: TimeWindow[];
  smoothing_workdays: number;
}

/** 启动快照（对应 domain::app_state::AppStateView） */
export interface AppStateView {
  settings: Settings;
  server_now: string;
}

/** 示例接缝命令：读启动快照（设置 + 服务端时间） */
export function getAppState(): Promise<AppStateView> {
  return invoke<AppStateView>("get_app_state");
}

/* ---- 桌宠（工单 08）---- */

/** AutoOpenMainBoard 判定（06 预留、08 接线：启动序列完成后 / 手动切入工作模式时检测） */
export function shouldAutoOpenMainBoard(workMode: boolean): Promise<boolean> {
  return invoke<boolean>("should_auto_open_main_board", { workMode });
}

/** 再见：跳箱动画播完后退出整个应用（关闭全部窗口；托盘退出（工单 14）走同一通道） */
export function exitApp(): Promise<void> {
  return invoke("exit_app");
}

/* ---- 计划领域（工单 02+）---- */

/** 优先级枚举（CONTEXT「优先级」） */
export type Priority = "Low" | "Medium" | "High";

/** 计划状态（ADR-0001 单向瀑布） */
export type PlanStatus = "NotStarted" | "Active" | "Paused" | "Completed" | "Abandoned";

/** 任务状态 */
export type TaskStatus = "NotStarted" | "Active" | "Completed";

/** 子目标草稿（对应 domain::plans::SubGoalDraft；id 缺省 = 新子目标） */
export interface SubGoalDraft {
  id?: number;
  name: string;
  estimated_minutes: number;
}

/** 任务草稿（对应 domain::plans::TaskDraft；id 缺省 = 新任务） */
export interface TaskDraft {
  id?: number;
  name: string;
  summary?: string;
  detail?: string;
  has_subgoals?: boolean;
  estimated_minutes?: number;
  /** 子目标行（has_subgoals=false 时被服务层忽略） */
  subgoals?: SubGoalDraft[];
  /** 前置任务在 tasks 数组中的下标（草稿内相对引用，创建/编辑同一语义） */
  depends_on?: number[];
}

/** 计划草稿（对应 domain::plans::PlanDraft）——创建与编辑共用（CreationUI 双模式） */
export interface PlanDraft {
  name: string;
  summary?: string;
  detail?: string;
  priority: Priority;
  due_date?: string | null;
  tasks: TaskDraft[];
}

/** 子目标视图（对应 domain::plans::SubGoalView） */
export interface SubGoalView {
  id: number;
  name: string;
  estimated_minutes: number;
  position: number;
  completed: boolean;
}

/** 任务视图（对应 domain::plans::TaskView） */
export interface TaskView {
  id: number;
  name: string;
  summary: string;
  detail: string;
  has_subgoals: boolean;
  estimated_minutes: number | null;
  position: number;
  status: TaskStatus;
  subgoals: SubGoalView[];
  /** 前置任务 id（同计划内） */
  prerequisite_ids: number[];
  /** 派生进度百分比（0–100）：无子目标任务的汇报进度也在这里（工单 07 接线） */
  progress_percent: number;
}

/** 暂停原因（CONTEXT PauseReason 二值；AutoPreempted 由工单 12 抢占写入） */
export type PauseReason = "UserInitiated" | "AutoPreempted";

/** 计划视图（对应 domain::plans::PlanView，含嵌套任务） */
export interface PlanView {
  id: number;
  name: string;
  summary: string;
  detail: string;
  priority: Priority;
  due_date: string | null;
  status: PlanStatus;
  pause_reason: PauseReason | null;
  created_at: string;
  tasks: TaskView[];
}

/** 领域错误（对应 domain::plans::PlanError 的序列化形态 {kind, payload}） */
export interface PlanErrorShape {
  kind: string;
  payload?: unknown;
}

/** 创建计划（含任务），返回新计划 id */
export function createPlan(plan: PlanDraft): Promise<number> {
  return invoke<number>("create_plan", { new: plan });
}

/** 计划列表（PlanOrdering 默认排序，含嵌套任务） */
export function listPlans(): Promise<PlanView[]> {
  return invoke<PlanView[]>("list_plans");
}

/** 单个计划详情（含任务，按 position 排序） */
export function getPlan(planId: number): Promise<PlanView> {
  return invoke<PlanView>("get_plan", { planId });
}

/** 整计划编辑（CreationUI 编辑态保存） */
export function updatePlan(planId: number, draft: PlanDraft): Promise<void> {
  return invoke<void>("update_plan", { planId, draft });
}

/** 软删除任务进归档（确认弹窗在 UI，这里是权威删除通道） */
export function deleteTask(taskId: number): Promise<void> {
  return invoke<void>("delete_task", { taskId });
}

/* ---- 计划生命周期（工单 05）：状态机在服务层，UI 只按状态展示可得操作 ---- */

/** 未开始 → 进行中 */
export function startPlan(planId: number): Promise<void> {
  return invoke<void>("start_plan", { planId });
}

/** 进行中 → 已暂停（手动，原因记"用户主动"） */
export function pausePlan(planId: number): Promise<void> {
  return invoke<void>("pause_plan", { planId });
}

/** 已暂停 → 进行中 */
export function resumePlan(planId: number): Promise<void> {
  return invoke<void>("resume_plan", { planId });
}

/** 进行中 → 已完成（手动确认；服务层校验全部任务已完成） */
export function completePlan(planId: number): Promise<void> {
  return invoke<void>("complete_plan", { planId });
}

/** 进行中/已暂停 → 已放弃（二级确认在 UI） */
export function abortPlan(planId: number): Promise<void> {
  return invoke<void>("abort_plan", { planId });
}

/** 终态计划复制并新建（进度归零、"- 副本"、直接进行中），返回新计划 id */
export function copyPlanAsNew(planId: number): Promise<number> {
  return invoke<number>("copy_plan_as_new", { planId });
}

/* ---- 今日分配（工单 06）：装配与校验在 domain::allocation ---- */

/** 分组内一条候选任务（对应 domain::allocation::AllocationTask；
 *  被依赖阻塞的任务不进列表——只展示可选任务） */
export interface AllocationTask {
  id: number;
  name: string;
  /** 预计耗时（分钟）：无子目标 = 手填值；有子目标 = 子目标求和 */
  estimated_minutes: number;
}

/** 一个进行中计划的分组（对应 domain::allocation::AllocationGroup） */
export interface AllocationGroup {
  plan_id: number;
  plan_name: string;
  priority: Priority;
  tasks: AllocationTask[];
}

/** 大面板一次装配的完整视图（对应 domain::allocation::AllocationBoardView） */
export interface AllocationBoardView {
  /** 分配归属的工作日（YYYY-MM-DD） */
  date: string;
  /** 今日是否在设置的每周工作日里（false = 休息日态，分配不加载） */
  workday: boolean;
  /** 当日实际目标（分钟）。工单 06 = 基准工作时间；工单 10 升级为含结转 */
  target_minutes: number;
  /** 今日已分配的选中集（重开重选回显，已与当前可选集求交） */
  selected_task_ids: number[];
  /** 候选分组：所有进行中计划（PlanOrdering 排序） */
  groups: AllocationGroup[];
}

/** 大面板视图：候选分组（依赖过滤）+ 今日回显 + 当日目标 */
export function getAllocationBoard(): Promise<AllocationBoardView> {
  return invoke<AllocationBoardView>("get_allocation_board");
}

/** 提交当日分配（覆盖重选）；不可选任务由服务层兜底拒绝 */
export function commitTodayAllocation(taskIds: number[]): Promise<void> {
  return invoke<void>("commit_today_allocation", { taskIds });
}

/* ---- 进度汇报（工单 07）：账本与派生在 domain::progress ---- */

/** 当前任务的展示视图（对应 domain::progress::CurrentTaskView；
 *  status = Completed 是"任务完成"停留态，不自动切换下一个） */
export interface CurrentTaskView {
  task_id: number;
  plan_id: number;
  plan_name: string;
  task_name: string;
  status: TaskStatus;
  /** 是否有子目标（决定小看板走勾选还是百分比控件） */
  has_subgoals: boolean;
  /** 派生进度百分比（0–100，ADR-0002） */
  percent: number;
  completed_minutes: number;
  total_minutes: number;
  /** 全部子目标（按填写顺序；有子目标任务才有内容） */
  subgoals: SubGoalView[];
}

/** 更换任务候选的一个计划分组（对应 domain::progress::PickerGroup；
 *  服务端已把当前任务同计划排最前） */
export interface PickerGroup {
  plan_id: number;
  plan_name: string;
  priority: Priority;
  tasks: { id: number; name: string }[];
}

/** 小看板一次装配的完整视图（对应 domain::progress::MiniBoardView） */
export interface MiniBoardView {
  /** null = 空态：未指定当前任务 / 计划暂停 / 已移出今日列表 */
  current: CurrentTaskView | null;
  /** 今日完成量（分钟，按工作窗口开始日归属） */
  today_minutes: number;
  /** 当日实际目标（分钟）。工单 07 = 基准；工单 10 升级为含结转 */
  target_minutes: number;
  pickers: PickerGroup[];
}

/** 小看板视图：当前任务 + 今日完成量 + 当日目标 + 更换候选 */
export function getMiniBoard(): Promise<MiniBoardView> {
  return invoke<MiniBoardView>("get_mini_board");
}

/** 指定当前任务（须在今日推进列表内且可推进，服务层兜底校验） */
export function setCurrentTask(taskId: number): Promise<void> {
  return invoke<void>("set_current_task", { taskId });
}

/** 按序完成一个子目标（乱序由服务层拒绝） */
export function completeSubgoal(subgoalId: number): Promise<void> {
  return invoke<void>("complete_subgoal", { subgoalId });
}

/** 撤销最后一个已完成的子目标（补偿账，进度实时重算） */
export function undoSubgoal(subgoalId: number): Promise<void> {
  return invoke<void>("undo_subgoal", { subgoalId });
}

/** 无子目标任务增量汇报 +percent%（任意正数：最小 0.1%、最多一位小数，累计不超 100%；
 *  2026-08-24 验收修订，原"5% 倍数"颗粒度砍掉） */
export function reportPercent(taskId: number, percent: number): Promise<void> {
  return invoke<void>("report_percent", { taskId, percent });
}

/** 修正总进度（直接设定当前值 0–100、最多一位小数；仅无子目标任务） */
export function correctTotalProgress(taskId: number, percent: number): Promise<void> {
  return invoke<void>("correct_total_progress", { taskId, percent });
}
