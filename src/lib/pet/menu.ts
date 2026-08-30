/**
 * 桌宠浮层的共享词汇（工单 08 起，2026-08-30 反馈重构）：PetWindow 与 PetMenuWindow
 * 两个窗口经 Tauri 事件通信，载荷类型与事件名集中在这里，避免两窗口各写一份漂移。
 * 浮层（菜单/提示气泡）显隐唯一持有者是 PetWindow——本窗只发亮出指令与回执
 * （失焦/到点，带亮出代数），不自行 hide；桌宠按代数裁决隐藏（竞态消解）。
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

/** 浮层亮出载荷（seq = 桌宠侧浮层代数，回执据此裁决显隐竞态） */
export interface PetMenuShowPayload {
  locked: boolean;
  mode: PetMode;
  seq: number;
}

/** 浮层回执载荷（失焦/到点：携带亮出时的代数） */
export interface PetMenuSeqPayload {
  seq: number;
}

/** 事件名常量（pet ↔ pet-menu） */
export const MENU_OPEN_EVENT = "pet-menu:open";
export const MENU_STATE_EVENT = "pet-menu:state";
export const MENU_HINT_EVENT = "pet-menu:hint";
/** 浮层失去焦点回执：pet-menu 只报告不自行隐藏，桌宠按代数裁决是否隐藏 */
export const MENU_BLUR_EVENT = "pet-menu:blur";
/** 提示气泡到点回执：同上（气泡窗不清形态，避免窗口可见的最后一瞬闪出菜单卡） */
export const MENU_EXPIRE_EVENT = "pet-menu:expire";
export const MENU_ACTION_EVENT = "pet-menu:action";

/** 托盘退出 → 桌宠告别（工单 14）：Rust 托盘「退出」发出（src-tauri/src/lib.rs
 *  TRAY_EXIT_EVENT 对应），PetWindow 收到后走与菜单「再见」同一 goodbye 通道 */
export const TRAY_EXIT_EVENT = "pet:tray-exit";
