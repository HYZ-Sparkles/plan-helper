import { createApp } from "vue";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import App from "./App.vue";
import { routeForLabel, router, transparentLabels } from "./router";
import "./styles/tokens.css";

/** 浏览器直开（无 Tauri 运行时，如 #/dev/anim 调试页）时 getCurrentWebviewWindow 会抛错 */
const inTauri = "__TAURI_INTERNALS__" in window;
const label = inTauri ? getCurrentWebviewWindow().label : "";
if (transparentLabels.has(label)) {
  document.body.classList.add("transparent-root");
}

// 原生右键菜单全局静默（2026-08-30 反馈）：所有窗口（控制面板/大小看板/总结/桌宠/菜单）
// 统一拦掉 WebView2 原生右键弹窗（刷新/另存为等），无特殊说明不放行；文本粘贴走 Ctrl+V
document.addEventListener("contextmenu", (e) => e.preventDefault());

const app = createApp(App);
app.use(router);
// Tauri 窗口按 label 落位；浏览器直开沿用 URL hash（#/dev/anim）
if (inTauri) router.push(routeForLabel(label));
app.mount("#app");
