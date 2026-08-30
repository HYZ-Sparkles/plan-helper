/**
 * 今日总结（工单 11）跨窗口编排：事件名常量 + 打开/调出共用的窗口通道。
 * 自动触发（PetWindow 定时）与手动调出（控制面板）走同一套"先装载再显示"的次序，
 * 只在"是否登记已弹"上分叉——由调用方显式声明。
 */
import { emitTo } from "@tauri-apps/api/event";
import { markDailySummaryShown } from "./api";
import { revealWindow } from "./windows";

/** 请求总结窗口装载并显示指定日期（payload {date}；PetWindow 自动触发 / 控制面板手动调出共用） */
export const SUMMARY_SHOW_EVENT = "daily-summary:show";
/** 进展事件到达（汇报/撤销/修正落账）→ 打开中的总结重取最新（内容实时重算，ADR-0009） */
export const SUMMARY_REFRESH_EVENT = "daily-summary:refresh";

/**
 * 打开（或聚焦）总结窗并装载指定日期（与总结窗对 SUMMARY_SHOW_EVENT 的监听配套；
 * 次序同大面板重开语义：先发事件让窗口装载最新账，再由 revealWindow 亮到前台）。
 * register = 是否登记"已弹"（只弹一次）：自动触发恒 true（不登记下次会重弹）；
 * 控制面板调出仅在看的就是**待弹**那份时 true——手动补看即注销，到点不再自动弹。
 * 窗口不存在（理论不可达，随应用启动创建）时不登记，避免把没弹出的总结记成已弹。
 */
export async function openDailySummaryWindow(date: string, register: boolean) {
  await emitTo("daily-summary", SUMMARY_SHOW_EVENT, { date });
  const shown = await revealWindow("daily-summary");
  if (!shown) return;
  if (register) await markDailySummaryShown(date);
}
