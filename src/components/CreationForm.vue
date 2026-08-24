<template>
  <!--
    CreationUI：创建/编辑双模式共用的单页可折叠表单（CONTEXT「创建流程的 UI 模式」）。
    Plan 段默认展开；任务卡默认收起为一行摘要（拖拽手柄 + 名称 + 耗时/子目标标记），
    点行展开完整字段——长字段只在展开态出现（2026-08-23 grill 决策）。
    编辑态额外规则：已完成任务锁定（字段禁用 + 不可拖），计划开始后优先级锁定。
  -->
  <div class="creation-form">
    <CollapsibleSection title="计划" :default-open="true">
      <div class="plan-fields">
        <FormField label="名称" :required="true" :error="planErrors.name">
          <input v-model="plan.name" class="input" placeholder="如：备考英语 6 级" @input="planErrors.name = ''" />
        </FormField>
        <FormField label="简述">
          <input v-model="plan.summary" class="input" placeholder="留空默认与名称相同" />
        </FormField>
        <FormField label="详细内容">
          <AutoTextarea v-model="plan.detail" placeholder="计划背景、步骤、想法……（纯文本，不限长度）" />
        </FormField>
        <div class="field-row">
          <FormField label="优先级">
            <div class="segmented" :class="{ locked: priorityLocked }">
              <button
                v-for="p in priorities"
                :key="p"
                type="button"
                class="segment"
                :class="{ selected: plan.priority === p }"
                :disabled="priorityLocked"
                @click="plan.priority = p"
              >
                <PriorityLabel :priority="p" />
              </button>
            </div>
            <span v-if="priorityLocked" class="hint lock-hint">计划开始后优先级不可调整</span>
          </FormField>
          <FormField label="截止日期">
            <input v-model="plan.dueDate" type="date" class="input" />
          </FormField>
        </div>
      </div>
    </CollapsibleSection>

    <CollapsibleSection title="任务列表" :badge="tasks.length" :default-open="false">
      <div class="tasks">
        <p v-if="tasks.length === 0" class="hint">还没有任务。计划保存时至少需要 1 个任务。</p>
        <article
          v-for="(task, i) in tasks"
          :key="task.id ?? `new-${i}`"
          class="task-card"
          :class="{ expanded: !task.collapsed, dragging: dragFrom === i, locked: isLocked(task) }"
        >
          <!-- 常驻摘要行：拖拽手柄 + 名称 + 耗时/子目标标记 + 删除；点行展开 -->
          <header
            class="task-line"
            :draggable="draggable(task)"
            @click="task.collapsed = !task.collapsed"
            @dragstart="onDragStart(i, $event)"
            @dragover.prevent
            @drop="onDrop(i)"
            @dragend="dragFrom = -1"
          >
            <PhDotsSixVertical v-if="draggable(task)" class="drag-handle" :size="16" />
            <span class="task-name" :class="{ unnamed: !task.name.trim() }">
              {{ task.name.trim() || `任务 ${i + 1}` }}
            </span>
            <StatusBadge v-if="isLocked(task)" :status="task.status" />
            <span class="task-mark">
              <PhListChecks v-if="task.hasSubgoals" :size="14" />
              <template v-else>{{ task.hours ? `${task.hours} h` : "—" }}</template>
            </span>
            <button
              type="button"
              class="icon-btn"
              title="删除任务"
              @click.stop="removeTask(i)"
            >
              <PhTrashSimple :size="16" />
            </button>
            <PhCaretDown class="chevron" :class="{ open: !task.collapsed }" :size="14" />
          </header>

          <!-- 展开态：完整字段（长字段只在这里出现） -->
          <div v-show="!task.collapsed" class="task-fields">
            <p v-if="isLocked(task)" class="hint locked-hint">已完成任务字段锁定，不可修改</p>
            <FormField label="名称" :required="true" :error="taskErrors[i]?.name">
              <input
                v-model="task.name"
                class="input"
                :disabled="isLocked(task)"
                @input="taskErrors[i] && (taskErrors[i].name = '')"
              />
            </FormField>
            <FormField label="简述">
              <input v-model="task.summary" class="input" placeholder="留空默认与名称相同" :disabled="isLocked(task)" />
            </FormField>
            <FormField label="详细内容">
              <AutoTextarea v-model="task.detail" :disabled="isLocked(task)" />
            </FormField>
            <div class="field-row">
              <label class="toggle" title="需要子目标吗？">
                <input v-model="task.hasSubgoals" type="checkbox" :disabled="isLocked(task)" />
                <PhListChecks :size="16" />
                需要子目标
              </label>
              <FormField
                v-if="!task.hasSubgoals"
                label="预计耗时（小时）"
                :required="true"
                :error="taskErrors[i]?.hours"
              >
                <input
                  v-model="task.hours"
                  type="number"
                  min="0.5"
                  step="0.5"
                  class="input small"
                  :disabled="isLocked(task)"
                  @input="taskErrors[i] && (taskErrors[i].hours = '')"
                />
              </FormField>
              <p v-else class="hint subgoal-hint">子目标录入即将提供——当前版本请关闭此开关并填写预计耗时</p>
            </div>
          </div>
        </article>
        <button type="button" class="add-task" @click="addTask">
          <PhPlus :size="16" /> {{ isEdit ? "追加任务" : "添加任务" }}
        </button>
      </div>
    </CollapsibleSection>

    <CollapsibleSection title="子目标" :default-open="false">
      <p class="hint">
        在任务卡勾选「需要子目标」后，对应的精简输入行会出现在这里（子目标录入在下一张工单提供）。
        当前版本请保持开关关闭并填写任务预计耗时。
      </p>
    </CollapsibleSection>

    <footer v-if="serverError" class="server-error">{{ serverError }}</footer>
    <footer class="actions">
      <button v-if="isEdit" type="button" class="ghost-btn" :disabled="saving" @click="emit('cancel')">
        取消
      </button>
      <button type="button" class="primary-btn" :disabled="saving" @click="save">
        {{ saving ? "保存中……" : isEdit ? "保存修改" : "创建计划" }}
      </button>
    </footer>

    <!-- 删除已入库任务的打字确认（未入库草稿直接移除，无进度可丢） -->
    <TypeConfirmDialog
      v-if="pendingDelete != null"
      :title="`删除任务「${tasks[pendingDelete].name}」？`"
      @confirm="confirmDelete"
      @cancel="pendingDelete = null"
    >
      <p>将丢失的内容：</p>
      <ul class="loss-list">
        <li>预计耗时 {{ tasks[pendingDelete].hours || "—" }} 小时</li>
        <li v-if="tasks[pendingDelete].status === 'Completed'">已完成状态记录（该任务已完成）</li>
        <li v-else>当前状态：{{ statusLabel[tasks[pendingDelete].status] }}</li>
      </ul>
      <p>删除后任务进入归档，不再出现在计划中。</p>
    </TypeConfirmDialog>
  </div>
