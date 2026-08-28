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

const app = createApp(App);
app.use(router);
// Tauri 窗口按 label 落位；浏览器直开沿用 URL hash（#/dev/anim）
if (inTauri) router.push(routeForLabel(label));
app.mount("#app");
