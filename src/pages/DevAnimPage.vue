<script setup lang="ts">
/**
 * 桌宠动画调试页（工单 08，dev-only）：浏览器直开 http://localhost:1420/#/dev/anim。
 * 用途：
 * - 帧清单回放终审：若某帧出现"半只猫/两只猫粘连"说明切帧有误，改 scripts 规则重生成
 * - 节奏调参：fps 无资源元数据，在此试拍后把合意值写进 scripts/gen-pet-frames.mjs 的 META
 * - 位移验证：模拟 mover 驱动 64×64 小窗在假想屏幕内滑动，看方向判断与边界钳制
 * 手动步进与引擎播放互斥（引擎 stop 后手动步进）。
 */
import { computed, reactive, ref, watch } from "vue";
import PetSprite from "../components/PetSprite.vue";
import { ANIMATIONS, type FrameRect } from "../lib/pet/animations";
import { PetEngine, type EngineState, type PetMover, type StepSpec } from "../lib/pet/engine";
import {
  PhArrowCounterClockwise,
  PhArrowLeft,
  PhArrowRight,
  PhCaretDown,
  PhCaretLeft,
  PhCaretRight,
  PhCaretUp,
  PhMinus,
  PhPlus,
} from "@phosphor-icons/vue";

/** 假想屏幕：480×220，64×64 的桌宠窗口可横向滑动 */
const SIM_AREA = { width: 480, height: 220 };
const SIM_WIN = 64;

const sel = ref(7);
const def = computed(() => ANIMATIONS[sel.value]);
const fps = ref(def.value.fps);
const flip = ref(false);
const loop = ref(def.value.loop);
const zoom = ref(1); // 1/2/4 → 实际 2×/4×/8×
watch(sel, () => {
  fps.value = def.value.fps;
  loop.value = def.value.loop;
  stepFrame(0);
  stop();
});

/** 手动帧状态（引擎停止时生效） */
const manual = reactive({ anim: 7, frame: 0, flip: false });
const playing = ref(false);
const engineState = reactive<EngineState>({ anim: 0, frame: 0, flip: false, locked: false, busy: false, flick: 0 });

/** 模拟 mover：预览盒位置用 ref 暴露给模板 */
const simPos = ref({ x: (SIM_AREA.width - SIM_WIN) / 2, y: SIM_AREA.height - SIM_WIN - 24 });
const simMover: PetMover = {
  position: () => ({ ...simPos.value }),
  size: () => ({ width: SIM_WIN, height: SIM_WIN }),
  workArea: () => ({ x: 0, y: 0, ...SIM_AREA }),
  scaleFactor: () => 1,
  moveTo(x, y) {
    simPos.value = {
      x: Math.min(Math.max(Math.round(x), 0), SIM_AREA.width - SIM_WIN),
      y: Math.min(Math.max(Math.round(y), 0), SIM_AREA.height - SIM_WIN),
    };
  },
};

const engine = new PetEngine(simMover);
engine.subscribe((s) => Object.assign(engineState, s));

/** 引擎正在播的步（fps/flip 引用同对象，滑杆实时生效） */
let liveStep: StepSpec | null = null;
watch(fps, (v) => {
  if (liveStep) liveStep.fps = v;
});
watch(flip, (v) => {
  if (liveStep) liveStep.flip = v;
});

const view = computed(() => (playing.value ? engineState : { ...manual }));

/** 播放当前动画（引擎计时 + 位移） */
function play(move = false) {
  liveStep = {
    anim: sel.value,
    loop: loop.value,
    fps: fps.value,
    flip: flip.value,
    movement: move ? { distance: moveDistance.value, direction: moveDir.value } : undefined,
  };
  simPos.value = { x: (SIM_AREA.width - SIM_WIN) / 2, y: SIM_AREA.height - SIM_WIN - 24 };
  engine.request({ lock: false, steps: [liveStep] });
  playing.value = true;
}

function stop() {
  if (!playing.value) return;
  manual.anim = engineState.anim;
  manual.frame = engineState.frame;
  manual.flip = engineState.flip;
  engine.stop();
  playing.value = false;
}

/** 逐帧步进（暂停态） */
function stepFrame(delta: number) {
  const n = def.value.frames.length;
  manual.anim = sel.value;
  manual.frame = ((manual.anim === sel.value ? manual.frame : 0) + delta + n) % n;
  manual.flip = flip.value;
}

