<template>
  <!--
    自适应 textarea（CONTEXT「详细内容字段」）：纯文本、最少 3 行、按内容长高、8 行起滚动。
    计划与任务的详细内容共用。
  -->
  <textarea
    ref="el"
    class="input auto-textarea"
    :value="modelValue"
    rows="3"
    @input="onInput"
  ></textarea>
</template>

<script setup lang="ts">
import { nextTick, onMounted, ref } from "vue";

defineProps<{ modelValue: string }>();
const emit = defineEmits<{ (e: "update:modelValue", value: string): void }>();

const el = ref<HTMLTextAreaElement>();

/** 高度 = 内容高度，夹在 3 行与 8 行之间。行高 22px / 上下 padding 16px 与下方 CSS 常量保持一致 */
function resize() {
  const node = el.value;
  if (!node) return;
  node.style.height = "auto";
  const line = 22;
  const pad = 16;
  node.style.height = `${Math.min(Math.max(node.scrollHeight, line * 3 + pad), line * 8 + pad)}px`;
}

function onInput(e: Event) {
  emit("update:modelValue", (e.target as HTMLTextAreaElement).value);
  resize();
}

onMounted(() => nextTick(resize));
</script>

<style scoped>
.auto-textarea {
  line-height: 22px; /* 与 resize() 的行常量一致 */
  resize: none;
  overflow-y: auto;
}
</style>
