<template>
  <!--
    大面板：今日任务分配（工单 06，术语 MainBoardTodayAllocation）。
    按计划分组展示所有进行中计划的可选任务；勾选实时累计对照当日目标（柔性边界，
    不阻止提交）。被依赖阻塞的任务不展示——只展示可选任务（2026-08-24 用户决策，
    原"置灰展示等待项"砍掉，解锁当日自然回到列表）。
    确认 = 持久化当日分配并隐藏窗口；重开（控制面板入口）回显旧选择可覆盖重选。
  -->
  <div class="board">
    <header class="board-header">
      <div>
        <h1 class="title">今日分配</h1>
        <p class="hint">勾选今日要推进的任务，累计预计耗时对齐当日目标</p>
      </div>
      <p class="date">{{ dateLabel }}</p>
    </header>

    <main class="board-body">
      <p v-if="loadError" class="error">{{ loadError }}</p>
      <p v-else-if="!board" class="hint loading">加载中…</p>

      <!-- 休息日：分配不加载（CONTEXT WorkingHours），临时推进照常记进度 -->
      <div v-else-if="!board.workday" class="empty">
        <PhMoonStars :size="28" />
        <p class="empty-title">今日不在工作日</p>
        <p class="hint">休息日无需分配；临时推进照常记入任务进度，工时按超额并入账户</p>
      </div>

      <!-- 空态：没有进行中的计划（或全部完成等待确认） -->
      <div v-else-if="board.groups.length === 0" class="empty">
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
      </template>
    </main>

    <footer v-if="board && board.workday && board.groups.length > 0" class="board-footer">
      <!-- 左：状态条（固定口径"今日累计 X.X 小时 / 目标 Y 小时"）+ 进度可视化 -->
      <div class="status">
        <p class="status-line">
          今日累计 <b>{{ hoursLabel(accumulatedMinutes) }}</b> 小时
          <span class="status-sep">/</span> 目标 {{ hoursFromMinutes(board.target_minutes) }} 小时
        </p>
        <div class="bar">
          <div
            class="fill"
            :class="{ reached: accumulatedMinutes >= board.target_minutes }"
            :style="{ width: barWidth }"
          ></div>
        </div>
      </div>

      <!-- 中：确认（底部中间，点击持久化并隐藏窗口） -->
      <button type="button" class="primary-btn confirm" :disabled="busy" @click="confirm">
        <PhCheck :size="16" /> 确认
      </button>

      <!-- 右：差额软提示（黄色，未达标才有；超额是正常状态，无警告） / 提交错误 -->
      <p v-if="commitError" class="error footer-note">{{ commitError }}</p>
      <p v-else-if="shortfallMinutes > 0" class="gap footer-note">
        还差 {{ hoursLabel(shortfallMinutes) }} 小时
      </p>
    </footer>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  PhCheck,
  PhCoffee,
  PhFlag,
  PhMoonStars,
} from "@phosphor-icons/vue";
import PriorityLabel from "../components/PriorityLabel.vue";
import {
  commitTodayAllocation,
  getAllocationBoard,
  type AllocationBoardView,
  type PlanErrorShape,
} from "../lib/api";
import { hoursFromMinutes, hoursLabel, planErrorMessage } from "../lib/labels";

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

/** 进度条宽度：达到目标即满格（超出不再延伸，超额是正常状态） */
const barWidth = computed(() => {
  const target = board.value?.target_minutes ?? 0;
  if (target === 0) return "0%";
  return `${Math.min(100, (accumulatedMinutes.value / target) * 100)}%`;
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
  await listen("main-board:reopen", load);
  // 系统关闭请求拦截为隐藏（与「确认」同一语义；正式的关闭语义归工单 14 统一）
  await win.onCloseRequested(async (e) => {
    e.preventDefault();
    await win.hide();
  });
});
</script>

<style scoped>
.board {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--bg-base);
}

/* ---- 头部：标题 + 归属日期 ---- */
.board-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  padding: 24px 28px 16px;
  border-bottom: var(--border-default);
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

/* ---- 主体：按计划分组的候选列表 ---- */
.board-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
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

/* ---- 底部状态条：累计/目标 + 进度条；确认居中；差额软提示居右 ---- */
.board-footer {
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  align-items: center;
  gap: 16px;
  padding: 14px 28px 16px;
  border-top: var(--border-default);
  background: var(--bg-group);
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

/* 微型进度条：未达标记进行色（黄系软提示），达标转完成色。
   圆角取 --radius-sm，浏览器按 4px 高自动钳到半高胶囊（token 表无亚 8px 档） */
.bar {
  width: 220px;
  height: 4px;
  border-radius: var(--radius-sm);
  background: var(--border-default);
  overflow: hidden;
}

.fill {
  height: 100%;
  border-radius: var(--radius-sm);
  background: var(--color-progress);
  transition: width 0.2s;
}

.fill.reached {
  background: var(--color-done);
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
</style>
