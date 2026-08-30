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
 * - 再见（菜单「再见」/ 托盘「退出」共用通道，工单 14）：17 Jump in to the Box 播完 →
 *   exitApp()（关闭全部窗口）；动画占用时等播完再接（重试），保证常驻托盘入口永远有效
 * - 小看板按需显隐（2026-08-30 反馈，MiniBoardVisibility）：不再常驻——平时隐藏，
 *   左键点击桌宠唤起/收起（休息模式不响应，67），事件亮相（分配确认/切回工作/启动续接）
 *   自动亮出；每次亮出发 mini-board:show 让看板重置视图并启动 10s 自动隐藏计时（悬停
 *   暂停，✕/到点看板发 dismiss 请求回来由本窗口统一隐藏并解除挂靠）。显隐唯一持有者
 *   是本窗口（拖拽耦合状态 boardAttached 与显隐同源）
 * - 右键拖拽（09，PetDragBounds）：moveTo 每帧经 dragBounds 钳制（任务栏禁入/全可见/跨屏
 *   重叠面积选屏），松手 <20px 吸附最近工作区边缘；拖动期间 freeze 保持当前帧不动、
 *   松手 unfreeze 继续（不打断播放状态）；小看板可见时与桌宠成**刚性组合体**按整体
 *   矩形钳制（一个被边界挡住另一个也一起停，相对位置固定；PetBoardCoupling 位置联动、
 *   动作独立）；休息模式拖后非站立先 3 Sit to Stand 再 15 Attack（用户动作占锁）。
 *   左键拖动不移动桌宠（2026-08-30 反馈：移动归右键，左键留给唤看板）
 * - 今日总结触发（11，DailySummaryTrigger）：常驻心跳负责定时——启动查补登（当天没开
 *   应用 → 次日首开弹"昨日总结"）、定时到最晚工作窗口结束自动弹（只弹一次，服务端登记）
 * - 工作窗口开始触发（13，AutoOpenMainBoard）：心跳定时到下一个工作窗口开始，届时
 *   处于工作模式且今日未分配才弹大面板（处于休息模式不打扰，模式切换本身是另一触发）；
 *   设置保存（settings:changed）后两个定时触发都重排
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
import { MENU_ACTION_EVENT, MENU_CLOSED_EVENT, MENU_CLOSE_EVENT, MENU_OPEN_EVENT, MENU_STATE_EVENT, TRAY_EXIT_EVENT, type MenuAction, type PetMode } from "../lib/pet/menu";
import { openDailySummaryWindow } from "../lib/summary";
import { revealWindow } from "../lib/windows";
import { MAIN_BOARD_REOPEN_EVENT, MINI_BOARD_DISMISS_EVENT, MINI_BOARD_REFRESH_EVENT, MINI_BOARD_SHOW_EVENT, SETTINGS_CHANGED_EVENT } from "../lib/events";
import { exitApp, getDailySummaryStatus, getMiniBoard, getNextWindowStart, isWorkTime, shouldAutoOpenMainBoard } from "../lib/api";

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
  void syncBoardAtStartup();
  // 今日总结触发接线（工单 11，DailySummaryTrigger）：桌宠窗口是常驻心跳，
  // 定时排程放这里（启动补登检查 + 最晚窗口结束的定时触发，与动画序列无关）
  void armDailySummary();
  // 工作窗口开始触发接线（工单 13，AutoOpenMainBoard）：同一心跳排下一个窗口开始
  void armWindowStart();

  // 系统关闭请求拦截：桌宠无关闭按钮，退出只能走「再见」/托盘（工单 14）
  await win.onCloseRequested((e) => e.preventDefault());

  await listen<{ action: MenuAction }>(MENU_ACTION_EVENT, (e) => {
    menuOpen.value = false;
    onMenuAction(e.payload.action);
  });
  // 托盘退出（工单 14）：绕过 onMenuAction 的 normal 守卫——启动序列/过渡动画期间
  // 托盘退出也要生效（托盘是常驻兜底入口），占用问题由 goodbye 内部排队重试承担
  await listen(TRAY_EXIT_EVENT, () => goodbye());
  await listen(MENU_CLOSED_EVENT, () => {
    menuOpen.value = false;
    menuClosedAt = performance.now();
  });
  // 大面板确认（06）会点亮小看板：工作模式按刚性组合就位（亮出即启动自动隐藏计时），
  // 休息模式坚持隐藏（67 休息即不工作——显隐跟着模式走，2026-08-29 反馈）
  await listen(MINI_BOARD_REFRESH_EVENT, () => {
    if (mode.value === "rest") {
      boardAttached = false;
      void hideWindow("mini-board");
    } else {
      void attachBoardRigidly();
    }
  });
  // 小看板请求收起（✕ / 自动隐藏到点，2026-08-30 反馈）：显隐唯一持有者执行隐藏并解除挂靠
  await listen(MINI_BOARD_DISMISS_EVENT, async () => {
    boardAttached = false;
    await hideWindow("mini-board");
  });
  // 设置保存（13）：生效时刻由服务端按版本历史裁决，两个定时触发都重查重排
  await listen(SETTINGS_CHANGED_EVENT, () => {
    void armDailySummary();
    void armWindowStart();
  });
});

