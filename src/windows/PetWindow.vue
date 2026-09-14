<script setup lang="ts">
/**
 * 桌宠窗口（工单 08 主接线、09 交互编排、16 起 codex 契约词汇表、17 生命周期重铸）：
 * 启动序列 → 正常态（菜单 / 模式切换 / 再见 / 拖拽 / 随机动作）。
 *
 * - 启动序列 = waving 挥手（约 0.7s，ADR-0010）：期间点击/菜单/拖拽全部忽略（比
 *   "动画播放中"更严格）；播完按初始模式**三分流**——工作且大面板即将自动弹出 →
 *   直接 waiting，工作 → running，休息 → idle；大面板在 waving 播完前不弹
 * - 初始模式按工作时间判定（isWorkTime：工作日 + 时间窗口内 = 工作，否则休息）；
 *   工作常驻 running（陪你伏案）、休息常驻 idle（语义翻转自 Sleep Idle，ADR-0010）
 * - 模式切换（菜单项）：硬切常驻 + flick 快速淡出淡入兜底（codex 无入睡/醒来过渡）；
 *   小看板显隐联动（休息隐藏/工作恢复）
 * - 随机动作（19，PetRandomAction）：休息模式距上一次（用户或随机）动作结束 300s、
 *   50% 概率触发；系统动作不占锁、仅稳态发起、可被用户动作抢占（引擎裁决）
 * - 再见（菜单「再见」/ 托盘「退出」共用通道，工单 14/17）：桌宠窗口直接淡出
 *   （280ms CSS 过渡，无告别仪式）后 exitApp 关闭全部窗口并退进程；双入口去重、
 *   告别期静默保证常驻托盘入口永远有效
 * - 小看板按需显隐（2026-08-30 反馈，MiniBoardVisibility）：平时隐藏，左键点击桌宠
 *   唤起/收起（休息模式不响应，67），事件亮相（分配确认/切回工作/启动续接）自动亮出；
 *   每次亮出发 mini-board:show 让看板重置视图并启动 10s 自动隐藏计时（悬停暂停，
 *   ✕/到点看板发 dismiss 请求回来由本窗口统一隐藏并解除挂靠）。显隐唯一持有者
 *   是本窗口（拖拽耦合状态 boardAttached 与显隐同源）
 * - 拖拽（PetDragBounds 四约束 + 刚性看板组合）：moveTo 每帧经 dragBounds 钳制
 *   （任务栏禁入/全可见/跨屏重叠面积选屏），松手 <20px 吸附最近工作区边缘；
 *   拖拽反馈动画（拖起 jumping → 140ms 滑窗方向跑）在工单 18 接线。
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
import { PetEngine, type EngineState, type PetMover } from "../lib/pet/engine";
import { createTauriMover } from "../lib/pet/tauriMover";
import { clampDragPosition, snapToEdges, type MonitorArea } from "../lib/pet/dragBounds";
import { chassisAction, chassisAnim, jumpSteps, pickRandomKind, RANDOM_INTERVAL_MS } from "../lib/pet/actions";
import { DEFAULT_SKIN, type SkinDef } from "../lib/pet/skins";
import { MENU_ACTION_EVENT, MENU_BLUR_EVENT, MENU_EXPIRE_EVENT, MENU_HINT_EVENT, MENU_OPEN_EVENT, MENU_STATE_EVENT, TRAY_EXIT_EVENT, type MenuAction, type PetMode } from "../lib/pet/menu";
import { openDailySummaryWindow } from "../lib/summary";
import { revealWindow } from "../lib/windows";
import { MAIN_BOARD_REOPEN_EVENT, MINI_BOARD_DISMISS_EVENT, MINI_BOARD_REFRESH_EVENT, MINI_BOARD_SHOW_EVENT, SETTINGS_CHANGED_EVENT } from "../lib/events";
import { exitApp, getDailySummaryStatus, getMiniBoard, getNextWindowStart, isWorkTime, shouldAutoOpenMainBoard } from "../lib/api";

/** 判定为"点击"的最大位移（逻辑像素，小于它不算拖拽） */
const CLICK_SLOP_PX = 4;
/** 边缘吸附阈值（逻辑像素，PetDragBounds 第 4 约束） */
const SNAP_PX = 20;

