<script setup lang="ts">
/**
 * 桌宠菜单窗口（工单 08，PetMenuActions）：控制面板 / 切换模式 / 再见。
 * 独立小窗（桌宠窗口只有 64×64 装不下菜单），由 PetWindow 计算位置后 show。
 * - 播放用户动作期间菜单**可打开查看**但项禁用（PetActionExecution；tooltip 说明原因）
 * - 失焦自动隐藏（点击别处 = 关菜单）；选择后发事件给桌宠窗口执行
 */
import { computed, onMounted, ref } from "vue";
import { emitTo, listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { PhSlidersHorizontal, PhCoffee, PhBriefcase, PhHandWaving } from "@phosphor-icons/vue";
import {
  MENU_ACTION_EVENT,
  MENU_CLOSE_EVENT,
  MENU_CLOSED_EVENT,
  MENU_OPEN_EVENT,
  MENU_STATE_EVENT,
  type MenuAction,
  type PetMode,
} from "../lib/pet/menu";

const win = getCurrentWebviewWindow();
const locked = ref(false);
const mode = ref<PetMode>("work");

/** 菜单项表（数据驱动渲染）：图标按当前模式选择，文案显示目标模式 */
const items = computed(() => [
  { action: "control-panel" as MenuAction, icon: PhSlidersHorizontal, label: "控制面板" },
  {
    action: "toggle-mode" as MenuAction,
    icon: mode.value === "work" ? PhCoffee : PhBriefcase,
    label: mode.value === "work" ? "进入休息模式" : "进入工作模式",
  },
  { action: "goodbye" as MenuAction, icon: PhHandWaving, label: "再见" },
]);

onMounted(async () => {
  await listen<{ locked: boolean; mode: PetMode }>(MENU_OPEN_EVENT, (e) => {
    locked.value = e.payload.locked;
    mode.value = e.payload.mode;
  });
  await listen<{ locked: boolean }>(MENU_STATE_EVENT, (e) => {
    locked.value = e.payload.locked;
  });
  await listen(MENU_CLOSE_EVENT, hide);
  // 关闭请求拦截为隐藏：菜单窗被 Alt+F4 摧毁后菜单功能就废了，收起语义 = 隐藏（同工单 14 关闭语义）
  await win.onCloseRequested(async (e) => {
    e.preventDefault();
    await hide();
  });
  // 失焦即关：点击桌面/其它窗口 = 收起菜单（tauri://blur 是窗口级事件）
  await win.listen("tauri://blur", async () => {
    await hide();
    await emitTo("pet", MENU_CLOSED_EVENT);
  });
});

async function hide() {
  await win.hide();
}

/** 菜单项点击：通知桌宠窗口执行并自隐藏 */
async function choose(action: MenuAction) {
  if (locked.value) return;
  await hide();
  await emitTo("pet", MENU_ACTION_EVENT, { action });
}
</script>

<template>
  <div class="menu-card">
    <button
      v-for="item in items"
      :key="item.action"
      class="menu-item"
      :disabled="locked"
      :title="locked ? '桌宠正在执行动作' : undefined"
      @click="choose(item.action)"
    >
      <component :is="item.icon" :size="18" />
      <span>{{ item.label }}</span>
    </button>
  </div>
</template>

<style scoped>
/* 透明窗口 + 内缩卡片：圆角与阴影不被窗口矩形裁掉 */
.menu-card {
  margin: 10px; /* 留出阴影空间 */
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 6px;
  background: var(--surface);
  border: var(--border-default);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-lg);
}

.menu-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 12px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-primary);
  font-size: 13px;
  font-family: inherit;
  cursor: pointer;
  text-align: left;
}

.menu-item:hover {
  background: var(--bg-group);
}

.menu-item:disabled {
  color: var(--text-muted);
  cursor: not-allowed;
}

.menu-item:disabled:hover {
  background: transparent;
}
</style>
