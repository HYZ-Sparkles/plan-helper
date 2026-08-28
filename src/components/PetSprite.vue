<script setup lang="ts">
/**
 * 桌宠帧渲染器（工单 08）：单张 sprite sheet 用 background-position 切帧。
 * 关键约束：
 * - 底部锚定：帧矩形含整行高（32px），行底即地面线——蹲/坐/站/睡都踩同一条线（用户验收要求）
 * - 像素锐利：整数倍缩放（SHEET_SCALE=2）+ image-rendering: pixelated
 * - 水平翻转：scaleX(-1) 镜像（素材朝右，朝左移动时用）
 * - 摆放微调：帧级 ox/oy（素材像素）叠加在锚定之上；translate 写在 scaleX 之前，
 *   偏移方向是屏幕空间（翻转不镜像偏移），正 = 右 / 下
 * 水平居中与地面线定位由外层容器负责（flex 底对齐）。
 */
import { computed } from "vue";
import { ANIMATIONS, PET_SHEET_URL, SHEET_SCALE, SHEET_WIDTH } from "../lib/pet/animations";

/** 帧摆放微调（调试页实时预览用；缺省回落到帧清单里的持久值） */
export interface SpriteNudge {
  ox: number;
  oy: number;
}

const props = defineProps<{
  /** 动画编号（作者标注体系，见 animations.ts） */
  anim: number;
  frame: number;
  flip?: boolean;
  nudge?: SpriteNudge;
}>();

const rect = computed(() => {
  const def = ANIMATIONS[props.anim];
  if (!def) return null;
  return def.frames[Math.min(props.frame, def.frames.length - 1)] ?? def.frames[0];
});

const style = computed(() => {
  const r = rect.value;
  if (!r) return { display: "none" };
  const ox = props.nudge?.ox ?? r.ox ?? 0;
  const oy = props.nudge?.oy ?? r.oy ?? 0;
  const transform = [
    ox || oy ? `translate(${ox * SHEET_SCALE}px, ${oy * SHEET_SCALE}px)` : "",
    props.flip ? "scaleX(-1)" : "",
  ]
    .filter(Boolean)
    .join(" ");
  return {
    width: `${r.w * SHEET_SCALE}px`,
    height: `${r.h * SHEET_SCALE}px`,
    backgroundImage: `url(${PET_SHEET_URL})`,
    backgroundSize: `${SHEET_WIDTH * SHEET_SCALE}px auto`,
    backgroundPosition: `${-r.x * SHEET_SCALE}px ${-r.y * SHEET_SCALE}px`,
    transform: transform || undefined,
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
