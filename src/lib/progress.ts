/**
 * 前端进度派生工具（ADR-0002 耗时完成度）：百分比自工单 07 起由服务层统一派生
 * （有子目标 = 已完成子目标耗时占比；无子目标 = ProgressLog 增量占比），
 * 前端只做展示换算，不再各自求和——单一事实源在服务端。
 */
import type { TaskView } from "./api";

/** 任务的（已完成分钟, 总分钟）。分钟由派生百分比 × 总耗时换算（展示口径）。 */
export function taskProgress(task: TaskView): { completed: number; total: number } {
  const total = task.estimated_minutes ?? 0;
  return { completed: (task.progress_percent / 100) * total, total };
}

/** 进度百分比字符串（如 "60%"）；总量为 0 返回空串表示"不展示"（防御，正常数据不会出现）。 */
export function taskProgressLabel(task: TaskView): string {
  if ((task.estimated_minutes ?? 0) === 0) return "";
  return `${Math.round(task.progress_percent)}%`;
}