/** 启动时对小看板（Rust 侧已按"工作时间 + 有当前任务"决定显隐）：缓存几何并对齐挂靠
 *  状态——休息模式兜底隐藏（Rust/前端判定间的毫秒级漂移不留下可见破绽） */
async function syncBoardAtStartup() {
  const mini = await WebviewWindow.getByLabel("mini-board");
  if (!mini) return;
  const [p, s] = await Promise.all([mini.outerPosition(), mini.outerSize()]);
  Object.assign(board, { x: p.x, y: p.y, width: s.width, height: s.height, ready: true });
  if (mode.value === "rest") {
    boardAttached = false;
    await mini.hide();
  } else {
    boardAttached = await mini.isVisible();
    // 启动亮板（Rust 侧按"工作时间 + 有当前任务"显示）也是亮相：同样走自动隐藏计时
    if (boardAttached) await emitTo("mini-board", MINI_BOARD_SHOW_EVENT);
  }
}

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
  // 重开必须带回最新数据（06 的重开语义），再共享通道亮到前台（unminimize→show→setFocus）
  await emitTo("main-board", MAIN_BOARD_REOPEN_EVENT);
  await revealWindow("main-board");
}

function onMenuAction(action: MenuAction) {
  if (action === "control-panel") {
    void revealWindow("control-panel");
    return;
  }
  if (engine.state().locked || phase.value !== "normal") return; // 防御：菜单禁用外的兜底
  if (action === "toggle-mode") switchMode();
  if (action === "goodbye") goodbye();
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
    boardAttached = false;
    void hideWindow("mini-board"); // 休息即不工作（67）
  }
}

/** 左键单击桌宠：工作模式唤起/收起小看板（休息即不工作，休息模式不响应）。小看板
 *  显隐的唯一持有者（2026-08-30 反馈：常驻改按需——平时隐藏、点击唤起、自动隐藏） */
async function toggleMiniBoard() {
  if (mode.value !== "work") return;
  if (boardAttached) {
    boardAttached = false;
    await hideWindow("mini-board");
    return;
  }
  const mini = await WebviewWindow.getByLabel("mini-board");
  if (!mini) return;
  await emitTo("mini-board", MINI_BOARD_REFRESH_EVENT); // 唤起读最新（隔日/生命周期后可能已变）
  await attachBoardRigidly(); // 刚性就位 + 亮出 + 启动自动隐藏计时
}

async function hideWindow(label: string) {
  const target = await WebviewWindow.getByLabel(label);
  await target?.hide();
}

/** 切回工作模式恢复小看板。可见性不变式：小看板可见 ⇔ 工作模式且存在当前任务（含
 *  "任务完成"停留态，board.current 为 Some）；无当前任务时它本就未显示，等大面板
 *  确认分配后由 refresh 监听点亮。 */
