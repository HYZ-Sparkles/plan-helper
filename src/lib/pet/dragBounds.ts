/**
 * 拖动边界（工单 09，CONTEXT PetDragBounds）四约束的纯函数数学核心：
 * 1. 任务栏禁入 —— 落点钳制进目标显示器的**工作区**（工作区已扣除任务栏）
 * 2. 不出物理边界 —— 钳制保证窗口完整可见
 * 3. 多显示器可跨 —— 按窗口矩形与各显示器**重叠面积最大**者选屏（拖过半自动切换）
 * 4. 边缘吸附 —— 拖动停止时距工作区任一边 < 阈值贴齐最近边
 * 单位一律物理像素（与 Tauri outerPosition / monitor 同口径）；无 DOM/Tauri 依赖，Node 可直跑。
 */

/** 显示器几何快照（物理像素）：bounds 为物理边界，work* 为工作区（任务栏已扣除） */
export interface MonitorArea {
  x: number;
  y: number;
  width: number;
  height: number;
  workX: number;
  workY: number;
  workWidth: number;
  workHeight: number;
}

/** 拖动落点：钳制后的坐标 + 钳制所依据的显示器（吸附要贴它的边） */
export interface DragLanding {
  x: number;
  y: number;
  monitor: MonitorArea;
}

/** 窗口矩形与显示器物理边界的重叠面积（不相交为 0） */
function overlap(x: number, y: number, w: number, h: number, m: MonitorArea): number {
  const ox = Math.min(x + w, m.x + m.width) - Math.max(x, m.x);
  const oy = Math.min(y + h, m.y + m.height) - Math.max(y, m.y);
  return ox > 0 && oy > 0 ? ox * oy : 0;
}

/** 目标显示器：重叠面积最大者；全部不相交退化为中心点所在者（再无 → 第一个） */
export function monitorForRect(
  x: number,
  y: number,
  w: number,
  h: number,
  monitors: MonitorArea[],
): MonitorArea {
  let best = monitors[0];
  let bestArea = -1;
  for (const m of monitors) {
    const a = overlap(x, y, w, h, m);
    if (a > bestArea) {
      best = m;
      bestArea = a;
    }
  }
  if (bestArea > 0) return best;
  const cx = x + w / 2;
  const cy = y + h / 2;
  return (
    monitors.find((m) => cx >= m.x && cx < m.x + m.width && cy >= m.y && cy < m.y + m.height) ??
    best
  );
}

/** 钳制进工作区：任务栏禁入 + 全可见两约束一并成立（工作区 ⊆ 物理边界） */
export function clampIntoWorkArea(
  x: number,
  y: number,
  w: number,
  h: number,
  m: MonitorArea,
): { x: number; y: number } {
  return {
    x: Math.min(Math.max(x, m.workX), m.workX + m.workWidth - w),
    y: Math.min(Math.max(y, m.workY), m.workY + m.workHeight - h),
  };
}

/** 拖动落点 = 选屏 + 钳制（约束 1/2/3）。moveTo 的每一帧都走这里 */
export function clampDragPosition(
  x: number,
  y: number,
  w: number,
  h: number,
  monitors: MonitorArea[],
): DragLanding {
  const monitor = monitorForRect(x, y, w, h, monitors);
  const p = clampIntoWorkArea(x, y, w, h, monitor);
  return { ...p, monitor };
}

/** 边缘吸附（约束 4）：距工作区最近一边 < threshold（物理像素）则贴齐该边，否则原样返回 */
export function snapToEdges(
  x: number,
  y: number,
  w: number,
  h: number,
  m: MonitorArea,
  threshold: number,
): { x: number; y: number } {
  const dists = [
    { d: x - m.workX, ax: "x" as const, v: m.workX }, // 左
    { d: m.workX + m.workWidth - w - x, ax: "x" as const, v: m.workX + m.workWidth - w }, // 右
    { d: y - m.workY, ax: "y" as const, v: m.workY }, // 上
    { d: m.workY + m.workHeight - h - y, ax: "y" as const, v: m.workY + m.workHeight - h }, // 下
  ];
  const near = dists.reduce((a, b) => (b.d < a.d ? b : a)); // 距最近边
  if (near.d >= threshold || near.d < 0) return { x, y };
  return near.ax === "x" ? { x: near.v, y } : { x, y: near.v };
}
