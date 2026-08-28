<script setup lang="ts">
/**
 * 桌宠帧渲染器（工单 08）：单张 sprite sheet 用 background-position 切帧。
 * 关键约束：
 * - 底部锚定：帧矩形含整行高（32px），行底即地面线——蹲/坐/站/睡都踩同一条线（用户验收要求）
 * - 像素锐利：整数倍缩放（SHEET_SCALE=2）+ image-rendering: pixelated
 * - 水平翻转：scaleX(-1) 镜像（素材朝右，朝左移动时用）
 * 水平居中与地面线定位由外层容器负责（flex 底对齐）。
 */
import { computed } from "vue";
import { ANIMATIONS, PET_SHEET_URL, SHEET_SCALE, SHEET_WIDTH } from "../lib/pet/animations";

const props = defineProps<{
  /** 动画编号（作者标注体系，见 animations.ts） */
  anim: number;
  frame: number;
  flip?: boolean;
}>();

const rect = computed(() => {
  const def = ANIMATIONS[props.anim];
  if (!def) return null;
  return def.frames[Math.min(props.frame, def.frames.length - 1)] ?? def.frames[0];
});

const style = computed(() => {
  const r = rect.value;
  if (!r) return { display: "none" };
  return {
    width: `${r.w * SHEET_SCALE}px`,
    height: `${r.h * SHEET_SCALE}px`,
    backgroundImage: `url(${PET_SHEET_URL})`,
    backgroundSize: `${SHEET_WIDTH * SHEET_SCALE}px auto`,
    backgroundPosition: `${-r.x * SHEET_SCALE}px ${-r.y * SHEET_SCALE}px`,
    transform: props.flip ? "scaleX(-1)" : undefined,
    imageRendering: "pixelated" as const,
  };
});
</script>

<template>
  <div class="pet-sprite" :style="style" />
</template>

<style scoped>
.pet-sprite {
  flex: none; /* 不被 flex 容器压缩，帧宽随动画逐帧变化 */
  will-change: background-position, transform;
}
</style>
