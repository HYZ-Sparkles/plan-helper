<script setup lang="ts">
/**
 * 桌宠动画调试页（工单 08 建立、16 改 codex 契约词汇表，dev-only）：
 * 浏览器直开 http://localhost:1420/#/dev/anim。
 * 用途：
 * - 契约回放终审：逐动作逐帧平铺（9 标准动作 + v2 环视 16 姿势），每帧时长可查
 * - 形象核对：9 个皮肤任选（v1/v2 标注），网格同构换装
 * - 位移验证：模拟 mover 驱动 96×104 小窗在假想屏幕内滑动，看方向判断与边界钳制
 * 契约时长硬性规定（animations.ts），无试拍调参管线（Oreo 的 NUDGE/帧序/权重已移除）。
 * 手动步进与引擎播放互斥（引擎 stop 后手动步进）。
 */
import { computed, reactive, ref, watch } from "vue";
import PetSprite from "../components/PetSprite.vue";
import { ANIMATIONS, LOOK_DIRECTIONS, PET_WIN, type PetAnim } from "../lib/pet/animations";
import { SKINS, DEFAULT_SKIN, skinCanGaze } from "../lib/pet/skins";
import { PetEngine, type EngineState, type PetMover } from "../lib/pet/engine";
import { PhCaretLeft, PhCaretRight } from "@phosphor-icons/vue";

/** 假想屏幕：640×300，96×104 的桌宠窗口可横向滑动 */
const SIM_AREA = { width: 640, height: 300 };
const SIM_WIN = PET_WIN;

const skinSlug = ref(DEFAULT_SKIN.slug);
const skin = computed(() => SKINS.find((s) => s.slug === skinSlug.value) ?? DEFAULT_SKIN);

const animKeys = Object.keys(ANIMATIONS) as PetAnim[];
const sel = ref<PetAnim>("idle");
const def = computed(() => ANIMATIONS[sel.value]);
const lookSel = ref<number | null>(null); // 非空 = 平铺选中环视姿势（覆盖动作区）
/** 环视区段只对有环视能力的形象展开（v1 无行；个别 v2 形象被显式忽略——2026-09-14 验收） */
const showLook = computed(() => skinCanGaze(skin.value));

/** 帧累计时间轴读数（如 "0 / 280 / 390 ms"）：契约时长可查 */
const timeline = computed(() => def.value.cum.slice(0, -1));

/** 手动帧状态（引擎停止时生效） */
const manual = reactive<{ anim: PetAnim; frame: number }>({ anim: "idle", frame: 0 });
const playing = ref(false);
const engineState = reactive<EngineState>({ anim: "idle", frame: 0, locked: false, busy: false, flick: 0 });

/** 模拟 mover：预览盒位置用 ref 暴露给模板 */
const simPos = ref({ x: (SIM_AREA.width - SIM_WIN.width) / 2, y: SIM_AREA.height - SIM_WIN.height - 24 });
const simMover: PetMover = {
  position: () => ({ ...simPos.value }),
  size: () => ({ width: SIM_WIN.width, height: SIM_WIN.height }),
  workArea: () => ({ x: 0, y: 0, ...SIM_AREA }),
  scaleFactor: () => 1,
  moveTo(x, y) {
    simPos.value = {
      x: Math.min(Math.max(Math.round(x), 0), SIM_AREA.width - SIM_WIN.width),
      y: Math.min(Math.max(Math.round(y), 0), SIM_AREA.height - SIM_WIN.height),
    };
  },
};

const engine = new PetEngine(simMover);
engine.subscribe((s) => Object.assign(engineState, s));

const view = computed(() =>
  playing.value && lookSel.value == null ? engineState : { ...manual },
);

watch(sel, () => stepFrame(0));

/** 播放当前动作（引擎计时 + 位移） */
function play(move = false) {
  lookSel.value = null;
  simPos.value = { x: (SIM_AREA.width - SIM_WIN.width) / 2, y: SIM_AREA.height - SIM_WIN.height - 24 };
  engine.request({
    lock: false,
    steps: [
      {
        anim: sel.value,
        movement: move ? { distance: moveDistance.value, direction: moveDir.value } : undefined,
      },
    ],
  });
  playing.value = true;
}

function stop() {
  if (!playing.value) return;
  manual.anim = engineState.anim;
  manual.frame = engineState.frame;
  engine.stop();
  playing.value = false;
}

/** 逐帧步进（暂停态） */
function stepFrame(delta: number) {
  stop();
  manual.anim = sel.value;
  manual.frame = (manual.frame + delta + def.value.cols) % def.value.cols;
  lookSel.value = null;
}

/** 平铺格点选：单看某帧 */
function showFrame(f: number) {
  stop();
  manual.anim = sel.value;
  manual.frame = f;
  lookSel.value = null;
}

/** 环视姿势点选：单看某方向（静态姿势，不走引擎） */
function showLookPose(d: number) {
  stop();
  lookSel.value = d;
}

const moveDistance = ref(200);
const moveDir = ref<"auto" | "left" | "right">("auto");

