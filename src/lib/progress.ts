/**
 * 前端进度派生工具（ADR-0002 耗时完成度）：与服务层 PlanService::task_progress 同公式。
 * 有子目标任务直接从子目标视图求和；无子目标任务的汇报路径在工单 07 接通前不显示进度。
 */
import type { TaskView } from "./api";

/** 任务的（已完成分钟, 总分钟）。无子目标任务当前恒 (0, 0)——不展示。 */
export function taskProgress(task: TaskView): { completed: number; total: number } {
  if (!task.has_subgoals) return { completed: 0, total: 0 };
  return {
    completed: task.subgoals.filter((s) => s.completed).reduce((sum, s) => sum + s.estimated_minutes, 0),
    total: task.subgoals.reduce((sum, s) => sum + s.estimated_minutes, 0),
  };
}

/** 进度百分比字符串（如 "60%"）；总量为 0（或无子目标）返回空串表示"不展示"。 */
export function taskProgressLabel(task: TaskView): string {
  const { completed, total } = taskProgress(task);
  if (total === 0) return "";
  return `${Math.round((completed / total) * 100)}%`;
}
