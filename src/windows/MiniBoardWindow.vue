<template>
  <!--
    小看板（工单 07，术语 MiniBoard / CurrentTask / PercentAdjustControl / SubGoalUndo）。
    常驻桌宠旁的置顶卡片：头部计划名 + 更换任务；主体按任务形态切换——
    有子目标 = 按序勾选（已完成行可点撤销）、无子目标 = 百分比增量控件（会话级不持久化）；
    当前任务完成后停留"任务完成"态等用户换下一个，不自动切换；
    底部常驻今日总量微型进度条（今日推进 X / 目标 Y）。
    拖拽跟随归 09、休息模式显隐归 08；关闭请求拦截为隐藏（归 14）。
  -->
  <div class="board-card">
    <!-- 头部：所属计划 + 更换任务（图标优先，hover 有 tooltip） -->
    <header v-if="current" class="head">
      <p class="plan"><PhFlagBanner :size="13" /> {{ current.plan_name }}</p>
      <button
        type="button"
        class="icon-btn"
        title="更换任务"
        @click="picking = true"
      >
        <PhArrowsLeftRight :size="15" />
      </button>
    </header>

    <main class="body">
      <p v-if="loadError" class="error">{{ loadError }}</p>
      <p v-else-if="!view" class="hint loading">加载中…</p>

      <!-- 更换任务选择器：今日推进列表按计划分组，当前任务同计划排最前（服务端已排好）；
           组列表独立滚动，标题与“返回”钉在卡内 -->
      <div v-else-if="picking" class="picker">
        <p class="picker-title">从今日推进里挑一个</p>
        <div class="picker-list">
          <p v-if="view.pickers.length === 0" class="hint">
            今日推进列表里没有可换的任务——去大面板重新分配
          </p>
          <section v-for="g in view.pickers" :key="g.plan_id" class="picker-group">
            <p class="group-name">{{ g.plan_name }}</p>
            <button
              v-for="t in g.tasks"
              :key="t.id"
              type="button"
              class="picker-row"
              :class="{ chosen: t.id === current?.task_id }"
              :disabled="busy"
              @click="pick(t.id)"
            >
              <PhCircle v-if="t.id !== current?.task_id" :size="13" class="dot" />
              <PhCheckCircle v-else :size="13" class="dot done" />
              <span class="picker-name">{{ t.name }}</span>
            </button>
          </section>
        </div>
        <button
          v-if="current"
          type="button"
          class="link-btn picker-cancel"
          @click="picking = false"
        >
          返回
        </button>
      </div>

      <!-- 空态：未指定当前任务（或已失效回空） -->
      <div v-else-if="!current" class="state">
        <PhCircleDashed :size="30" />
        <p class="state-title">今天还没有当前任务</p>
        <p class="hint">从今日推进列表里挑一个开始</p>
        <button type="button" class="primary-btn" @click="picking = true">
          <PhCursorClick :size="15" /> 选择任务
        </button>
      </div>

      <!-- 停留态：当前任务已自动完成，等用户主动换下一个 -->
      <div v-else-if="current.status === 'Completed'" class="state">
        <PhCheckCircle :size="30" class="done-icon" />
        <p class="state-title">任务完成</p>
        <p class="hint">做得漂亮——换下一个，或先歇口气</p>
        <button type="button" class="ghost-btn" @click="picking = true">
          <PhArrowsLeftRight :size="15" /> 更换任务
        </button>
      </div>

      <!-- 进行中任务：任务行 + 进度细线 + 汇报区 -->
      <template v-else>
        <div class="task-line">
          <span class="task-name">{{ current.task_name }}</span>
          <!-- 百分比 + 耗时口径并列（ADR-0002：展示不跳过耗时映射） -->
          <span class="task-percent">
            {{ current.percent }}%
            <span class="percent-hours">
              {{ hoursFromMinutes(current.completed_minutes) }}/{{ hoursFromMinutes(current.total_minutes) }}h
            </span>
          </span>
        </div>
        <MicroBar class="task-bar" :ratio="current.percent / 100" :reached="current.percent >= 100" />

        <!-- 有子目标：更早的已完成项折叠为计数行，只展开最近一行（可撤销——撤销本来就
             只能从最后一项开始）；列表高度有界，不出现滚动条 -->
        <!-- 有子目标：<template v-if> 把 <ul> + “还有 N 项”一起包起来，保持 v-if/v-else
             与下面 percent-area 的完整分支语义（有子目标 / 无子目标）；若直接写
             “<ul v-if=has_subgoals> + <p v-if=remainingCount> + <div v-else>” 会让 v-else
             绑到 remainingCount 上、有子目标但只剩一项时仍误显 percent-area 推进框 -->
        <template v-if="current.has_subgoals">
          <ul class="subgoals">
            <li v-if="olderCompletedCount > 0" class="sg-fold">已完成 {{ olderCompletedCount }} 项</li>
            <li v-if="lastCompleted">
              <button
                type="button"
                class="sg-row done"
                title="撤销这个子目标的完成"
                :disabled="busy"
                @click="undoLast"
              >
                <PhCheckCircle :size="15" class="sg-icon done" />
                <span class="sg-name">{{ lastCompleted.name }}</span>
                <span class="sg-min">{{ lastCompleted.estimated_minutes }}m</span>
              </button>
            </li>
            <li v-if="pendingSubgoal">
              <button
                type="button"
                class="sg-row current"
                :disabled="busy"
                @click="complete(pendingSubgoal.id)"
              >
                <PhCircle :size="15" class="sg-icon" />
                <span class="sg-name">{{ pendingSubgoal.name }}</span>
                <span class="sg-min">{{ pendingSubgoal.estimated_minutes }}m</span>
              </button>
            </li>
          </ul>
          <p v-if="remainingCount > 0" class="hint remaining">
            还有 {{ remainingCount }} 项待推进
          </p>
        </template>

        <!-- 无子目标：PercentAdjustControl（[-] -10% / 点数字直填 5% 倍数 / [+] +10%）+ 增量推进 -->
        <div v-else class="percent-area">
          <p class="percent-label">本次增量</p>
          <div class="percent-control">
            <button
              type="button"
              class="icon-btn"
              title="-10%（最低 0.1%）"
              :disabled="busy"
              @click="step = clampStep(step - 10)"
            >
              <PhMinus :size="15" />
            </button>
            <input
              v-if="editingStep"
              ref="stepInput"
              v-model="stepDraft"
              class="input step-input"
              type="number"
              min="0.1"
              max="100"
              step="0.1"
              @keyup.enter="commitStep"
              @keyup.esc="editingStep = false"
              @blur="commitStep"
            />
            <button
              v-else
              type="button"
              class="step-num"
              title="点击直填增量"
              @click="startEditStep"
            >
              {{ step }}%
            </button>
            <button
              type="button"
              class="icon-btn"
              title="+10%（最高 100%）"
              :disabled="busy"
              @click="step = clampStep(step + 10)"
            >
              <PhPlus :size="15" />
            </button>
            <button
              type="button"
              class="primary-btn report-btn"
              :disabled="busy"
              @click="report"
            >
              推进 +{{ step }}%
            </button>
          </div>
        </div>
      </template>
    </main>

    <!-- 底部常驻：今日总量微型进度条（今日推进 X / 目标 Y；目标暂用基准，10 升级含结转） -->
    <footer class="foot">
      <p class="foot-line">
        今日
        <b>{{ hoursLabel(todayMinutes) }}</b>
        <span class="foot-sep">/</span>
        目标 {{ hoursLabel(targetMinutes) }} h
      </p>
      <MicroBar
        class="foot-bar"
        :ratio="todayMinutes / (targetMinutes || 1)"
        :reached="todayMinutes >= targetMinutes"
      />
    </footer>

    <!-- 轻提示（撤销重算等一次性反馈） -->
    <Transition name="toast">
      <p v-if="toast" class="toast">{{ toast }}</p>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  PhArrowsLeftRight,
  PhCheckCircle,
  PhCircle,
  PhCircleDashed,
  PhCursorClick,
  PhFlagBanner,
  PhMinus,
  PhPlus,
} from "@phosphor-icons/vue";
import MicroBar from "../components/MicroBar.vue";
import {
  completeSubgoal,
  getMiniBoard,
  reportPercent,
  setCurrentTask,
  undoSubgoal,
  type MiniBoardView,
} from "../lib/api";
import { hoursFromMinutes, hoursLabel, planErrorMessage } from "../lib/labels";
import { isValidPercentValue } from "../lib/validation";

