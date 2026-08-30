<template>
  <!--
    计划详情页（工单 03/05；CONTEXT「计划详情」）：头部 = 计划字段 + 优先级标签 + 状态徽章
    + 生命周期操作（按状态显示可得操作，工单 05 接线）+「编辑」；正文 = 任务列表。
    点「编辑」正文切换为 CreationUI 编辑态（与创建页共用 CreationForm 双模式）；
    生命周期按钮不进编辑态（spec 56a）。
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
        <div class="title-col">
          <div class="title-row">
            <h2 class="page-title">{{ plan.name }}</h2>
            <PriorityLabel :priority="plan.priority" />
            <StatusBadge :status="plan.status" />
          </div>
          <!-- 暂停原因（CONTEXT PauseReason）：已暂停时透出，说明是谁按下了暂停 -->
          <p v-if="plan.status === 'Paused' && plan.pause_reason" class="hint pause-reason">
            暂停原因：{{ pauseReasonLabel[plan.pause_reason] }}
          </p>
        </div>
        <div class="header-actions">
          <!-- 生命周期操作矩阵（ADR-0001）：
               未开始 → 开始；进行中 → 完成计划(任务全完成后亮起)/暂停/放弃；
               已暂停 → 继续/放弃；终态 → 复制并新建（唯一重启路径，无"重新开始"）。
               抢占不变式（工单 12）：存在进行中的更高优先级计划时，开始/继续/复制
               （复制即开始）禁用——外层 span 承载 tooltip（disabled 按钮本身不冒泡 hover） -->
          <div v-if="!editing" class="lifecycle">
            <span v-if="plan.status === 'NotStarted'" :title="lifecycleBlockedTitle">
              <button
                type="button"
                class="primary-btn"
                :disabled="busy || !!higherActive"
                @click="runLifecycle(() => startPlan(plan!.id))"
              >
                <PhPlay :size="16" /> 开始
              </button>
            </span>
            <template v-else-if="plan.status === 'Active'">
              <button
                v-if="allTasksDone"
                type="button"
                class="primary-btn"
                :disabled="busy"
                @click="pendingComplete = true"
              >
                <PhCheckCircle :size="16" /> 完成计划
              </button>
              <button
                type="button"
                class="ghost-btn"
                :disabled="busy"
                @click="runLifecycle(() => pausePlan(plan!.id))"
              >
                <PhPause :size="16" /> 暂停
              </button>
              <button
                type="button"
                class="danger-ghost-btn"
                :disabled="busy"
                @click="abortStage = 'info'"
              >
                <PhXCircle :size="16" /> 放弃
              </button>
            </template>
            <template v-else-if="plan.status === 'Paused'">
              <span :title="lifecycleBlockedTitle">
                <button
                  type="button"
                  class="primary-btn"
                  :disabled="busy || !!higherActive"
                  @click="runLifecycle(() => resumePlan(plan!.id), {}, '继续')"
                >
                  <PhPlay :size="16" /> 继续
                </button>
              </span>
              <button
                type="button"
                class="danger-ghost-btn"
                :disabled="busy"
                @click="abortStage = 'info'"
              >
                <PhXCircle :size="16" /> 放弃
              </button>
            </template>
            <span v-else :title="lifecycleBlockedTitle">
              <button
                type="button"
                class="primary-btn"
                :disabled="busy || !!higherActive"
                @click="runLifecycle(copyAsNew, {}, '复制新建')"
              >
                <PhCopySimple :size="16" /> 复制并新建
              </button>
            </span>
          </div>
          <button v-if="!editing" type="button" class="ghost-btn" :disabled="busy" @click="editing = true">
            <PhPencilSimple :size="16" /> 编辑
          </button>
        </div>
      </header>

      <p v-if="serverError" class="server-error">{{ serverError }}</p>

      <!-- 自动暂停反馈（AutoPauseFeedback 第二层，工单 12）：抢占发生时在触发地即时透出 -->
      <div v-if="preemptNotice.length > 0" class="preempt-notice">
        <p v-for="line in preemptNotice" :key="line">{{ line }}</p>
      </div>

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
                <!-- 进度均为派生值（ADR-0002）：有子目标带图标；无子目标百分比来自汇报日志 -->
                <template v-if="t.has_subgoals">
                  <PhListChecks :size="14" /> {{ taskProgressLabel(t) || "0%" }}
                </template>
                <template v-else-if="t.estimated_minutes != null">
                  {{ taskProgressLabel(t) || "0%" }} · {{ hoursFromMinutes(t.estimated_minutes) }} h
                </template>
              </span>
              <!-- 修正总进度（无子目标任务专属改口通道；已完成锁定不显示） -->
              <button
                v-if="!t.has_subgoals && t.status !== 'Completed'"
                type="button"
                class="icon-btn"
                title="修正总进度"
                @click="openCorrection(t)"
              >
                <PhSlidersHorizontal :size="15" />
              </button>
            </div>
            <!-- 子目标层级（CONTEXT PlanDetail：任务 → 子目标，按填写顺序；圆圈符号已承载行标识） -->
            <ul v-if="t.has_subgoals && t.subgoals.length > 0" class="subgoal-list">
              <li v-for="s in t.subgoals" :key="s.id" :class="{ done: s.completed }">
                <PhCheckCircle v-if="s.completed" :size="14" class="sg-state done" />
                <PhCircle v-else :size="14" class="sg-state" />
                <span class="sg-name">{{ s.name }}</span>
                <span class="sg-hours">{{ hoursFromMinutes(s.estimated_minutes) }} h</span>
              </li>
            </ul>
            <!-- 依赖双向可见（顺序关系的诚实载体是边不是序号）：入边"谁挡我" + 出边"我挡谁" -->
            <p v-if="depNames(t).length > 0" class="dep-line">前置：{{ depNames(t).join("、") }}</p>
            <p v-if="waitingFor(t).length > 0" class="dep-line">被等待：{{ waitingFor(t).join("、") }}</p>
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

    <!-- 完成计划确认（PlanCompletionConfirm：手动确认"我做到了"） -->
    <ConfirmDialog
      v-if="pendingComplete"
      title="完成这个计划？"
      action-label="完成计划"
      @confirm="runLifecycle(() => completePlan(plan!.id), { closeComplete: true })"
      @cancel="pendingComplete = false"
    >
      <p>{{ plan!.tasks.length }} 个任务已全部完成。确认后计划进入「已完成」终态。</p>
      <p>完成后不可重启；如需再来一轮，用「复制并新建」。</p>
    </ConfirmDialog>

    <!-- 放弃一级确认（CONTEXT AbortPlan：先说清保留什么历史，再进打字确认） -->
    <ConfirmDialog
      v-if="abortStage === 'info'"
      title="放弃这个计划？"
      action-label="继续"
      @confirm="abortStage = 'type'"
      @cancel="abortStage = null"
    >
      <p>将保留 {{ plan!.tasks.length }} 个任务 / {{ progressedHours }} 小时进度作为历史，随时可回看。</p>
      <p>放弃是终态，不可重启。</p>
    </ConfirmDialog>

    <!-- 放弃二级确认：打字「再删」（防误操作，措辞沿用任务删除） -->
    <TypeConfirmDialog
      v-if="abortStage === 'type'"
      title="确认放弃计划"
      action-label="放弃计划"
      @confirm="runLifecycle(() => abortPlan(plan!.id), { closeAbort: true })"
      @cancel="abortStage = null"
    >
      <p>输入「再删」后，计划「{{ plan!.name }}」将进入「已放弃」终态。</p>
    </TypeConfirmDialog>

    <!-- 修正总进度（spec 36：直接设定当前值、带确认、以事件落账；仅无子目标任务） -->
    <ConfirmDialog
      v-if="correcting"
      title="修正总进度"
      action-label="修正"
      @confirm="applyCorrection"
      @cancel="correcting = null"
    >
      <p>
        任务「{{ correcting.name }}」当前 {{ correcting.progress_percent }}%，
        直接设定为
        <input
          v-model="correctValue"
          class="input correct-input"
          type="number"
          min="0"
          max="100"
          step="0.1"
          @keyup.enter="applyCorrection"
        />
        %（0–100，最多一位小数）
      </p>
      <p v-if="correctError" class="correct-error">{{ correctError }}</p>
      <p class="hint">修正以事件落账，今日统计随之重算。</p>
    </ConfirmDialog>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  PhArrowLeft,
  PhCheckCircle,
  PhCircle,
  PhCopySimple,
  PhListChecks,
  PhPause,
  PhPencilSimple,
  PhPlay,
  PhSlidersHorizontal,
  PhXCircle,
} from "@phosphor-icons/vue";
import { useRoute, useRouter } from "vue-router";
import { emitTo } from "@tauri-apps/api/event";
import ConfirmDialog from "../components/ConfirmDialog.vue";
import CreationForm from "../components/CreationForm.vue";
import PriorityLabel from "../components/PriorityLabel.vue";
import StatusBadge from "../components/StatusBadge.vue";
import TypeConfirmDialog from "../components/TypeConfirmDialog.vue";
import {
  abortPlan,
  completePlan,
  copyPlanAsNew,
  correctTotalProgress,
  getPlan,
  listPlans,
  pausePlan,
  resumePlan,
  startPlan,
  type LifecycleOutcome,
  type PlanView,
  type Priority,
  type TaskView,
} from "../lib/api";
import { hoursFromMinutes, pauseReasonLabel, planErrorMessage, preemptedByHigherMessage } from "../lib/labels";
import { taskProgress, taskProgressLabel } from "../lib/progress";
import { isValidPercentValue } from "../lib/validation";
import { MAIN_BOARD_REFRESH_EVENT, MINI_BOARD_REFRESH_EVENT } from "../lib/events";
import { SUMMARY_REFRESH_EVENT } from "../lib/summary";