const win = getCurrentWebviewWindow();
const engine = new PetEngine();
let mover: PetMover | null = null;
const sprite = reactive<EngineState>({ anim: "idle", frame: 0, locked: false, busy: false, flick: 0 });
/** startup = 启动序列中（一切交互禁用）；goodbye = 退出流程中（同禁用） */
const phase = ref<"startup" | "normal" | "goodbye">("startup");
/** 当前模式：初始值在 onMounted 里按工作时间判定覆写（后端不可达时保持默认工作） */
const mode = ref<PetMode>("work");
/** 告别淡出中（模板 class：opacity → 0） */
const fadingOut = ref(false);
/** 当前形象（工单 22 接设置页切换与持久化；本单先取注册表默认） */
const skin = ref<SkinDef>(DEFAULT_SKIN);
/** 浮层（菜单/提示气泡）代数：每次亮出/按下预定自增。pet-menu 的失焦/到点回执携带
 *  它亮出时的代数，落后于当前代数 = 已被本次点击的换形态亮出顶替，不执行隐藏——
 *  跨窗口 hide/show 竞态的裁决点（2026-08-30 反馈：交替点击闪现的根因是两个 webview
 *  各自发 hide/show、到达顺序不保证） */
let floatSeq = 0;
/** 本次按下预定的浮层代数（右键=菜单、休息左键=气泡；0=无预定）。按下即预定，让
 *  同一次点击触发的失焦回执必然落败；pointerup 时消费或作废 */
let reservedSeq = 0;
/** 上次同步给浮层的锁状态（按变化发 MENU_STATE，见 engine.subscribe 内注释） */
let lastSyncedLocked: boolean | null = null;
/** 随机动作调度代号：每次重新排程/离开休息模式自增，旧定时器自弃 */
let randomGen = 0;

const interactionsOff = () => phase.value !== "normal";