const win = getCurrentWebviewWindow();
const view = ref<MiniBoardView | null>(null);
const loadError = ref("");
/** 一次性轻提示（撤销重算 / 操作失败），2.6s 自动消失 */
const toast = ref("");
let toastTimer: ReturnType<typeof setTimeout> | undefined;
/** 汇报/换任务进行中（防连点） */
const busy = ref(false);
/** 更换任务选择器展开中 */
const picking = ref(false);
/** 本次增量档位（PercentAdjustControl，会话级：默认 5%，切换任务重置） */
const step = ref(5);
/** 直填编辑态：stepDraft 为输入草稿，非法值提交时回退原档位 */
const editingStep = ref(false);
const stepDraft = ref("");
const stepInput = ref<HTMLInputElement>();

const current = computed(() => view.value?.current ?? null);
/** 底部今日总量（模板两处 + 进度条共用） */
const todayMinutes = computed(() => view.value?.today_minutes ?? 0);
const targetMinutes = computed(() => view.value?.target_minutes ?? 0);

/** 已完成子目标（折叠展示的数据源：只展开最近一行） */
const completedSubgoals = computed(() =>
  current.value?.subgoals.filter((s) => s.completed) ?? [],
);
/** 更早的已完成项折叠成计数行（面板高度有界、无滚动条；撤销也只对最后一行有意义） */
const olderCompletedCount = computed(() => Math.max(0, completedSubgoals.value.length - 1));
/** 最近一个已完成子目标 = 唯一可撤销项（服务层只允许从末尾撤销） */
const lastCompleted = computed(() => {
  const done = completedSubgoals.value;
  return done.length > 0 ? done[done.length - 1] : null;
});
/** 当前待完成子目标 = 第一个未完成（按序推进的唯一入口） */
const pendingSubgoal = computed(
  () => current.value?.subgoals.find((s) => !s.completed) ?? null,
);
const remainingCount = computed(() =>
  Math.max(0, (current.value?.subgoals.length ?? 0) - completedSubgoals.value.length - 1),
);