</template>

<script setup lang="ts">
import { reactive, ref } from "vue";
import {
  PhCaretDown,
  PhDotsSixVertical,
  PhListChecks,
  PhPlus,
  PhTrashSimple,
} from "@phosphor-icons/vue";
import AutoTextarea from "./AutoTextarea.vue";
import CollapsibleSection from "./CollapsibleSection.vue";
import FormField from "./FormField.vue";
import PriorityLabel from "./PriorityLabel.vue";
import StatusBadge from "./StatusBadge.vue";
import TypeConfirmDialog from "./TypeConfirmDialog.vue";
import {
  createPlan,
  deleteTask,
  updatePlan,
  type PlanDraft,
  type PlanView,
  type Priority,
  type TaskStatus,
} from "../lib/api";
import { hoursFromMinutes, planErrorMessage, statusLabel } from "../lib/labels";

/** 任务表单行（hours 是输入态字符串，提交时换算分钟；collapsed 是紧凑卡收起态） */
interface TaskForm {
  id?: number; // 编辑态已入库任务 id；缺省 = 尚未入库的新任务
  status: TaskStatus; // 已完成 → 字段锁定不可拖（仅编辑态有实义）
  name: string;
  summary: string;
  detail: string;
  hasSubgoals: boolean;
  hours: string;
  collapsed: boolean;
}

