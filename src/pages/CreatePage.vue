<template>
  <!--
    创建计划（工单 02：无子目标计划）。单页可折叠表单（CreationUI）：
    Plan 段默认展开；任务列表段默认折叠；按段独立校验，无向导步骤条。
  -->
  <section class="create-page">
    <h2 class="page-title">创建计划</h2>

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
            <div class="segmented">
              <button
                v-for="p in priorities"
                :key="p"
                type="button"
                class="segment"
                :class="{ selected: plan.priority === p }"
                @click="plan.priority = p"
              >
                <PriorityLabel :priority="p" />
              </button>
            </div>
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
        <article v-for="(task, i) in tasks" :key="i" class="task-card">
          <header class="task-head">
            <span class="task-no">任务 {{ i + 1 }}</span>
            <button type="button" class="icon-btn" title="移除此任务" @click="tasks.splice(i, 1)">
              <PhTrashSimple :size="16" />
            </button>
          </header>
          <div class="task-fields">
            <FormField label="名称" :required="true" :error="taskErrors[i]?.name">
              <input v-model="task.name" class="input" @input="taskErrors[i] && (taskErrors[i].name = '')" />
            </FormField>
            <FormField label="简述">
              <input v-model="task.summary" class="input" placeholder="留空默认与名称相同" />
            </FormField>
            <FormField label="详细内容">
              <AutoTextarea v-model="task.detail" />
            </FormField>
            <div class="field-row">
              <label class="toggle" title="需要子目标吗？">
                <input v-model="task.hasSubgoals" type="checkbox" />
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
                  @input="taskErrors[i] && (taskErrors[i].hours = '')"
                />
              </FormField>
              <p v-else class="hint subgoal-hint">子目标录入即将提供——当前版本请关闭此开关并填写预计耗时</p>
            </div>
          </div>
        </article>
        <button type="button" class="add-task" @click="addTask">
          <PhPlus :size="16" /> 添加任务
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
      <button type="button" class="primary-btn" :disabled="saving" @click="save">
        {{ saving ? "保存中……" : "创建计划" }}
      </button>
    </footer>
  </section>
</template>

<script setup lang="ts">
import { reactive, ref } from "vue";
import { PhListChecks, PhPlus, PhTrashSimple } from "@phosphor-icons/vue";
import { useRouter } from "vue-router";
import AutoTextarea from "../components/AutoTextarea.vue";
import CollapsibleSection from "../components/CollapsibleSection.vue";
import FormField from "../components/FormField.vue";
import PriorityLabel from "../components/PriorityLabel.vue";
import { createPlan, type NewPlan, type Priority } from "../lib/api";
import { planErrorMessage } from "../lib/labels";

/** 任务表单行（小时字段是输入态字符串，保存时换算分钟） */
interface TaskForm {
  name: string;
  summary: string;
  detail: string;
  hasSubgoals: boolean;
  hours: string;
}

const priorities: Priority[] = ["High", "Medium", "Low"];
const router = useRouter();

const plan = reactive({ name: "", summary: "", detail: "", priority: "Medium" as Priority, dueDate: "" });
const tasks = reactive<TaskForm[]>([{ ...blankTask() }]);

/** 各段独立校验错误：计划段一条、每张任务卡一条（按段/卡显示，互不阻塞其它段） */
const planErrors = reactive({ name: "" });
const taskErrors = reactive<Record<number, { name?: string; hours?: string }>>({});
const serverError = ref("");
const saving = ref(false);

function blankTask(): TaskForm {
  return { name: "", summary: "", detail: "", hasSubgoals: false, hours: "" };
}

function addTask() {
  tasks.push(blankTask());
}

/** 保存前 UI 按段校验（领域层还有权威校验兜底）；返回是否全部通过 */
function validate(): boolean {
  planErrors.name = plan.name.trim() ? "" : "请填写计划名称";
  let ok = !planErrors.name;
  for (const [i, t] of tasks.entries()) {
    const errs: { name?: string; hours?: string } = {};
    if (!t.name.trim()) errs.name = "请填写任务名称";
    if (!t.hasSubgoals && !(parseFloat(t.hours) > 0)) errs.hours = "无子目标任务必填预计耗时";
    taskErrors[i] = errs;
    if (errs.name || errs.hours) ok = false;
  }
  return ok;
}

async function save() {
  serverError.value = "";
  if (!validate()) return;
  const payload: NewPlan = {
    name: plan.name,
    summary: plan.summary,
    detail: plan.detail,
    priority: plan.priority,
    due_date: plan.dueDate || null,
    tasks: tasks.map((t) => ({
      name: t.name,
      summary: t.summary,
      detail: t.detail,
      has_subgoals: t.hasSubgoals,
      estimated_minutes: t.hasSubgoals ? undefined : Math.round(parseFloat(t.hours) * 60),
    })),
  };
  saving.value = true;
  try {
    await createPlan(payload);
    await router.push("/control-panel/plans");
  } catch (err) {
    serverError.value = planErrorMessage(err as { kind?: string });
  } finally {
    saving.value = false;
  }
}
</script>

<style scoped>
.create-page {
  max-width: 720px;
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

.tasks {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.task-card {
  border: var(--border-default);
  border-radius: var(--radius-md);
  padding: 12px 16px;
  background: var(--bg-group);
}

.task-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 10px;
}

.task-no {
  font-weight: 600;
  color: var(--text-secondary);
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
}
</style>