/** 切换任务：增量档位回到默认 5%，收起直填框（会话级不持久化） */
watch(
  () => current.value?.task_id,
  () => {
    step.value = 5;
    editingStep.value = false;
  },
);

/** 弹一条轻提示并 2.6s 后自动收起 */
function flash(message: string) {
  toast.value = message;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (toast.value = ""), 2600);
}

/** 拉取小看板视图（当前任务 + 今日总量 + 更换候选） */
async function load() {
  try {
    view.value = await getMiniBoard();
    loadError.value = "";
  } catch (err) {
    loadError.value = planErrorMessage(err as { kind?: string });
  }
}

/** 统一动作通道：busy 互斥 + 失败 toast，成功后重载（视图从日志派生，读到即最新） */
async function act(action: () => Promise<void>, note?: string) {
  if (busy.value) return;
  busy.value = true;
  try {
    await action();
    if (note) flash(note);
    await load();
  } catch (err) {
    flash(planErrorMessage(err as { kind?: string }));
  } finally {
    busy.value = false;
  }
}

const complete = (id: number) => act(() => completeSubgoal(id));

/** 撤销最近一个已完成子目标（toast 提示进度已重算） */
function undoLast() {
  const s = lastCompleted.value;
  if (s) act(() => undoSubgoal(s.id), `已撤销「${s.name}」的完成，进度已重算`);
}