const route = useRoute();
const router = useRouter();
const plan = ref<PlanView | null>(null);
/** 全部计划（抢占不变式的 UI 判定数据源：查存在进行中的更高优先级计划） */
const allPlans = ref<PlanView[]>([]);
const loadError = ref("");
const editing = ref(false);

/** 生命周期操作进行中（防连点：全部按钮禁用） */
const busy = ref(false);
/** 生命周期操作失败的用户可读文案（成功路径不产生文案，直接重载视图） */
const serverError = ref("");
/** 自动暂停反馈（AutoPauseFeedback 第二层）：本次操作波及的"已被自动暂停"文案行 */
const preemptNotice = ref<string[]>([]);
/** 完成计划确认弹窗；放弃两步弹窗当前所在阶段（null = 关闭） */
const pendingComplete = ref(false);
const abortStage = ref<"info" | "type" | null>(null);
/** 修正总进度弹窗（正在修正的任务）；输入值与本地校验文案 */
const correcting = ref<TaskView | null>(null);
const correctValue = ref("0");
const correctError = ref("");

/** 所有任务已完成（≥1 个任务）——「完成计划」亮起条件（PlanCompletionConfirm） */
const allTasksDone = computed(
  () => plan.value != null && plan.value.tasks.length > 0 && plan.value.tasks.every((t) => t.status === "Completed"),
);

