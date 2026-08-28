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