const report = () => act(() => reportPercent(current.value!.task_id, step.value));

/** 选定更换任务：服务层校验今日列表归属，成功后收起选择器 */
async function pick(taskId: number) {
  await act(async () => {
    await setCurrentTask(taskId);
    picking.value = false;
  });
}

/** PercentAdjustControl 钳制（CONTEXT，2026-08-24 修订）：[-] 最低 0.1%、[+] 最高 100% */
const clampStep = (v: number) => Math.min(100, Math.max(0.1, v));

/** 点数字直填：进入编辑态并聚焦选中（任意正数 0.1–100、最多一位小数，非法回退原档位） */
function startEditStep() {
  stepDraft.value = String(step.value);
  editingStep.value = true;
  nextTick(() => stepInput.value?.select());
}

/** 直填提交：合法档位生效，非法（非正数 / 两位小数 / 越界）静默回退 */
function commitStep() {
  if (!editingStep.value) return;
  editingStep.value = false;
  const v = Number(stepDraft.value);
  if (isValidPercentValue(v, 0.1)) step.value = v;
}

onMounted(async () => {
  await load();
  // 大面板确认分配后刷新（MainBoard 确认时先发事件再 show 本窗口）
  await listen("mini-board:refresh", load);
  // 关闭 = 隐藏（CONTEXT 窗口关闭语义；完整语义归工单 14）
  await win.onCloseRequested(async (e) => {
    e.preventDefault();
    await win.hide();
  });
});
</script>

<style scoped>
.board-card {
  position: relative;
  display: flex;
  flex-direction: column;
  height: calc(100% - 16px);
  margin: 8px;
  /* 窗口缩至 240×260 后的紧凑档：横向留白收窄，保内容区可用宽度 */
  padding: 10px 12px 10px;
  background: var(--surface);
  border: var(--border-default);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg); /* 桌面浮层 */
  box-sizing: border-box;
  /* 容器内兜底裁切（外层 tokens.css body.transparent-root 已 overflow:hidden 切断 webview 滚动条） */
  overflow: hidden;
}

/* ---- 头部 ---- */
.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 4px;
}