const moveDistance = ref(200);
const moveDir = ref<"auto" | "left" | "right">("auto");

/* ---- 帧序 / 位移权重 / 摆放微调（验收机制）----
 * 三张草稿都只在本页生效（同 fps 滑杆不持久化）；落盘 = 页底草稿粘进生成脚本对应表重跑。
 * 帧序与权重直接改写 def（引擎每 tick 取 def，播放中立即可见）；微调走 nudge prop。 */

/** 摆放微调草稿：anim → (0-based 播放位) → {ox, oy}（素材像素，正 = 右 / 下） */
const nudges = ref<Record<number, Record<number, { ox: number; oy: number }>>>({});

/** 帧序草稿：anim → 播放顺序（元素 = 素材从左数第几帧，1-based）；缺省 = 素材原序 */
const orders = ref<Record<number, number[]>>({});

/** 权重草稿：anim → 素材帧号(1-based) → 位移权重（默认 1，按素材帧号存——换序时权重跟着帧走） */
const weights = ref<Record<number, Record<number, number>>>({});

/** 首次改动某动画时快照素材原始帧序（重排与复位的基准） */
const origFrames = new Map<number, FrameRect[]>();

const nudgeOf = (anim: number, frame: number) => nudges.value[anim]?.[frame];

/** 当前播放位序列（元素 = 素材帧号 1-based）：平铺格即按此序排布 */
const matSeqOf = (anim: number): number[] =>
  orders.value[anim] ?? ANIMATIONS[anim].frames.map((_, i) => i + 1);

function nudge(anim: number, frame: number, dx: number, dy: number) {
  const per = (nudges.value[anim] ??= {});
  const cur = per[frame] ?? { ox: 0, oy: 0 };
  per[frame] = { ox: cur.ox + dx, oy: cur.oy + dy };
}

/** 权重按当前播放序写入 def.moveWeights（引擎取步时读取；全默认不注入） */
function applyWeights(anim: number) {
  const per = weights.value[anim];
  const arr = per && Object.keys(per).length ? matSeqOf(anim).map((m) => per[m] ?? 1) : null;
  ANIMATIONS[anim].moveWeights = arr && arr.some((v) => v !== 1) ? arr : undefined;
}

/** 重排 def.frames（切分矩形不动只换序）并按新序重挂权重数组 */
function applyOrder(anim: number) {
  const orig = origFrames.get(anim)!;
  ANIMATIONS[anim].frames = orders.value[anim].map((i) => orig[i - 1]);
  applyWeights(anim);
}

function moveFrame(anim: number, pos: number, d: -1 | 1) {
  const seq = [...matSeqOf(anim)];
  const t = pos + d;
  if (t < 0 || t >= seq.length) return;
  if (!origFrames.has(anim)) origFrames.set(anim, [...ANIMATIONS[anim].frames]);
  [seq[pos], seq[t]] = [seq[t], seq[pos]];
  orders.value[anim] = seq;
  applyOrder(anim);
}

function resetOrder(anim: number) {
  const orig = origFrames.get(anim);
  if (!orig) return;
  ANIMATIONS[anim].frames = [...orig];
  delete orders.value[anim];
  applyWeights(anim);
}

const weightOf = (anim: number, mat: number) => weights.value[anim]?.[mat] ?? 1;

function bumpWeight(anim: number, mat: number, d: number) {
  const per = (weights.value[anim] ??= {});
  const v = Math.max(0, (per[mat] ?? 1) + d);
  if (v === 1) delete per[mat];
  else per[mat] = v;
  if (!Object.keys(per).length) delete weights.value[anim];
  applyWeights(anim);
}

function resetWeights(anim: number) {
  delete weights.value[anim];
  applyWeights(anim);
}

/** 本帧恢复默认（摆放微调 + 权重；帧序是队列级操作，整条恢复用「恢复原序」） */
function resetFrame(anim: number, pos: number, mat: number) {
  if (nudges.value[anim]) delete nudges.value[anim][pos];
  if (weights.value[anim]) {
    delete weights.value[anim][mat];
    if (!Object.keys(weights.value[anim]).length) delete weights.value[anim];
  }
  applyWeights(anim);
}

