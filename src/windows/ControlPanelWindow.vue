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
      <!-- 页面内容列：壳层统一限宽居中（工单 15 第二轮反馈：三页各自定义列宽时
           切换导航左右边距大幅位移——统一 720px 后边距恒定）。列宽经 --col-max
           注入 tokens.css 全局 .content-col，与大面板/总结窗同一模式 -->
      <div class="content-col">
        <RouterView />
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from "vue";
import { PhGear, PhListBullets, PhPlus } from "@phosphor-icons/vue";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

onMounted(async () => {
  // 关闭 = 隐藏，不退出应用（spec 68；工单 08 起启动隐藏、桌宠菜单为主要入口，
  // 点 X 真销毁窗口的话入口就失效了——同 mini-board/main-board 的拦截模式）
  const win = getCurrentWebviewWindow();
  await win.onCloseRequested(async (e) => {
    e.preventDefault();
    await win.hide();
  });
});
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
  --col-max: 720px; /* 页面统一内容列宽（创建/详情表单的原宽度，列表/设置随之看齐） */
  flex: 1;
  min-width: 0;
  padding: 24px;
  box-sizing: border-box;
  overflow: auto;
}
</style>
