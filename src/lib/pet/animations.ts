/**
 * codex 桌宠契约动画数据（工单 16，CodexPetContract / ADR-0010）：
 * 单张雪碧图 = 固定 8 列网格、格 192×208（v1 图集 1536×1872 = 9 行、v2 1536×2288 = 11 行），
 * 第 0–8 行是 9 个标准动作、v2 第 9–10 行是 16 向环视静态姿势；每帧时长由契约硬性规定
 * （awesome-codex-pet .agents/skills/hatch-pet-v1/references/animation-rows.md）。
 * 形象只是皮肤（skins.ts 注册表），本文件对所有形象成立；替代 Oreo 的
 * "fps + 非透明列段检测 + NUDGE/帧序/位移权重" 验收调整管线（契约无调参空间）。
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
/** 渲染缩放：÷2 整数缩放保证像素干净（ADR-0010） */
export const SHEET_SCALE = 2;
/** 桌宠窗口逻辑尺寸 = 格 × 缩放 */
export const PET_WIN = { width: (CELL_W * SHEET_SCALE) / 2, height: (CELL_H * SHEET_SCALE) / 2 };

export interface AnimationDef {
  /** 图集行号（0–8） */
  row: number;
  /** 本行使用的列数（其余列契约保证全透明） */
  cols: number;
  /** 每帧时长（ms，长 = cols；契约硬性规定） */
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

/** 9 个标准动作（行 0–8）。时长表来自契约原文：
 *  idle 280,110,110,140,140,320；running-right/left 120×7+末帧 220；waving 140×3+280；
 *  jumping 140×4+280；failed 140×7+240；waiting 150×5+260；running 120×5+220；review 150×5+280 */
export const ANIMATIONS: Record<PetAnim, AnimationDef> = {
  idle: def(0, 6, true, [280, 110, 110, 140, 140, 320]),
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
