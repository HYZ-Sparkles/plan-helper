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
            <DatePicker v-model="plan.dueDate" />
          </FormField>
        </div>
      </div>
    </CollapsibleSection>

    <CollapsibleSection title="任务列表" :badge="tasks.length" :default-open="false">
      <div ref="tasksEl" class="tasks" :class="{ 'drag-live': drag.active }">
        <p v-if="tasks.length === 0" class="hint">还没有任务。计划保存时至少需要 1 个任务。</p>
        <article
          v-for="(task, i) in tasks"
          :key="task.key"
          class="task-card"
          :class="{
            expanded: !task.collapsed,
            dragging: drag.active && drag.index === i,
            settling: drag.settling && drag.index === i,
            locked: isLocked(task),
          }"
          :style="cardStyle(i)"
        >
          <!-- 常驻摘要行：整行可拖（手柄是视觉提示）+ 名称 + 耗时/子目标标记 + 删除；点行展开 -->
          <header
            class="task-line"
            @pointerdown="onLinePointerDown(i, $event)"
            @click="onLineClick(task)"
          >
            <PhDotsSixVertical v-if="draggable(task)" class="drag-handle" :size="16" />
            <span class="task-name" :class="{ unnamed: !task.name.trim() }">
              {{ task.name.trim() || `任务 ${i + 1}` }}
            </span>
            <StatusBadge v-if="isLocked(task)" :status="task.status" />
            <span class="task-mark">
              <template v-if="task.hasSubgoals">
                <PhListChecks :size="14" /> {{ task.subgoals.length }} 项
              </template>
              <template v-else>{{ task.hours ? `${task.hours} h` : "—" }}</template>
            </span>
            <button
              type="button"
              class="icon-btn danger"
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
                <input
                  type="checkbox"
                  :checked="task.hasSubgoals"
                  :disabled="isLocked(task)"
                  @change="onSubgoalToggle(task, $event)"
                />
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
            </div>

            <!-- 精简输入行（CONTEXT「子目标填写表单」）：内容 + 耗时，行末 + 与回车都加行聚焦内容框 -->
            <div v-if="task.hasSubgoals" class="subgoal-rows">
              <div
                v-for="(s, j) in task.subgoals"
                :key="s.id ?? `new-${j}`"
                class="subgoal-row"
                :class="{ done: s.completed }"
              >
                <PhCheckCircle v-if="s.completed" class="sg-done" :size="16" />
                <input
                  v-model="s.name"
                  class="input sg-name"
                  :class="{ invalid: taskErrors[i]?.subRows?.[j]?.name }"
                  :disabled="s.completed || isLocked(task)"
                  placeholder="子目标内容"
                  :data-sg="task.key"
                  @input="clearRowError(i, j, 'name')"
                  @keydown.enter.prevent="addSubgoal(task)"
                />
                <input
                  v-model="s.hours"
                  type="number"
                  min="0.25"
                  step="0.25"
                  class="input sg-hours"
                  :class="{ invalid: taskErrors[i]?.subRows?.[j]?.hours }"
                  :disabled="s.completed || isLocked(task)"
                  placeholder="耗时"
                  @input="clearRowError(i, j, 'hours')"
                  @keydown.enter.prevent="addSubgoal(task)"
                />
                <span class="sg-unit">h</span>
                <button
                  v-if="!s.completed"
                  type="button"
                  class="icon-btn danger"
                  title="删除这行子目标"
                  @click="removeSubgoal(i, j)"
                >
                  <PhX :size="14" />
                </button>
                <button
                  type="button"
                  class="icon-btn"
                  title="添加下一行（在输入框按回车同效）"
                  :disabled="isLocked(task)"
                  @click="addSubgoal(task)"
                >
                  <PhPlus :size="14" />
                </button>
              </div>
              <p v-if="taskErrors[i]?.subgoals" class="row-error">{{ taskErrors[i].subgoals }}</p>
              <p class="hint sg-tip">顺序 = 填写顺序，不可拖拽；已完成行锁定</p>
            </div>

            <!-- 次要区域：前置任务多选（创建与编辑同一控件，候选仅同计划其他任务） -->
            <div class="task-secondary">
              <DependencyEditor v-model="task.deps" :candidates="depCandidates(task)" :disabled="isLocked(task)" />
            </div>
          </div>
        </article>
        <button type="button" class="add-task" @click="addTask">
          <PhPlus :size="16" /> {{ isEdit ? "追加任务" : "添加任务" }}
        </button>
      </div>
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
        <li v-if="tasks[pendingDelete].hasSubgoals">
          子目标 {{ tasks[pendingDelete].subgoals.length }} 行（共 {{ subgoalHours(tasks[pendingDelete]) }} 小时）
        </li>
        <li v-else>预计耗时 {{ tasks[pendingDelete].hours || "—" }} 小时</li>
        <li v-if="tasks[pendingDelete].status === 'Completed'">已完成状态记录（该任务已完成）</li>
        <li v-else>当前状态：{{ statusLabel[tasks[pendingDelete].status] }}</li>
      </ul>
      <p>删除后任务进入归档，不再出现在计划中。</p>
    </TypeConfirmDialog>

    <!-- 取消勾选「需要子目标」：确认后清空全部子目标行 -->
    <ConfirmDialog
      v-if="pendingUncheck != null"
      title="取消「需要子目标」？"
      action-label="清空"
      @confirm="confirmUncheck"
      @cancel="pendingUncheck = null"
    >
      <p>将删除该任务的 {{ tasks[pendingUncheck].subgoals.length }} 个子目标，删除后不可恢复。</p>
    </ConfirmDialog>

    <!-- 删除单行子目标：确认后移除，后续行顺序递进 -->
    <ConfirmDialog
      v-if="pendingSubDelete != null"
      title="删除这行子目标？"
      @confirm="confirmSubDelete"
      @cancel="pendingSubDelete = null"
    >
      <p>
        子目标「{{ tasks[pendingSubDelete.task].subgoals[pendingSubDelete.row].name || "（未命名）" }}」
        将被移除，后续子目标顺序自动递进。
      </p>
    </ConfirmDialog>
  </div>
