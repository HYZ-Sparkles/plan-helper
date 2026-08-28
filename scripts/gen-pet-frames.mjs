/**
 * 桌宠帧清单生成器（工单 08）：从 resourses 的 Oreo Cat sprite sheet 程序化切帧。
 *
 * 用法：
 *   node scripts/gen-pet-frames.mjs                 # 生成 src/lib/pet/animations.ts
 *   node scripts/gen-pet-frames.mjs --inspect DIR   # 额外导出每行条带与每帧裁剪 PNG（人工核对用）
 *
 * 原理：sprite sheet 为 24 行 × 32px 行高的帧条（462×766，末行被裁 2px）；
 * 帧与帧之间有透明间隙 → 按"非透明列段"检测帧边界；段内再按内容裁剪。
 * 资源无时序元数据，fps 为实现侧默认值（dev 调试页可实时调）。
 */
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { deflateSync } from "node:zlib";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const SHEET = join(ROOT, "resourses", "Oreo Cat - Aichan_owo", "Sprite Sheet Cat - Aichan_owo.png");
const OUT = join(ROOT, "src", "lib", "pet", "animations.ts");
const ROW_H = 32;

/** 动画元数据：名称（作者标注）+ 默认 fps + 是否循环。fps 无资源依据，为合理默认值 */
const META = {
  1: ["Stand to Sit", 7, false],
  2: ["Sit Idle", 4, true],
  3: ["Sit to Stand", 7, false],
  4: ["Stand to Sleep", 7, false],
  5: ["Sleep Idle", 4, true],
  6: ["Sleep to Stand", 7, false],
  7: ["Stand Idle", 4, true],
  8: ["Eat", 6, false],
  9: ["Walk", 7, false],
  10: ["Run", 7, false],
  11: ["Prepare Stealth", 7, false],
  12: ["Stealth", 5, true],
  13: ["Cancel Stealth", 7, false],
  14: ["Jump", 7, false],
  15: ["Attack", 7, false],
  16: ["Loop Attack", 7, true],
  17: ["Jump in to the Box", 7, false],
  18: ["Push Hand Up", 7, false],
  19: ["Play Box", 7, false],
  20: ["Push Hand Down", 7, false],
  21: ["Ear Up", 6, false],
  22: ["Scan", 6, false],
  23: ["Ear Down", 6, false],
  24: ["Jump out of the Box", 7, false],
};

/** 人工核对后的帧边界覆写（行号 → 每帧内容的 x 起止列表）；空 = 纯自动检测 */
const FRAME_OVERRIDES = {};

/** 帧摆放微调（验收沟通机制）：动画编号 → { 帧号(1-based，同调试页显示): [ox, oy] }。
 *  单位素材像素（×2 显示后 1 = 屏幕 2px），正方向 ox=右、oy=下。只调摆放不动切分。
 *  在 /dev/anim 调试页逐帧步进拨好，把页面底部导出的草稿片段粘进来，重跑本脚本生效。 */
const NUDGE = {
  // 例：12: { 3: [0, 1] }, // 12 Stealth 第 3 帧往下 1 素材像素
};

/** 帧序覆写（验收沟通机制）：动画编号 → 播放顺序（元素 = 素材从左数第几帧，1-based）。
 *  素材排帧顺序 ≠ 播放顺序时在此重排（切分矩形不动，只换播放序）；缺省 = 素材从左到右。
 *  在 /dev/anim 调试页平铺格 ◁▷ 拨好，把页面底部导出的草稿片段粘进来，重跑本脚本生效。 */
const FRAME_ORDER = {
  // 例：10: [3, 1, 2, 4], // 10 Run：素材第 3 帧先播
};

/** 每帧位移权重（验收沟通机制）：动画编号 → 与帧数等长数组（按播放顺序）。
 *  运动类动作某帧该多走/少走时用：第 i 帧期间走过的距离占比 = w[i]/Σw；
 *  缺省/未列 = 全 1（均匀，现状）。调试页逐帧 ± 步进拨好，草稿粘进来重跑生效。 */
const MOVE_WEIGHTS = {
  // 例：10: [0.5, 1, 1, 2], // 10 Run：起步半速、第 4 帧冲刺两倍
};

// ---- PNG 解析（无依赖：IHDR + IDAT inflate + unfilter） ----