/** 清空草稿并还原本页对 ANIMATIONS 的一切临时改动 */
function clearDraft() {
  const anims = new Set([...origFrames.keys(), ...Object.keys(weights.value).map(Number)]);
  for (const a of anims) {
    if (origFrames.has(a)) ANIMATIONS[a].frames = [...origFrames.get(a)!];
    ANIMATIONS[a].moveWeights = undefined;
  }
  orders.value = {};
  weights.value = {};
  nudges.value = {};
}

/** 微调读数文案（如 "x+1 y-2"，全 0 省略） */
const signed = (v: number) => (v > 0 ? `+${v}` : `${v}`);
const nudgeLabel = (n?: { ox: number; oy: number }) =>
  !n || (!n.ox && !n.oy) ? "" : `x${signed(n.ox)} y${signed(n.oy)}`;

/** 草稿 → 可直接粘进 scripts/gen-pet-frames.mjs 的片段（帧号/位次 1-based，同平铺显示） */
const draftSnippet = computed(() => {
  const orderLines: string[] = [];
  for (const [a, seq] of Object.entries(orders.value))
    if (seq.some((m, i) => m !== i + 1)) orderLines.push(`  ${a}: [${seq.join(", ")}],`);
  const weightLines: string[] = [];
  for (const [a, per] of Object.entries(weights.value)) {
    const arr = matSeqOf(+a).map((m) => per[m] ?? 1);
    if (arr.some((v) => v !== 1)) weightLines.push(`  ${a}: [${arr.join(", ")}],`);
  }
  const nudgeLines: string[] = [];
  for (const [a, per] of Object.entries(nudges.value)) {
    const frames = Object.entries(per)
      .filter(([, v]) => v.ox || v.oy)
      .map(([f, v]) => `    ${Number(f) + 1}: [${v.ox}, ${v.oy}],`)
      .join("\n");
    if (frames) nudgeLines.push(`  ${a}: {\n${frames}\n  },`);
  }
  return [
    orderLines.length && `const FRAME_ORDER = {\n${orderLines.join("\n")}\n};`,
    weightLines.length && `const MOVE_WEIGHTS = {\n${weightLines.join("\n")}\n};`,
    nudgeLines.length && `const NUDGE = {\n${nudgeLines.join("\n")}\n};`,
  ]
    .filter(Boolean)
    .join("\n\n");
});

async function copySnippet() {
  await navigator.clipboard.writeText(draftSnippet.value);
}
</script>

