/**
 * 领域枚举的中文显示与错误文案：UI 展示统一从这取，不散落组件。
 */
import type { PauseReason, PlanStatus, Priority, TaskStatus } from "./api";

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

/** 暂停原因中文（CONTEXT PauseReason 二值） */
export const pauseReasonLabel: Record<PauseReason, string> = {
  UserInitiated: "用户主动",
  AutoPreempted: "自动抢占",
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
    case "PlanStatusInvalid":
      return "当前计划状态不允许此操作";
    case "PlanNotTerminal":
      return "只有已完成或已放弃的计划可以复制并新建";
    case "TasksNotCompleted":
      return "还有未完成的任务——全部完成后才能确认完成计划";
    case "TaskLocked":
      return `${taskNo(payload.index ?? 0)}已完成，字段锁定不可修改`;
    case "TaskSetMismatch":
      return "任务集与库中不一致——删除任务请用任务卡上的删除按钮";
    case "NotFound":
      return "目标计划或任务不存在";
    case "TaskNotAllocatable":
      return "选中集中有不可分配的任务（前置未完成、已完成或计划不在进行中）";
    case "TaskNotInToday":
      return "该任务不在今日推进列表里，请先在大面板选中它";
    case "SubGoalOutOfOrder":
      return "子目标需按顺序推进，撤销也只能从最后一个已完成项开始";
    case "SubGoalNotCompleted":
      return "该子目标还没有完成，无需撤销";
    case "ProgressLocked":
      return "任务已完成，进度锁定不可再变更";
    case "PercentInvalid":
      return "百分比必须是正数且不超过 100（汇报至少 0.1%，修正可归零；最多一位小数）";
    case "PercentOverflow":
      return "累计汇报会超过 100%，请调小本次增量";
    case "NotPercentTask":
      return "有子目标的任务按子目标推进与撤销，不走百分比";
    case "Storage":
      return `存储异常：${err.payload}`;
    default:
      return "未知错误";
  }
}

/** 分钟 → 小时展示（一位小数 round，与 ProgressGranularity 0.1% 颗粒度对齐）：
 *  `Math.round((minutes/60)*10)/10` 解决 IEEE 754 浮点尾巴暴露（例 258.6/60 = 4.3099999...）
 *  被 toString 直接吐成 "4.309999999999999" / 10h；整数小时经 round 后 .toString() 保持自然位数
 *  （120 → "2"、90 → "1.5"），不强制追加 ".0"。 */
export function hoursFromMinutes(minutes: number): string {
  return (Math.round((minutes / 60) * 10) / 10).toString();
}

/** 分钟 → 固定一位小数的小时文案（X.X 口径，如 90 → "1.5"、180 → "3.0"）；
 *  大面板状态条累计/差额与工单 10 的结转标注（"基准 5.0h"）复用 */
export function hoursLabel(minutes: number): string {
  return (minutes / 60).toFixed(1);
}