/** 动作行的中文名（调试页速查；语义详注在 animations.ts / ADR-0010） */
const ANIM_LABELS: Record<PetAnim, string> = {
  idle: "待机（休息常驻）",
  "running-right": "向右跑（拖拽反馈/自主移动）",
  "running-left": "向左跑（拖拽反馈/自主移动）",
  waving: "挥手（启动）",
  jumping: "跳跃（随机/庆祝/拖起）",
  failed: "失败（总结未达标）",
  waiting: "等待（大面板打开）",
  running: "奔跑（工作常驻）",
  review: "审查（推进汇报反馈）",
};
</script>

<template>
  <div class="page">
    <header class="bar">
      <h1>桌宠动画调试</h1>
      <span class="hint">codex 契约：8 列网格、逐帧时长硬性规定；帧残缺 = 形象资产问题</span>
    </header>

    <div class="layout">
      <aside class="list">
        <label class="skin-pick">
          形象
          <select v-model="skinSlug">
            <option v-for="s in SKINS" :key="s.slug" :value="s.slug">
              {{ s.name }}（v{{ s.spriteVersion }}）
            </option>
          </select>
        </label>
        <button
          v-for="k in animKeys"
          :key="k"
          class="item"
          :class="{ active: sel === k }"
          @click="sel = k"
        >
          <b>{{ ANIMATIONS[k].row }}</b> {{ k }}
          <i>{{ ANIMATIONS[k].cols }}f</i>
        </button>
      </aside>

      <section class="main">
        <!-- 舞台：地面线 + 居中形象 -->
        <div class="stage-wrap">
          <div class="stage">
            <div class="ground" />
            <PetSprite
              :anim="view.anim"
              :frame="view.frame"
              :sheet="skin.sheet"
              :look="lookSel"
            />
          </div>
        </div>

        <!-- 位移预览：假想屏幕 + 96×104 窗口盒 -->
        <div class="sim" :style="{ width: SIM_AREA.width + 'px', height: SIM_AREA.height + 'px' }">
          <div
            class="sim-win"
            :style="{ transform: `translateX(${simPos.x - (SIM_AREA.width - SIM_WIN.width) / 2}px)` }"
          >
            <PetSprite :anim="view.anim" :frame="view.frame" :sheet="skin.sheet" :look="lookSel" />
          </div>
          <span class="sim-label">模拟屏幕（位移方向/边界钳制预览）</span>
        </div>

        <div class="controls">
          <button class="ghost-btn" @click="playing ? stop() : play()">{{ playing ? "暂停" : "播放" }}</button>
          <button class="ghost-btn" :disabled="playing || lookSel != null" @click="stepFrame(-1)" title="上一帧">
            <PhCaretLeft :size="14" /> 帧
          </button>
          <button class="ghost-btn" :disabled="playing || lookSel != null" @click="stepFrame(1)" title="下一帧">
            帧 <PhCaretRight :size="14" />
          </button>
          <span class="meta">{{ ANIM_LABELS[sel] }}</span>
          <label class="ctl">位移 <input v-model.number="moveDistance" type="number" min="10" max="400" style="width: 64px" /> px</label>
          <select v-model="moveDir">
            <option value="auto">auto</option>
            <option value="left">left</option>
            <option value="right">right</option>
          </select>
          <button class="primary-btn" @click="play(true)">带位移播放</button>
        </div>

        <!-- 逐帧平铺：契约时长读数；点格子单看一帧 -->
        <div class="strip">
          <div
            v-for="f in def.cols"
            :key="f"
            class="strip-cell"
            :class="{ cur: lookSel == null && view.frame === f - 1 }"
            @click="showFrame(f - 1)"
          >
            <PetSprite :anim="sel" :frame="f - 1" :sheet="skin.sheet" />
            <span class="dur">{{ timeline[f - 1] }}ms</span>
            <span class="dur muted">+{{ def.durations[f - 1] }}</span>
          </div>
        </div>

        <!-- v2 环视 16 姿势（v1 无此行，区段隐藏） -->
        <template v-if="showLook">
          <div class="strip">
            <div
              v-for="d in LOOK_DIRECTIONS"
              :key="d"
              class="strip-cell"
              :class="{ cur: lookSel === d - 1 }"
              @click="showLookPose(d - 1)"
            >
              <PetSprite :anim="'idle'" :frame="0" :sheet="skin.sheet" :look="d - 1" />
              <span class="dur">{{ (d - 1) * 22.5 }}°</span>
            </div>
          </div>
        </template>
        <p v-else class="muted">该形象无环视能力（v1 无环视行，或 v2 转向被忽略）——16 向视线跟随自动关闭</p>
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
  grid-template-columns: 240px 1fr;
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

.skin-pick {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px 10px;
  font-size: 13px;
  color: var(--text-secondary);
  border-bottom: var(--border-default);
  margin-bottom: 4px;
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
  color: var(--text-muted);
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
  padding: 0 40px;
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
  left: calc(50% - 48px);
  bottom: 24px;
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

.meta {
  font-size: 12px;
  color: var(--text-secondary);
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
}

.strip-cell {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  cursor: pointer;
}

.strip-cell.cur {
  outline: 2px solid var(--primary);
  outline-offset: 2px;
}

.dur {
  font-size: 11px;
  color: var(--text-secondary);
}

.muted {
  color: var(--text-muted);
}
</style>