</template>

<script setup lang="ts">
import { nextTick, reactive, ref } from "vue";
import {
  PhCaretDown,
  PhCheckCircle,
  PhDotsSixVertical,
  PhListChecks,
  PhPlus,
  PhTrashSimple,
  PhX,
} from "@phosphor-icons/vue";
import AutoTextarea from "./AutoTextarea.vue";
import CollapsibleSection from "./CollapsibleSection.vue";
import ConfirmDialog from "./ConfirmDialog.vue";
import DatePicker from "./DatePicker.vue";
import DependencyEditor from "./DependencyEditor.vue";
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
import { normalizeDateString } from "../lib/validation";

/** 子目标表单行（hours 是输入态字符串，提交时换算分钟；completed 仅编辑态实义——锁定不可改） */
interface SubGoalRow {
  id?: number; // 已入库子目标 id；缺省 = 尚未入库的新行
  name: string;
  hours: string;
  completed: boolean;
}

/** 任务表单行（hours 同上；deps 存草稿内稳定 key 而非数组下标，拖拽重排不失效） */
interface TaskForm {
  key: number; // 草稿内稳定标识：依赖引用与追加行聚焦都靠它，与库中 id 无关
  id?: number; // 编辑态已入库任务 id；缺省 = 尚未入库的新任务
  status: TaskStatus; // 已完成 → 字段锁定不可拖（仅编辑态有实义）
  name: string;
  summary: string;
  detail: string;
  hasSubgoals: boolean;
  hours: string;
  collapsed: boolean;
  subgoals: SubGoalRow[];
  deps: number[]; // 前置任务的 key 集（保存时换算为数组下标 depends_on）
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

/** 任务稳定 key 发号器（表单会话内递增，创建/编辑共用） */
let keySeq = 0;
const nextKey = () => ++keySeq;

/** 编辑态从库中装载（默认收起成紧凑摘要行）；创建态给一张空白卡（展开即填） */
function taskFormOf(t: PlanView["tasks"][number]): TaskForm {
  return {
    key: nextKey(),
    id: t.id,
    status: t.status,
    name: t.name,
    summary: t.summary,
    detail: t.detail,
    hasSubgoals: t.has_subgoals,
    hours: t.estimated_minutes != null ? hoursFromMinutes(t.estimated_minutes) : "",
    collapsed: true,
    subgoals: t.subgoals.map((s) => ({
      id: s.id,
      name: s.name,
      hours: hoursFromMinutes(s.estimated_minutes),
      completed: s.completed,
    })),
    deps: [],
  };
}
function blankTask(): TaskForm {
  return {
    key: nextKey(),
    status: "NotStarted",
    name: "",
    summary: "",
    detail: "",
    hasSubgoals: false,
    hours: "",
    collapsed: true,
    subgoals: [],
    deps: [],
  };
}
/** 编辑装载：先建行拿 key，再把库中前置 id 换算成 key（依赖引用跨拖拽重排稳定） */
function loadTasks(): TaskForm[] {
  if (!isEdit || !props.plan) return [{ ...blankTask() }];
  const forms = props.plan.tasks.map(taskFormOf);
  const keyOfId = new Map(props.plan.tasks.map((t, i) => [t.id, forms[i].key]));
  props.plan.tasks.forEach((t, i) => {
    forms[i].deps = t.prerequisite_ids.flatMap((id) => {
      const key = keyOfId.get(id);
      return key != null ? [key] : [];
    });
  });
  return forms;
}
const tasks = reactive<TaskForm[]>(loadTasks());

/** 各段独立校验错误：计划段名称一条、每张任务卡一条（按段/卡显示，互不阻塞其它段）；
 *  subRows 为子目标行级红框标记（保存尝试后才有值） */
const planErrors = reactive({ name: "" });
const taskErrors = reactive<
  Record<number, { name?: string; hours?: string; subgoals?: string; subRows?: Record<number, { name?: boolean; hours?: boolean }> }>
>({});
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

/* ---- 指针拖拽排序（2026-08-24 共识：连续流动，无显式磁吸）----
   整行可拖 + 手柄仅作视觉提示；点击（展开/收起）与拖拽用位移阈值区分；
   未完成任务是连续前缀（新任务插在已完成块之前），已锁定块沉底不动也不让位。 */
const tasksEl = ref<HTMLElement>();
/** 拖拽会话：index = 拖的卡、active = 已越过阈值、dy = 指针位移、drop = 流动预览槽位、settling = 松手滑入中 */
const drag = reactive({ index: -1, active: false, settling: false, dy: 0, drop: -1 });
/** 起拖时快照：各卡中心（视口坐标）与被拖卡高度——槽位几何在松手提交前不变 */
let slotCenters: number[] = [];
let rowHeight = 0;
let rowGap = 0;
let startY = 0;
/** 未完成任务数（可拖/可让位的前缀长度；已锁定任务加载时必然沉底成后缀） */
let movableCount = 0;
/** 松手滑入动画的收尾计时器；新一次按下会先提交未完成的收尾 */
let settleTimer: number | undefined;
/** 真拖拽后的第一次 click 只吞掉（不切换展开） */
let suppressClick = false;
/** 起拖阈值：位移超过它才算拖拽，否则视为摘要行上的普通点击 */
const DRAG_THRESHOLD = 4;

/** 摘要行按下：快照几何、捕获指针（出窗口不丢事件）；交互元素（删除钮等）不触发 */
function onLinePointerDown(i: number, e: PointerEvent) {
  if (e.button !== 0 || !tasksEl.value || !draggable(tasks[i])) return;
  if ((e.target as HTMLElement).closest("button, input, textarea, label")) return;
  commitDrop();
  const cards = Array.from(tasksEl.value.children).filter((el) => el.classList.contains("task-card"));
  const rects = cards.map((el) => (el as HTMLElement).getBoundingClientRect());
  slotCenters = rects.map((r) => r.top + r.height / 2);
  rowHeight = rects[i].height;
  rowGap = rects.length > 1 ? rects[1].top - (rects[0].top + rects[0].height) : 0;
  startY = e.clientY;
  movableCount = tasks.filter((t) => !isLocked(t)).length;
  drag.index = i;
  drag.drop = i;
  const line = e.currentTarget as HTMLElement;
  line.setPointerCapture(e.pointerId);
  line.addEventListener("pointermove", onLinePointerMove);
  line.addEventListener("pointerup", onLinePointerUp);
  line.addEventListener("pointercancel", onLinePointerUp);
}

/** 指针移动：越过阈值起拖；拖动卡当前中心 → 最近的可让位槽位（锁定块不参与），
 *  他卡随槽位变化连续平移让位/合拢（CSS 过渡承担流动感） */
function onLinePointerMove(e: PointerEvent) {
  drag.dy = e.clientY - startY;
  if (!drag.active && Math.abs(drag.dy) <= DRAG_THRESHOLD) return;
  drag.active = true;
  const centerNow = slotCenters[drag.index] + drag.dy;
  let nearest = 0;
  for (let j = 1; j < movableCount; j++) {
    if (Math.abs(centerNow - slotCenters[j]) < Math.abs(centerNow - slotCenters[nearest])) nearest = j;
  }
  drag.drop = nearest;
}

/** 指针抬起：摘监听；纯点击交回 click 展开，真拖拽切换为滑入动画后提交 */
function onLinePointerUp(e: PointerEvent) {
  const line = e.currentTarget as HTMLElement;
  line.removeEventListener("pointermove", onLinePointerMove);
  line.removeEventListener("pointerup", onLinePointerUp);
  line.removeEventListener("pointercancel", onLinePointerUp);
  if (!drag.active) return;
  suppressClick = true;
  drag.dy = slotCenters[drag.drop] - slotCenters[drag.index]; // 目标：滑入预览槽位
  drag.settling = true; // 跟手时无过渡，此刻起开启过渡
  settleTimer = window.setTimeout(commitDrop, 200); // 略长于 160ms 滑入，避免截尾微跳
}

/** 拖拽收尾：数组落位（key 稳定，DOM 随之移动到与视觉一致的位置）并复位会话 */
function commitDrop() {
  window.clearTimeout(settleTimer);
  if (drag.index < 0) return;
  const { index, drop } = drag;
  drag.index = -1;
  drag.drop = -1;
  drag.active = false;
  drag.settling = false;
  drag.dy = 0;
  if (index !== drop) {
    const [moved] = tasks.splice(index, 1);
    tasks.splice(drop, 0, moved);
  }
}

/** 拖拽中的卡位移：被拖卡跟手（无过渡）/滑入（过渡），区间内他卡 ± 一个被拖卡高让位 */
function cardStyle(i: number) {
  if (!drag.active || drag.index < 0) return undefined;
  let offset: number | undefined;
  if (i === drag.index) {
    offset = drag.dy;
  } else {
    const shift = rowHeight + rowGap;
    if (drag.index < drag.drop && i > drag.index && i <= drag.drop) offset = -shift;
    else if (drag.drop < drag.index && i >= drag.drop && i < drag.index) offset = shift;
  }
  if (offset == null) return undefined;
  return { transform: `translateY(${offset}px)`, zIndex: i === drag.index ? 10 : 1 };
}

/** 摘要行点击 = 展开/收起；拖拽手势刚结束的那次 click 吞掉不切换 */
function onLineClick(task: TaskForm) {
  if (suppressClick) {
    suppressClick = false;
    return;
  }
  task.collapsed = !task.collapsed;
}

/* ---- 任务增删 ---- */

/** 追加任务：插在所有未完成任务之后、已完成块之前（与服务端落库顺序一致，spec 用户故事 9） */
function addTask() {
  tasks.splice(tasks.filter((t) => !isLocked(t)).length, 0, blankTask());
}

/** 待删除任务的下标（null = 无弹窗）；只有已入库任务走打字确认 */
const pendingDelete = ref<number | null>(null);

/** 移除所有指向该任务的前置引用（删任务后依赖悬空在服务层是 TaskSetMismatch，这里先清干净） */
function purgeDeps(key: number) {
  for (const t of tasks) {
    t.deps = t.deps.filter((k) => k !== key);
  }
}

function removeTask(i: number) {
  if (tasks[i].id == null) {
    purgeDeps(tasks[i].key);
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
    purgeDeps(tasks[i].key);
    tasks.splice(i, 1);
  } catch (err) {
    serverError.value = planErrorMessage(err as { kind?: string });
  } finally {
    pendingDelete.value = null;
  }
}

/* ---- 子目标精简输入行 ---- */

/** 勾选/取消「需要子目标」：勾选即给第一行开始流式录入；取消时已有行先确认再清空；
 *  存在已完成行时直接阻止（服务端必拒 SubGoalLocked，UI 不给能兑现的假承诺） */
function onSubgoalToggle(task: TaskForm, e: Event) {
  const box = e.target as HTMLInputElement;
  const i = tasks.indexOf(task);
  if (box.checked) {
    task.hasSubgoals = true;
    if (task.subgoals.length === 0) addSubgoal(task);
    return;
  }
  if (task.subgoals.length === 0) {
    task.hasSubgoals = false; // 没有可删的行，直接取消
    return;
  }
  box.checked = true; // 撤销视觉勾选，等确认/阻止提示
  if (task.subgoals.some((s) => s.completed)) {
    taskErrors[i] = { ...taskErrors[i], subgoals: "有已完成子目标，不能取消勾选（已完成内容锁定）" };
    task.collapsed = false;
    return;
  }
  pendingUncheck.value = i;
}

/** 待确认「取消子目标勾选」的任务下标 */
const pendingUncheck = ref<number | null>(null);

function confirmUncheck() {
  const t = tasks[pendingUncheck.value!];
  t.hasSubgoals = false;
  t.subgoals = [];
  const errs = taskErrors[pendingUncheck.value!];
  if (errs) {
    errs.subgoals = "";
    errs.subRows = {};
  }
  pendingUncheck.value = null;
}

/** 加一行并聚焦其内容框（+ 按钮与回车共用；顺序 = 填写顺序，恒追加在末尾） */
async function addSubgoal(task: TaskForm) {
  task.subgoals.push({ name: "", hours: "", completed: false });
  await nextTick();
  const inputs = document.querySelectorAll<HTMLInputElement>(`[data-sg="${task.key}"]`);
  inputs[inputs.length - 1]?.focus();
}

/** 待确认删除的子目标行（task = 任务下标，row = 行下标）；空白新行直接删不走确认 */
const pendingSubDelete = ref<{ task: number; row: number } | null>(null);

function removeSubgoal(ti: number, ri: number) {
  const row = tasks[ti].subgoals[ri];
  if (row.id == null && !row.name.trim() && !row.hours.trim()) {
    tasks[ti].subgoals.splice(ri, 1); // 空白草稿行：无内容可丢
    return;
  }
  pendingSubDelete.value = { task: ti, row: ri };
}

function confirmSubDelete() {
  const { task, row } = pendingSubDelete.value!;
  tasks[task].subgoals.splice(row, 1);
  pendingSubDelete.value = null;
}

/** 行级错误随输入清除（字段标记与该卡子目标聚合消息一起清） */
function clearRowError(ti: number, ri: number, field: "name" | "hours") {
  const errs = taskErrors[ti];
  if (!errs) return;
  errs.subgoals = "";
  if (errs.subRows?.[ri]) {
    delete errs.subRows[ri][field];
    if (!errs.subRows[ri].name && !errs.subRows[ri].hours) delete errs.subRows[ri];
  }
}

/** 删除任务确认弹窗里的子目标总耗时展示（小时，一位小数内自然显示） */
function subgoalHours(task: TaskForm): string {
  const sum = task.subgoals.reduce((acc, s) => acc + (parseFloat(s.hours) || 0), 0);
  return Math.round(sum * 10) / 10 + "";
}

/** 前置任务候选：同计划其它任务（不含自己；名称未填时显示占位序号） */
function depCandidates(task: TaskForm) {
  return tasks
    .filter((t) => t.key !== task.key)
    .map((t) => ({ key: t.key, name: t.name.trim() || `任务 ${tasks.indexOf(t) + 1}`, status: t.status }));
}

/** 保存前 UI 按段校验（领域层还有权威校验兜底）；出错的任务卡自动展开。
 *  截止日期不在这里查——DatePicker 组件自身保证值要么为空要么是合法 YYYY-MM-DD。 */
function validate(): boolean {
  planErrors.name = plan.name.trim() ? "" : "请填写计划名称";
  let ok = !planErrors.name;
  for (const [i, t] of tasks.entries()) {
    const errs: { name?: string; hours?: string; subgoals?: string } = {};
    if (!t.name.trim()) errs.name = "请填写任务名称";
    if (!t.hasSubgoals && !(parseFloat(t.hours) > 0)) errs.hours = "无子目标任务必填预计耗时";
    if (t.hasSubgoals) {
      const subRows: Record<number, { name?: boolean; hours?: boolean }> = {};
      t.subgoals.forEach((s, j) => {
        subRows[j] = { name: !s.name.trim(), hours: !(parseFloat(s.hours) > 0) };
      });
      if (t.subgoals.length === 0) errs.subgoals = "勾选了子目标，至少需要 1 行";
      else if (t.subgoals.some((s) => !s.name.trim() || !(parseFloat(s.hours) > 0))) {
        errs.subgoals = "有子目标行缺少内容或预计耗时";
      }
      taskErrors[i] = { ...errs, subRows };
    } else {
      taskErrors[i] = errs;
    }
    if (errs.name || errs.hours || errs.subgoals) {
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
    due_date: normalizeDateString(plan.dueDate), // 边界兜底：垃圾输入归 null（无截止），合法归一 YYYY-MM-DD
    tasks: tasks.map((t) => ({
      id: t.id,
      name: t.name,
      summary: t.summary,
      detail: t.detail,
      has_subgoals: t.hasSubgoals,
      estimated_minutes: t.hasSubgoals ? undefined : Math.round(parseFloat(t.hours) * 60),
      // 取消勾选（确认后）subgoals 已清空，这里自然提交空集
      subgoals: t.subgoals.map((s) => ({
        id: s.id,
        name: s.name,
        estimated_minutes: Math.round(parseFloat(s.hours) * 60),
      })),
      // key → 当前数组下标（服务层按草稿下标解析依赖，含新任务）
      depends_on: t.deps
        .map((k) => tasks.findIndex((x) => x.key === k))
        .filter((i) => i >= 0),
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

/* 计划段字段行（优先级+截止日期）顶部对齐：两字段都有标签行，控件起点一致；
   行内错误/控件高度差不再把旁边字段顶歪 */
.plan-fields .field-row {
  align-items: flex-start;
}

.input.small {
  width: 120px;
}

.segmented {
  display: inline-flex;
  align-items: stretch;
  border: var(--border-default);
  border-radius: var(--radius-sm);
  overflow: hidden;
}

/* 行内两控件（优先级分段 / 日期输入）严格等高：显式 36px 且统一 border-box——
   .input 无 border-box，min-height 会算在内容区上导致实际约 52px，比邻控件高一大截 */
.plan-fields .field-row .segmented,
.plan-fields .field-row :deep(.date-picker .input) {
  box-sizing: border-box;
  height: 36px;
}

.segmented.locked {
  opacity: 0.55;
}

.lock-hint {
  display: block;
  margin-top: 4px;
}

.segment {
  flex: 1;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 4px 14px;
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
  position: relative; /* 拖拽时被拖卡需置顶浮起 */
  border: var(--border-default);
  border-radius: var(--radius-md);
  background: var(--bg-group);
  overflow: hidden;
  /* 落位后回实/收条平滑；transform 的过渡绝不能放这——commitDrop 清位移时若仍有
     transition，浏览器会把"位移→无位移"动画化，卡片弹回旧位置再滑回来（闪动） */
  transition: opacity 0.15s, box-shadow 0.15s;
}

/* 拖拽进行中：整列表禁选中、摘要行变抓取光标；transform 过渡只在这段生效——
   落位帧 drag-live 与位移同帧移除，"变化后样式"无 transition 即不播动画，直接吸附到位 */
.tasks.drag-live {
  user-select: none;
}

.tasks.drag-live .task-line {
  cursor: grabbing;
}

/* 他卡让位/合拢的流动感 */
.tasks.drag-live .task-card {
  transition: transform 0.15s, opacity 0.15s, box-shadow 0.15s;
}

/* 被拖卡：半透明 + 顶部 3px 主色条；跟手位移不能有过渡（会滞后于指针） */
.task-card.dragging {
  opacity: 0.5;
  box-shadow: inset 0 3px 0 var(--primary);
  z-index: 10;
}

.tasks.drag-live .task-card.dragging {
  transition: none;
}

/* 松手后：被拖卡以过渡滑入预览槽位，随后 commitDrop 落定 */
.tasks.drag-live .task-card.settling {
  transition: transform 0.16s ease-out;
}

/* 紧凑摘要行（常驻，矮行）：一行放下手柄/名称/标记/操作；指针拖拽不被触摸滚动手势抢走 */
.task-line {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 38px;
  padding: 6px 10px;
  cursor: pointer;
  user-select: none;
  touch-action: none;
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

/* 图标按钮形态走 tokens.css 全局 .icon-btn（删除类带 danger 变体） */

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

.toggle input {
  accent-color: var(--primary);
}

/* 子目标精简输入行：内容 + 耗时 + 删除/加行，一行放下（紧凑流式录入） */
.subgoal-rows {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.subgoal-row {
  display: flex;
  align-items: center;
  gap: 6px;
}

.subgoal-row.done .sg-name {
  color: var(--text-muted);
}

.sg-done {
  color: var(--color-done);
  flex: none;
}

.sg-name {
  flex: 1;
  min-width: 0;
}

.sg-hours {
  width: 72px;
  text-align: right;
  flex: none;
}

.sg-unit {
  color: var(--text-muted);
  font-size: 12px;
}

/* 行级校验红框（保存尝试后标记，随输入清除） */
.input.invalid {
  border-color: var(--color-danger);
}

.row-error {
  margin: 0;
  font-size: 12px;
  color: var(--color-danger);
}

.sg-tip {
  margin: 0;
  font-size: 12px;
}

/* 任务卡展开态次要区域：前置任务多选收在这里，与主字段区分 */
.task-secondary {
  margin-top: 4px;
  padding-top: 10px;
  border-top: var(--border-default);
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
