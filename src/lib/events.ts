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
/** 小看板被亮出（点击桌宠唤起 / 事件亮相，2026-08-30 反馈）→ 看板自重置到任务视图并
 *  启动自动隐藏计时（悬停暂停）；PetWindow 是显隐唯一持有者，亮板后必发 */
export const MINI_BOARD_SHOW_EVENT = "mini-board:show";
/** 小看板请求收起（✕ 按钮 / 自动隐藏计时到点）→ PetWindow 统一执行隐藏并解除挂靠 */
export const MINI_BOARD_DISMISS_EVENT = "mini-board:dismiss";
/** 设置保存（工单 13）→ 桌宠重排两个定时触发（最晚窗口结束的今日总结 / 工作窗口
 *  开始的大面板）——设置页保存后发射，PetWindow 监听重查服务端时刻 */
export const SETTINGS_CHANGED_EVENT = "settings:changed";
/** 大面板可见性（工单 20，waiting 的起止裁决源）：MainBoardWindow 在被亮出/隐藏后
 *  发射 {open}，PetWindow 据此切 waiting 常驻——"面板开着 = 等你分配"，不区分今日
 *  是否已确认、不区分当前模式；PetWindow 自己亮面板时直接置 boardOpen 不走事件 */
export const MAIN_BOARD_VISIBILITY_EVENT = "main-board:visibility";
/** 桌宠业务里程碑（工单 20，PetActionPolicy 第 4 来源）：payload = {kind}——小看板
 *  推进型汇报（子目标勾选 / +X% 增量）→ "review"；今日总结弹出且未达标 → "failed"；
 *  今日任务全部完成（false→true 跃迁）→ "celebrate"。修正/撤销等往回改不发 */
export const PET_MILESTONE_EVENT = "pet:milestone";
/** 里程碑种类（waiting 为面板开关驱动，不在此列——它是常驻替换不是一次性演出） */
export type MilestoneKind = "review" | "failed" | "celebrate";
/** 桌宠形象切换（工单 22）：设置页「桌宠偏好」选择即发 {slug}（getPref/setPref 已
 *  落库，事件只负责让桌宠窗口即时换装——帧网格同构，常驻动画就地换不重播生命周期） */
export const PET_SKIN_CHANGED_EVENT = "pet:skin-changed";
