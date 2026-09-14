<template>
  <!--
    计划管理（工单 03/05：状态筛选栏 + 单列表，2026-08-23 grill 修订，取代 4-tab）。
    筛选项六项各带数量，默认「全部」；点击卡片进入计划详情页。
    排序 = PlanOrdering 默认规则（优先级降序 + 创建倒序，唯一排序；
    2026-08-24 决策砍掉手动拖拽排序——实施顺序在大面板自主选择，列表手动序无意义）。
  -->
  <!-- 页面宽度归壳层统一内容列（ControlPanelWindow，720px） -->
  <section>
    <div class="title-row">
      <h2 class="page-title">计划管理</h2>
      <!-- 页面级入口聚右（2026-08-30 用户决策）：图标按钮 + 悬停原生 tooltip——
           原文字按钮是 title-row space-between 的中间子元素，被顶到行中央且与
           "打开大面板"主次不分。今日总结（工单 11，story 58）：随时调出看最新版
           （内容从日志实时重算）；日期 = 待弹的 > 最近已弹的 > 今天 -->
      <div class="title-actions">
        <button type="button" class="icon-btn" title="今日总结" @click="openSummary">
          <PhChartBar :size="18" />
        </button>
        <!-- 打开大面板：重开今日分配（覆盖重选；已推进的进度从日志来不受影响） -->
        <button type="button" class="icon-btn" title="打开大面板（重新分配今日任务）" @click="openMainBoard">
          <PhCalendarCheck :size="18" />
        </button>
      </div>
    </div>

    <div class="filter-bar">
      <button
        v-for="f in filters"
        :key="f.key"
        type="button"
        class="filter"
        :class="{ active: activeFilter === f.key }"
        @click="activeFilter = f.key"
      >
        {{ f.label }}
        <span class="count">{{ countOf(f) }}</span>
      </button>
    </div>

    <p v-if="loadError" class="load-error">{{ loadError }}</p>

    <div v-else-if="plans.length === 0" class="empty">
      <p>还没有计划。立下第一个目标，让桌宠陪你完成。</p>
      <RouterLink to="/control-panel/create" class="primary-btn">创建第一个计划</RouterLink>
    </div>

    <div v-else class="plan-list">
      <RouterLink
        v-for="p in shownPlans"
        :key="p.id"
        class="plan-card"
        :class="`edge-${p.priority}`"
        :to="`/control-panel/plans/${p.id}`"
      >
        <div class="plan-main">
          <div class="plan-title-row">
            <span class="plan-name">{{ p.name }}</span>
            <PriorityLabel :priority="p.priority" />
            <StatusBadge :status="p.status" />
          </div>
          <!-- 简述与名称相同（留空回退的产物）时不重复显示 -->
          <p v-if="p.summary && p.summary !== p.name" class="plan-summary">{{ p.summary }}</p>
          <p class="plan-meta">
            {{ p.tasks.length }} 个任务 · 合计 {{ totalHours(p) }} 小时
            <template v-if="p.due_date"> · 截止 {{ p.due_date }}</template>
          </p>
        </div>
      </RouterLink>
      <p v-if="shownPlans.length === 0" class="hint">此分类暂无计划</p>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { emitTo } from "@tauri-apps/api/event";
import { PhCalendarCheck, PhChartBar } from "@phosphor-icons/vue";
import PriorityLabel from "../components/PriorityLabel.vue";
import StatusBadge from "../components/StatusBadge.vue";
import {
  getDailySummaryStatus,
  listPlans,
  type PlanStatus,
  type PlanView,
} from "../lib/api";
import { hoursFromMinutes, statusLabel } from "../lib/labels";
import { localToday } from "../lib/validation";
import { openDailySummaryWindow } from "../lib/summary";
import { MAIN_BOARD_REOPEN_EVENT } from "../lib/events";
import { revealWindow } from "../lib/windows";

/** 筛选项：key + 文案 + 命中状态集（「全部」不过滤）；五个状态都是一等公民 */
const filters: { key: string; label: string; statuses: PlanStatus[] | null }[] = [
  { key: "all", label: "全部", statuses: null },
  ...(["NotStarted", "Active", "Paused", "Completed", "Abandoned"] as PlanStatus[]).map(
    (s) => ({ key: s, label: statusLabel[s], statuses: [s] }),
  ),
];

