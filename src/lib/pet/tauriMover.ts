/**
 * Tauri 版 PetMover（工单 08）：直接驱动桌宠窗口的物理坐标。
 * Tauri JS 窗口 API 全异步而引擎按 rAF 同步取值——位置用本地缓存（自己发起的 moveTo
 * 即时回写，起点启动时取一次），工作区/尺寸/缩放同理缓存（跨显示器热切换的刷新留给
 * 工单 09 的拖拽）。所有落点都钳制在当前显示器工作区内，保证不出屏。
 */
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { currentMonitor } from "@tauri-apps/api/window";
import { PhysicalPosition } from "@tauri-apps/api/dpi";
import type { PetMover } from "./engine";

export async function createTauriMover(): Promise<PetMover> {
  const win = getCurrentWebviewWindow();
  const [pos0, monitor, size0, factor] = await Promise.all([
    win.outerPosition(),
    currentMonitor(),
    win.outerSize(),
    win.scaleFactor(),
  ]);
  const area = {
    x: monitor?.workArea.position.x ?? pos0.x,
    y: monitor?.workArea.position.y ?? pos0.y,
    width: monitor?.workArea.size.width ?? 1920,
    height: monitor?.workArea.size.height ?? 1080,
  };
  const size = { width: size0.width, height: size0.height };
  let x = pos0.x;
  let y = pos0.y;
  return {
    position: () => ({ x, y }),
    size: () => ({ ...size }),
    workArea: () => ({ ...area }),
    scaleFactor: () => factor,
    moveTo(nx, ny) {
      x = Math.min(Math.max(nx, area.x), area.x + area.width - size.width);
      y = Math.min(Math.max(ny, area.y), area.y + area.height - size.height);
      void win.setPosition(new PhysicalPosition(x, y));
    },
  };
}
