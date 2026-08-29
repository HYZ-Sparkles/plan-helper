<script setup lang="ts">
/**
 * 桌宠窗口（工单 08 主接线、09 交互编排）：启动序列 → 正常态（菜单 / 模式切换 / 再见 / 拖拽 / 随机动作）。
 *
 * - 启动序列 21→22→23→24→7(循环)：期间点击/菜单/拖拽全部忽略（比"动画播放中"更严格）
 * - 初始模式按工作时间判定（isWorkTime：工作日 + 时间窗口内 = 工作，否则休息）。
 *   序列完成（7 开始循环那一刻）→ 进入正常态：工作模式 700ms 后 Stand to Sleep 过渡
 *   进 Sleep Idle，且 AutoOpenMainBoard 检测接入（今日未分配 → 弹大面板）；休息模式
 *   序列本就收在 7 Stand Idle（休息常驻），直接排随机动作计时，不弹大面板
 * - 模式切换（菜单项，用户动作占锁）：休息→工作 4 Stand to Sleep；工作→休息 6 Sleep to
 *   Stand；过渡完进对应 idle（工作=5 循环；休息=7/2 常驻，切换由随机动作的"闲坐"承担）
 * - 随机动作（09，PetRandomAction）：休息模式距上一次（用户或随机）动作结束 300s 抽取，
 *   吃:跳:闲坐 = 4:3:3；系统动作不占锁、仅空闲发起、可被用户动作抢占（引擎裁决）；
 *   播放中硬切由引擎 flick 计数驱动 PetSprite 快速淡出淡入
 * - 再见：17 Jump in to the Box 播完 → exitApp()（关闭全部窗口）
 * - 拖拽（09，PetDragBounds）：moveTo 每帧经 dragBounds 钳制（任务栏禁入/全可见/跨屏
 *   重叠面积选屏），松手 <20px 吸附最近工作区边缘；拖动期间 freeze 保持当前帧不动、
 *   松手 unfreeze 继续（不打断播放状态）；小看板按桌宠**实际**位移随动（PetBoardCoupling：
 *   位置联动、动作独立）；休息模式拖后非站立先 3 Sit to Stand 再 15 Attack（用户动作占锁）
 */
