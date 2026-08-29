/**
 * Tauri 版 PetMover（工单 08 起，09 扩多显示器）：直接驱动桌宠窗口的物理坐标。
 * Tauri JS 窗口 API 全异步而引擎按 rAF 同步取值——位置用本地缓存（自己发起的 moveTo
 * 即时回写，起点启动时取一次），显示器快照同理缓存、拖拽开始前 refresh() 刷新
 * （显示器热插拔/跨屏 DPI 变化）。moveTo 是**原始落位**：钳制归调用方——拖拽走
 * dragBounds（跨屏选屏 + 工作区钳制 + 吸附），动画位移走引擎 resolveMove。
 */
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { availableMonitors } from "@tauri-apps/api/window";
import { PhysicalPosition } from "@tauri-apps/api/dpi";
import type { MonitorArea } from "./dragBounds";
import type { PetMover } from "./engine";

export async function createTauriMover(): Promise<PetMover> {
  const win = getCurrentWebviewWindow();
  let monitors: MonitorArea[] = [];
  let factor = await win.scaleFactor();
  // 读取失败保持旧快照（拖拽中刷新不翻车）；初始即失败留给 current()/monitors() 的兜底屏
  const readMonitors = async () => {
    const list = (await availableMonitors()).map((m) => ({
      x: m.position.x,
      y: m.position.y,
      width: m.size.width,
      height: m.size.height,
      workX: m.workArea.position.x,
      workY: m.workArea.position.y,
      workWidth: m.workArea.size.width,
      workHeight: m.workArea.size.height,
    }));
    if (list.length) monitors = list;
  };
  try {
    await readMonitors();
  } catch {
    /* 拿不到显示器列表：单屏兜底 */
  }
  const [pos0, size0] = await Promise.all([win.outerPosition(), win.outerSize()]);
  const size = { width: size0.width, height: size0.height };
  let x = pos0.x;
  let y = pos0.y;
  const fallback: MonitorArea = {
    x: 0,
    y: 0,
    width: 1920,
    height: 1080,
    workX: 0,
    workY: 0,
    workWidth: 1920,
    workHeight: 1040,
  };
  // 工作区 = 桌宠中心点所在显示器的工作区（动画位移不出该屏；缺显示器信息用兜底屏）
  const current = () =>
    monitors.find((m) => {
      const cx = x + size.width / 2;
      return cx >= m.x && cx < m.x + m.width;
    }) ?? monitors[0] ?? fallback;
  return {
    position: () => ({ x, y }),
    size: () => ({ ...size }),
    workArea: () => {
      const m = current();
      return { x: m.workX, y: m.workY, width: m.workWidth, height: m.workHeight };
    },
    scaleFactor: () => factor,
    monitors: () => (monitors.length ? monitors.map((m) => ({ ...m })) : [{ ...fallback }]),
    refresh: async () => {
      try {
        await readMonitors();
      } catch {
        /* 刷新失败保持旧快照 */
      }
      factor = await win.scaleFactor();
    },
    moveTo(nx, ny) {
      x = Math.round(nx);
      y = Math.round(ny);
      void win.setPosition(new PhysicalPosition(x, y));
    },
  };
}
