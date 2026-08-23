<template>
  <!--
    计划管理（工单 02：4-tab 壳 + 数量徽章）。
    「进行中」tab 承载工作区：未开始 + 进行中（未开始计划从这里点「开始」，按钮在工单 05 接线）。
  -->
  <section>
    <h2 class="page-title">计划管理</h2>

    <div class="tabs">
      <button
        v-for="tab in tabs"
        :key="tab.key"
        type="button"
        class="tab"
        :class="{ active: activeTab === tab.key }"
        @click="activeTab = tab.key"
      >
        {{ tab.title }}
        <span class="count">{{ countOf(tab) }}</span>
      </button>
    </div>

    <p v-if="loadError" class="load-error">{{ loadError }}</p>

    <div v-else-if="plans.length === 0" class="empty">
      <p>还没有计划。立下第一个目标，让 Oreo 陪你完成。</p>
      <RouterLink to="/control-panel/create" class="primary-btn">创建第一个计划</RouterLink>
    </div>

    <div v-else class="plan-list">
      <article
        v-for="p in shownPlans"
        :key="p.id"
        class="plan-card"
        :class="`edge-${p.priority}`"
      >
        <div class="plan-main">
          <div class="plan-title-row">
            <span class="plan-name">{{ p.name }}</span>
            <PriorityLabel :priority="p.priority" />
            <StatusBadge :status="p.status" />
          </div>
          <p class="plan-summary">{{ p.summary }}</p>
          <p class="plan-meta">
            {{ p.tasks.length }} 个任务 · 合计 {{ totalHours(p) }} 小时
            <template v-if="p.due_date"> · 截止 {{ p.due_date }}</template>
          </p>
        </div>
      </article>
      <p v-if="shownPlans.length === 0" class="hint">此分类暂无计划</p>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import PriorityLabel from "../components/PriorityLabel.vue";
import StatusBadge from "../components/StatusBadge.vue";
import { listPlans, type PlanStatus, type PlanView } from "../lib/api";
import { hoursFromMinutes } from "../lib/labels";

/** tab 定义：key + 标题 + 收纳的状态集合（进行中 tab 含未开始 = 可推进工作区） */
const tabs = [
  { key: "active", title: "进行中", statuses: ["NotStarted", "Active"] as PlanStatus[] },
  { key: "paused", title: "已暂停", statuses: ["Paused"] as PlanStatus[] },
  { key: "completed", title: "已完成", statuses: ["Completed"] as PlanStatus[] },
  { key: "abandoned", title: "已放弃", statuses: ["Abandoned"] as PlanStatus[] },
];

const activeTab = ref("active");
const plans = ref<PlanView[]>([]);
const loadError = ref("");

onMounted(async () => {
  try {
    plans.value = await listPlans();
  } catch (err) {
    loadError.value = `计划列表加载失败：${err}`;
  }
});

const shownPlans = computed(() => {
  const tab = tabs.find((t) => t.key === activeTab.value)!;
  return plans.value.filter((p) => tab.statuses.includes(p.status));
});

function countOf(tab: (typeof tabs)[number]) {
  return plans.value.filter((p) => tab.statuses.includes(p.status)).length;
}

/** 任务预计耗时合计（小时） */
function totalHours(p: PlanView) {
  return hoursFromMinutes(p.tasks.reduce((sum, t) => sum + (t.estimated_minutes ?? 0), 0));
}
</script>

<style scoped>
.tabs {
  display: flex;
  gap: 4px;
  border-bottom: var(--border-default);
  margin-bottom: 16px;
}

.tab {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 8px 14px;
  border: none;
  border-bottom: 2px solid transparent;
  background: transparent;
  font: inherit;
  color: var(--text-secondary);
  cursor: pointer;
}

.tab.active {
  color: var(--primary);
  border-bottom-color: var(--primary);
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

.tab.active .count {
  background: var(--bg-accent-group);
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
