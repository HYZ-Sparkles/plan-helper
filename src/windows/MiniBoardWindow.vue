<template>
  <!-- 小看板壳：置顶随桌宠，具体内容（当前任务/汇报）由工单 07 接管 -->
  <div class="board-card">
    <p class="title">小看板</p>
    <p class="hint">当前任务与汇报将在这里出现</p>
  </div>
</template>

<style scoped>
.board-card {
  height: calc(100% - 16px);
  margin: 8px;
  padding: 12px;
  background: var(--surface);
  border: var(--border-default);
  border-radius: var(--radius-md);
  box-sizing: border-box;
}

.title {
  margin: 0;
  font-weight: 600;
  color: var(--text-primary);
}
</style>
