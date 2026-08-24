<template>
  <!--
    计划详情页（工单 03；CONTEXT「计划详情」）：头部 = 计划字段 + 优先级标签 + 状态徽章
    + 生命周期按钮占位（工单 05 接线）+「编辑」；正文 = 任务列表。
    点「编辑」正文切换为 CreationUI 编辑态（与创建页共用 CreationForm 双模式）。
  -->
  <section class="detail-page">
    <RouterLink class="back" to="/control-panel/plans">
      <PhArrowLeft :size="14" /> 返回列表
    </RouterLink>

    <p v-if="loadError" class="load-error">
      {{ loadError }}
      <RouterLink to="/control-panel/plans">回计划管理</RouterLink>
    </p>

    <template v-else-if="plan">
      <header class="detail-header">
        <div class="title-row">
          <h2 class="page-title">{{ plan.name }}</h2>
          <PriorityLabel :priority="plan.priority" />
          <StatusBadge :status="plan.status" />
        </div>
        <div class="header-actions">
          <!-- 生命周期操作占位：按状态显示哪些按钮、状态机接线都在工单 05；编辑态不进生命周期按钮（spec 56a） -->
          <div v-if="!editing" class="lifecycle" title="生命周期操作即将到来（工单 05 接线）">
            <button type="button" class="ghost-btn" disabled>开始</button>
            <button type="button" class="ghost-btn" disabled>暂停</button>
            <button type="button" class="ghost-btn" disabled>放弃</button>
            <button type="button" class="ghost-btn" disabled>复制并新建</button>
          </div>
          <button v-if="!editing" type="button" class="primary-btn" @click="editing = true">
            <PhPencilSimple :size="16" /> 编辑
          </button>
        </div>
      </header>

      <!-- 查看态正文：计划字段 + 任务列表。空字段整行隐藏、简述与名称相同也不重复显示 -->
      <div v-if="!editing" class="detail-body">
        <div v-if="hasFields" class="detail-fields">
          <p v-if="showSummary" class="field-view">
            <span class="field-name">简述</span>{{ plan.summary }}
          </p>
          <p v-if="plan.detail" class="field-view">
            <span class="field-name">详细内容</span><span class="detail-text">{{ plan.detail }}</span>
          </p>
          <p v-if="plan.due_date" class="field-view">
            <span class="field-name">截止日期</span>{{ plan.due_date }}
          </p>
        </div>

        <div class="task-list">
          <p v-if="plan.tasks.length === 0" class="hint">
            计划暂无任务——点「编辑」追加（进行中的计划随时可以补任务）。
          </p>
          <div v-for="t in plan.tasks" :key="t.id" class="task-card">
            <div class="task-line">
              <span class="task-name">{{ t.name }}</span>
              <StatusBadge :status="t.status" />
              <span class="task-mark">
                <!-- 有子目标：显示派生进度（ADR-0002）；无子目标：显示手填耗时 -->
                <template v-if="t.has_subgoals">
                  <PhListChecks :size="14" /> {{ taskProgressLabel(t) || "0%" }}
                </template>
                <template v-else-if="t.estimated_minutes != null">{{ hoursFromMinutes(t.estimated_minutes) }} h</template>
              </span>
            </div>
            <!-- 子目标层级（CONTEXT PlanDetail：任务 → 子目标，按填写顺序） -->
            <ul v-if="t.has_subgoals && t.subgoals.length > 0" class="subgoal-list">
              <li v-for="s in t.subgoals" :key="s.id" :class="{ done: s.completed }">
                <PhCheckCircle v-if="s.completed" :size="14" class="sg-state done" />
                <PhCircle v-else :size="14" class="sg-state" />
                <span class="sg-name">{{ s.name }}</span>
                <span class="sg-hours">{{ hoursFromMinutes(s.estimated_minutes) }} h</span>
              </li>
            </ul>
            <!-- 前置依赖（同计划内）：查看态只读提示等待谁 -->
            <p v-if="depNames(t).length > 0" class="dep-line">
              前置：{{ depNames(t).join("、") }}
            </p>
          </div>
        </div>
      </div>

      <!-- 编辑态正文：CreationUI 编辑模式（同一表单组件） -->
      <CreationForm
        v-else
        mode="edit"
        :plan="plan"
        @saved="onSaved"
        @cancel="onCancel"
      />
    </template>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  PhArrowLeft,
  PhCheckCircle,
  PhCircle,
  PhListChecks,
  PhPencilSimple,
} from "@phosphor-icons/vue";
import { useRoute } from "vue-router";
import CreationForm from "../components/CreationForm.vue";
import PriorityLabel from "../components/PriorityLabel.vue";
import StatusBadge from "../components/StatusBadge.vue";
import { getPlan, type PlanView, type TaskView } from "../lib/api";
import { hoursFromMinutes, planErrorMessage } from "../lib/labels";
import { taskProgressLabel } from "../lib/progress";

