<template>
  <!-- 表单字段外壳：label + 必填标记 + 错误行；输入控件经插槽传入 -->
  <label class="field" :class="{ required }">
    <span class="field-label">
      {{ label }}
      <span v-if="required" class="mark">*</span>
    </span>
    <slot />
    <span v-if="error" class="field-error">{{ error }}</span>
  </label>
</template>

<script setup lang="ts">
defineProps<{
  label: string;
  required?: boolean;
  error?: string;
}>();
</script>

<style scoped>
.field {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.field-label {
  font-size: 13px;
  color: var(--text-secondary);
}

.mark {
  color: var(--color-danger);
}

.field-error {
  font-size: 12px;
  color: var(--color-danger);
}
</style>