.plan {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  margin: 0;
  color: var(--text-muted);
  font-size: 12px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ---- 主体 ---- */
.body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.loading {
  margin: auto;
}

.error {
  margin: auto;
  color: var(--color-danger);
  font-size: 13px;
}

/* 任务行 + 进度细线 */
.task-line {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 10px;
}

.task-name {
  font-weight: 600;
  font-size: 15px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-percent {
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
  font-size: 14px;
  flex-shrink: 0;
  white-space: nowrap;
}

/* 耗时口径为百分比的次级注脚（ADR-0002：耗时映射不可跳过） */
.percent-hours {
  margin-left: 4px;
  color: var(--text-muted);
  font-size: 12px;
}

.task-bar {
  margin: 6px 0 8px;
}

/* ---- 子目标：折叠计数行 + 最近已完成（可撤销）+ 当前待完成 ---- */
.subgoals {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

/* 更早已完成项的折叠计数（2026-08-24 验收修订：去滚动条、面板高度有界） */
.sg-fold {
  margin: 0;
  padding: 2px 10px;
  color: var(--text-muted);
  font-size: 12px;
}

.sg-row {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 5px 8px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.sg-row:hover:not(:disabled) {
  background: var(--bg-group);
}

.sg-row.current {
  background: var(--bg-accent-group);
  border: var(--border-active);
}

.sg-row .sg-icon {
  color: var(--text-muted);
  flex-shrink: 0;
}

.sg-row .sg-icon.done {
  color: var(--color-done);
}

.sg-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
}

.sg-row.done .sg-name {
  color: var(--text-muted);
  text-decoration: line-through;
}

.sg-min {
  color: var(--text-muted);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
}

.remaining {
  margin: 8px 0 0;
  font-size: 12px;
}

/* ---- 无子目标：PercentAdjustControl ---- */
.percent-area {
  margin-top: 2px;
}

.percent-label {
  margin: 0 0 8px;
  color: var(--text-muted);
  font-size: 12px;
}

.percent-control {
  display: flex;
  align-items: center;
  gap: 5px;
}

.step-num {
  min-width: 40px;
  padding: 5px 0;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-muted); /* 次级操作不突出（CONTEXT PercentAdjustControl） */
  font: inherit;
  font-size: 14px;
  font-variant-numeric: tabular-nums;
  text-align: center;
  cursor: pointer;
}

.step-num:hover {
  color: var(--text-primary);
}

.step-input {
  width: 48px;
  padding: 4px 6px;
  text-align: center;
  font-variant-numeric: tabular-nums;
}

.report-btn {
  margin-left: auto;
  padding: 6px 10px; /* 卡内紧凑档（全局类的形态不变，只缩了呼吸空间） */
  font-size: 12px;
  white-space: nowrap;
}

/* ---- 空态 / 停留态 ---- */
.state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  color: var(--text-muted);
}

.state-title {
  margin: 2px 0 0;
  color: var(--text-primary);
  font-weight: 600;
}

.state .hint {
  margin: 0 0 10px;
}

.done-icon {
  color: var(--color-done);
}

/* ---- 更换任务选择器 ---- */
.picker {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

/* 组列表滚动区（2026-08-28 验收修订：任务多时要能滚到，标题/返回不跟着滚） */
.picker-list {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
  overflow-y: auto;
  padding-right: 4px; /* 行与滚动条之间的呼吸空间 */
}

/* 细滚动条：透明浮层卡片里 WebView 默认粗条太突兀 */
.picker-list::-webkit-scrollbar {
  width: 6px;
}

.picker-list::-webkit-scrollbar-thumb {
  border-radius: var(--radius-sm);
  background: var(--text-muted);
}

.picker-list::-webkit-scrollbar-track {
  background: transparent;
}

.picker-title {
  margin: 0;
  color: var(--text-secondary);
  font-size: 13px;
}

.picker .hint {
  margin: 0;
}

.picker-group {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.group-name {
  margin: 0 0 2px;
  color: var(--text-muted);
  font-size: 12px;
}

.picker-row {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 5px 8px;
  border: var(--border-default);
  border-radius: var(--radius-sm);
  background: var(--surface);
  font: inherit;
  font-size: 13px;
  text-align: left;
  cursor: pointer;
}

.picker-row:hover:not(:disabled):not(.chosen) {
  border-color: var(--primary);
}

.picker-row.chosen {
  background: var(--bg-accent-group);
  border: var(--border-active);
  cursor: default;
}

.picker-row .dot {
  color: var(--text-muted);
  flex-shrink: 0;
}

.picker-row .dot.done {
  color: var(--color-done);
}

.picker-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.picker-cancel {
  align-self: center;
}

/* ---- 底部今日总量 ---- */
.foot {
  margin-top: 8px;
  padding-top: 8px;
  border-top: var(--border-default);
}

.foot-line {
  margin: 0 0 6px;
  color: var(--text-secondary);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}

.foot-line b {
  color: var(--text-primary);
  font-weight: 600;
}

.foot-sep {
  margin: 0 2px;
  color: var(--text-muted);
}

/* ---- 轻提示（toast） ---- */
.toast {
  position: absolute;
  left: 50%;
  bottom: 54px;
  transform: translateX(-50%);
  margin: 0;
  padding: 6px 14px;
  border-radius: var(--radius-md);
  background: var(--text-primary); /* 深底浅字，浮于卡片之上 */
  color: var(--bg-base);
  font-size: 12px;
  white-space: nowrap;
  max-width: calc(100% - 24px);
  overflow: hidden;
  text-overflow: ellipsis;
  z-index: 10;
  pointer-events: none;
}

.toast-enter-active,
.toast-leave-active {
  transition: opacity 0.2s;
}

.toast-enter-from,
.toast-leave-to {
  opacity: 0;
}
</style>
