/**
 * "弹出窗口"语义的跨窗口通道（同 summary.ts 模式）：把已创建的顶层窗口可靠地亮到前台。
 * 所有用户主动唤起的弹窗（大面板/今日总结/桌宠菜单/控制面板）统一走这里，不各写一遍。
 */
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

/**
 * unminimize → show → setFocus 三步缺一不可：Windows 上 show() 对**最小化**的窗口
 * 无效（"面板明明在后台开着，点了却没反应"的根因），setFocus 负责从被遮挡/失焦中抬升。
 * 小看板是贴桌宠的常驻伴侣件——出现但不抢焦点，不走这里（2026-08-30 用户决策）。
 * 返回窗口是否存在：label 无窗口返回 false，调用方据此跳过后续登记类副作用
 * （如"只弹一次"的登记——窗口没亮成，不能把总结记成已弹）。
 */
export async function revealWindow(label: string): Promise<boolean> {
  const win = await WebviewWindow.getByLabel(label);
  if (!win) return false;
  await win.unminimize();
  await win.show();
  await win.setFocus();
  return true;
}
