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
}

/** 计划视图（对应 domain::plans::PlanView，含嵌套任务） */
export interface PlanView {
  id: number;
  name: string;
  summary: string;
  detail: string;
  priority: Priority;
  due_date: string | null;
  status: PlanStatus;
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
