<script setup lang="ts">
/**
 * 桌宠帧渲染器（工单 08 建立、16 改 codex 契约网格）：单张雪碧图按固定 8 列网格
 * 用 background-position 切帧——格 192×208、÷2 缩放（96×104），每帧同尺寸同基线
 * （契约保证，Oreo 的底部锚定/摆放微调不再需要）。
 * 缩放用**平滑插值**（不用 pixelated——2026-09-14 反馈：codex 素材是带抗锯齿的绘画
 * 风，最近邻在 ÷2 缩放下逐帧抽取不同像素行，轮廓忽隐忽现看起来像帧忽大忽小；
 * pixelated 是 Oreo 像素风的遗产）。
 * 形象 = sheet prop（skins.ts 注册表 URL），换装 = 换 URL，引擎状态不动。
 * look prop（0–15）= v2 环视静态姿势，非空时覆盖 anim/frame（工单 21）。
 * 硬切衔接（工单 09）：fadeSignal 计数变化时快速淡出→淡入一次，掩盖抢占产生的帧跳变。
 */
import { computed, ref, watch } from "vue";
import {
  ANIMATIONS,
  CELL_H,
  CELL_W,
  GRID_COLS,
  lookCell,
  SHEET_SCALE,
  type PetAnim,
} from "../lib/pet/animations";

const props = defineProps<{
  anim: PetAnim;
  frame: number;
  /** 雪碧图 URL（形象皮肤） */
  sheet: string;
  /** v2 环视姿势序号（0–15，顺时针、0 = 正上方）；非空时覆盖 anim/frame */
  look?: number | null;
  /** 硬切淡出淡入信号（引擎 flick 计数，仅变化时触发一次动画） */
  fadeSignal?: number;
}>();

const el = ref<HTMLElement>();
watch(
  () => props.fadeSignal,
  () => {
    const node = el.value;
    if (!node || !props.fadeSignal) return;
    node.classList.remove("quick-fade");
    void node.offsetWidth; // 强制 reflow，连续两次硬切各自触发动画
    node.classList.add("quick-fade");
  },
);

/** 当前帧的格坐标：环视姿势覆盖标准动作帧 */
const cell = computed(() => {
  if (props.look != null) return lookCell(props.look);
  const def = ANIMATIONS[props.anim];
  return { row: def.row, col: Math.min(props.frame, def.cols - 1) };
});

const style = computed(() => ({
  width: `${CELL_W * SHEET_SCALE}px`,
  height: `${CELL_H * SHEET_SCALE}px`,
  backgroundImage: `url(${props.sheet})`,
  backgroundSize: `${GRID_COLS * CELL_W * SHEET_SCALE}px auto`,
  backgroundPosition: `${-cell.value.col * CELL_W * SHEET_SCALE}px ${-cell.value.row * CELL_H * SHEET_SCALE}px`,
}));
</script>

<template>
  <div ref="el" class="pet-sprite" :style="style" />
</template>

<style scoped>
.pet-sprite {
  flex: none;
  will-change: background-position;
}

/* 无法衔接的硬切（引擎 flick）：快速淡出→淡入避免跳变 */
.quick-fade {
  animation: pet-quick-fade 220ms ease-out;
}

@keyframes pet-quick-fade {
  0% {
    opacity: 1;
  }
  45% {
    opacity: 0.15;
  }
  100% {
    opacity: 1;
  }
}
</style>