onMounted(async () => {
  engine.subscribe((s) => {
    Object.assign(sprite, s);
    // 锁状态变化才同步（菜单/气泡开着时项禁用态实时翻转；按变化发避免了原来每帧发一次的 IPC 刷屏）
    if (s.locked !== lastSyncedLocked) {
      lastSyncedLocked = s.locked;
      void emitTo("pet-menu", MENU_STATE_EVENT, { locked: s.locked });
    }
  });
  // 启动序列先行（不依赖位移），mover 异步就绪后补注入
  engine.request({
    lock: true,
    steps: [{ anim: "waving" }],
    onSettle: onStartupSettled,
  });
  // 初始模式按工作时间判定（invoke 毫秒级、waving 约 0.7s，来得及在播完前落定）
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
    void hideWindow("pet-menu"); // 菜单项已点选：浮层消费掉（隐藏归桌宠，单持有者）
    onMenuAction(e.payload.action);
  });
  // 托盘退出（工单 14）：绕过 onMenuAction 的 normal 守卫——启动序列/过渡动画期间
  // 托盘退出也要生效（托盘是常驻兜底入口），占用问题由 goodbye 内部排队重试承担
  await listen(TRAY_EXIT_EVENT, () => goodbye());
  // 浮层回执（失焦/气泡到点）：代数仍是当前才真藏，被换形态顶替的回执一律忽略
  await listen<{ seq: number }>(MENU_BLUR_EVENT, (e) => onFloatGone(e.payload.seq));
  await listen<{ seq: number }>(MENU_EXPIRE_EVENT, (e) => onFloatGone(e.payload.seq));
  // 大面板确认（06）会点亮小看板：工作模式按刚性组合就位（亮出即启动自动隐藏计时），
  // 休息模式坚持隐藏（67 休息即不工作——显隐跟着模式走，2026-08-29 反馈）。
  // 确认同时会关掉大面板：重落常驻（waiting → 该模式常驻，工单 20 补齐对面板关闭的
  // 事件化即时响应）
  await listen(MINI_BOARD_REFRESH_EVENT, () => {
    if (mode.value === "rest") {
      boardAttached = false;
      void hideWindow("mini-board");
    } else {
      void attachBoardRigidly();
    }
    void applyChassis();
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

/** 落常驻：按当下状态请求常驻循环（大面板开着 = waiting——工单 20 补面板关闭事件
 *  的即时回常驻，本单在每次落常驻时查实时可见性）。常驻未变不重排（循环从头会闪一帧）；
 *  正在播一次性动作时被引擎拒绝——其 onSettle 会再调一次，届时自然落到最新状态。
 *  flick = 模式硬切的淡出淡入兜底（换常驻来自不同动作族、无过渡帧可衔接）。 */
async function applyChassis(flick = false) {
  if (phase.value !== "normal") return;
  const target = chassisAnim(mode.value, await mainBoardVisible());
  if (sprite.anim === target && engine.state().busy) return; // 常驻已在播且未变
  engine.request(chassisAction(target, flick));
}

/** 大面板当前是否可见（waiting 的触发源；任何打开路径都反映到窗口可见性上） */
async function mainBoardVisible(): Promise<boolean> {
  try {
    return (await WebviewWindow.getByLabel("main-board"))?.isVisible() ?? false;
  } catch {
    return false;
  }
}

/** 启动序列完成：进入正常态并按初始模式三分流（工单 17）——工作且大面板即将自动
 *  弹出 → 直接 waiting；工作 → running；休息 → idle。大面板在 waving 播完前不弹
 *  （checkAutoOpen 在此才调用），落位前桌宠停在挥手末帧等检测结果。 */
async function onStartupSettled() {
  phase.value = "normal";
  if (mode.value === "rest") {
    await applyChassis();
    armRandom();
    return;
  }
  await checkAutoOpen(false);
  await applyChassis();
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
  if (action === "toggle-mode") void switchMode();
  if (action === "goodbye") goodbye();
}

/** 模式切换（用户动作）：硬切常驻 + flick 兜底（codex 无入睡/醒来过渡动画）；
 *  小看板显隐联动（休息隐藏/工作恢复）。切入工作先做 AutoOpen 检测再落常驻——
 *  面板将弹则直接落 waiting（三分流的手动加班支）。 */
async function switchMode() {
  if (mode.value === "rest") {
    mode.value = "work";
    randomGen++; // 作废随机动作计时（工作模式不触发）
    void restoreMiniBoard();
    await checkAutoOpen(true); // 手动切入 = 主动加班：非工作日也弹大面板选任务
    await applyChassis(true);
  } else {
    mode.value = "rest";
    randomGen++;
    boardAttached = false;
    void hideWindow("mini-board"); // 休息即不工作（67）
    await applyChassis(true);
    armRandom();
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

/* ---- 随机动作调度（19，PetRandomAction）：距上一次动作结束 300s，50% 触发 ---- */

/** 重排计时：每个动作（用户或随机）的 onSettle 调一次；离开休息模式/再见的 gen 自增使旧计时自弃 */
function armRandom() {
  if (mode.value !== "rest" || phase.value !== "normal") return;
  const gen = ++randomGen;
  window.setTimeout(() => fireRandom(gen), RANDOM_INTERVAL_MS);
}

/** 触发时刻的守卫全量复查（计时期间模式/阶段可能已变）；被拒（过渡中）自愈重排。
 *  池子 1:1（跳跃 : 自主移动）——自主移动的 home/away 接线在工单 19，本单两类都先
 *  落跳跃（PetActionPolicy 白名单已含随机跳跃）。 */
function fireRandom(gen: number) {
  if (gen !== randomGen || mode.value !== "rest" || phase.value !== "normal" || drag || !mover) return;
  pickRandomKind(Math.random()); // 抽签留痕（工单 19 接入移动池）
  const ok = engine.request({
    lock: false, // 系统随机动作不占锁
    steps: jumpSteps(),
    onSettle: () => applyChassisAfterRandom(),
  });
  if (!ok) armRandom();
}

/** 随机动作结束：回常驻并起下一轮计时（间隔从动作结束算起） */
function applyChassisAfterRandom() {
  applyChassis();
  armRandom();
}

/** 告别淡出时长：CSS 过渡 280ms + 余量（淡完才关窗，不闪黑框） */
const GOODBYE_FADE_MS = 300;
/** 再见：桌宠窗口直接淡出（ADR-0010：codex 契约无告别动作、不做告别仪式）→ 淡完
 *  exitApp 关闭全部窗口并退进程。菜单「再见」与托盘「退出」（工单 14）共用这一通道
 *  （两条退出路径的行为等价性由此保证）：已在告别中忽略（双入口去重）；淡出不依赖
 *  引擎（任何动画状态下都能立即开始，取代旧跳箱动画的占锁重试）；告别期间一切自动
 *  触发静默（总结/开窗定时作废，防止告别的最后一秒弹出总结窗并把"已弹"登记进库）。 */
function goodbye() {
  if (phase.value === "goodbye") return;
  phase.value = "goodbye";
  randomGen++;
  summaryGen++;
  windowStartGen++;
  fadingOut.value = true; // CSS opacity 过渡（280ms）
  window.setTimeout(() => void exitApp(), GOODBYE_FADE_MS);
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
    if (!next) return; // 未配置窗口：永不触发
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

/* ---- 点击 / 拖拽（PetDragBounds 四约束 + 刚性看板组合；反馈动画在工单 18 接线） ---- */

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
  unitY: number;
  unitW: number;
  unitH: number; // 组合体（桌宠∪看板；未挂靠 = 桌宠矩形）起点与尺寸
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
  // 浮层代数预定：本次点击若将亮出浮层（右键=菜单、休息左键=气泡），按下就占用新一
  // 代数——点击引发的浮层失焦回执（旧代数）届时必被 onFloatGone 裁决落败，不执行隐藏
  reservedSeq = drag.btn === 2 || (drag.btn === 0 && mode.value === "rest") ? ++floatSeq : 0;
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
    // 单击分流（2026-08-30 反馈，设定式）：左键工作模式唤/收小看板、休息模式弹气泡；
    // 右键亮菜单。按下时预定的浮层代数在此消费（换形态 = 旧失焦回执已落败，就地流畅切换）
    const seq = reservedSeq;
    reservedSeq = 0;
    if (d.btn === 0) {
      if (mode.value === "work") void toggleMiniBoard();
      else void showRestHint(seq);
    }
    if (d.btn === 2) void showMenu(seq);
    return;
  }
  const seq = reservedSeq;
  reservedSeq = 0; // 拖拽不亮浮层：按下时的预定作废
  if (seq) void hideWindow("pet-menu"); // 预定过浮层（右键/休息左键）却拖走了：桌宠挪位，浮层一并收起
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
}

/** 右键：亮出菜单形态（设定式——已开着就原样重亮，关闭靠失焦或点菜单项） */
async function showMenu(seq: number) {
  const s = engine.state();
  await emitTo("pet-menu", MENU_OPEN_EVENT, { locked: s.locked, mode: mode.value, seq });
  await positionMenu();
}

/** 浮层失焦/到点回执的裁决：代数仍是当前才真藏。被换形态顶替的回执（本次点击按下
 *  时已预定新一代数）在这里落败——跨窗口 hide/show 竞态由此消解，切换不闪不吞 */
function onFloatGone(seq: number) {
  if (seq !== floatSeq) return;
  void hideWindow("pet-menu");
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

/** 休息模式左键提示（2026-08-30 反馈）：桌宠头顶冒气泡提示菜单入口，2.5s 到点
 *  由 pet-menu 发回执、桌宠按代数裁决隐藏。摆位/亮出与菜单共用 positionMenu */
async function showRestHint(seq: number) {
  await emitTo("pet-menu", MENU_HINT_EVENT, { seq });
  await positionMenu();
}
</script>

<template>
  <!-- 拖拽/点击区 = 窗口整面（桌宠本体即窗口）；startup/goodbye 期 pointer-events 关掉交互。
       左键点击唤/收小看板、右键点击菜单、右键拖动移动（2026-08-30 反馈）；原生右键菜单已在 main.ts 全局静默 -->
  <div
    class="pet-shell"
    :class="{ frozen: interactionsOff(), goodbye: fadingOut }"
    @pointerdown="onPointerDown"
    @pointermove="onPointerMove"
    @pointerup="onPointerUp"
    @pointercancel="onPointerUp"
  >
    <PetSprite :anim="sprite.anim" :frame="sprite.frame" :sheet="skin.sheet" :fade-signal="sprite.flick" />
  </div>
</template>

<style scoped>
.pet-shell {
  height: 100%;
  display: flex;
  align-items: flex-end; /* 地面线 = 窗口底边（契约格同基线，底部锚定不变） */
  justify-content: center;
  touch-action: none; /* pointer 事件自己接管，禁浏览器手势 */
  cursor: pointer;
}

.frozen {
  pointer-events: none; /* 启动序列/告别期间：点击、拖拽全部忽略 */
}

/* 告别淡出（工单 17）：桌宠窗口直接淡出后关闭全部窗口，无告别仪式 */
.pet-shell.goodbye {
  opacity: 0;
  transition: opacity 280ms ease-out;
}
</style>
