/**
 * 领域枚举的中文显示与错误文案：UI 展示统一从这取，不散落组件。
 */
import type { PlanStatus, Priority, TaskStatus } from "./api";

/** 优先级文字标签（CONTEXT PriorityVisuals：文字标签 + 图标，图标在 PriorityLabel 组件） */
export const priorityLabel: Record<Priority, string> = {
  High: "高",
  Medium: "中",
  Low: "低",
};

/** 计划/任务状态中文（两枚举的共有值标签一致，合一张表） */
export const statusLabel: Record<PlanStatus | TaskStatus, string> = {
  NotStarted: "未开始",
  Active: "进行中",
  Paused: "已暂停",
  Completed: "已完成",
  Abandoned: "已放弃",
};

/**
 * 后端 PlanError（{kind, payload} 形态）转用户可读文案。
 * payload.index / payload.task_index 为任务序号（从 0 起），展示为第几个任务。
 */
export function planErrorMessage(err: { kind?: string; payload?: unknown }): string {
  const payload = (err.payload ?? {}) as { index?: number; task_index?: number | null };
  const taskNo = (i: number) => `第 ${i + 1} 个任务`;
  switch (err.kind) {
    case "EmptyName":
      return payload.task_index == null ? "计划名称不能为空" : `${taskNo(payload.task_index)}名称不能为空`;
    case "NoTasks":
      return "计划至少需要 1 个任务";
    case "TaskNeedsDuration":
      return `${taskNo(payload.index ?? 0)}缺少预计耗时（无子目标任务必填）`;
    case "SubGoalsRequired":
      return `${taskNo(payload.index ?? 0)}勾选了子目标，但还没有录入子目标`;
    case "Storage":
      return `存储异常：${err.payload}`;
    default:
      return "未知错误";
  }
}

/** 分钟 → 展示小时（如 90 → "1.5"）；供预计耗时展示复用 */
export function hoursFromMinutes(minutes: number): string {
  return (minutes / 60).toString();
}
