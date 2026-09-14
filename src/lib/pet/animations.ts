/**
 * codex 桌宠契约动画数据（工单 16，CodexPetContract / ADR-0010）：
 * 单张雪碧图 = 固定 8 列网格、格 192×208（v1 图集 1536×1872 = 9 行、v2 1536×2288 = 11 行），
 * 显示时 ÷2 缩放（窗口 96×104）；第 0–8 行是 9 个标准动作、v2 第 9–10 行是 16 向环视
 * 静态姿势。每帧时长按 OpenAI codex 应用实际播放参数（2026-09-14 二次校准）：
 * - idle 用应用内置的 **calm loop** `[1680,660,660,840,840,1920]`（codex-rs
 *   `tui/src/pets/model.rs` 的 idle_animation 断言）——是参考表〔hatch-pet 技能的
 *   animation-rows.md，面向自制宠物包〕的 6 倍慢速；
 * - 其余动作与 model.rs / 参考表一致（120/140/150ms 每帧，末帧加长）。
 * 状态一次性动画播 3 遍后回常驻（同 model.rs 的 app_state_animation），见 actions.ts
 * STATE_REPEATS。形象只是皮肤（skins.ts 注册表），本文件对所有形象成立；替代 Oreo
 * 的"fps + 非透明列段检测"验收调整管线（契约无调参空间）。
 */

/** 契约动作名（= 图集行序 0–8；键序即行序，勿重排） */
export type PetAnim =
  | "idle"
  | "running-right"
  | "running-left"
  | "waving"
  | "jumping"
  | "failed"
  | "waiting"
  | "running"
  | "review";

/** 契约网格常量：8 列 × 192×208 格 */
export const GRID_COLS = 8;
export const CELL_W = 192;
export const CELL_H = 208;
/** 渲染缩放：÷2（格 192×208 → 显示 96×104，坐标皆偶数、缩放后仍是整数像素——
 *  ADR-0010"÷2 整数缩放保证像素干净"） */
export const SHEET_SCALE = 0.5;
/** 桌宠窗口逻辑尺寸 = 格 × 缩放（96×104） */
export const PET_WIN = { width: CELL_W * SHEET_SCALE, height: CELL_H * SHEET_SCALE };

export interface AnimationDef {
  /** 图集行号（0–8） */
  row: number;
  /** 本行使用的列数（其余列契约保证全透明） */
  cols: number;
  /** 每帧时长（ms，长 = cols；idle 为应用实际 calm loop，其余为契约参考表） */
  durations: number[];
  /** 天然是否循环（一次性演出可在 StepSpec 用 loop:false + repeats 截断） */
  loop: boolean;
  /** durations 前缀和（长 = cols + 1，末项 = 行总时长）——引擎步进查表用，勿手填 */
  cum: number[];
}

/** 组装单个动作定义：同时算好前缀和 */
function def(row: number, cols: number, loop: boolean, durations: number[]): AnimationDef {
  const cum = [0];
  for (const d of durations) cum.push(cum[cum.length - 1] + d);
  return { row, cols, loop, durations, cum };
}

/** 9 个标准动作（行 0–8）。时长：idle = 应用 calm loop（6.6s 一圈，codex-rs 实证）；
 *  其余 = 参考表（120/140/150 每帧 + 末帧 220/240/260/280，与 model.rs 一致） */
export const ANIMATIONS: Record<PetAnim, AnimationDef> = {
  idle: def(0, 6, true, [1680, 660, 660, 840, 840, 1920]),
  "running-right": def(1, 8, true, [120, 120, 120, 120, 120, 120, 120, 220]),
  "running-left": def(2, 8, true, [120, 120, 120, 120, 120, 120, 120, 220]),
  waving: def(3, 4, false, [140, 140, 140, 280]),
  jumping: def(4, 5, false, [140, 140, 140, 140, 280]),
  failed: def(5, 8, false, [140, 140, 140, 140, 140, 140, 140, 240]),
  waiting: def(6, 6, true, [150, 150, 150, 150, 150, 260]),
  running: def(7, 6, true, [120, 120, 120, 120, 120, 220]),
  review: def(8, 6, true, [150, 150, 150, 150, 150, 280]),
};

/** v2 环视：16 向静态姿势，22.5° 顺时针一格（行 9 = 000°–157.5°，行 10 = 180°–337.5°；
 *  000° = 正上方，正前方是死区回落 idle——CodexPetContract） */
export const LOOK_DIRECTIONS = 16;

/** 环视序号（0–15，顺时针、0 = 正上方）→ 图集格坐标 */
export function lookCell(idx: number): { row: number; col: number } {
  return { row: 9 + Math.floor(idx / GRID_COLS), col: idx % GRID_COLS };
}
