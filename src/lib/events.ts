/**
 * 跨窗口事件名常量：发事件方与监听方不各写魔法串（src/lib/summary.ts 的总结事件同模式）。
 */
/** 重开大面板（控制面板「打开大面板」/ 桌宠模式切换）：先发事件再 show，窗口读到最新数据 */
export const MAIN_BOARD_REOPEN_EVENT = "main-board:reopen";
/** 生命周期变化落库（开始/暂停/恢复/完成/放弃/复制，工单 12）→ 大面板静默重取：
 *  候选分组与"暂停"分组随之刷新，不改变窗口显隐 */
export const MAIN_BOARD_REFRESH_EVENT = "main-board:refresh";
/** 当日分配落定 / 生命周期变化 → 小看板重取：当前任务失效回空态、更换候选随之刷新 */
export const MINI_BOARD_REFRESH_EVENT = "mini-board:refresh";
/** 设置保存（工单 13）→ 桌宠重排两个定时触发（最晚窗口结束的今日总结 / 工作窗口
 *  开始的大面板）——设置页保存后发射，PetWindow 监听重查服务端时刻 */
export const SETTINGS_CHANGED_EVENT = "settings:changed";
