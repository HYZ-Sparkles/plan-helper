<template>
  <!-- 桌宠窗口壳：透明背景 + 拖拽区（交互与动画由工单 08/09 接管） -->
  <div class="pet-shell" data-tauri-drag-region>
    <div class="placeholder" data-tauri-drag-region>Oreo Cat</div>
  </div>
</template>

<style scoped>
.pet-shell {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}

/* 占位方块：仅骨架期标示桌宠区域，帧动画接入后移除。
   64 = 32 像素素材 × 2 倍显示，与窗口尺寸（tauri.conf.json pet 64×64）同一倍数约定；
   接帧动画时保持整数倍 + image-rendering: pixelated，像素画才锐利 */
.placeholder {
  width: 64px;
  height: 64px;
  font-size: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: var(--border-default);
  border-radius: var(--radius-sm);
  color: var(--text-muted);
  user-select: none;
}
</style>