/** 解析 PNG 为 { width, height, rgba: Buffer }（仅支持 8bit RGBA，本资源即此格式） */
function parsePng(buf) {
  const width = buf.readUInt32BE(16);
  const height = buf.readUInt32BE(20);
  if (buf[24] !== 8 || buf[25] !== 6) throw new Error("仅支持 8bit RGBA PNG");
  const chunks = [];
  let off = 8;
  while (off < buf.length) {
    const len = buf.readUInt32BE(off);
    const type = buf.toString("ascii", off + 4, off + 8);
    if (type === "IDAT") chunks.push(buf.subarray(off + 8, off + 8 + len));
    off += 12 + len;
  }
  const raw = deflateInflate(chunks);
  const bpp = 4;
  const stride = width * bpp;
  const out = Buffer.alloc(height * stride);
  let p = 0;
  for (let y = 0; y < height; y++) {
    const filter = raw[p++];
    const row = y * stride;
    for (let x = 0; x < stride; x++) {
      const cur = raw[p++];
      const left = x >= bpp ? out[row + x - bpp] : 0;
      const up = y > 0 ? out[row - stride + x] : 0;
      const ul = y > 0 && x >= bpp ? out[row - stride + x - bpp] : 0;
      let v;
      if (filter === 0) v = cur;
      else if (filter === 1) v = cur + left;
      else if (filter === 2) v = cur + up;
      else if (filter === 3) v = cur + ((left + up) >> 1);
      else if (filter === 4) {
        const pp = left + up - ul;
        const da = Math.abs(pp - left), db = Math.abs(pp - up), dc = Math.abs(pp - ul);
        v = cur + (da <= db && da <= dc ? left : db <= dc ? up : ul);
      } else throw new Error(`未知 filter ${filter}`);
      out[row + x] = v & 0xff;
    }
  }
  return { width, height, rgba: out };
}

const inflateSync = (await import("node:zlib")).inflateSync;
function deflateInflate(chunks) {
  return inflateSync(Buffer.concat(chunks));
}

// ---- 帧边界检测 ----
// 结构规律（对 sheet 像素实测）：帧与帧之间透明间隙 ≥4px；帧内容内部可能有 ≤3px 的
// 断裂（抬起的前爪与身体之间出现整列透明）；两帧粘连时表现为 >36px 的宽段（单帧内容
// 最宽 32px）。据此三条规则切分，不再依赖人工数帧。
const FRAME_GAP = 4; // ≥ 此间隙 = 帧边界
const MAX_FRAME_W = 34; // 合并吸附的宽度上限（防把两张贴着的姿势并成一张）
const SPLIT_W = 36; // 超过此宽度 = 粘连帧，需谷底分割

/** 一行里非透明列的连续段（任意宽度），返回 [x0, x1) 左闭右开 */
function columnSegments(rgba, width, y0, y1) {
  const segs = [];
  let s = -1;
  for (let x = 0; x <= width; x++) {
    let on = false;
    if (x < width) {
      for (let y = y0; y < y1 && !on; y++) on = rgba[(y * width + x) * 4 + 3] > 8;
    }
    if (on && s < 0) s = x;
    if (!on && s >= 0) {
      segs.push([s, x]);
      s = -1;
    }
  }
  return segs;
}

/** 相邻段吸附：间隙 <FRAME_GAP 且合并后不超宽 → 同一帧（碎块归属）；返回帧级 [x0,x1) 列表 */
function mergeIntoFrames(segs) {
  return segs.reduce((acc, seg) => {
    const prev = acc[acc.length - 1];
    if (prev && seg[0] - prev[1] < FRAME_GAP && seg[1] - prev[0] <= MAX_FRAME_W) prev[1] = seg[1];
    else acc.push([...seg]);
    return acc;
  }, []);
}

/** 粘连帧分割：段宽 >SPLIT_W 时在内部"列覆盖数最低"处切开（两张贴着的姿势必有低谷） */
function splitWide(rgba, width, y0, y1, frames) {
  const out = [];
  for (const [x0, x1] of frames) {
    if (x1 - x0 <= SPLIT_W) {
      out.push([x0, x1]);
      continue;
    }
    const cover = [];
    for (let x = x0; x < x1; x++) {
      let n = 0;
      for (let y = y0; y < y1; y++) if (rgba[(y * width + x) * 4 + 3] > 8) n++;
      cover.push(n);
    }
    const v = cover.indexOf(Math.min(...cover));
    const cut = x0 + v + 1; // 低谷列之后切开
    out.push([x0, cut], [cut, x1]);
  }
  return out;
}

/** 全部 24 行的帧矩形：{ row, y0, baseline, frames: [{x, w}] }（y 全高裁剪保地面线，x 按内容） */
function sliceAll(rgba, width, height) {
  const rows = [];
  for (let n = 1; n <= 24; n++) {
    const y0 = (n - 1) * ROW_H;
    const y1 = Math.min(n * ROW_H, height);
    let spans = FRAME_OVERRIDES[n] ?? splitWide(rgba, width, y0, y1, mergeIntoFrames(columnSegments(rgba, width, y0, y1)));
    rows.push({ n, y0, h: y1 - y0, frames: spans.map(([a, b]) => ({ x: a, w: b - a })) });
  }
  return rows;
}

// ---- PNG 编码（filter 0，仅供 --inspect 导出核对图） ----

/** CRC32（PNG chunk 用） */
const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

function chunk(type, data) {
  const b = Buffer.alloc(12 + data.length);
  b.writeUInt32BE(data.length, 0);
  b.write(type, 4, "ascii");
  data.copy(b, 8);
  let c = 0xffffffff;
  for (const byte of Buffer.concat([Buffer.from(type, "ascii"), data])) c = CRC_TABLE[(c ^ byte) & 0xff] ^ (c >>> 8);
  b.writeUInt32BE((c ^ 0xffffffff) >>> 0, 8 + data.length);
  return b;
}

