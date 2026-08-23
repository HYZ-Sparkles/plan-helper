<template>
  <!-- 大面板壳：今日任务分配，由工单 06 接管 -->
  <div class="board">
    <header class="header">
      <h1>今日分配</h1>
    </header>
    <p class="hint">进入工作模式且今日未分配时自动打开</p>
  </div>
</template>

<style scoped>
.board {
  height: 100%;
  padding: 24px;
  box-sizing: border-box;
  background: var(--bg-base);
}

.header {
  border-bottom: var(--border-default);
  padding-bottom: 12px;
}

h1 {
  margin: 0;
  font-size: 18px;
}
</style>
