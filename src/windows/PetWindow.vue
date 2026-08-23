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

/* 占位方块：仅骨架期标示桌宠区域，帧动画接入后移除 */
.placeholder {
  width: 120px;
  height: 120px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: var(--border-default);
  border-radius: var(--radius-sm);
  color: var(--text-muted);
  user-select: none;
}
</style>
