<script setup lang="ts">
/**
 * 桌宠窗口（工单 08 主接线）：启动序列 → 正常态（菜单 / 模式切换 / 再见 / 拖拽）。
 *
 * - 启动序列 21→22→23→24→7(循环)：期间点击/菜单/拖拽全部忽略（比"动画播放中"更严格）
 * - 序列完成（7 开始循环那一刻）→ 进入正常态：默认工作模式，700ms 后 Stand to Sleep 过渡
 *   进 Sleep Idle；AutoOpenMainBoard 检测此刻接入（工作模式 + 今日未分配 → 弹大面板）
 * - 模式切换（菜单项，用户动作占锁）：休息→工作 4 Stand to Sleep；工作→休息 6 Sleep to
 *   Stand；过渡完进对应 idle（工作=5 循环；休息=7/2 站坐轮换，40s 一换，09 将换成加权随机）
 * - 再见：17 Jump in to the Box 播完 → exitApp()（关闭全部窗口）
 * - 拖拽：手动位移 + 工作区钳制（任务栏禁入/边缘吸附/看板跟随归 09）；拖动不打断动画
 *   （CONTEXT PetActionExecution 裁决：帧继续播）
 */
import { onMounted, reactive, ref } from "vue";
import { emitTo, listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow, WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { currentMonitor } from "@tauri-apps/api/window";
import { PhysicalPosition } from "@tauri-apps/api/dpi";
import PetSprite from "../components/PetSprite.vue";
import { PetEngine, type EngineState, type PetMover } from "../lib/pet/engine";
import { createTauriMover } from "../lib/pet/tauriMover";
import { MENU_ACTION_EVENT, MENU_CLOSED_EVENT, MENU_CLOSE_EVENT, MENU_OPEN_EVENT, MENU_STATE_EVENT, type MenuAction, type PetMode } from "../lib/pet/menu";
import { exitApp, getMiniBoard, shouldAutoOpenMainBoard } from "../lib/api";

/** 休息模式站↔坐轮换间隔（README"常驻一段时间"；09 的 PetRandomAction 上线后由 300s 权重抽取取代） */
const REST_ALTERNATE_MS = 40_000;
/** 启动序列落地（7 Stand Idle 开始）后站立的展示节拍，再转入工作睡眠 */
const STARTUP_STAND_BEAT_MS = 700;
/** 判定为"点击"的最大位移（逻辑像素，小于它不算拖拽） */
const CLICK_SLOP_PX = 4;

const win = getCurrentWebviewWindow();
const engine = new PetEngine();
let mover: PetMover | null = null;
const sprite = reactive<EngineState>({ anim: 0, frame: 0, flip: false, locked: false, busy: false });
/** startup = 启动序列中（一切交互禁用）；goodbye = 跳箱动画中（同禁用） */
const phase = ref<"startup" | "normal" | "goodbye">("startup");
const mode = ref<PetMode>("work");
const menuOpen = ref(false);
/** 菜单因失焦被关掉的时刻：紧接着的宠物点击属于"这次点击本身"，不再当开菜单 */
let menuClosedAt = 0;
/** 休息模式轮换调度代号：每次离开/重入休息模式自增，旧定时器自弃 */
let alternationGen = 0;

const interactionsOff = () => phase.value !== "normal";

onMounted(async () => {
  engine.subscribe((s) => {
    Object.assign(sprite, s);
    // 菜单开着时实时同步锁状态（播放中菜单项保持禁用，PetActionExecution）
    if (menuOpen.value) void emitTo("pet-menu", MENU_STATE_EVENT, { locked: s.locked });
  });
  // 启动序列先行（不依赖位移），mover 异步就绪后补注入
  engine.request({
    lock: true,
    steps: [
      { anim: 21 }, // Ear Up
      { anim: 22 }, // Scan
      { anim: 23 }, // Ear Down
      { anim: 24 }, // Jump out of the box
      { anim: 7, loop: true }, // Stand Idle
    ],
    onSettle: onStartupSettled,
  });
  engine.setMover((mover = await createTauriMover()));

  // 系统关闭请求拦截：桌宠无关闭按钮，退出只能走「再见」/托盘（工单 14）
  await win.onCloseRequested((e) => e.preventDefault());

  await listen<{ action: MenuAction }>(MENU_ACTION_EVENT, (e) => {
    menuOpen.value = false;
    onMenuAction(e.payload.action);
  });
  await listen(MENU_CLOSED_EVENT, () => {
    menuOpen.value = false;
    menuClosedAt = performance.now();
  });
});

/** 启动序列完成：进入正常态 + AutoOpenMainBoard 检测 + 稍后转入工作睡眠 */
function onStartupSettled() {
  phase.value = "normal";
  void checkAutoOpen();
  window.setTimeout(() => {
    if (phase.value === "normal" && mode.value === "work" && !engine.state().busy) {
      engine.request({ lock: false, steps: [{ anim: 4 }, { anim: 5, loop: true }] });
    }
  }, STARTUP_STAND_BEAT_MS);
}

/** AutoOpenMainBoard：工作模式 + 今日未分配 → 弹大面板（沿用控制面板的重开语义带回数据） */
async function checkAutoOpen() {
  if (mode.value !== "work") return;
  try {
    if (await shouldAutoOpenMainBoard(true)) await openMainBoard();
  } catch {
    /* 后端不可达时静默：大面板还有控制面板入口兜底 */
  }
}

async function openMainBoard() {
  // 重开必须带回最新数据（06 的重开语义），再复用通用的显示/聚焦
  await emitTo("main-board", "main-board:reopen");
  await openWindow("main-board");
}

function onMenuAction(action: MenuAction) {
  if (action === "control-panel") {
    void openWindow("control-panel");
    return;
  }
  if (engine.state().locked || phase.value !== "normal") return; // 防御：菜单禁用外的兜底
  if (action === "toggle-mode") switchMode();
  if (action === "goodbye") goodbye();
}

/** 打开（或聚焦）一个已存在的顶层窗口 */
async function openWindow(label: string) {
  const target = await WebviewWindow.getByLabel(label);
  await target?.show();
  await target?.setFocus();
}

/** 模式切换（用户动作，占锁）：过渡动画 → 对应 idle；小看板显隐联动（休息隐藏/工作恢复） */
function switchMode() {
  // 先试占锁再改状态：被拒（过渡播放中）时保持原模式，不污染 mode
  if (mode.value === "rest") {
    const ok = engine.request({
      lock: true,
      steps: [{ anim: 4 }, { anim: 5, loop: true }], // Stand to Sleep
      onSettle: () => {
        void restoreMiniBoard();
        void checkAutoOpen();
      },
    });
    if (!ok) return;
    mode.value = "work";
    alternationGen++; // 作废休息轮换调度
  } else {
    const ok = engine.request({
      lock: true,
      steps: [{ anim: 6 }, { anim: 7, loop: true }], // Sleep to Stand
      onSettle: scheduleAlternation,
    });
    if (!ok) return;
    mode.value = "rest";
    alternationGen++;
    void hideWindow("mini-board"); // 休息即不工作（67）
  }
}

async function hideWindow(label: string) {
  const target = await WebviewWindow.getByLabel(label);
  await target?.hide();
}

/** 切回工作模式恢复小看板。可见性不变式沿用 07 的启动规则：小看板可见 ⇔ 存在当前任务
 *  （含"任务完成"停留态，board.current 为 Some）；无当前任务时它本就未显示，谈不上恢复，
 *  等大面板确认分配后由 06 的确认流程点亮。 */
async function restoreMiniBoard() {
  try {
    const board = await getMiniBoard();
    if (board.current) {
      const mini = await WebviewWindow.getByLabel("mini-board");
      await mini?.show();
    }
  } catch {
    /* 读态失败保持隐藏 */
  }
}

/** 休息模式站↔坐轮换：7 循环 →(40s)→ 1 → 2 循环 →(40s)→ 3 → 7 循环 …（用户动作随时抢占） */
function scheduleAlternation() {
  const gen = ++alternationGen;
  const step = (toSit: boolean) => {
    if (gen !== alternationGen || mode.value !== "rest") return;
    const ok = engine.request({
      lock: false,
      steps: toSit
        ? [{ anim: 1 }, { anim: 2, loop: true }] // Stand to Sit → Sit Idle
        : [{ anim: 3 }, { anim: 7, loop: true }], // Sit to Stand → Stand Idle
    });
    if (ok) window.setTimeout(() => step(!toSit), REST_ALTERNATE_MS);
  };
  window.setTimeout(() => step(true), REST_ALTERNATE_MS);
}

/** 再见：跳箱动画播完退出整个应用（关闭全部窗口）。占锁失败不进告别态（否则交互全禁却永不退出） */
function goodbye() {
  const ok = engine.request({
    lock: true,
    steps: [{ anim: 17 }], // Jump in to the Box
    onSettle: () => void exitApp(),
  });
  if (!ok) return;
  phase.value = "goodbye";
  alternationGen++;
}

/* ---- 点击 / 拖拽（拖拽的完整形态——看板跟随、任务栏禁入、边缘吸附——归工单 09） ---- */

let dragFrom: { screenX: number; screenY: number; winX: number; winY: number; factor: number; moved: boolean } | null = null;

function onPointerDown(e: PointerEvent) {
  if (interactionsOff() || !mover) return;
  const p = mover.position();
  dragFrom = {
    screenX: e.screenX,
    screenY: e.screenY,
    winX: p.x,
    winY: p.y,
    factor: mover.scaleFactor?.() ?? 1,
    moved: false,
  };
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
}

function onPointerMove(e: PointerEvent) {
  if (!dragFrom) return;
  const dx = (e.screenX - dragFrom.screenX) * dragFrom.factor; // screenX 是逻辑像素 → 物理位移
  const dy = (e.screenY - dragFrom.screenY) * dragFrom.factor;
  if (Math.abs(dx) > CLICK_SLOP_PX || Math.abs(dy) > CLICK_SLOP_PX) dragFrom.moved = true;
  if (!dragFrom.moved) return;
  mover?.moveTo(dragFrom.winX + dx, dragFrom.winY + dy); // mover 内钳制，不出工作区
}

function onPointerUp() {
  if (!dragFrom) return;
  const wasClick = !dragFrom.moved;
  dragFrom = null;
  if (wasClick) void toggleMenu();
}

async function toggleMenu() {
  if (interactionsOff()) return;
  if (menuOpen.value) {
    menuOpen.value = false;
    await emitTo("pet-menu", MENU_CLOSE_EVENT);
    return;
  }
  // 失焦关闭与本次点击的竞态：菜单刚因这次点击失焦关掉，不再立刻重开（否则切换失效）
  if (performance.now() - menuClosedAt < 400) return;
  menuOpen.value = true;
  const s = engine.state();
  await emitTo("pet-menu", MENU_OPEN_EVENT, { locked: s.locked, mode: mode.value });
  await positionMenu();
}

/** 菜单摆到桌宠正上方（留 8px 间距），整体钳制在当前显示器工作区内 */
async function positionMenu() {
  const menu = await WebviewWindow.getByLabel("pet-menu");
  if (!menu) return;
  const [petPos, petSize, menuSize, monitor] = await Promise.all([
    win.outerPosition(),
    win.outerSize(),
    menu.outerSize(),
    currentMonitor(),
  ]);
  const area = monitor?.workArea;
  if (!area) return;
  const gap = 8;
  let x = petPos.x + (petSize.width - menuSize.width) / 2;
  let y = petPos.y - menuSize.height - gap;
  x = Math.min(Math.max(x, area.position.x), area.position.x + area.size.width - menuSize.width);
  y = Math.min(Math.max(y, area.position.y), area.position.y + area.size.height - menuSize.height);
  await menu.setPosition(new PhysicalPosition(x, y));
  await menu.show();
  await menu.setFocus();
}
</script>

<template>
  <!-- 拖拽/点击区 = 窗口整面（桌宠本体即窗口）；startup/goodbye 期 pointer-events 关掉交互 -->
  <div
    class="pet-shell"
    :class="{ frozen: interactionsOff() }"
    @pointerdown="onPointerDown"
    @pointermove="onPointerMove"
    @pointerup="onPointerUp"
    @pointercancel="onPointerUp"
  >
    <PetSprite :anim="sprite.anim" :frame="sprite.frame" :flip="sprite.flip" />
  </div>
</template>

<style scoped>
.pet-shell {
  height: 100%;
  display: flex;
  align-items: flex-end; /* 地面线 = 窗口底边：蹲/坐/站/睡同高（底部锚定） */
  justify-content: center; /* 每帧水平居中 */
  touch-action: none; /* pointer 事件自己接管，禁浏览器手势 */
  cursor: pointer;
}

.frozen {
  pointer-events: none; /* 启动序列/再见期间：点击、拖拽全部忽略 */
}
</style>