const props = defineProps<{
  mode: "create" | "edit";
  /** 编辑态的源计划（创建态不传） */
  plan?: PlanView;
}>();
const emit = defineEmits<{ saved: []; cancel: [] }>();

const isEdit = props.mode === "edit";
const priorities: Priority[] = ["High", "Medium", "Low"];

/** 计划开始后优先级锁定（spec 用户故事 12；服务层同规则兜底） */
const priorityLocked = isEdit && props.plan != null && props.plan.status !== "NotStarted";

const plan = reactive(
  isEdit && props.plan
    ? {
        name: props.plan.name,
        summary: props.plan.summary,
        detail: props.plan.detail,
        priority: props.plan.priority,
        dueDate: props.plan.due_date ?? "",
      }
    : { name: "", summary: "", detail: "", priority: "Medium" as Priority, dueDate: "" },
);

/** 编辑态从库中装载（默认收起成紧凑摘要行）；创建态给一张空白卡（展开即填） */
function taskFormOf(t: PlanView["tasks"][number]): TaskForm {
  return {
    id: t.id,
    status: t.status,
    name: t.name,
    summary: t.summary,
    detail: t.detail,
    hasSubgoals: t.has_subgoals,
    hours: t.estimated_minutes != null ? hoursFromMinutes(t.estimated_minutes) : "",
    collapsed: true,
  };
}
function blankTask(): TaskForm {
  return { status: "NotStarted", name: "", summary: "", detail: "", hasSubgoals: false, hours: "", collapsed: true };
}
const tasks = reactive<TaskForm[]>(
  isEdit && props.plan ? props.plan.tasks.map(taskFormOf) : [{ ...blankTask() }],
);

/** 各段独立校验错误：计划段一条、每张任务卡一条（按段/卡显示，互不阻塞其它段） */
const planErrors = reactive({ name: "" });
const taskErrors = reactive<Record<number, { name?: string; hours?: string }>>({});
const serverError = ref("");
const saving = ref(false);

/** 已完成任务锁定（README 增删改规则：已完成的不允许修改） */
function isLocked(task: TaskForm): boolean {
  return isEdit && task.status === "Completed";
}
/** 可拖行：锁定任务不可拖（重排后已完成必然沉底，服务层兜底归一化） */
function draggable(task: TaskForm): boolean {
  return !isLocked(task);
}

/* ---- 拖拽排序（创建与编辑同一交互；HTML5 DnD，落点交换） ---- */
const dragFrom = ref(-1);

function onDragStart(i: number, e: DragEvent) {
  dragFrom.value = i;
  e.dataTransfer!.effectAllowed = "move";
}
function onDrop(i: number) {
  const from = dragFrom.value;
  dragFrom.value = -1;
  if (from < 0 || from === i) return;
  const [moved] = tasks.splice(from, 1);
  tasks.splice(i, 0, moved);
}

/* ---- 任务增删 ---- */
function addTask() {
  tasks.push(blankTask());
}

/** 待删除任务的下标（null = 无弹窗）；只有已入库任务走打字确认 */
const pendingDelete = ref<number | null>(null);

function removeTask(i: number) {
  if (tasks[i].id == null) {
    tasks.splice(i, 1); // 未入库草稿：无进度可丢，直接移除
    return;
  }
  pendingDelete.value = i;
}
async function confirmDelete() {
  const i = pendingDelete.value!;
  serverError.value = "";
  try {
    await deleteTask(tasks[i].id!);
    tasks.splice(i, 1);
  } catch (err) {
    serverError.value = planErrorMessage(err as { kind?: string });
  } finally {
    pendingDelete.value = null;
  }
}

/** 保存前 UI 按段校验（领域层还有权威校验兜底）；出错的任务卡自动展开 */
function validate(): boolean {
  planErrors.name = plan.name.trim() ? "" : "请填写计划名称";
  let ok = !planErrors.name;
  for (const [i, t] of tasks.entries()) {
    const errs: { name?: string; hours?: string } = {};
    if (!t.name.trim()) errs.name = "请填写任务名称";
    if (!t.hasSubgoals && !(parseFloat(t.hours) > 0)) errs.hours = "无子目标任务必填预计耗时";
    taskErrors[i] = errs;
    if (errs.name || errs.hours) {
      ok = false;
      t.collapsed = false; // 收起态的错卡展开，让用户看到错误在哪
    }
  }
  return ok;
}

