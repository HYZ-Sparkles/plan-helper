<template>
  <!--
    打字确认的危险操作弹窗（spec 用户故事 11：必须打字「再删」才能执行）。
    删除任务（工单 03）与放弃计划（工单 05）共用；父组件用 v-if 控制显隐，
    弹窗本体无内部开合状态——每次挂载输入框都是干净的。
  -->
  <div class="overlay" @click.self="emit('cancel')">
    <div class="dialog" role="alertdialog" :aria-label="title">
      <h3 class="dialog-title">{{ title }}</h3>
      <div class="dialog-body"><slot /></div>
      <label class="confirm-row">
        <span class="confirm-hint">输入「{{ confirmText }}」确认：</span>
        <input
          v-model="typed"
          class="input"
          :placeholder="confirmText"
          @keydown.enter="typed === confirmText && emit('confirm')"
        />
      </label>
      <footer class="dialog-actions">
        <button type="button" class="ghost-btn" @click="emit('cancel')">取消</button>
        <button
          type="button"
          class="danger-btn"
          :disabled="typed !== confirmText"
          @click="emit('confirm')"
        >
          {{ actionLabel }}
        </button>
      </footer>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";

withDefaults(
  defineProps<{
    title: string;
    /** 需要打出的确认词（防误触核心） */
    confirmText?: string;
    /** 确认按钮文案 */
    actionLabel?: string;
  }>(),
  { confirmText: "再删", actionLabel: "删除" },
);
const emit = defineEmits<{ confirm: []; cancel: [] }>();

/** 已输入的确认词——与 confirmText 完全一致才放开确认按钮 */
const typed = ref("");
</script>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--overlay-dim);
  z-index: 100;
}

.dialog {
  width: min(420px, calc(100vw - 48px));
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 20px 24px;
  border: var(--border-default);
  border-radius: var(--radius-lg);
  background: var(--surface);
  box-shadow: var(--shadow-lg);
}

.dialog-title {
  margin: 0;
  font-size: 16px;
}

.dialog-body {
  color: var(--text-secondary);
  font-size: 14px;
}

.confirm-row {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 13px;
  color: var(--text-secondary);
}

.confirm-row .input {
  width: 100%;
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}

.danger-btn {
  padding: 6px 14px;
  border: 1px solid var(--color-danger);
  border-radius: var(--radius-sm);
  background: var(--color-danger);
  color: var(--bg-base);
  font: inherit;
  cursor: pointer;
}

.danger-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
</style>
