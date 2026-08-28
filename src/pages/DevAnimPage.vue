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
import { ANIMATIONS } from "../lib/pet/animations";
import { PetEngine, type EngineState, type PetMover, type StepSpec } from "../lib/pet/engine";

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
const engineState = reactive<EngineState>({ anim: 0, frame: 0, flip: false, locked: false, busy: false });

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
            <PetSprite :anim="view.anim" :frame="view.frame" :flip="view.flip" />
          </div>
        </div>

        <!-- 位移预览：假想屏幕 + 64×64 窗口盒 -->
        <div class="sim" :style="{ width: SIM_AREA.width + 'px', height: SIM_AREA.height + 'px' }">
          <div class="sim-win" :style="{ transform: `translateX(${simPos.x - (SIM_AREA.width - SIM_WIN) / 2}px)` }">
            <PetSprite :anim="view.anim" :frame="view.frame" :flip="view.flip" />
          </div>
          <span class="sim-label">模拟屏幕（位移方向/边界钳制预览）</span>
        </div>

        <div class="controls">
          <button class="ghost-btn" @click="playing ? stop() : play()">{{ playing ? "暂停" : "播放" }}</button>
          <button class="ghost-btn" :disabled="playing" @click="stepFrame(-1)">◀ 帧</button>
          <button class="ghost-btn" :disabled="playing" @click="stepFrame(1)">帧 ▶</button>
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
        </div>

        <!-- 全帧平铺：切帧终审（每帧独立渲染，残缺/粘连一眼可见） -->
        <div class="strip" :style="{ transform: `scale(${zoom})`, transformOrigin: 'top left' }">
          <div v-for="(_, i) in def.frames" :key="i" class="strip-cell" :class="{ cur: view.frame === i }">
            <PetSprite :anim="sel" :frame="i" :flip="flip" />
            <span>{{ i + 1 }}</span>
          </div>
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
</style>