<template>
  <div class="page">
    <header class="bar">
      <h1>桌宠动画调试</h1>
      <span class="hint">帧有残缺/粘连 → 改 scripts/gen-pet-frames.mjs 重生成；fps 合意 → 写回 META</span>
    </header>

    <div class="layout">
      <aside class="list">
        <button
          v-for="(d, n) in ANIMATIONS"
          :key="n"
          class="item"
          :class="{ active: sel === +n }"
          @click="sel = +n"
        >
          <b>{{ n }}</b> {{ d.name }}
          <i>{{ d.frames.length }}f</i>
        </button>
      </aside>

      <section class="main">
        <!-- 舞台：地面线 + 居中猫（zoom 时 transform 缩放，底部锚定不变） -->
        <div class="stage-wrap">
          <div class="stage" :style="{ transform: `scale(${zoom})`, transformOrigin: 'bottom center' }">
            <div class="ground" />
            <PetSprite :anim="view.anim" :frame="view.frame" :flip="view.flip" :nudge="nudgeOf(view.anim, view.frame)" />
          </div>
        </div>

        <!-- 位移预览：假想屏幕 + 64×64 窗口盒 -->
        <div class="sim" :style="{ width: SIM_AREA.width + 'px', height: SIM_AREA.height + 'px' }">
          <div class="sim-win" :style="{ transform: `translateX(${simPos.x - (SIM_AREA.width - SIM_WIN) / 2}px)` }">
            <PetSprite :anim="view.anim" :frame="view.frame" :flip="view.flip" :nudge="nudgeOf(view.anim, view.frame)" />
          </div>
          <span class="sim-label">模拟屏幕（位移方向/边界钳制预览）</span>
        </div>

        <div class="controls">
          <button class="ghost-btn" @click="playing ? stop() : play()">{{ playing ? "暂停" : "播放" }}</button>
          <button class="ghost-btn" :disabled="playing" @click="stepFrame(-1)" title="上一帧">
            <PhCaretLeft :size="14" /> 帧
          </button>
          <button class="ghost-btn" :disabled="playing" @click="stepFrame(1)" title="下一帧">
            帧 <PhCaretRight :size="14" />
          </button>
          <label class="ctl">fps <input v-model.number="fps" type="range" min="1" max="24" /> {{ fps }}</label>
          <label class="ctl"><input v-model="loop" type="checkbox" /> 循环</label>
          <label class="ctl"><input v-model="flip" type="checkbox" /> 翻转</label>
          <label class="ctl">缩放
            <select v-model.number="zoom">
              <option :value="1">2×</option>
              <option :value="2">4×</option>
              <option :value="4">8×</option>
            </select>
          </label>
          <label class="ctl">位移 <input v-model.number="moveDistance" type="number" min="10" max="400" style="width: 64px" /> px</label>
          <select v-model="moveDir">
            <option value="auto">auto</option>
            <option value="left">left</option>
            <option value="right">right</option>
          </select>
          <button class="primary-btn" @click="play(true)">带位移播放</button>
          <button v-if="orders[sel]" class="ghost-btn" @click="resetOrder(sel)">恢复原序</button>
          <button v-if="weights[sel]" class="ghost-btn" @click="resetWeights(sel)">权重归一</button>
        </div>

        <!-- 全帧平铺：切帧终审 + 逐帧调整（上排 ◁▷ 播放位 / −+ 位移权重，下排 ◀▲▼▶ 摆放微调；草稿页底导出） -->
        <div class="strip" :style="{ transform: `scale(${zoom})`, transformOrigin: 'top left' }">
          <div v-for="(mat, pos) in matSeqOf(sel)" :key="mat" class="strip-cell" :class="{ cur: view.frame === pos }">
            <PetSprite :anim="sel" :frame="pos" :flip="flip" :nudge="nudgeOf(sel, pos)" />
            <div class="nudge-ctl" :title="`播放位 ${pos + 1}（素材第 ${mat} 帧）`">
              <button :disabled="pos === 0" @click="moveFrame(sel, pos, -1)" title="播放位前移"><PhArrowLeft :size="10" /></button>
              <button :disabled="pos === matSeqOf(sel).length - 1" @click="moveFrame(sel, pos, 1)" title="播放位后移"><PhArrowRight :size="10" /></button>
              <button @click="bumpWeight(sel, mat, -0.5)" title="位移权重 -0.5（每帧行程占比，带位移播放生效）"><PhMinus :size="10" /></button>
              <span class="w-readout" :class="{ set: weightOf(sel, mat) !== 1 }">{{ weightOf(sel, mat) }}</span>
              <button @click="bumpWeight(sel, mat, 0.5)" title="位移权重 +0.5（每帧行程占比，带位移播放生效）"><PhPlus :size="10" /></button>
            </div>
            <div class="nudge-ctl" :title="`第 ${pos + 1} 帧摆放微调（素材像素）`">
              <button @click="nudge(sel, pos, -1, 0)" title="左 1px"><PhCaretLeft :size="10" /></button>
              <button @click="nudge(sel, pos, 0, -1)" title="上 1px"><PhCaretUp :size="10" /></button>
              <button @click="nudge(sel, pos, 0, 1)" title="下 1px"><PhCaretDown :size="10" /></button>
              <button @click="nudge(sel, pos, 1, 0)" title="右 1px"><PhCaretRight :size="10" /></button>
              <button
                v-if="nudgeLabel(nudgeOf(sel, pos)) || weightOf(sel, mat) !== 1"
                @click="resetFrame(sel, pos, mat)"
                title="本帧恢复默认（微调 + 权重）"
              >
                <PhArrowCounterClockwise :size="10" />
              </button>
            </div>
            <span class="nudge-readout">{{ nudgeLabel(nudgeOf(sel, pos)) || pos + 1 }}</span>
          </div>
        </div>

        <!-- 调整草稿导出：粘进 scripts/gen-pet-frames.mjs 对应表（FRAME_ORDER / MOVE_WEIGHTS / NUDGE）重跑生成即落盘 -->
        <div v-if="draftSnippet" class="nudge-export">
          <div class="nudge-export-head">
            <span>调整草稿（粘进 scripts/gen-pet-frames.mjs 对应表，重跑 <code>node scripts/gen-pet-frames.mjs</code> 生效）</span>
            <button class="ghost-btn" @click="copySnippet">复制</button>
            <button class="ghost-btn" @click="clearDraft">清空草稿</button>
          </div>
          <pre>{{ draftSnippet }}</pre>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.page {
  max-width: 1080px;
  margin: 0 auto;
  padding: 24px;
  font-family: inherit;
}

