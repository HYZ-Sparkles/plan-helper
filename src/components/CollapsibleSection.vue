<template>
  <!--
    可折叠区块（CreationUI 单页表单的段落容器，也可用于其他折叠面板）。
    头部 = 标题 + 可选徽章 + CaretRight 旋转指示；内容随 open 展开收起。
  -->
  <section class="section" :class="{ open }">
    <button type="button" class="header" @click="open = !open">
      <PhCaretRight class="caret" :size="14" />
      <span class="title">{{ title }}</span>
      <span v-if="badge != null" class="badge">{{ badge }}</span>
    </button>
    <div v-show="open" class="body">
      <slot />
    </div>
  </section>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { PhCaretRight } from "@phosphor-icons/vue";

const props = defineProps<{
  title: string;
  badge?: number | string;
  defaultOpen?: boolean;
}>();
const open = ref(props.defaultOpen ?? false);
</script>

<style scoped>
.section {
  border: var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface);
}

.header {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 16px;
  border: none;
  border-radius: var(--radius-md);
  background: var(--bg-group);
  font: inherit;
  color: var(--text-primary);
  cursor: pointer;
}

.section.open .header {
  border-radius: var(--radius-md) var(--radius-md) 0 0;
  border-bottom: var(--border-default);
}

.caret {
  color: var(--text-muted);
  transition: transform 0.15s;
}

.section.open .caret {
  transform: rotate(90deg);
}

.title {
  font-weight: 600;
}

.badge {
  min-width: 22px;
  padding: 0 7px;
  border-radius: var(--radius-sm);
  background: var(--bg-accent-group);
  border: 1px solid var(--primary);
  color: var(--text-primary);
  font-size: 12px;
  line-height: 20px;
  text-align: center;
}

.body {
  padding: 16px;
}
</style>
