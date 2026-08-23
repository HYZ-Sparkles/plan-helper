<template>
  <!-- 控制面板壳：固定三项侧边栏（CONTEXT ControlPanelNav） -->
  <div class="layout">
    <aside class="sidebar">
      <p class="brand">Plan Helper</p>
      <nav class="nav">
        <RouterLink to="/control-panel/plans" class="nav-item">
          <PhListBullets :size="18" /> 计划管理
        </RouterLink>
        <RouterLink to="/control-panel/create" class="nav-item">
          <PhPlus :size="18" /> 创建
        </RouterLink>
        <RouterLink to="/control-panel/settings" class="nav-item">
          <PhGear :size="18" /> 设置
        </RouterLink>
      </nav>
    </aside>
    <main class="content">
      <RouterView />
    </main>
  </div>
</template>

<script setup lang="ts">
import { PhGear, PhListBullets, PhPlus } from "@phosphor-icons/vue";
</script>

<style scoped>
.layout {
  display: flex;
  height: 100%;
}

.sidebar {
  width: 184px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  border-right: var(--border-default);
  background: var(--bg-group);
  padding: 16px 12px;
  box-sizing: border-box;
}

.brand {
  margin: 0 8px 16px;
  font-weight: 700;
  color: var(--text-primary);
}

.nav {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  text-decoration: none;
  border: 1px solid transparent;
}

.nav-item:hover {
  background: var(--bg-base);
}

.nav-item.router-link-active {
  background: var(--bg-accent-group);
  border: var(--border-active);
  color: var(--text-primary);
}

.content {
  flex: 1;
  min-width: 0;
  padding: 24px;
  box-sizing: border-box;
  overflow: auto;
}
</style>
