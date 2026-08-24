<template>
  <!--
    轻量确认弹窗（非高危操作的二选一确认）。
    与 TypeConfirmDialog 的分工：需要打字防误删的破坏性操作走 TypeConfirm，
    普通确认（如删除未完成子目标、取消子目标勾选）走这里。
    父组件 v-if 控制显隐（本体无内部状态），emit confirm / cancel。
  -->
  <div class="overlay" @click.self="emit('cancel')">
    <div class="dialog" role="dialog" :aria-label="title">
      <h3 class="dialog-title">{{ title }}</h3>
      <div class="dialog-body"><slot /></div>
      <footer class="dialog-actions">
        <button type="button" class="ghost-btn" @click="emit('cancel')">取消</button>
        <button type="button" class="primary-btn" @click="emit('confirm')">{{ actionLabel }}</button>
      </footer>
    </div>
  </div>
</template>

<script setup lang="ts">
withDefaults(
  defineProps<{
    title: string;
    /** 确认按钮文案 */
    actionLabel?: string;
  }>(),
  { actionLabel: "确认" },
);
const emit = defineEmits<{ confirm: []; cancel: [] }>();
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

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}
</style>
