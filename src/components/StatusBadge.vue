<template>
  <!-- 计划/任务状态徽章：全应用统一的状态展示 -->
  <span class="status-badge" :class="`status-${status}`">{{ statusLabel[status] }}</span>
</template>

<script setup lang="ts">
import { statusLabel } from "../lib/labels";
import type { PlanStatus, TaskStatus } from "../lib/api";

defineProps<{ status: PlanStatus | TaskStatus }>();
</script>

<style scoped>
.status-badge {
  display: inline-block;
  padding: 1px 8px;
  border: var(--border-default);
  border-radius: var(--radius-sm);
  font-size: 12px;
  line-height: 20px;
  color: var(--text-secondary);
}

.status-Active {
  color: var(--color-progress);
  border-color: var(--color-progress);
}

.status-Paused {
  color: var(--text-muted);
}

.status-Completed {
  color: var(--color-done);
  border-color: var(--color-done);
}

.status-Abandoned {
  color: var(--color-danger);
  border-color: var(--color-danger);
}
</style>