function buildDraft(): PlanDraft {
  return {
    name: plan.name,
    summary: plan.summary,
    detail: plan.detail,
    priority: plan.priority,
    due_date: plan.dueDate || null,
    tasks: tasks.map((t) => ({
      id: t.id,
      name: t.name,
      summary: t.summary,
      detail: t.detail,
      has_subgoals: t.hasSubgoals,
      estimated_minutes: t.hasSubgoals ? undefined : Math.round(parseFloat(t.hours) * 60),
    })),
  };
}

async function save() {
  serverError.value = "";
  if (!validate()) return;
  saving.value = true;
  try {
    if (isEdit && props.plan) {
      await updatePlan(props.plan.id, buildDraft());
    } else {
      await createPlan(buildDraft());
    }
    emit("saved");
  } catch (err) {
    serverError.value = planErrorMessage(err as { kind?: string });
  } finally {
    saving.value = false;
  }
}
</script>

<style scoped>
.creation-form {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.plan-fields,
.task-fields {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.field-row {
  display: flex;
  align-items: flex-end;
  gap: 16px;
  flex-wrap: wrap;
}

.input.small {
  width: 120px;
}

.segmented {
  display: inline-flex;
  border: var(--border-default);
  border-radius: var(--radius-sm);
  overflow: hidden;
}

.segmented.locked {
  opacity: 0.55;
}

.lock-hint {
  display: block;
  margin-top: 4px;
}

.segment {
  display: inline-flex;
  padding: 4px 12px;
  border: none;
  border-right: var(--border-default);
  background: var(--surface);
  cursor: pointer;
}

.segment:last-child {
  border-right: none;
}

.segment.selected {
  background: var(--bg-accent-group);
  box-shadow: inset 0 -3px 0 var(--primary);
}

.segment:disabled {
  cursor: not-allowed;
}

.tasks {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.task-card {
  border: var(--border-default);
  border-radius: var(--radius-md);
  background: var(--bg-group);
  overflow: hidden;
}

.task-card.dragging {
  opacity: 0.5;
  box-shadow: inset 0 3px 0 var(--primary);
}

/* 紧凑摘要行（常驻，矮行）：一行放下手柄/名称/标记/操作 */
.task-line {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 38px;
  padding: 6px 10px;
  cursor: pointer;
  user-select: none;
}

.drag-handle {
  color: var(--text-muted);
  cursor: grab;
}

.task-name {
  flex: 1;
  font-weight: 600;
  font-size: 14px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.task-name.unnamed {
  color: var(--text-muted);
  font-weight: 400;
}

.task-card.locked .task-line {
  cursor: default;
}

.task-mark {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--text-muted);
  font-size: 12px;
  white-space: nowrap;
}

.icon-btn {
  display: inline-flex;
  padding: 4px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
}

.icon-btn:hover {
  color: var(--color-danger);
}

.chevron {
  color: var(--text-muted);
  transition: transform 0.15s;
}

.chevron.open {
  transform: rotate(180deg);
}

.task-fields {
  padding: 12px 16px;
  border-top: var(--border-default);
}

.locked-hint {
  margin: 0;
}

.toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--text-secondary);
  cursor: pointer;
  padding-bottom: 8px;
}

.subgoal-hint {
  padding-bottom: 8px;
}

.add-task {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 8px 14px;
  border: var(--border-default);
  border-radius: var(--radius-sm);
  background: var(--surface);
  color: var(--text-secondary);
  font: inherit;
  cursor: pointer;
  align-self: flex-start;
}

.add-task:hover {
  border: var(--border-active);
  color: var(--text-primary);
}

.server-error {
  color: var(--color-danger);
  border: 1px solid var(--color-danger);
  border-radius: var(--radius-sm);
  padding: 8px 12px;
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}

.loss-list {
  margin: 4px 0;
  padding-left: 20px;
}
</style>
