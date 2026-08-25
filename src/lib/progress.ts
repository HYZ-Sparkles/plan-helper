/**
 * 前端进度派生工具（ADR-0002 耗时完成度）：百分比自工单 07 起由服务层统一派生
 * （有子目标 = 已完成子目标耗时占比；无子目标 = ProgressLog 增量占比），
 * 前端只做展示换算，不再各自求和——单一事实源在服务端。
 */
import type { TaskView } from "./api";

/** 任务的（已完成分钟, 总分钟）。已完成分钟按后端 `TaskProgress::percent()` 一位小数 round
 *  同步口径——二次派生 `percent/100 * total` 会再引入 IEEE 754 浮点尾巴（如 43.1 × 600 /100
 *  = 258.5999...），不 round 会让上层 `hoursFromMinutes` 显示 "4.3099999..."；总耗时来自用户
 *  输入整数 ×60 通常干净，但统一 round 兜底。 */
export function taskProgress(task: TaskView): { completed: number; total: number } {
  const total = task.estimated_minutes ?? 0;
  const completed = Math.round(((task.progress_percent / 100) * total) * 10) / 10;
  return { completed, total };
}

/** 进度百分比字符串（如 "60.3%"、"60%"）；总量为 0 返回空串表示"不展示"（防御，正常数据不会出现）。
 *  后端 `CurrentTaskView.percent` 已 round 一位小数，前端 toString 自然输出：整数不带 ".0"，
 *  一位小数带小数点（与 `hoursFromMinutes` 口径一致）；不要再二次 round——会抹掉一位小数。 */
export function taskProgressLabel(task: TaskView): string {
  if ((task.estimated_minutes ?? 0) === 0) return "";
  return `${task.progress_percent}%`;
}
