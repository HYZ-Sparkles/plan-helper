/**
 * Oreo Cat 帧清单（由 scripts/gen-pet-frames.mjs 从 resourses/ 的 sprite sheet 程序化生成——勿手改，
 * 资产更新或帧边界修正后重跑脚本）。帧矩形为 sheet 像素坐标；行高 32、地面线 = 帧矩形底边，
 * 渲染时按底部锚定统一站高。fps 为默认值（资源无时序元数据），可在 /dev/anim 调试页试拍后改 META。
 */
export const PET_SHEET_URL = "/pet/oreo-sheet.png";
export const SHEET_SCALE = 2;
export const SHEET_WIDTH = 462;
export const SHEET_HEIGHT = 766;

/** 单帧在 sheet 上的矩形（像素坐标） */
export interface FrameRect {
  x: number;
  y: number;
  w: number;
  h: number;
  /** 摆放微调（素材像素，正 = 右 / 下；×SHEET_SCALE 后生效）。切分不动只调摆放——
   *  值来自生成脚本的 NUDGE 表（验收时在 /dev/anim 调试页试出后写回） */
  ox?: number;
  oy?: number;
}

export interface AnimationDef {
  name: string;
  /** 默认播放速度（帧/秒） */
  fps: number;
  /** 播完是否循环（idle 类为 true） */
  loop: boolean;
  frames: FrameRect[];
}