.bar {
  display: flex;
  align-items: baseline;
  gap: 16px;
  margin-bottom: 16px;
}

.bar h1 {
  font-size: 18px;
  margin: 0;
}

.layout {
  display: grid;
  grid-template-columns: 220px 1fr;
  gap: 16px;
  align-items: start;
}

.list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  max-height: 70vh;
  overflow: auto;
  border: var(--border-default);
  border-radius: var(--radius-md);
  padding: 6px;
}

.item {
  display: flex;
  gap: 8px;
  align-items: baseline;
  padding: 6px 10px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  font-size: 13px;
  cursor: pointer;
  text-align: left;
  color: var(--text-primary);
}

.item b {
  color: var(--text-secondary);
  width: 2ch;
}

.item i {
  margin-left: auto;
  color: var(--text-muted);
  font-style: normal;
  font-size: 12px;
}

.item.active {
  background: var(--bg-accent-group);
  outline: 1px solid var(--primary);
}

.stage-wrap {
  height: 220px;
  display: flex;
  align-items: flex-end;
  justify-content: center;
  border: var(--border-default);
  border-radius: var(--radius-md);
  background: var(--bg-group);
  overflow: hidden;
}

.stage {
  display: flex;
  align-items: flex-end;
  justify-content: center;
  position: relative;
  width: 200px;
  height: 128px;
}

.ground {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  border-bottom: 2px dashed var(--text-muted);
}

.sim {
  position: relative;
  margin-top: 12px;
  border: var(--border-default);
  border-radius: var(--radius-sm);
  background: repeating-linear-gradient(45deg, transparent 0 12px, var(--bg-group) 12px 24px);
  overflow: hidden;
}

.sim-win {
  position: absolute;
  left: calc(50% - 32px);
  bottom: 24px;
  width: 64px;
  height: 64px;
  display: flex;
  align-items: flex-end;
  justify-content: center;
  border: 1px dashed var(--text-secondary);
  background: var(--surface); /* 模拟窗口用实底（token 禁硬编码色值） */
}

.sim-label {
  position: absolute;
  top: 6px;
  left: 8px;
  font-size: 11px;
  color: var(--text-muted);
}

.controls {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
  margin-top: 12px;
}

.ctl {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  color: var(--text-secondary);
}

.strip {
  display: flex;
  gap: 12px;
  margin-top: 16px;
  padding: 12px;
  border: var(--border-default);
  border-radius: var(--radius-md);
  overflow: auto;
  transform-origin: top left;
}

.strip-cell {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
}

.strip-cell.cur {
  outline: 2px solid var(--primary);
  outline-offset: 2px;
}

.strip-cell span {
  font-size: 11px;
  color: var(--text-muted);
}

.nudge-ctl {
  display: flex;
  gap: 2px;
}

.nudge-ctl button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 14px;
  padding: 0;
  border: var(--border-default);
  background: var(--surface);
  color: var(--text-secondary);
  cursor: pointer;
}

.nudge-ctl button:hover:not(:disabled) {
  border-color: var(--border-active);
  color: var(--text-primary);
}

.nudge-ctl button:disabled {
  opacity: 0.35;
  cursor: default;
}

.w-readout {
  min-width: 22px;
  text-align: center;
  font-size: 10px;
  line-height: 14px;
  color: var(--text-muted);
}

.w-readout.set {
  color: var(--primary);
}

.nudge-readout {
  font-size: 10px;
  color: var(--primary);
  min-height: 12px;
}

.nudge-export {
  margin-top: 12px;
  border: var(--border-active);
  border-radius: var(--radius-sm);
  padding: 10px 12px;
  background: var(--bg-accent-group);
}

.nudge-export-head {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--text-secondary);
}

.nudge-export pre {
  margin: 8px 0 0;
  font-size: 12px;
  color: var(--text-primary);
  overflow: auto;
}
</style>
