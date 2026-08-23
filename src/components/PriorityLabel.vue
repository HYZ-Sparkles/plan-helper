<template>
  <!--
    优先级标签（CONTEXT PriorityVisuals：文字标签 + Phosphor 图标）。
    全应用的优先级展示统一走这个组件：高 [CaretUp] / 中 [Minus] / 低 [CaretDown]。
  -->
  <span class="priority" :class="`priority-${priority}`">
    <PhCaretUp v-if="priority === 'High'" :size="14" />
    <PhMinus v-else-if="priority === 'Medium'" :size="14" />
    <PhCaretDown v-else :size="14" />
    {{ label }}
  </span>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { PhCaretDown, PhCaretUp, PhMinus } from "@phosphor-icons/vue";
import { priorityLabel } from "../lib/labels";
import type { Priority } from "../lib/api";

const props = defineProps<{ priority: Priority }>();
const label = computed(() => priorityLabel[props.priority]);
</script>

<style scoped>
.priority {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 1px 8px;
  border-radius: var(--radius-sm);
  font-size: 12px;
  line-height: 20px;
}

.priority-High {
  color: var(--priority-high);
  background: var(--priority-high-bg);
}

.priority-Medium {
  color: var(--priority-medium);
  background: var(--priority-medium-bg);
}

.priority-Low {
  color: var(--priority-low);
  background: var(--priority-low-bg);
}
</style>