/** 把 RGBA 缓冲编码成 PNG（无压缩优化诉求，核对用） */
function encodePng(width, height, rgba) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8; ihdr[9] = 6; // 8bit RGBA
  const raw = Buffer.alloc((width * 4 + 1) * height);
  for (let y = 0; y < height; y++) {
    raw[y * (width * 4 + 1)] = 0; // filter 0
    rgba.copy(raw, y * (width * 4 + 1) + 1, y * width * 4, (y + 1) * width * 4);
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw)),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

// ---- 主流程 ----

const inspectDir = process.argv.includes("--inspect")
  ? process.argv[process.argv.indexOf("--inspect") + 1]
  : null;

const { width, height, rgba } = parsePng(readFileSync(SHEET));
const rows = sliceAll(rgba, width, height);

if (inspectDir) {
  mkdirSync(inspectDir, { recursive: true });
  for (const row of rows) {
    // 整行条带（含帧边界红线不可行——纯像素导出，核对靠裁剪图）
    const strip = crop(rgba, width, 0, row.y0, width, row.h);
    writeFileSync(join(inspectDir, `row-${String(row.n).padStart(2, "0")}-strip.png`), encodePng(width, row.h, strip));
    row.frames.forEach((f, i) => {
      const c = crop(rgba, width, f.x, row.y0, f.w, row.h);
      writeFileSync(
        join(inspectDir, `row-${String(row.n).padStart(2, "0")}-f${i + 1}.png`),
        encodePng(f.w, row.h, c),
      );
    });
  }
}

/** 从 rgba 里裁一块 */
function crop(rgba, fullW, x0, y0, w, h) {
  const out = Buffer.alloc(w * h * 4);
  for (let y = 0; y < h; y++) {
    rgba.copy(out, y * w * 4, (y0 + y) * fullW * 4 + x0 * 4, (y0 + y) * fullW * 4 + (x0 + w) * 4);
  }
  return out;
}

// 生成 animations.ts
const sheetUrl = "/pet/oreo-sheet.png";

/** 应用帧序覆写：返回按 FRAME_ORDER 重排后的帧矩形序列（NUDGE/MOVE_WEIGHTS 的帧号都指重排后的播放位） */
function applyOrder(n, frames) {
  const order = FRAME_ORDER[n];
  if (!order) return frames;
  if (order.length !== frames.length)
    throw new Error(`FRAME_ORDER[${n}] 长度 ${order.length} ≠ 切分帧数 ${frames.length}`);
  return order.map((i) => frames[i - 1]);
}

const lines = rows.map((row) => {
  const [name, fps, loop] = META[row.n];
  const ordered = applyOrder(row.n, row.frames);
  const frames = ordered
    .map((f, i) => {
      const n = NUDGE[row.n]?.[i + 1]; // 帧号 1-based = 播放位（同调试页显示）
      const nudge = n ? `, ox: ${n[0]}, oy: ${n[1]}` : "";
      return `{ x: ${f.x}, y: ${row.y0}, w: ${f.w}, h: ${row.h}${nudge} }`;
    })
    .join(", ");
  const w = MOVE_WEIGHTS[row.n];
  if (w && w.length !== ordered.length)
    throw new Error(`MOVE_WEIGHTS[${row.n}] 长度 ${w.length} ≠ 切分帧数 ${ordered.length}`);
  const weights = w ? `, moveWeights: [${w.join(", ")}]` : "";
  return `  ${row.n}: { name: ${JSON.stringify(name)}, fps: ${fps}, loop: ${loop}${weights}, frames: [${frames}] },`;
});

const ts = `/**
 * Oreo Cat 帧清单（由 scripts/gen-pet-frames.mjs 从 resourses/ 的 sprite sheet 程序化生成——勿手改，
 * 资产更新或帧边界修正后重跑脚本）。帧矩形为 sheet 像素坐标；行高 32、地面线 = 帧矩形底边，
 * 渲染时按底部锚定统一站高。fps 为默认值（资源无时序元数据），可在 /dev/anim 调试页试拍后改 META。
 */
export const PET_SHEET_URL = ${JSON.stringify(sheetUrl)};
export const SHEET_SCALE = 2;
export const SHEET_WIDTH = ${width};
export const SHEET_HEIGHT = ${height};

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
  /** 每帧位移权重（按 frames 播放顺序）：第 i 帧期间走过的距离占比 = w[i]/Σw；缺省 = 均匀。
   *  值来自生成脚本的 MOVE_WEIGHTS 表（运动类动画在 /dev/anim 调试页试出后写回） */
  moveWeights?: number[];
  frames: FrameRect[];
}

/** 24 个动画，键 = 作者标注的动画编号（README 动作映射引用的编号体系） */
export const ANIMATIONS: Record<number, AnimationDef> = {
${lines.join("\n")}
};
`;

mkdirSync(dirname(OUT), { recursive: true });
writeFileSync(OUT, ts);
console.log(`✓ ${rows.length} 行 → ${OUT}`);
for (const row of rows) {
  console.log(`  ${String(row.n).padStart(2)} ${META[row.n][0].padEnd(20)} ${row.frames.length} 帧  [${row.frames.map((f) => f.w).join(", ")}]`);
}