import { onMounted, reactive, ref } from "vue";
import { emitTo, listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow, WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { currentMonitor } from "@tauri-apps/api/window";
import { PhysicalPosition } from "@tauri-apps/api/dpi";
import PetSprite from "../components/PetSprite.vue";
import { autoDirection, PetEngine, type EngineState, type PetMover } from "../lib/pet/engine";
import { createTauriMover } from "../lib/pet/tauriMover";
import { clampDragPosition, snapToEdges, type MonitorArea } from "../lib/pet/dragBounds";
import { afterDragSteps, pickRandomKind, RANDOM_INTERVAL_MS, randomSteps, type Pose } from "../lib/pet/actions";
import { MENU_ACTION_EVENT, MENU_CLOSED_EVENT, MENU_CLOSE_EVENT, MENU_OPEN_EVENT, MENU_STATE_EVENT, type MenuAction, type PetMode } from "../lib/pet/menu";
import { exitApp, getMiniBoard, isWorkTime, shouldAutoOpenMainBoard } from "../lib/api";

/** 启动序列落地（7 Stand Idle 开始）后站立的展示节拍，再转入工作睡眠 */
const STARTUP_STAND_BEAT_MS = 700;
/** 判定为"点击"的最大位移（逻辑像素，小于它不算拖拽） */
const CLICK_SLOP_PX = 4;
/** 边缘吸附阈值（逻辑像素，PetDragBounds 第 4 约束） */
const SNAP_PX = 20;

const win = getCurrentWebviewWindow();
const engine = new PetEngine();
let mover: PetMover | null = null;
const sprite = reactive<EngineState>({ anim: 0, frame: 0, flip: false, locked: false, busy: false, flick: 0 });
/** startup = 启动序列中（一切交互禁用）；goodbye = 跳箱动画中（同禁用） */
const phase = ref<"startup" | "normal" | "goodbye">("startup");
/** 当前模式：初始值在 onMounted 里按工作时间判定覆写（后端不可达时保持默认工作） */
const mode = ref<PetMode>("work");
/** 休息模式常驻姿态（拖后是否先起身、闲坐往哪边切都看它） */
const pose = ref<Pose>("stand");
const menuOpen = ref(false);
/** 菜单因失焦被关掉的时刻：紧接着的宠物点击属于"这次点击本身"，不再当开菜单 */
let menuClosedAt = 0;
/** 随机动作调度代号：每次重新排程/离开休息模式自增，旧定时器自弃 */
let randomGen = 0;

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
  // 初始模式按工作时间判定（invoke 毫秒级、序列数秒，来得及在序列播完前落定）
  try {
    mode.value = (await isWorkTime()) ? "work" : "rest";
  } catch {
    /* 后端不可达保持默认工作 */
  }
  engine.setMover((mover = await createTauriMover()));
  void cacheBoardGeometry();

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

/** 启动序列完成：进入正常态并按初始模式分流——工作 = AutoOpen 检测 + 稍后转睡眠；休息 = 随机动作计时 */
function onStartupSettled() {
  phase.value = "normal";
  if (mode.value === "rest") {
    pose.value = "stand"; // 序列本就收在 7 Stand Idle（休息常驻动作）
    armRandom();
    return;
  }
  void checkAutoOpen(false);
  window.setTimeout(() => {
    // 守卫判 locked 而非 busy：稳态 idle 循环里 action 永不清空、busy 恒真，
    // 误判 busy 会让转睡眠永不触发（工作模式下一直站立）
    if (phase.value === "normal" && mode.value === "work" && !engine.state().locked) {
      engine.request({ lock: false, steps: [{ anim: 4 }, { anim: 5, loop: true }] });
    }
  }, STARTUP_STAND_BEAT_MS);
}

/** AutoOpenMainBoard 检测：工作模式 + 今日未分配 → 弹大面板（沿用控制面板的重开语义带回数据）。
 *  manual = 手动切入工作模式（用户主动选加班，非工作日也弹）；启动后的自动检测传
 *  false（非工作日不打扰）。 */
async function checkAutoOpen(manual: boolean) {
  if (mode.value !== "work") return;
  try {
    if (await shouldAutoOpenMainBoard(true, manual)) await openMainBoard();
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
        void checkAutoOpen(true); // 手动切入 = 主动加班：非工作日也弹大面板选任务
      },
    });
    if (!ok) return;
    mode.value = "work";
    randomGen++; // 作废随机动作计时（工作模式不触发）
  } else {
    const ok = engine.request({
      lock: true,
      steps: [{ anim: 6 }, { anim: 7, loop: true }], // Sleep to Stand
      onSettle: () => {
        pose.value = "stand";
        armRandom();
      },
    });
    if (!ok) return;
    mode.value = "rest";
    randomGen++;
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

/* ---- 随机动作调度（09，PetRandomAction）：距上一次动作结束 300s，吃:跳:闲坐 = 4:3:3 ---- */

/** 重排计时：每个动作（用户或随机）的 onSettle 调一次；离开休息模式/再见的 gen 自增使旧计时自弃 */
function armRandom() {
  if (mode.value !== "rest" || phase.value !== "normal") return;
  const gen = ++randomGen;
  window.setTimeout(() => fireRandom(gen), RANDOM_INTERVAL_MS);
}

/** 触发时刻的守卫全量复查（计时期间模式/阶段可能已变）；被拒（过渡中）自愈重排 */
function fireRandom(gen: number) {
  if (gen !== randomGen || mode.value !== "rest" || phase.value !== "normal" || drag || !mover) return;
  const area = mover.workArea();
  const side = autoDirection(mover.position().x, area.x, area.x + area.width - mover.size().width);
  const { steps, nextPose } = randomSteps(pickRandomKind(Math.random()), pose.value, side);
  const ok = engine.request({
    lock: false, // 系统随机动作不占锁
    steps,
    onSettle: () => {
      pose.value = nextPose;
      armRandom(); // 动作结束（进入稳态循环）→ 下一轮 300s
    },
  });
  if (!ok) armRandom();
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
  randomGen++;
}

/* ---- 点击 / 拖拽（09 完整形态：PetDragBounds 四约束 + 看板随动 + 冻结帧） ---- */

let drag: {
  px: number;
  py: number; // 指针起点（逻辑像素）
  winX: number;
  winY: number; // 桌宠窗口起点（物理像素）
  boardX: number;
  boardY: number; // 小看板窗口起点（几何未就绪为 NaN，随动跳过）
  factor: number;
  moved: boolean;
} | null = null;

/** 小看板几何缓存（PetBoardCoupling 位置联动）：mount 取一次，之后只被本窗口的随动改写 */
const board = { x: 0, y: 0, width: 0, height: 0, ready: false };

async function cacheBoardGeometry() {
  const mini = await WebviewWindow.getByLabel("mini-board");
  if (!mini) return;
  const [p, s] = await Promise.all([mini.outerPosition(), mini.outerSize()]);
  Object.assign(board, { x: p.x, y: p.y, width: s.width, height: s.height, ready: true });
}

/** 显示器快照：mover 未提供时退化为单屏（workArea 等价物） */
function monitorsSnapshot(): MonitorArea[] {
  if (mover?.monitors) return mover.monitors();
  const a = mover!.workArea();
  return [
    { x: a.x, y: a.y, width: a.width, height: a.height, workX: a.x, workY: a.y, workWidth: a.width, workHeight: a.height },
  ];
}

/** 小看板随动落位：钳制进它自己的显示器工作区（始终全可见）；桌宠动作不带动它 */
function placeBoard(x: number, y: number) {
  if (!board.ready || !Number.isFinite(x)) return;
  const next = clampDragPosition(x, y, board.width, board.height, monitorsSnapshot());
  board.x = next.x;
  board.y = next.y;
  void WebviewWindow.getByLabel("mini-board").then((mini) =>
    mini?.setPosition(new PhysicalPosition(next.x, next.y)),
  );
}

function onPointerDown(e: PointerEvent) {
  if (interactionsOff() || !mover) return;
  void mover.refresh?.(); // 拖拽前刷新屏幕拓扑（显示器热插拔/跨屏 DPI 变化）
  const p = mover.position();
  drag = {
    px: e.screenX,
    py: e.screenY,
    winX: p.x,
    winY: p.y,
    boardX: board.ready ? board.x : NaN,
    boardY: board.ready ? board.y : NaN,
    factor: mover.scaleFactor?.() ?? 1,
    moved: false,
  };
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
}

function onPointerMove(e: PointerEvent) {
  const d = drag;
  if (!d || !mover) return;
  const dx = (e.screenX - d.px) * d.factor; // screenX 是逻辑像素 → 物理位移
  const dy = (e.screenY - d.py) * d.factor;
  if (!d.moved) {
    if (Math.abs(dx) <= CLICK_SLOP_PX && Math.abs(dy) <= CLICK_SLOP_PX) return;
    d.moved = true;
    engine.freeze(); // 拖动期间保持当前帧不动（README 原义），播放状态保留
  }
  const size = mover.size();
  // PetDragBounds 约束 1/2/3：跨屏重叠面积选屏 + 钳进工作区（任务栏禁入、全可见）
  const next = clampDragPosition(d.winX + dx, d.winY + dy, size.width, size.height, monitorsSnapshot());
  mover.moveTo(next.x, next.y);
  // 小看板按桌宠实际位移（钳制后）随动，边缘处同步停住不漂移
  placeBoard(d.boardX + (next.x - d.winX), d.boardY + (next.y - d.winY));
}

function onPointerUp() {
  const d = drag;
  drag = null;
  if (!d || !mover) return;
  if (!d.moved) {
    void toggleMenu();
    return;
  }
  // PetDragBounds 约束 4：停靠 <20px（逻辑）自动吸附最近工作区边缘
  const size = mover.size();
  const cur = mover.position();
  const landing = clampDragPosition(cur.x, cur.y, size.width, size.height, monitorsSnapshot());
  const snapped = snapToEdges(
    landing.x,
    landing.y,
    size.width,
    size.height,
    landing.monitor,
    SNAP_PX * (mover.scaleFactor?.() ?? 1),
  );
  mover.moveTo(snapped.x, snapped.y);
  placeBoard(d.boardX + (snapped.x - d.winX), d.boardY + (snapped.y - d.winY));
  engine.unfreeze(); // 动画从冻结帧继续（拖动不打断播放状态）
  if (mode.value === "rest" && phase.value === "normal") requestAfterDrag();
}

/** 拖拽后（休息模式）：非站立先 3 Sit to Stand，然后 15 Attack（用户动作占锁，settle 重排随机计时） */
function requestAfterDrag() {
  engine.request({
    lock: true,
    steps: afterDragSteps(pose.value),
    onSettle: () => {
      pose.value = "stand";
      armRandom();
    },
  });
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
    <PetSprite :anim="sprite.anim" :frame="sprite.frame" :flip="sprite.flip" :fade-signal="sprite.flick" />
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
