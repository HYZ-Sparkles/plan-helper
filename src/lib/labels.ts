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
  const payload = (err.payload ?? {}) as {
    index?: number;
    task_index?: number | null;
    subgoal_index?: number;
  };
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
    case "SubGoalInvalid":
      return `${taskNo(payload.task_index ?? 0)}的第 ${(payload.subgoal_index ?? 0) + 1} 行子目标缺少内容或预计耗时`;
    case "SubGoalLocked":
      return `${taskNo(payload.task_index ?? 0)}的已完成子目标锁定，不可修改或删除`;
    case "DependencyInvalid":
      return `${taskNo(payload.task_index ?? 0)}的前置任务引用无效`;
    case "DependencyCrossPlan":
      return "前置任务只能选择同一计划内的任务";
    case "DependencyCycle":
      return payload.task_index == null
        ? "前置任务形成循环依赖"
        : `${taskNo(payload.task_index)}的前置任务形成循环依赖`;
    case "PriorityLocked":
      return "计划开始后优先级不可调整";
    case "TaskLocked":
      return `${taskNo(payload.index ?? 0)}已完成，字段锁定不可修改`;
    case "TaskSetMismatch":
      return "任务集与库中不一致——删除任务请用任务卡上的删除按钮";
    case "NotFound":
      return "目标计划或任务不存在";
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