async function restoreMiniBoard() {
  try {
    const view = await getMiniBoard();
    if (!view.current) return;
    await attachBoardRigidly();
  } catch {
    /* 读态失败保持隐藏 */
  }
}

/** 小看板刚性挂靠：以桌宠当前位置为锚重摆（板在桌宠正下方 12px、水平居中——同启动
 *  摆位）；贴边/贴底放不下时**组合体让位**（2026-08-29 反馈：一个被边界挡住另一个也
 *  一起让位）。贴边钳制后桌宠**回正到看板正中上方**（2026-08-30 反馈：宁可移动桌宠，
 *  不把看板歪着放——"桌宠在小看板正中上方"是显示不变式）。 */
async function attachBoardRigidly() {
  const mini = await WebviewWindow.getByLabel("mini-board");
  if (!mini) return;
  if (mover && board.ready) {
    const pet = mover.position();
    const s = mover.size();
    const area = mover.workArea();
    const gap = 12;
    const boardX = Math.round(
      Math.min(Math.max(pet.x + (s.width - board.width) / 2, area.x), area.x + area.width - board.width),
    );
    let boardY = pet.y + s.height + gap;
    let petY = pet.y;
    const overflow = boardY + board.height - (area.y + area.height);
    if (overflow > 0) {
      petY = Math.max(petY - overflow, area.y); // 顶部极端时以桌宠可见优先
      boardY = petY + s.height + gap;
    }
    // 桌宠对准看板中心（boardX 已钳进工作区且看板比桌宠宽 → 结果必在工作区内，无需再钳）
    const petX = Math.round(boardX + (board.width - s.width) / 2);
    board.x = boardX;
    board.y = Math.round(boardY);
    mover.moveTo(petX, petY);
    await mini.setPosition(new PhysicalPosition(board.x, board.y));
  }
  await mini.show();
  boardAttached = true;
  // 亮出即亮相：看板回任务视图干净状态并启动自动隐藏计时（悬停暂停，2026-08-30 反馈）
  await emitTo("mini-board", MINI_BOARD_SHOW_EVENT);
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

/** 再见请求被动画占用拒绝后的重试间隔：等当前动作播完再接跳箱（动画最长数秒，轮询开销可忽略） */
const GOODBYE_RETRY_MS = 300;

/** 再见：跳箱动画播完退出整个应用（关闭全部窗口）。菜单「再见」与托盘「退出」（工单 14）
 *  共用：已在告别中忽略（双入口去重）；动画锁/启动序列占用时重试排队——占锁失败若直接
 *  放弃，交互全禁却永不退出。告别期间一切自动触发静默（总结/开窗定时作废，防止告别
 *  动画的最后一秒弹出总结窗并把"已弹"登记进库）。 */
function goodbye() {
  if (phase.value === "goodbye") return;
  const ok = engine.request({
    lock: true,
    steps: [{ anim: 17 }], // Jump in to the Box
    onSettle: () => void exitApp(),
  });
  if (!ok) {
    window.setTimeout(goodbye, GOODBYE_RETRY_MS);
    return;
  }
  phase.value = "goodbye";
  randomGen++;
  summaryGen++;
  windowStartGen++;
}

/* ---- 今日总结触发（工单 11，DailySummaryTrigger）：最晚窗口结束自动弹 / 次日首开补登 ---- */

/** 触发排程代号：重排时自增，旧定时器自弃（与随机动作调度同构） */
let summaryGen = 0;

/** 启动与每次触发后各排一次：有待弹总结立即弹（补登/补弹），否则定时到下次窗口结束 */
async function armDailySummary() {
  const gen = ++summaryGen;
  try {
    const st = await getDailySummaryStatus();
    if (gen !== summaryGen) return; // 已被更新的排程取代
    if (st.due) void openDailySummary(st.due);
    scheduleSummaryFire(gen, st.next_fire_at);
  } catch {
    /* 后端不可达：控制面板"今日总结"入口兜底 */
  }
}

/** 定时到下一次最晚窗口结束（服务端给的时刻为准）；触发时重查状态（权威裁决）再重排 */
function scheduleSummaryFire(gen: number, nextFireAt: string | null) {
  if (!nextFireAt) return; // 未配置窗口：永不自动触发
  const delay = new Date(nextFireAt).getTime() - Date.now();
  // setTimeout 上限 2^31-1 ms：极端空档（如超长假期）超限时先短睡再重排
  window.setTimeout(
    () => {
      if (gen !== summaryGen) return;
      void armDailySummary(); // 到点重查：due 由服务端判定并弹出，然后排下一次
    },
    Math.min(Math.max(delay, 0), 2 ** 31 - 1),
  );
}

/** 弹出某日总结（自动触发路径）：打开即登记已弹（只弹一次）；失败静默——下次启动补登自愈 */
function openDailySummary(date: string) {
  openDailySummaryWindow(date, true).catch(() => { /* 窗口/登记失败：控制面板"今日总结"入口兜底 */ });
}

/* ---- 工作窗口开始触发（工单 13，AutoOpenMainBoard）：定时到下一个窗口开始 ---- */

/** 触发排程代号：重排时自增，旧定时器自弃（与总结触发同构） */
let windowStartGen = 0;

/** 启动与每次触发/设置保存后各排一次：定时到下一个工作窗口开始（服务端给的时刻为准） */
async function armWindowStart() {
  const gen = ++windowStartGen;
  try {
    const next = await getNextWindowStart();
    if (gen !== windowStartGen) return; // 已被更新的排程取代
    if (!next) return; // 未配置窗口：永不自动触发
    window.setTimeout(
      () => {
        if (gen !== windowStartGen) return;
        // 触发时仍由服务端权威裁决（工作模式 + 今日未分配）；处于休息模式不打扰——
        // 用户手动切回工作模式本身是另一条触发（manual，主动加班）
        void checkAutoOpen(false);
        void armWindowStart(); // 排再下一个窗口开始
      },
      Math.min(Math.max(new Date(next).getTime() - Date.now(), 0), 2 ** 31 - 1),
    );
  } catch {
    /* 后端不可达：下次设置保存/重启自愈 */
  }
}

/* ---- 点击 / 拖拽（09 完整形态：PetDragBounds 四约束 + 刚性看板组合 + 冻结帧） ---- */

/** 小看板几何缓存（mount 取一次，之后只被本窗口的摆位/随动改写） */
const board = { x: 0, y: 0, width: 0, height: 0, ready: false };
/** 小看板挂靠状态（可见 = 与桌宠成刚性组合体）。显隐来源：Rust 启动（工作时段 + 有
 *  当前任务）、大面板确认（06）、模式切换（08/67）——PetWindow 持续跟踪。 */
let boardAttached = false;

let drag: {
  px: number;
  py: number; // 指针起点（逻辑像素）
  btn: number; // 按下键（0 左 / 2 右）：左键点击唤看板、右键点击菜单、右键拖动移动（2026-08-30 反馈）
  winX: number;
  winY: number; // 桌宠窗口起点（物理像素）
  boardX: number;
  boardY: number; // 小看板窗口起点（未挂靠为 NaN，随动跳过）
  unitX: number;
  unitY: number; // 组合体（桌宠∪看板；未挂靠 = 桌宠矩形）起点
  unitW: number;
  unitH: number;
  factor: number;
  moved: boolean;
} | null = null;

/** 组合体矩形 = 桌宠 ∪ 小看板（挂靠时）；返回拖拽起点全套几何 */
function unitOrigin(petX: number, petY: number, petW: number, petH: number) {
  if (!boardAttached || !board.ready) {
    return { boardX: NaN, boardY: NaN, unitX: petX, unitY: petY, unitW: petW, unitH: petH };
  }
  const unitX = Math.min(petX, board.x);
  const unitY = Math.min(petY, board.y);
  return {
    boardX: board.x,
    boardY: board.y,
    unitX,
    unitY,
    unitW: Math.max(petX + petW, board.x + board.width) - unitX,
    unitH: Math.max(petY + petH, board.y + board.height) - unitY,
  };
}

/** 显示器快照：mover 未提供时退化为单屏（workArea 等价物） */
function monitorsSnapshot(): MonitorArea[] {
  if (mover?.monitors) return mover.monitors();
  const a = mover!.workArea();
  return [
    { x: a.x, y: a.y, width: a.width, height: a.height, workX: a.x, workY: a.y, workWidth: a.width, workHeight: a.height },
  ];
}

/** 小看板刚性落位：组合体已整体钳制，这里只写缓存 + 移窗（未挂靠/几何未就绪跳过）。
 *  桌宠动作不带动看板（PetBoardCoupling：位置联动、动作独立）。 */
function placeBoardRaw(x: number, y: number) {
  if (!Number.isFinite(x)) return;
  board.x = Math.round(x);
  board.y = Math.round(y);
  void WebviewWindow.getByLabel("mini-board").then((mini) =>
    mini?.setPosition(new PhysicalPosition(board.x, board.y)),
  );
}

function onPointerDown(e: PointerEvent) {
  if (interactionsOff() || !mover) return;
  void mover.refresh?.(); // 拖拽前刷新屏幕拓扑（显示器热插拔/跨屏 DPI 变化）
  const p = mover.position();
  const s = mover.size();
  drag = {
    px: e.screenX,
    py: e.screenY,
    btn: e.button,
    winX: p.x,
    winY: p.y,
    factor: mover.scaleFactor?.() ?? 1,
    moved: false,
    ...unitOrigin(p.x, p.y, s.width, s.height),
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
    if (d.btn === 2) engine.freeze(); // 右键拖动期间保持当前帧不动（README 原义），播放状态保留
  }
  if (d.btn !== 2) return; // 左键拖动不移动桌宠（2026-08-30 反馈：移动归右键），位移只作点击判定
  // PetDragBounds 约束 1/2/3 按**组合体**（未挂靠 = 桌宠单体）矩形：跨屏重叠面积选屏 +
  // 钳进工作区。刚性：一个成员被边界挡住，全体一起停（相对位置固定，2026-08-29 反馈）
  const landing = clampDragPosition(d.unitX + dx, d.unitY + dy, d.unitW, d.unitH, monitorsSnapshot());
  const ax = landing.x - d.unitX;
  const ay = landing.y - d.unitY;
  mover.moveTo(d.winX + ax, d.winY + ay);
  placeBoardRaw(d.boardX + ax, d.boardY + ay);
}

function onPointerUp() {
  const d = drag;
  drag = null;
  if (!d || !mover) return;
  if (!d.moved) {
    // 单击分流（2026-08-30 反馈）：左键唤/收小看板，右键菜单
    if (d.btn === 0) void toggleMiniBoard();
    if (d.btn === 2) void toggleMenu();
    return;
  }
  if (d.btn !== 2) return; // 左键拖动：无动作（不吸附、不移动、无拖后动作）
  // PetDragBounds 约束 4：组合体贴近工作区边（<20px 逻辑）吸附最近边，增量同样刚性
  const appliedX = mover.position().x - d.winX;
  const appliedY = mover.position().y - d.winY;
  const cur = clampDragPosition(d.unitX + appliedX, d.unitY + appliedY, d.unitW, d.unitH, monitorsSnapshot());
  const snapped = snapToEdges(
    cur.x,
    cur.y,
    d.unitW,
    d.unitH,
    cur.monitor,
    SNAP_PX * (mover.scaleFactor?.() ?? 1),
  );
  const ax = snapped.x - d.unitX;
  const ay = snapped.y - d.unitY;
  mover.moveTo(d.winX + ax, d.winY + ay);
  placeBoardRaw(d.boardX + ax, d.boardY + ay);
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
  await revealWindow("pet-menu");
}
</script>

<template>
  <!-- 拖拽/点击区 = 窗口整面（桌宠本体即窗口）；startup/goodbye 期 pointer-events 关掉交互。
       左键点击唤/收小看板、右键点击菜单、右键拖动移动（2026-08-30 反馈）；原生右键菜单已在 main.ts 全局静默 -->
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