const route = useRoute();
const plan = ref<PlanView | null>(null);
const loadError = ref("");
const editing = ref(false);

/** 简述与名称相同（留空回退的产物）或为空时不显示——避免两行一模一样 */
const showSummary = computed(
  () => plan.value != null && plan.value.summary !== "" && plan.value.summary !== plan.value.name,
);
/** 三个字段全空时整个字段框也不渲染 */
const hasFields = computed(
  () => showSummary.value || !!plan.value?.detail || !!plan.value?.due_date,
);

/** 任务的前置任务名列表（同计划内 id → 名称；悬空 id 服务层已随删除解除，这里自然为空） */
function depNames(t: TaskView): string[] {
  if (!plan.value) return [];
  return t.prerequisite_ids.flatMap((id) => {
    const pred = plan.value!.tasks.find((p) => p.id === id);
    return pred ? [pred.name] : [];
  });
}

async function load() {
  loadError.value = "";
  plan.value = null;
  try {
    plan.value = await getPlan(Number(route.params.id));
  } catch (err) {
    loadError.value = planErrorMessage(err as { kind?: string });
  }
}

/** 编辑保存成功：重载详情并回到查看态（看到的就是库中最新） */
function onSaved() {
  editing.value = false;
  load();
}

/** 取消编辑：同样重载——编辑态里可能已软删除任务，查看态不能显示陈旧数据 */
function onCancel() {
  editing.value = false;
  load();
}

onMounted(load);
watch(() => route.params.id, load);
</script>

<style scoped>
/* 内容列：限宽 + 居中（窗口拉大时空白均分两侧），窄窗口自动收缩 */
.detail-page {
  max-width: 720px;
  width: 100%;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.back {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--text-secondary);
  font-size: 13px;
  text-decoration: none;
  width: fit-content;
}

.back:hover {
  color: var(--primary);
}

.load-error {
  color: var(--color-danger);
}

.load-error a {
  margin-left: 8px;
  color: var(--primary);
}

.detail-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  flex-wrap: wrap;
}

.title-row {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

.lifecycle {
  display: inline-flex;
  gap: 6px;
}

.detail-body {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.detail-fields {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 14px 16px;
  border: var(--border-default);
  border-radius: var(--radius-md);
  background: var(--bg-group);
}

.field-view {
  margin: 0;
  font-size: 14px;
  color: var(--text-primary);
}

.field-name {
  display: inline-block;
  min-width: 64px;
  color: var(--text-muted);
  font-size: 13px;
}

.detail-text {
  white-space: pre-wrap;
}

.task-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

/* 任务卡：摘要行 + 子目标层级 + 前置提示，纵向叠放 */
.task-card {
  display: flex;
  flex-direction: column;
  padding: 6px 12px;
  border: var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface);
}

.task-line {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 38px;
}

.task-name {
  flex: 1;
  font-weight: 600;
  font-size: 14px;
}

.task-mark {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--text-muted);
  font-size: 12px;
}

.subgoal-list {
  margin: 2px 0 6px;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.subgoal-list li {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 2px 0 2px 4px;
  font-size: 13px;
  color: var(--text-secondary);
}

.subgoal-list li .sg-state {
  color: var(--text-muted);
  flex: none;
}

.subgoal-list li .sg-state.done {
  color: var(--color-done);
}

.subgoal-list li.done .sg-name {
  color: var(--text-muted);
  text-decoration: line-through;
}

.subgoal-list .sg-name {
  flex: 1;
  min-width: 0;
}

.subgoal-list .sg-hours {
  color: var(--text-muted);
  font-size: 12px;
  white-space: nowrap;
}

.dep-line {
  margin: 0 0 4px;
  padding: 2px 0 2px 4px;
  font-size: 12px;
  color: var(--text-muted);
}
</style>
