<template>
  <!--
    轻量确认弹窗（非高危操作的二选一确认）。
    与 TypeConfirmDialog 的分工：需要打字防误删的破坏性操作走 TypeConfirm，
    普通确认（生命周期确认、子目标删除等）走这里。
    父组件 v-if 控制显隐（本体无内部状态），emit confirm / cancel；
    骨架样式走 tokens.css 全局类，与 TypeConfirmDialog 同一形态。
  -->
  <div class="dialog-overlay" @click.self="emit('cancel')">
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
