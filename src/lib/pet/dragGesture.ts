/**
 * 拖拽手势方向分类（工单 18，PetDragGesture）：照抄 codex 原版 playground 手势语义——
 * 拖动中按 140ms 滑动采样窗口的主方向决定反馈动作：
 * - 主水平（两轴偏差比 > 1.12）→ running-right / running-left（方向反转实时跟切）
 * - 主竖直 → jumping
 * - 轴向模糊（斜向、偏差比不足 1.12）→ null（保持当前动作不抖动）
 * 纯函数（samples 由调用方收集，时间升序），Node 确定性回归覆盖。
 */

/** 滑动采样窗口宽度（ms）：窗口外样本不参与方向判定 */
export const DRAG_WINDOW_MS = 140;
/** 轴向偏差比：|一轴| > |另一轴| × 此值才算"主方向"清晰 */
export const AXIS_RATIO = 1.12;

/** 一个指针采样：t = 事件时间戳（ms），x/y = 屏幕逻辑像素 */
export interface DragSample {
  t: number;
  x: number;
  y: number;
}

/** 拖拽反馈动作（与契约动作一一对应；null = 保持当前） */
export type DragFeedback = "jumping" | "running-right" | "running-left";

/** 按 140ms 窗口内首尾位移的主方向分类；样本不足/轴向模糊返回 null */
export function classifyDrag(samples: DragSample[], now: number): DragFeedback | null {
  // 样本时间升序：从头找第一个仍在窗口内的样本作为窗口起点
  let start: DragSample | null = null;
  for (const s of samples) {
    if (now - s.t <= DRAG_WINDOW_MS) {
      start = s;
      break;
    }
  }
  const last = samples[samples.length - 1];
  if (!start || !last || start === last) return null; // 窗口内不足两点：无位移可判
  const dx = last.x - start.x;
  const dy = last.y - start.y;
  if (Math.abs(dy) > Math.abs(dx) * AXIS_RATIO) return "jumping";
  if (Math.abs(dx) > Math.abs(dy) * AXIS_RATIO) return dx > 0 ? "running-right" : "running-left";
  return null;
}
