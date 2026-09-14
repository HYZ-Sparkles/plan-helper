/**
 * v2 环视方位判定（工单 21，CodexPetContract）：指针相对桌宠中心的方位角查 16 向
 * 环视表——22.5° 顺时针分档、序 0 = 正上方（12 点方向）；指针落在**正前方死区**
 * （距中心不足阈值，即基本压在桌宠身上）返回 null 回落 idle（契约原文：正前方是指针
 * 死区）。纯函数（dx/dy 为物理像素差），Node 确定性回归覆盖分档与死区。
 * v1 形象无环视行——调用方按 spriteVersion 不启用（静默降级），与本函数无关。
 */

/** 死区半径（逻辑像素，调用方 ×scaleFactor 换物理）：约桌宠窗口高的六成 */
export const GAZE_DEADZONE_PX = 64;

/** 方位分档宽度（度）：16 向 = 360 / 16 */
export const LOOK_STEP_DEG = 22.5;

/** 指针 → 16 向环视序号（0 = 正上方、顺时针）；死区内返回 null */
export function lookIndex(dx: number, dy: number, deadzone: number): number | null {
  if (Math.hypot(dx, dy) < deadzone) return null;
  // 屏幕坐标 y 向下：atan2(dx, -dy) 得 0 = 上、顺时针为正的方位角
  const angle = (Math.atan2(dx, -dy) * 180) / Math.PI;
  const norm = (angle + 360) % 360;
  return Math.round(norm / LOOK_STEP_DEG) % 16;
}