const activeFilter = ref("all");
const plans = ref<PlanView[]>([]);
const loadError = ref("");

onMounted(async () => {
  try {
    plans.value = await listPlans();
  } catch (err) {
    loadError.value = `计划列表加载失败：${err}`;
  }
});

/** 筛选项是否命中某计划（「全部」statuses 为 null 不过滤）；列表与数量统计共用 */
function matches(f: (typeof filters)[number], p: PlanView): boolean {
  return f.statuses == null || f.statuses.includes(p.status);
}

/** 当前筛选下的列表（顺序 = 服务端 PlanOrdering 默认排序，前端不重排） */
const shownPlans = computed(() => {
  const f = filters.find((x) => x.key === activeFilter.value)!;
  return plans.value.filter((p) => matches(f, p));
});

function countOf(f: (typeof filters)[number]) {
  return plans.value.filter((p) => matches(f, p)).length;
}

/** 任务预计耗时合计（小时） */
function totalHours(p: PlanView) {
  return hoursFromMinutes(p.tasks.reduce((sum, t) => sum + (t.estimated_minutes ?? 0), 0));
}

/** 打开大面板：先发重开事件让面板刷新到最新数据，再可靠地亮到前台
 *  （revealWindow = unminimize→show→setFocus——最小化中的面板也能唤回） */
async function openMainBoard() {
  await emitTo("main-board", MAIN_BOARD_REOPEN_EVENT);
  await revealWindow("main-board");
}

/** 打开今日总结：日期 = 待弹的 > 最近已弹的 > 今天本地日期；
 *  补看的就是"待弹"那份时登记已弹（自动触发不再重弹），看历史/今天则不登记 */
async function openSummary() {
  const st = await getDailySummaryStatus();
  const date = st.due ?? st.last_shown ?? localToday();
  await openDailySummaryWindow(date, date === st.due);
}
</script>

<style scoped>
/* 标题行：标题在左，页面级入口（图标按钮）聚右 */
.title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.title-actions {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.filter-bar {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-bottom: 16px;
}

.filter {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px 12px;
  border: var(--border-default);
  border-radius: var(--radius-sm);
  background: var(--surface);
  font: inherit;
  font-size: 13px;
  color: var(--text-secondary);
  cursor: pointer;
}

.filter.active {
  border-color: var(--primary);
  background: var(--bg-accent-group);
  color: var(--text-primary);
}

.count {
  min-width: 20px;
  padding: 0 6px;
  border-radius: var(--radius-sm);
  background: var(--bg-group);
  border: var(--border-default);
  font-size: 12px;
  line-height: 18px;
  text-align: center;
  color: var(--text-muted);
}

.filter.active .count {
  background: var(--surface);
  border-color: var(--primary);
  color: var(--text-primary);
}

.empty {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 12px;
  padding: 32px 24px;
  border: var(--border-default);
  border-radius: var(--radius-md);
  background: var(--bg-group);
  color: var(--text-secondary);
}

.load-error {
  color: var(--color-danger);
}

.plan-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.plan-card {
  display: flex;
  border: var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface);
  overflow: hidden;
  text-decoration: none;
  color: inherit;
  transition: border-color 0.15s;
}

.plan-card:hover {
  border-color: var(--primary);
}

/* PriorityVisuals：列表项左侧 4px 优先级边条 */
.edge-High {
  border-left: 4px solid var(--priority-high);
}

.edge-Medium {
  border-left: 4px solid var(--priority-medium);
}

.edge-Low {
  border-left: 4px solid var(--priority-low);
}

.plan-main {
  flex: 1;
  padding: 14px 16px;
}

.plan-title-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.plan-name {
  font-weight: 600;
  font-size: 15px;
}

.plan-summary {
  margin: 6px 0 4px;
  color: var(--text-secondary);
}

.plan-meta {
  margin: 0;
  color: var(--text-muted);
  font-size: 12px;
}
</style>
