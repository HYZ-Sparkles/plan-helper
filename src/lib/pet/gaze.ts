/**
 * v2 环视方位判定（工单 21，CodexPetContract；2026-09-14 验收修订）：**激活半径内**
 * 才跟视——指针进入桌宠附近（≤ GAZE_RANGE_PX）时按 16 向环视表（22.5° 顺时针分档、
 * 序 0 = 正上方）显示对应姿势；超出半径回落 idle（安静不注目，靠近才互动）；指针基本
 * 压在桌宠正中（距中心不足死区）方向无意义，也回落 idle。纯函数（dx/dy 为物理像素
 * 差，两阈值由调用方 ×scaleFactor 换算），Node 确定性回归覆盖分档、死区与激活边界。
 * 环视能力 = skinCanGaze（v1 无环视行天然关闭；个别 v2 形象被显式忽略）。
 */

/** 死区半径（逻辑像素，调用方 ×scaleFactor 换物理）：指针压在桌宠身上 = 无方位可言 */
export const GAZE_DEADZONE_PX = 64;
/** 激活半径（逻辑像素）：鼠标靠近桌宠到这个距离内才跟视，超出回落 idle */
export const GAZE_RANGE_PX = 240;

/** 方位分档宽度（度）：16 向 = 360 / 16 */
export const LOOK_STEP_DEG = 22.5;

/** 指针 → 16 向环视序号（0 = 正上方、顺时针）；死区内 / 激活半径外返回 null */
export function lookIndex(dx: number, dy: number, deadzone: number, range: number): number | null {
  const dist = Math.hypot(dx, dy);
  if (dist < deadzone || dist > range) return null;
  // 屏幕坐标 y 向下：atan2(dx, -dy) 得 0 = 上、顺时针为正的方位角
  const angle = (Math.atan2(dx, -dy) * 180) / Math.PI;
  const norm = (angle + 360) % 360;
  return Math.round(norm / LOOK_STEP_DEG) % 16;
}