/** 抢占不变式（ADR-0006）的 UI 面：存在进行中的更高优先级计划时，开始/继续/复制
 *  （复制即开始）禁用——服务层同规则兜底拒绝 */
const higherActive = computed<PlanView | null>(() => {
  const rank: Record<Priority, number> = { Low: 0, Medium: 1, High: 2 };
  const self = plan.value;
  if (!self) return null;
  return (
    allPlans.value.find((p) => p.status === "Active" && rank[p.priority] > rank[self.priority]) ??
    null
  );
});

/** 禁用 tooltip 的原因说明（span 承载——disabled 按钮不冒泡 hover；文案与错误提示同源） */
const lifecycleBlockedTitle = computed(() =>
  higherActive.value ? preemptedByHigherMessage(higherActive.value.name) : undefined,
);

/** 已推进的小时数（子目标任务按已完成子目标耗时；无子目标任务的汇报 07 接线后计入） */
const progressedHours = computed(() => {
  if (!plan.value) return "0";
  const done = plan.value.tasks.reduce(
    (sum, t) => sum + (t.has_subgoals ? taskProgress(t).completed : 0),
    0,
  );
  return hoursFromMinutes(done);
});

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

/** 任务的后续任务名列表（prerequisite_ids 指向本任务的任务）——"我挡谁"，
 *  与 depNames（"谁挡我"）对偶，依赖链在详情页双向可读 */
function waitingFor(t: TaskView): string[] {
  if (!plan.value) return [];
  return plan.value.tasks
    .filter((p) => p.id !== t.id && p.prerequisite_ids.includes(t.id))
    .map((p) => p.name);
}

