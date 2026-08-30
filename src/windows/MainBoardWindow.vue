<template>
  <!--
    大面板：今日任务分配（工单 06，术语 MainBoardTodayAllocation）。
    按计划分组展示所有进行中计划的可选任务；勾选实时累计对照当日目标（柔性边界，
    不阻止提交）。被依赖阻塞的任务不展示——只展示可选任务（2026-08-24 用户决策，
    原"置灰展示等待项"砍掉，解锁当日自然回到列表）。
    被抢占暂停的计划（工单 12）灰显移入"暂停"分组、不可勾选（AutoPauseFeedback
    第一层）；生命周期变化经 main-board:refresh 静默重取，立刻可见。
    确认 = 持久化当日分配并隐藏窗口；重开（控制面板入口）回显旧选择可覆盖重选。
    休息日打开 = 加班态（2026-08-29 用户决策，取代原"分配不加载"休息日空态）：
    手动切入工作模式即可选任务提交；自动触发仍限工作日（不打扰）。
    当日目标 = 基准 + 工时账户结转（工单 10 实时派生），状态条透明标注。
  -->
  <div class="board">
    <header class="board-header">
      <!-- 内容列限宽居中（2026-08-30 反馈：最大化后任务卡不横跨整屏）；
           线条与底色仍通栏，只有内容收进列（列宽经 --col-max 注入，见 tokens.css） -->
      <div class="header-inner content-col">
        <div>
          <h1 class="title">今日分配</h1>
          <p class="hint">
            {{
              board && !board.workday
                ? "休息日加班：勾选要推进的任务，工时按超额并入账户"
                : "勾选今日要推进的任务，累计预计耗时对齐当日目标"
            }}
          </p>
        </div>
        <p class="date">{{ dateLabel }}</p>
      </div>
    </header>

    <main class="board-body">
      <!-- 滚动收进列自身（content-col-scroll）：滚动条贴列右缘，头/体/底对齐不因滚动条漂移 -->
      <div class="content-col content-col-scroll thin-scrollbar">
        <p v-if="loadError" class="error">{{ loadError }}</p>
        <p v-else-if="!board" class="hint loading">加载中…</p>

        <!-- 空态：没有进行中的计划（或全部完成等待确认），也没有被抢占暂停的计划 -->
        <div v-else-if="board.groups.length === 0 && board.paused_groups.length === 0" class="empty">
          <PhCoffee :size="28" />
          <p class="empty-title">今天没有可推进的任务</p>
          <p class="hint">在控制面板开始一个计划后，回到这里分配今日任务</p>
        </div>

        <template v-else>
          <section v-for="g in board.groups" :key="g.plan_id" class="group">
            <header class="group-head">
              <PhFlag :size="15" class="group-flag" />
              <span class="group-name">{{ g.plan_name }}</span>
              <PriorityLabel :priority="g.priority" />
            </header>
            <ul class="task-card">
              <li v-for="t in g.tasks" :key="t.id">
                <label class="task-row">
                  <input v-model="selected" type="checkbox" :value="t.id" />
                  <span class="task-name">{{ t.name }}</span>
                  <span class="task-time">{{ hoursFromMinutes(t.estimated_minutes) }} 小时</span>
                </label>
              </li>
            </ul>
          </section>

          <!-- 被抢占暂停的计划（AutoPauseFeedback 第一层，工单 12）：灰显移入"暂停"分组，
               无勾选框不可选；高优先级清空自动恢复后随刷新回到上方候选 -->
          <section v-for="g in board.paused_groups" :key="`paused-${g.plan_id}`" class="group paused">
            <header class="group-head">
              <PhFlag :size="15" class="group-flag" />
              <span class="group-name">{{ g.plan_name }}</span>
              <PriorityLabel :priority="g.priority" />
              <StatusBadge status="Paused" />
            </header>
            <ul class="task-card">
              <li v-for="t in g.tasks" :key="t.id">
                <div class="task-row paused-row">
                  <span class="task-name">{{ t.name }}</span>
                  <span class="task-time">{{ hoursFromMinutes(t.estimated_minutes) }} 小时</span>
                </div>
              </li>
            </ul>
          </section>
        </template>
      </div>
    </main>

    <footer v-if="board && board.groups.length > 0" class="board-footer">
      <!-- 左：状态条——工作日固定口径"今日累计 X.X 小时 / 目标 Y 小时"并透明标注结转
           （工单 10："目标 5.6h（基准 5.0h + 结转 0.6h）"，无结转不加标注）；
           休息日加班无目标义务，只看累计（无目标/进度条/差额提示） -->
      <div class="footer-inner content-col">
        <div class="status">
          <p class="status-line">
            今日累计 <b>{{ hoursLabel(accumulatedMinutes) }}</b> 小时
            <template v-if="board.workday">
              <span class="status-sep">/</span> 目标 {{ hoursLabel(board.target_minutes) }} 小时
              <span v-if="carryText" class="carry-note">（{{ carryText }}）</span>
            </template>
          </p>
          <MicroBar
            v-if="board.workday"
            class="status-bar"
            :ratio="accumulatedMinutes / board.target_minutes"
            :reached="accumulatedMinutes >= board.target_minutes"
          />
        </div>

        <!-- 中：确认（底部中间，点击持久化并隐藏窗口） -->
        <button type="button" class="primary-btn confirm" :disabled="busy" @click="confirm">
          <PhCheck :size="16" /> 确认
        </button>

        <!-- 右：差额软提示（黄色，未达标才有） / 休息日加班说明 / 提交错误 -->
        <p v-if="commitError" class="error footer-note">{{ commitError }}</p>
        <p v-else-if="!board.workday" class="footer-note overtime">休息日加班 · 推进按超额并入工时账户</p>
        <p v-else-if="shortfallMinutes > 0" class="gap footer-note">
          还差 {{ hoursLabel(shortfallMinutes) }} 小时
        </p>
      </div>
    </footer>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { emitTo, listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  PhCheck,
  PhCoffee,
  PhFlag,
} from "@phosphor-icons/vue";
import MicroBar from "../components/MicroBar.vue";
import PriorityLabel from "../components/PriorityLabel.vue";
import StatusBadge from "../components/StatusBadge.vue";
import {
  MAIN_BOARD_REOPEN_EVENT,
  MAIN_BOARD_REFRESH_EVENT,
  MINI_BOARD_REFRESH_EVENT,
} from "../lib/events";
import {
  commitTodayAllocation,
  getAllocationBoard,
  type AllocationBoardView,
  type PlanErrorShape,
} from "../lib/api";
import { hoursFromMinutes, hoursLabel, carryLabel, planErrorMessage } from "../lib/labels";

