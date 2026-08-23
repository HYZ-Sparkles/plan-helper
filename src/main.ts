import { createApp } from "vue";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import App from "./App.vue";
import { routeForLabel, router, transparentLabels } from "./router";
import "./styles/tokens.css";

const label = getCurrentWebviewWindow().label;
if (transparentLabels.has(label)) {
  document.body.classList.add("transparent-root");
}

const app = createApp(App);
app.use(router);
router.push(routeForLabel(label));
app.mount("#app");
