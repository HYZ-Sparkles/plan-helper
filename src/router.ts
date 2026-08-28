/**
 * 路由：窗口 label → 路由一一对应（main.ts 按 label 落位）。
 * 多窗口共享同一 SPA，用 hash 历史避免各窗口 URL 互相干扰。
 */
import { createRouter, createWebHashHistory } from "vue-router";
import ControlPanelWindow from "./windows/ControlPanelWindow.vue";
import MainBoardWindow from "./windows/MainBoardWindow.vue";
import MiniBoardWindow from "./windows/MiniBoardWindow.vue";
import PetWindow from "./windows/PetWindow.vue";
import PetMenuWindow from "./windows/PetMenuWindow.vue";
import DevAnimPage from "./pages/DevAnimPage.vue";
import CreatePage from "./pages/CreatePage.vue";
import PlanDetailPage from "./pages/PlanDetailPage.vue";
import PlansPage from "./pages/PlansPage.vue";
import SettingsPage from "./pages/SettingsPage.vue";

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", redirect: "/control-panel/plans" },
    { path: "/pet", component: PetWindow },
    { path: "/pet-menu", component: PetMenuWindow },
    { path: "/mini-board", component: MiniBoardWindow },
    { path: "/main-board", component: MainBoardWindow },
    {
      path: "/control-panel",
      component: ControlPanelWindow,
      redirect: "/control-panel/plans",
      children: [
        { path: "plans", component: PlansPage },
        { path: "plans/:id", component: PlanDetailPage },
        { path: "create", component: CreatePage },
        { path: "settings", component: SettingsPage },
      ],
    },
    // 开发调试页（工单 08）：动画回放/逐帧/调 fps/翻转/位移预览，浏览器直开 #/dev/anim
    { path: "/dev/anim", component: DevAnimPage },
  ],
});

/** 窗口 label → 初始路由；未知 label 兜底到控制面板（main.ts 按窗口落位） */
const routeByLabel: Record<string, string> = {
  pet: "/pet",
  "pet-menu": "/pet-menu",
  "mini-board": "/mini-board",
  "main-board": "/main-board",
  "control-panel": "/control-panel/plans",
};

/** 无边框透明窗口需要透明根背景（tokens.css），label 判定集中在这 */
export const transparentLabels = new Set(["pet", "pet-menu", "mini-board"]);

export function routeForLabel(label: string): string {
  return routeByLabel[label] ?? "/control-panel/plans";
}
