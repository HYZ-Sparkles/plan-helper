<template>
  <!--
    微型进度条（线条基调）：大面板状态条与小看板今日总量行共用。
    未达标记进行色（amber 软提示），达标转完成色；圆角取 --radius-sm，
    浏览器按低条高自动钳到半高胶囊（token 表无亚 8px 档）。
  -->
  <div class="micro-bar">
    <div class="micro-fill" :class="{ reached }" :style="{ width }"></div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";

const props = withDefaults(
  defineProps<{
    /** 填充比例（0–1；超出满格，非法值归 0） */
    ratio: number;
    /** 达标标记：true 转完成色 */
    reached?: boolean;
  }>(),
  { reached: false },
);

const width = computed(() => {
  if (!Number.isFinite(props.ratio) || props.ratio <= 0) return "0%";
  return `${Math.min(100, props.ratio * 100)}%`;
});
</script>

<style scoped>
.micro-bar {
  height: 4px;
  border-radius: var(--radius-sm);
  background: var(--border-default);
  overflow: hidden;
}

.micro-fill {
  height: 100%;
  border-radius: var(--radius-sm);
  background: var(--color-progress);
  transition: width 0.2s;
}

.micro-fill.reached {
  background: var(--color-done);
}
</style>