const win = getCurrentWebviewWindow();
const board = ref<AllocationBoardView | null>(null);
const loadError = ref("");
const commitError = ref("");
const busy = ref(false);
/** 选中任务 id 集（checkbox 数组绑定）；重开时从服务端回显，确认时整集覆盖提交 */
const selected = ref<number[]>([]);

/** 候选任务全集（分组摊平），累计与回显过滤共用 */
const allTasks = computed(() => board.value?.groups.flatMap((g) => g.tasks) ?? []);

/** 勾选实时累计：选中任务的预计耗时之和（被阻塞任务不在候选列表，天然不参与） */
const accumulatedMinutes = computed(() =>
  allTasks.value
    .filter((t) => selected.value.includes(t.id))
    .reduce((sum, t) => sum + t.estimated_minutes, 0),
);

const shortfallMinutes = computed(() => {
  const target = board.value?.target_minutes ?? 0;
  return Math.max(0, target - accumulatedMinutes.value);
});

/** 结转标注（工单 10）：调整后目标 ≠ 基准时透明标出"基准 X + 结转 Y"；无结转为空串 */
const carryText = computed(() => {
  const b = board.value;
  return b ? carryLabel(b.target_minutes, b.base_minutes) : "";
});

/** 日期文案（"8月24日 星期一"）——从服务端给的归属日解析，避免本地时区漂移 */
const dateLabel = computed(() => {
  const raw = board.value?.date;
  if (!raw) return "";
  const [y, m, d] = raw.split("-").map(Number);
  return new Date(y, m - 1, d).toLocaleDateString("zh-CN", {
    month: "long",
    day: "numeric",
    weekday: "long",
  });
});

async function load() {
  try {
    const v = await getAllocationBoard();
    board.value = v;
    selected.value = [...v.selected_task_ids];
    loadError.value = "";
  } catch (err) {
    loadError.value = `今日分配加载失败：${planErrorMessage(err as PlanErrorShape)}`;
  }
}

async function confirm() {
  if (busy.value) return;
  busy.value = true;
  try {
    await commitTodayAllocation(selected.value);
    commitError.value = "";
    // 分配落定 → 刷新小看板数据；亮板与自动隐藏计时由 PetWindow 的 refresh 监听统一执行
    // （2026-08-30 反馈：显隐唯一持有者是桌宠，这里不再直接 show）
    await emitTo("mini-board", MINI_BOARD_REFRESH_EVENT);
    await win.hide(); // 关闭 = 隐藏（CONTEXT 窗口关闭语义），重开入口在控制面板
  } catch (err) {
    commitError.value = planErrorMessage(err as PlanErrorShape);
  } finally {
    busy.value = false;
  }
}