async function load() {
  loadError.value = "";
  plan.value = null;
  try {
    // 详情与全量列表并行取：后者是"存在进行中的更高优先级"判定的数据源
    const [detail, all] = await Promise.all([getPlan(Number(route.params.id)), listPlans()]);
    plan.value = detail;
    allPlans.value = all;
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

/** 生命周期操作统一通道：busy 互斥、错误转文案、成功后重载（close* 收掉对应弹窗）。
 *  verb = 反馈文案的动作词（开始/继续/复制新建）。成功后跨窗口广播三个重取事件：
 *  大面板（候选/暂停分组刷新）、小看板（当前任务失效回空态）、今日总结（抢占标注实时重算）。
 *  actor 在 action 前捕获——复制并新建会跳转路由，不能用重载后的 plan 名。 */
async function runLifecycle(
  action: () => Promise<unknown>,
  close: { closeComplete?: boolean; closeAbort?: boolean } = {},
  verb = "开始",
) {
  busy.value = true;
  serverError.value = "";
  preemptNotice.value = [];
  const actor = plan.value?.name ?? "";
  try {
    const outcome = (await action()) as Partial<LifecycleOutcome> | undefined;
    if (outcome?.paused?.length) {
      preemptNotice.value = outcome.paused.map(
        (p) => `「${p.name}」已被自动暂停（高优先级「${actor}」${verb}）`,
      );
    }
    if (close.closeComplete) pendingComplete.value = false;
    if (close.closeAbort) abortStage.value = null;
    await load();
    void emitTo("main-board", MAIN_BOARD_REFRESH_EVENT);
    void emitTo("mini-board", MINI_BOARD_REFRESH_EVENT);
    void emitTo("daily-summary", SUMMARY_REFRESH_EVENT);
  } catch (err) {
    serverError.value = planErrorMessage(err as { kind?: string });
  } finally {
    busy.value = false;
  }
}

/** 复制并新建（终态唯一重启路径）：成功后跳到新计划详情；
 *  抢占清单回传给 runLifecycle 出反馈行 */
async function copyAsNew(): Promise<LifecycleOutcome> {
  const out = await copyPlanAsNew(plan.value!.id);
  await router.push(`/control-panel/plans/${out.new_plan_id}`);
  return { paused: out.paused };
}

/** 打开修正总进度弹窗：输入初值 = 当前进度（就近取一位小数） */
function openCorrection(t: TaskView) {
  correcting.value = t;
  correctValue.value = String(Math.round(t.progress_percent * 10) / 10);
  correctError.value = "";
}

/** 修正总进度：本地校验 0–100 任意正数（最多一位小数，2026-08-24 修订），
 *  服务层权威落账（差额以事件记账）；跨窗口重取由 runLifecycle 统一广播 */
async function applyCorrection() {
  const v = Number(correctValue.value);
  if (!isValidPercentValue(v, 0)) {
    correctError.value = "请填 0–100 的数值（最多一位小数）";
    return;
  }
  correctError.value = "";
  await runLifecycle(() => correctTotalProgress(correcting.value!.id, v));
  if (!serverError.value) {
    correcting.value = null; // 失败保留弹窗，错误文案透出
  }
}

onMounted(load);
watch(() => route.params.id, load);
</script>

<style scoped>
/* 页面骨架：纵向叠放（宽度归壳层统一内容列 ControlPanelWindow，720px） */
.detail-page {
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

.title-col {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.title-row {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.pause-reason {
  margin: 0;
  font-size: 12px;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

.lifecycle {
  display: inline-flex;
  gap: 8px;
}

.server-error {
  margin: 0;
  color: var(--color-danger);
  border: 1px solid var(--color-danger);
  border-radius: var(--radius-sm);
  padding: 8px 12px;
}

/* 自动暂停反馈（AutoPauseFeedback 第二层）：琥珀提示行，语气同总结的高优先级提示 */
.preempt-notice {
  border: 1px solid var(--color-progress);
  border-radius: var(--radius-sm);
  padding: 8px 12px;
  color: var(--color-progress);
}

.preempt-notice p {
  margin: 0;
  font-size: 13px;
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

/* 修正总进度弹窗：行内数字输入 */
.correct-input {
  width: 72px;
  padding: 5px 8px;
  text-align: center;
  font-variant-numeric: tabular-nums;
}

.correct-error {
  margin: 0;
  color: var(--color-danger);
  font-size: 13px;
}
</style>
