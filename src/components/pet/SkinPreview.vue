<script setup lang="ts">
/**
 * 形象预览（工单 22，设置页「桌宠偏好」）：按契约逐帧时长循环播 idle——能区分
 * 画风与呼吸感；复用 PetSprite（同一渲染路径，所见即桌宠）。轻量计时器不走引擎
 * （九宫格各起各的循环，互不同步也无妨）。
 */
import { onMounted, onUnmounted, ref } from "vue";
import PetSprite from "../PetSprite.vue";
import { ANIMATIONS } from "../../lib/pet/animations";
import type { SkinDef } from "../../lib/pet/skins";

defineProps<{ skin: SkinDef }>();

const frame = ref(0);
let timer: ReturnType<typeof setTimeout> | undefined;

/** 按契约时长步进 idle 帧（约 1.1s 一圈） */
function tick() {
  const def = ANIMATIONS.idle;
  timer = setTimeout(
    () => {
      frame.value = (frame.value + 1) % def.cols;
      tick();
    },
    def.durations[Math.min(frame.value, def.cols - 1)],
  );
}

onMounted(tick);
onUnmounted(() => clearTimeout(timer));
</script>

<template>
  <div class="skin-preview">
    <PetSprite :anim="'idle'" :frame="frame" :sheet="skin.sheet" />
  </div>
</template>

<style scoped>
.skin-preview {
  display: flex;
  align-items: flex-end;
  justify-content: center;
  height: 104px; /* 契约窗口高（192×208 格 ÷2）——预览 = 实际尺寸 */
}
</style>
