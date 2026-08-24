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
          <div v-for="t in plan.tasks" :key="t.id" class="task-row">
            <span class="task-name">{{ t.name }}</span>
            <StatusBadge :status="t.status" />
            <span class="task-mark">
              <PhListChecks v-if="t.has_subgoals" :size="14" title="按子目标推进" />
              <template v-else-if="t.estimated_minutes != null">{{ hoursFromMinutes(t.estimated_minutes) }} h</template>
            </span>
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
import { PhArrowLeft, PhListChecks, PhPencilSimple } from "@phosphor-icons/vue";
import { useRoute } from "vue-router";
import CreationForm from "../components/CreationForm.vue";
import PriorityLabel from "../components/PriorityLabel.vue";
import StatusBadge from "../components/StatusBadge.vue";
import { getPlan, type PlanView } from "../lib/api";
import { hoursFromMinutes, planErrorMessage } from "../lib/labels";

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

.task-row {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 38px;
  padding: 6px 12px;
  border: var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface);
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
</style>