onMounted(async () => {
  await load();
  // 重开刷新：控制面板「打开大面板」先发事件再 show，这里回到最新数据（依赖/状态可能已变）
  await listen(MAIN_BOARD_REOPEN_EVENT, load);
  // 生命周期变化（工单 12）：开始/暂停/恢复/完成/放弃/复制落库后静默重取——
  // 被抢占的计划立刻灰显进入"暂停"分组，自动恢复的计划回到候选（显隐不变）
  await listen(MAIN_BOARD_REFRESH_EVENT, load);
  // 系统关闭请求拦截为隐藏（与「确认」同一语义；正式的关闭语义归工单 14 统一）
  await win.onCloseRequested(async (e) => {
    e.preventDefault();
    await win.hide();
  });
});
</script>

<style scoped>
.board {
  --col-max: 724px; /* 内容列宽 = 默认 780 窗口 − 2×28 padding（tokens.css .content-col 消费） */
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--bg-base);
}

/* ---- 头部：标题 + 归属日期 ----
   窗口可最大化：线条（下边框）通栏，内容收进限宽居中列（全局 .content-col）——拉大不散架 */
.board-header {
  padding: 24px 28px 16px;
  border-bottom: var(--border-default);
}

.header-inner {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.title {
  margin: 0;
  font-size: 18px;
}

.board-header .hint {
  margin: 4px 0 0;
}

.date {
  margin: 2px 0 0;
  color: var(--text-muted);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

/* ---- 主体：按计划分组的候选列表（滚动归内容列 .content-col-scroll，本体只留框架） ---- */
.board-body {
  flex: 1;
  min-height: 0;
  padding: 20px 28px;
}

.loading {
  text-align: center;
  margin-top: 40px;
}

.error {
  color: var(--color-danger);
}

.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  margin-top: 120px;
  color: var(--text-muted);
}

.empty-title {
  margin: 0;
  color: var(--text-secondary);
  font-weight: 600;
}

.empty .hint {
  margin: 0;
}

.group {
  margin-bottom: 20px;
}

/* section header：旗帜图标 + 计划名 + 优先级 */
.group-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
  padding-left: 2px;
}

.group-flag {
  color: var(--text-muted);
}

.group-name {
  font-weight: 600;
  font-size: 15px;
}

/* 任务卡：白底表面 + 行分隔（线条基调） */
.task-card {
  margin: 0;
  padding: 4px 0;
  list-style: none;
  border: var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface);
}

.task-card li + li {
  border-top: var(--border-default);
}

.task-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 11px 16px;
  cursor: pointer;
}

.task-row:hover {
  background: var(--bg-group);
}

.task-row input[type="checkbox"] {
  width: 16px;
  height: 16px;
  margin: 0;
  accent-color: var(--primary);
  flex-shrink: 0;
}

.task-name {
  flex: 1;
  min-width: 0;
}

.task-time {
  min-width: 72px;
  text-align: right;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
  font-size: 13px;
}

/* 被抢占暂停的分组（AutoPauseFeedback 第一层）：整体灰显、不可交互 */
.group.paused .group-name,
.paused-row .task-name,
.paused-row .task-time {
  color: var(--text-muted);
}

.paused-row {
  cursor: default;
}

.paused-row:hover {
  background: transparent;
}

/* ---- 底部状态条：累计/目标 + 进度条；确认居中；差额软提示居右。
     上边框与底色通栏，三区内容（全局 .content-col）与上方列表同列对齐 ---- */
.board-footer {
  padding: 14px 28px 16px;
  border-top: var(--border-default);
  background: var(--bg-group);
}

.footer-inner {
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  align-items: center;
  gap: 16px;
}

.status {
  justify-self: start;
}

.status-line {
  margin: 0 0 6px;
  font-size: 13px;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.status-line b {
  color: var(--text-primary);
  font-weight: 600;
}

.status-sep {
  margin: 0 2px;
  color: var(--text-muted);
}

/* 结转标注（工单 10）：目标旁的透明说明，弱化展示——回答"目标怎么变了" */
.carry-note {
  color: var(--text-muted);
  font-size: 12px;
}

/* 微型进度条（组件本体在 MicroBar，这里只定宽） */
.status-bar {
  width: 220px;
}

.confirm {
  justify-self: center;
}

.footer-note {
  justify-self: end;
  margin: 0;
  font-size: 13px;
}

/* 黄色差额软提示（未达标才有；柔性边界不阻止提交） */
.gap {
  color: var(--color-progress);
}

/* 休息日加班说明（信息性，弱化展示；无目标义务故无差额提示） */
.overtime {
  color: var(--text-muted);
}
</style>
