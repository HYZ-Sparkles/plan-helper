<script setup lang="ts">
/**
 * 桌宠浮层窗（工单 08，PetMenuActions）：菜单（控制面板 / 切换模式 / 再见）+
 * 提示气泡（休息模式左键「右键才是菜单喵」，2026-08-30 反馈）两种形态。
 * **显隐唯一持有者是 PetWindow**（2026-08-30 重构，与拖拽耦合状态同源思想）：本窗
 * 只报告回执——失焦（tauri://blur）/ 气泡到点，各带亮出代数 seq；是否真藏由桌宠按
 * 代数裁决（被换形态顶替的回执落败）。本窗永不自行 hide，形态切换只发生在事件驱动
 * 的重渲染上 → 交替左/右键不闪不吞。
 * - 播放用户动作期间菜单**可打开查看**但项禁用（PetActionExecution；tooltip 说明原因）
 */
import { computed, onMounted, ref } from "vue";
import { emitTo, listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { PhSlidersHorizontal, PhCoffee, PhBriefcase, PhHandWaving } from "@phosphor-icons/vue";
import {
  MENU_ACTION_EVENT,
  MENU_BLUR_EVENT,
  MENU_EXPIRE_EVENT,
  MENU_HINT_EVENT,
  MENU_OPEN_EVENT,
  MENU_STATE_EVENT,
  type MenuAction,
  type PetMenuSeqPayload,
  type PetMenuShowPayload,
  type PetMode,
} from "../lib/pet/menu";

const win = getCurrentWebviewWindow();
const locked = ref(false);
const mode = ref<PetMode>("work");
/** 提示气泡文案（非空 = 气泡形态，替代菜单渲染；到点发回执由桌宠隐藏） */
const hint = ref("");
/** 提示气泡存活时长：冒泡读完即收，不遮桌面 */
const HINT_AUTO_HIDE_MS = 2500;
let hintTimer: ReturnType<typeof setTimeout> | undefined;
/** 本窗当前渲染的浮层代数（桌宠侧 floatSeq 的回声，回执据此被裁决） */
let seq = 0;

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

/** 气泡形态：亮文案 + 到点发回执（**不清形态、不自行 hide**——窗口可见期间不做任何
 *  形态切换，桌宠按代数隐藏后 hint 留待下次亮出时重置，杜绝菜单卡闪现） */
function flashHint(p: PetMenuSeqPayload) {
  seq = p.seq;
  hint.value = "右键才是菜单喵";
  clearTimeout(hintTimer);
  const expired = p.seq;
  hintTimer = setTimeout(
    () => void emitTo("pet", MENU_EXPIRE_EVENT, { seq: expired }),
    HINT_AUTO_HIDE_MS,
  );
}

onMounted(async () => {
  await listen<PetMenuShowPayload>(MENU_OPEN_EVENT, (e) => {
    locked.value = e.payload.locked;
    mode.value = e.payload.mode;
    seq = e.payload.seq;
    // 进菜单形态：作废气泡计时（回执带旧代数，即便发出也会被桌宠裁决落败——双保险）
    clearTimeout(hintTimer);
    hint.value = "";
  });
  await listen<PetMenuSeqPayload>(MENU_HINT_EVENT, (e) => flashHint(e.payload));
  await listen<{ locked: boolean }>(MENU_STATE_EVENT, (e) => {
    locked.value = e.payload.locked;
  });
  // 关闭请求拦截：菜单窗被 Alt+F4 摧毁后菜单功能就废了；同失焦一样只发回执，桌宠裁决
  await win.onCloseRequested(async (e) => {
    e.preventDefault();
    await emitTo("pet", MENU_BLUR_EVENT, { seq });
  });
  // 失焦只报告：点击桌面/其它窗口 → 桌宠按代数裁决隐藏（本窗不自行 hide）
  await win.listen("tauri://blur", () => emitTo("pet", MENU_BLUR_EVENT, { seq }));
});

/** 菜单项点击：通知桌宠窗口执行（隐藏归桌宠的 MENU_ACTION 处理，本窗不自藏） */
async function choose(action: MenuAction) {
  if (locked.value) return;
  await emitTo("pet", MENU_ACTION_EVENT, { action });
}
</script>

<template>
  <!-- 气泡形态贴窗底（离桌宠最近）；菜单形态保持原位（窗顶） -->
  <div class="float-root" :class="{ bottom: hint }">
    <div v-if="hint" class="hint-bubble">
      <span>{{ hint }}</span>
    </div>
    <div v-else class="menu-card">
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
  </div>
</template>

<style scoped>
/* 形态容器：菜单贴窗顶（原位）；气泡贴窗底 = 离桌宠最近 */
.float-root {
  height: 100%;
  display: flex;
  flex-direction: column;
  justify-content: flex-start;
}

.float-root.bottom {
  justify-content: flex-end;
}

/* 提示气泡：内容自适应宽，不撑满窗宽 */
.hint-bubble {
  margin: 10px;
  align-self: center;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 7px 12px;
  background: var(--surface);
  border: var(--border-default);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-lg);
  color: var(--text-primary);
  font-size: 12px;
  white-space: nowrap;
}

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
