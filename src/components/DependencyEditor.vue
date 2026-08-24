<template>
  <!--
    前置任务多选（CONTEXT TaskDependency「录入 UI」）：从同计划其他任务中勾选，
    收在任务卡展开态次要区域。创建表单与详情编辑态同一控件（v-model 为草稿内稳定 key 集，
    保存时由 CreationForm 换算为数组下标 depends_on）。所见即所存：勾选集 = 依赖边全集。
  -->
  <div class="dep-editor" :class="{ disabled }">
    <span class="dep-label">前置任务</span>
    <p v-if="candidates.length === 0" class="hint">同计划暂无其他任务可选</p>
    <label
      v-for="c in candidates"
      :key="c.key"
      class="dep-option"
      :class="{ checked: modelValue.includes(c.key) }"
    >
      <input
        type="checkbox"
        :checked="modelValue.includes(c.key)"
        :disabled="disabled"
        @change="toggle(c.key)"
      />
      <StatusBadge v-if="c.status === 'Completed'" :status="c.status" />
      <span class="dep-name">{{ c.name }}</span>
    </label>
  </div>
</template>

<script setup lang="ts">
import StatusBadge from "./StatusBadge.vue";
import type { TaskStatus } from "../lib/api";

/** 候选项：key = CreationForm 内的任务稳定标识（非数组下标，拖拽重排不变） */
export interface DepCandidate {
  key: number;
  name: string;
  status: TaskStatus;
}

const props = defineProps<{
  /** 同计划其他任务（不含本任务） */
  candidates: DepCandidate[];
  /** 已选前置的 key 集 */
  modelValue: number[];
  /** 已完成任务锁定态只读展示 */
  disabled?: boolean;
}>();
const emit = defineEmits<{ "update:modelValue": [number[]] }>();

function toggle(key: number) {
  const next = props.modelValue.includes(key)
    ? props.modelValue.filter((k) => k !== key)
    : [...props.modelValue, key];
  emit("update:modelValue", next);
}
</script>

<style scoped>
.dep-editor {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.dep-label {
  color: var(--text-muted);
  font-size: 12px;
}

.dep-editor.disabled {
  opacity: 0.55;
}

/* 勾选项：线条风格小胶囊，选中亮主色边框（ADR-0005） */
.dep-option {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border: var(--border-default);
  border-radius: var(--radius-sm);
  background: var(--surface);
  font-size: 13px;
  color: var(--text-secondary);
  cursor: pointer;
  user-select: none;
}

.dep-option.checked {
  border-color: var(--primary);
  color: var(--text-primary);
  background: var(--bg-accent-group);
}

.dep-editor.disabled .dep-option {
  cursor: not-allowed;
}

.dep-option input {
  accent-color: var(--primary);
}

.dep-name {
  max-width: 220px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
