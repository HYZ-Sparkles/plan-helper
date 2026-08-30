/**
 * 桌宠菜单的共享词汇（工单 08）：PetWindow 与 PetMenuWindow 两个窗口经 Tauri 事件
 * 通信，载荷类型与事件名集中在这里，避免两窗口各写一份漂移。
 */

/** 桌宠模式（CONTEXT WorkMode / RestMode） */
export type PetMode = "work" | "rest";

/** 菜单项（CONTEXT PetMenuActions） */
export type MenuAction = "control-panel" | "toggle-mode" | "goodbye";

/** 菜单打开/状态同步事件载荷 */
export interface PetMenuState {
  locked: boolean;
  mode: PetMode;
}

/** 事件名常量（pet ↔ pet-menu） */
export const MENU_OPEN_EVENT = "pet-menu:open";
export const MENU_STATE_EVENT = "pet-menu:state";
export const MENU_CLOSE_EVENT = "pet-menu:close";
export const MENU_CLOSED_EVENT = "pet-menu:closed";
export const MENU_ACTION_EVENT = "pet-menu:action";
/** 休息模式左键提示气泡（2026-08-30 反馈）：PetWindow 发出，菜单窗以气泡形态展示
 *  「右键才是菜单喵」后自动收起——菜单窗身兼桌宠头顶浮层（菜单/提示两种形态） */
export const MENU_HINT_EVENT = "pet-menu:hint";

/** 托盘退出 → 桌宠告别（工单 14）：Rust 托盘「退出」发出（src-tauri/src/lib.rs
 *  TRAY_EXIT_EVENT 对应），PetWindow 收到后走与菜单「再见」同一 goodbye 通道 */
export const TRAY_EXIT_EVENT = "pet:tray-exit";