/** 24 个动画，键 = 作者标注的动画编号（README 动作映射引用的编号体系） */
export const ANIMATIONS: Record<number, AnimationDef> = {
  1: { name: "Stand to Sit", fps: 7, loop: false, frames: [{ x: 5, y: 0, w: 25, h: 32 }, { x: 34, y: 0, w: 29, h: 32 }, { x: 69, y: 0, w: 27, h: 32 }, { x: 103, y: 0, w: 26, h: 32 }] },
  2: { name: "Sit Idle", fps: 4, loop: true, frames: [{ x: 4, y: 32, w: 26, h: 32 }, { x: 37, y: 32, w: 26, h: 32 }, { x: 70, y: 32, w: 26, h: 32 }, { x: 103, y: 32, w: 26, h: 32 }] },
  3: { name: "Sit to Stand", fps: 7, loop: false, frames: [{ x: 4, y: 64, w: 26, h: 32 }, { x: 36, y: 64, w: 27, h: 32 }, { x: 67, y: 64, w: 29, h: 32 }, { x: 104, y: 64, w: 25, h: 32 }] },
  4: { name: "Stand to Sleep", fps: 7, loop: false, frames: [{ x: 5, y: 96, w: 25, h: 32 }, { x: 34, y: 96, w: 29, h: 32 }, { x: 69, y: 96, w: 27, h: 32 }, { x: 103, y: 96, w: 26, h: 32 }, { x: 136, y: 96, w: 24, h: 32 }] },
  5: { name: "Sleep Idle", fps: 4, loop: true, frames: [{ x: 4, y: 128, w: 24, h: 32 }, { x: 37, y: 128, w: 24, h: 32 }, { x: 70, y: 128, w: 24, h: 32 }, { x: 103, y: 128, w: 24, h: 32 }, { x: 136, y: 128, w: 25, h: 32 }] },
  6: { name: "Sleep to Stand", fps: 7, loop: false, frames: [{ x: 4, y: 160, w: 24, h: 32 }, { x: 37, y: 160, w: 26, h: 32 }, { x: 69, y: 160, w: 27, h: 32 }, { x: 100, y: 160, w: 29, h: 32 }, { x: 137, y: 160, w: 25, h: 32 }] },
  7: { name: "Stand Idle", fps: 4, loop: true, frames: [{ x: 5, y: 192, w: 25, h: 32 }, { x: 36, y: 192, w: 27, h: 32 }, { x: 67, y: 192, w: 29, h: 32 }, { x: 102, y: 192, w: 27, h: 32 }, { x: 137, y: 192, w: 25, h: 32 }] },
  8: { name: "Eat", fps: 6, loop: false, frames: [{ x: 1, y: 224, w: 29, h: 32 }, { x: 36, y: 224, w: 27, h: 32 }, { x: 67, y: 224, w: 29, h: 32 }, { x: 102, y: 224, w: 27, h: 32 }] },
  9: { name: "Walk", fps: 7, loop: false, frames: [{ x: 3, y: 256, w: 27, h: 32 }, { x: 34, y: 256, w: 29, h: 32 }, { x: 69, y: 256, w: 27, h: 32 }, { x: 104, y: 256, w: 25, h: 32 }, { x: 135, y: 256, w: 27, h: 32 }, { x: 166, y: 256, w: 29, h: 32 }, { x: 201, y: 256, w: 27, h: 32 }, { x: 236, y: 256, w: 25, h: 32 }] },
  10: { name: "Run", fps: 7, loop: false, frames: [{ x: 0, y: 288, w: 30, h: 32 }, { x: 36, y: 288, w: 27, h: 32 }, { x: 71, y: 288, w: 25, h: 32 }, { x: 104, y: 288, w: 25, h: 32 }] },
  11: { name: "Prepare Stealth", fps: 7, loop: false, frames: [{ x: 5, y: 320, w: 25, h: 32 }, { x: 33, y: 320, w: 30, h: 32 }, { x: 69, y: 320, w: 27, h: 32 }] },
  12: { name: "Stealth", fps: 5, loop: true, frames: [{ x: 3, y: 352, w: 27, h: 32 }, { x: 36, y: 352, w: 27, h: 32 }, { x: 70, y: 352, w: 26, h: 32 }, { x: 102, y: 352, w: 27, h: 32 }, { x: 135, y: 352, w: 27, h: 32 }, { x: 168, y: 352, w: 27, h: 32 }, { x: 202, y: 352, w: 26, h: 32 }] },
  13: { name: "Cancel Stealth", fps: 7, loop: false, frames: [{ x: 3, y: 384, w: 27, h: 32 }, { x: 33, y: 384, w: 30, h: 32 }, { x: 71, y: 384, w: 25, h: 32 }] },
  14: { name: "Jump", fps: 7, loop: false, frames: [{ x: 5, y: 416, w: 25, h: 32 }, { x: 36, y: 416, w: 27, h: 32 }, { x: 71, y: 416, w: 25, h: 32 }, { x: 102, y: 416, w: 27, h: 32 }, { x: 137, y: 416, w: 25, h: 32 }, { x: 168, y: 416, w: 27, h: 32 }, { x: 198, y: 416, w: 30, h: 32 }] },
  15: { name: "Attack", fps: 7, loop: false, frames: [{ x: 5, y: 448, w: 25, h: 32 }, { x: 39, y: 448, w: 22, h: 32 }, { x: 73, y: 448, w: 20, h: 32 }, { x: 107, y: 448, w: 22, h: 32 }, { x: 140, y: 448, w: 21, h: 32 }, { x: 173, y: 448, w: 21, h: 32 }, { x: 206, y: 448, w: 21, h: 32 }, { x: 239, y: 448, w: 24, h: 32 }, { x: 272, y: 448, w: 21, h: 32 }, { x: 305, y: 448, w: 22, h: 32 }, { x: 337, y: 448, w: 20, h: 32 }, { x: 369, y: 448, w: 22, h: 32 }, { x: 401, y: 448, w: 25, h: 32 }] },
  16: { name: "Loop Attack", fps: 7, loop: true, frames: [{ x: 8, y: 480, w: 22, h: 32 }, { x: 41, y: 480, w: 21, h: 32 }, { x: 74, y: 480, w: 21, h: 32 }, { x: 107, y: 480, w: 21, h: 32 }, { x: 140, y: 480, w: 21, h: 32 }, { x: 173, y: 480, w: 24, h: 32 }] },
  17: { name: "Jump in to the Box", fps: 7, loop: false, frames: [{ x: 4, y: 512, w: 26, h: 32 }, { x: 35, y: 512, w: 28, h: 32 }, { x: 70, y: 512, w: 26, h: 32 }, { x: 101, y: 512, w: 29, h: 32 }, { x: 133, y: 512, w: 30, h: 32 }, { x: 165, y: 512, w: 32, h: 32 }, { x: 198, y: 512, w: 32, h: 32 }, { x: 234, y: 512, w: 29, h: 32 }] },
  18: { name: "Push Hand Up", fps: 7, loop: false, frames: [{ x: 3, y: 544, w: 29, h: 32 }, { x: 36, y: 544, w: 29, h: 32 }, { x: 69, y: 544, w: 29, h: 32 }] },
  19: { name: "Play Box", fps: 7, loop: false, frames: [{ x: 3, y: 576, w: 29, h: 32 }, { x: 36, y: 576, w: 29, h: 32 }, { x: 69, y: 576, w: 29, h: 32 }, { x: 102, y: 576, w: 29, h: 32 }, { x: 135, y: 576, w: 29, h: 32 }] },
  20: { name: "Push Hand Down", fps: 7, loop: false, frames: [{ x: 3, y: 608, w: 29, h: 32 }, { x: 36, y: 608, w: 29, h: 32 }, { x: 69, y: 608, w: 29, h: 32 }] },
  21: { name: "Ear Up", fps: 6, loop: false, frames: [{ x: 4, y: 640, w: 29, h: 32 }, { x: 37, y: 640, w: 29, h: 32 }] },
  22: { name: "Scan", fps: 6, loop: false, frames: [{ x: 4, y: 672, w: 29, h: 32 }, { x: 37, y: 672, w: 29, h: 32 }, { x: 70, y: 672, w: 29, h: 32 }, { x: 103, y: 672, w: 29, h: 32 }] },
  23: { name: "Ear Down", fps: 6, loop: false, frames: [{ x: 4, y: 704, w: 29, h: 32 }, { x: 37, y: 704, w: 29, h: 32 }] },
  24: { name: "Jump out of the Box", fps: 7, loop: false, frames: [{ x: 4, y: 736, w: 29, h: 30 }, { x: 37, y: 736, w: 29, h: 30 }, { x: 69, y: 736, w: 29, h: 30 }, { x: 102, y: 736, w: 27, h: 30 }, { x: 137, y: 736, w: 25, h: 30 }, { x: 168, y: 736, w: 27, h: 30 }, { x: 203, y: 736, w: 25, h: 30 }] },
};
