/**
 * 桌宠引擎确定性回归（Node 直跑，rAF 手动推进——工单 08 建立的验证方式，16 起随
 * codex 契约词汇表迁移）：
 *
 *   node_modules/.bin/esbuild scripts/pet-regression.ts --bundle --format=esm \
 *     --platform=node --outfile=node_modules/.tmp/pet-regression.mjs \
 *   && node node_modules/.tmp/pet-regression.mjs
 *
 * 覆盖：dragBounds 四约束（任务栏禁入/物理边界/跨屏选屏/边缘吸附）、引擎契约步进
 * （逐帧时长查表、循环取模、repeats 遍数）、freeze/unfreeze（帧冻结、时长不累积、
 * 在飞位移重锚定）、"return" 回程位移、rAF 叠加回归（08 遗留 bug 修复）、锁矩阵 +
 * flick 衔接（含 flickOnSwap 模式硬切兜底）、动作编排（常驻裁决、随机 1:1 抽取、
 * home/away 自主移动规划的钳制与降级）。
 * 每条注释写明：测试的是什么情况，出现什么情况才算正确。
 */
import { clampDragPosition, snapToEdges, type MonitorArea } from "../src/lib/pet/dragBounds";
import { PetEngine, type PetMover } from "../src/lib/pet/engine";
import { ANIMATIONS } from "../src/lib/pet/animations";
import {
  chassisAnim,
  jumpSteps,
  pickRandomKind,
  planRoam,
  reviewSteps,
} from "../src/lib/pet/actions";

/* ---- rAF 桩：手动推进，全确定性 ---- */
let rafQ: Array<(t: number) => void> = [];
let clock = 0;
(globalThis as Record<string, unknown>).requestAnimationFrame = (cb: (t: number) => void) => {
  rafQ.push(cb);
  return rafQ.length;
};
(globalThis as Record<string, unknown>).cancelAnimationFrame = () => {};
/** 推进 n 帧、每帧 dt 毫秒（回调内重排的下一帧在后续迭代执行） */
function flush(n: number, dt = 1000 / 60) {
  for (let i = 0; i < n; i++) {
    clock += dt;
    const q = rafQ;
    rafQ = [];
    for (const cb of q) cb(clock);
  }
}

let failed = 0;
function check(name: string, cond: boolean, detail?: string) {
  if (cond) console.log(`  ok  ${name}`);
  else {
    failed++;
    console.error(`FAIL  ${name}${detail ? " — " + detail : ""}`);
  }
}

/* ---- 模拟 mover：640×300 假想屏 + 96×104 契约窗口（/dev/anim 同规格） ---- */
function simMover(startX = 100) {
  let pos = { x: startX, y: 100 };
  const mover: PetMover = {
    position: () => ({ ...pos }),
    size: () => ({ width: 96, height: 104 }),
    workArea: () => ({ x: 0, y: 0, width: 640, height: 300 }),
    scaleFactor: () => 1,
    moveTo(x: number, y: number) {
      pos = { x, y };
    },
  };
  return { mover, x: () => pos.x, setX: (v: number) => (pos.x = v) };
}

/* ================= 契约数据：逐帧时长与网格 ================= */
console.log("\n[contract]");
// 情况：契约时长表（animation-rows.md）逐动作核对。正确：帧数与时长逐项一致、
// 行号 0–8 连续、前缀和末项 = 行总时长。
check(
  "idle = 6 帧 [280,110,110,140,140,320]",
  ANIMATIONS.idle.durations.join(",") === "280,110,110,140,140,320" && ANIMATIONS.idle.row === 0,
);
check(
  "running-right/left = 8 帧 120×7+220 且循环",
  ANIMATIONS["running-right"].durations[7] === 220 &&
    ANIMATIONS["running-left"].durations.slice(0, 7).every((d) => d === 120) &&
    ANIMATIONS["running-right"].loop &&
    ANIMATIONS["running-left"].loop,
);
check(
  "waving/jumping/failed 一次性且时长和 ≈ 0.7/0.84/1.22s",
  !ANIMATIONS.waving.loop &&
    ANIMATIONS.waving.cum[4] === 700 &&
    ANIMATIONS.jumping.cum[5] === 840 &&
    ANIMATIONS.failed.cum[8] === 1220,
);
check("waiting/running/review 循环", ANIMATIONS.waiting.loop && ANIMATIONS.running.loop && ANIMATIONS.review.loop);
check(
  "前缀和末项 = 总时长",
  Object.values(ANIMATIONS).every((d) => d.cum[d.cols] === d.durations.reduce((a, b) => a + b, 0)),
);

/* ================= dragBounds：PetDragBounds 四约束 ================= */
console.log("\n[dragBounds]");
// 情况：单屏 1920×1080、底部 40px 任务栏（工作区 1040 高）、96×104 窗口。
// 正确：落点永远在工作区内——任务栏区（y > 936）拖不进去，物理边界外也拖不出去。
const MONO: MonitorArea[] = [
  { x: 0, y: 0, width: 1920, height: 1080, workX: 0, workY: 0, workWidth: 1920, workHeight: 1040 },
];
check("任务栏禁入：y 落在工作区底内", clampDragPosition(100, 1010, 96, 104, MONO).y === 936);
check("物理边界：左侧不出屏", clampDragPosition(-50, 0, 96, 104, MONO).x === 0);
check("物理边界：右侧不出屏", clampDragPosition(1900, 0, 96, 104, MONO).x === 1824);

// 情况：双屏（左屏物理 x∈[-1920,0)）。正确：窗口矩形与哪块屏重叠面积大就钳进哪块的
// 工作区（拖过半自动切屏），跨屏负坐标可用。
const DUAL: MonitorArea[] = [
  { x: -1920, y: 0, width: 1920, height: 1080, workX: -1920, workY: 0, workWidth: 1920, workHeight: 1040 },
  ...MONO,
];
check("跨屏：大半在左屏 → 钳进左屏工作区（全可见）", clampDragPosition(-60, 0, 96, 104, DUAL).x === -96);
check("跨屏：选中的是左屏", clampDragPosition(-60, 0, 96, 104, DUAL).monitor === DUAL[0]);
check("跨屏：大半在右屏 → 钳右屏工作区", clampDragPosition(1900, 0, 96, 104, DUAL).x === 1824);

// 情况：拖动停止时贴近工作区边（< 20px）。正确：贴齐最近边；距离达标不吸附。
const m = MONO[0];
check("左吸附：8px < 20px → 贴左", snapToEdges(8, 500, 96, 104, m, 20).x === 0);
check("右吸附：12px < 20px → 贴右", snapToEdges(1812, 500, 96, 104, m, 20).x === 1824);
check("下吸附：贴工作区底（任务栏上沿）", snapToEdges(500, 921, 96, 104, m, 20).y === 936);
check("不吸附：30px 距离保持原位", snapToEdges(30, 500, 96, 104, m, 20).x === 30);
check("最近边优先：距上 5 < 距左 10 → 吸上", snapToEdges(10, 5, 96, 104, m, 20).y === 0);

/* ================= engine：契约逐帧时长步进 ================= */
console.log("\n[engine step]");
{
  // 情况：idle 循环（时长 280,110,110,140,140,320，一圈 1100ms）以 100ms 步进。
  // 正确：帧边界落在契约前缀和上（280/390/500/640/780），过圈回 0。
  const eng = new PetEngine(simMover().mover);
  eng.request({ lock: false, steps: [{ anim: "idle" }] });
  let total = 0;
  const at = (ms: number) => {
    flush(Math.round((ms - total) / 100), 100); // 补齐到绝对时刻（flush 是累积推进）
    total = ms;
    return eng.state().frame;
  };
  // 注：引擎首 tick 是基线（dt=0，防动作发起瞬间的调度延迟跳变），elapsed 恒滞后
  // 一个步进——探针时刻按此校准。
  check(
    "idle 帧边界按契约时长（0|1|2|3|4|5）",
    at(100) === 0 && at(400) === 1 && at(500) === 2 && at(600) === 3 && at(800) === 4 && at(900) === 5,
  );
  check("idle 过圈回 0（1100ms）", at(1200) === 0 && at(1500) === 1);
}
{
  // 情况：waving 一次性（700ms）播完。正确：动作结束（busy=false）、onSettle 恰好一次、
  // 停在末帧。
  const eng = new PetEngine(simMover().mover);
  let settles = 0;
  eng.request({ lock: true, steps: [{ anim: "waving" }], onSettle: () => settles++ });
  flush(6, 100); // 600ms < 700：仍在播
  check("waving 600ms 仍在播且锁住", eng.state().busy && eng.state().locked);
  flush(2, 100); // 800ms ≥ 700：播完
  check("waving 700ms 播完进稳态", settles === 1 && !eng.state().busy && !eng.state().locked);
}
{
  // 情况：review 业务演出 = 契约圈（1030ms）× 2 遍 ≈ 2.06s。正确：第二圈从头播帧、
  // 2060ms 播完结束，不会提前也不会多播。
  const eng = new PetEngine(simMover().mover);
  let settles = 0;
  eng.request({ lock: false, steps: reviewSteps(), onSettle: () => settles++ });
  flush(10, 100); // 1000ms：第一圈未完
  check("review 1000ms 仍在第一圈", eng.state().busy && eng.state().frame === 5);
  flush(2, 100); // 1200ms：第二圈已回帧 0
  check("review 第二圈从头播", eng.state().frame === 0);
  flush(10, 100); // 2200ms ≥ 2060：播完
  check("review 两圈播完结束", settles === 1 && !eng.state().busy);
}

/* ================= engine：freeze / unfreeze ================= */
console.log("\n[engine freeze]");
{
  // 情况：idle 循环播放中冻结。正确：冻结期间不再产生新帧（emit 停止），解冻后继续。
  const sim = simMover();
  const eng = new PetEngine(sim.mover);
  let emits = 0;
  eng.subscribe(() => emits++);
  eng.request({ lock: false, steps: [{ anim: "idle" }] });
  flush(20, 100);
  const beforeFreeze = emits;
  check("freeze 前帧在推进（有 emit）", beforeFreeze > 0);
  eng.freeze();
  flush(40, 100);
  check("freeze 期间帧不动（无 emit）", emits === beforeFreeze, `emits=${emits}`);
  eng.unfreeze();
  flush(40, 100);
  check("unfreeze 后继续播放（恢复 emit）", emits > beforeFreeze);
}
{
  // 情况：多步序列（waving→idle）在 waving 播到一半时冻结很久再解冻。正确：冻结时长
  // 不累积（不会瞬移到末步），解冻后按剩余时长播完 waving 再进 idle；锁随稳态释放。
  const sim = simMover();
  const eng = new PetEngine(sim.mover);
  let settles = 0;
  eng.request({ lock: true, steps: [{ anim: "waving" }, { anim: "idle" }], onSettle: () => settles++ });
  flush(2, 100); // 200ms → waving 第 1 帧
  eng.freeze();
  flush(100, 100); // 冻结 10 秒
  eng.unfreeze();
  flush(1, 100);
  check("冻结时长不快进（仍在 waving 中途）", eng.state().anim === "waving", `anim=${eng.state().anim}`);
  flush(30, 100);
  check("解冻后播完进末步 idle", eng.state().anim === "idle");
  check("稳态锁释放", eng.state().locked === false);
  check("onSettle 恰好一次", settles === 1);
}
{
  // 情况：在飞位移（running-right 单程 150px，行时长 1060ms）途中冻结，拖拽把窗口
  // 挪到别处再解冻。正确：以当前位置重锚定、仍落原目标（窗口不回跳、不超目标）。
  const sim = simMover(100);
  const eng = new PetEngine(sim.mover);
  eng.request({
    lock: false,
    steps: [{ anim: "running-right", loop: false, movement: { direction: "right", distance: 150 } }, { anim: "idle" }],
  });
  flush(2, 100); // 行程前 200ms
  eng.freeze();
  sim.setX(60); // 用户拖拽期间窗口被挪到 60
  eng.unfreeze();
  flush(30, 100);
  check("解冻重锚定：仍落原目标 250", Math.round(sim.x()) === 250, `x=${sim.x()}`);
}

/* ================= engine："return" 回程位移 ================= */
console.log("\n[engine return]");
{
  // 情况：同一动作内"跑出去 → return 走回"（Oreo 吃饭走回的原型，codex 自主移动的
  // 引擎机制）。正确：return 步结束回到本动作开始时的 x=100。
  const sim = simMover(100);
  const eng = new PetEngine(sim.mover);
  eng.request({
    lock: false,
    steps: [
      { anim: "running-right", loop: false, movement: { direction: "right", distance: 150 } },
      { anim: "running-left", loop: false, movement: { direction: "return" } },
      { anim: "idle" },
    ],
  });
  flush(40, 100);
  check("return 回程落回动作起点", Math.round(sim.x()) === 100, `x=${sim.x()}`);
}

/* ================= engine：rAF 叠加回归（08 遗留 bug） ================= */
console.log("\n[engine rAF]");
{
  // 情况：反复替换运行动作（稳态轮换/抢占都走 begin）。修复前每替换一次叠一个 tick
  // 循环、动画倍速；修复后 tick 单一。正确：替换 3 次后固定窗口内的帧变化次数正常。
  const sim = simMover();
  const eng = new PetEngine(sim.mover);
  eng.request({ lock: false, steps: [{ anim: "idle" }] });
  flush(10, 16);
  eng.request({ lock: false, steps: [{ anim: "waiting" }] });
  flush(10, 16);
  eng.request({ lock: false, steps: [{ anim: "idle" }] });
  flush(10, 16);
  eng.request({ lock: false, steps: [{ anim: "running" }] });
  flush(10, 16);
  let emits = 0;
  eng.subscribe(() => emits++);
  flush(120, 16); // 1920ms，running 圈 820ms ≈ 2.3 圈 → 帧变化约 13 次；双倍速 ≥ 26 次
  check("多次替换后动画不倍速", emits < 20, `emits=${emits}`);
}

/* ================= engine：锁矩阵 + flick 衔接 ================= */
console.log("\n[engine lock/flick]");
{
  // 情况：用户过渡（waving）播放中。正确：用户/系统新动作都被拒（当前动作锁）；
  // 稳态后系统常驻可替换。
  const sim = simMover();
  const eng = new PetEngine(sim.mover);
  eng.request({ lock: true, steps: [{ anim: "waving" }, { anim: "idle" }] });
  check("用户过渡中：拒新用户动作", eng.request({ lock: true, steps: jumpSteps() }) === false);
  check("用户过渡中：拒系统动作", eng.request({ lock: false, steps: reviewSteps() }) === false);
  flush(100, 100);
  check("稳态后：系统常驻可替换", eng.request({ lock: false, steps: [{ anim: "idle" }] }) === true);
  flush(100, 100);
}
{
  // 情况：系统一次性动作（review 两圈）未播完被用户动作抢占（帧硬切）；稳态循环被
  // 系统动作替换。正确：前者 flick +1（渲染层淡出淡入），后者不 flick。
  const sim = simMover();
  const eng = new PetEngine(sim.mover);
  eng.request({ lock: false, steps: reviewSteps() });
  flush(1, 16); // 未稳态
  const before = eng.state().flick;
  eng.request({ lock: true, steps: jumpSteps() });
  check("抢占硬切：flick +1", eng.state().flick === before + 1);
  flush(60, 100); // jumping 播完（无收尾步 → 动作结束）
  const mid = eng.state().flick;
  eng.request({ lock: false, steps: [{ anim: "idle" }] });
  check("空闲后起常驻：不 flick", eng.state().flick === mid);
}
{
  // 情况：模式硬切（常驻 running → idle，无过渡帧可衔接）。正确：flickOnSwap 让稳态
  // 替换也 +1（渲染层淡出淡入兜底，工单 17）。
  const sim = simMover();
  const eng = new PetEngine(sim.mover);
  eng.request({ lock: false, steps: [{ anim: "running" }] });
  flush(10, 16); // 进入稳态循环
  const before = eng.state().flick;
  eng.request({ lock: false, flickOnSwap: true, steps: [{ anim: "idle" }] });
  check("模式硬切：flickOnSwap 稳态替换也 +1", eng.state().flick === before + 1);
}

/* ================= actions：常驻裁决 / 随机抽取 / 自主移动规划 ================= */
console.log("\n[actions]");
// 情况：常驻裁决（chassisAnim）。正确：面板开 = waiting 优先；否则工作 running / 休息 idle。
check(
  "常驻裁决：waiting > 模式",
  chassisAnim("work", true) === "waiting" && chassisAnim("rest", true) === "waiting" &&
    chassisAnim("work", false) === "running" && chassisAnim("rest", false) === "idle",
);

// 情况：随机池 1:1（边界 0.5）。正确：边界两侧各归其类。
check(
  "随机 1:1 边界",
  pickRandomKind(0) === "jump" && pickRandomKind(0.499) === "jump" && pickRandomKind(0.5) === "roam" && pickRandomKind(0.999) === "roam",
);

// 情况：自主移动规划（factor=1、屏 0–644 可行域、窗 96）。正确：目标钳进工作区且
// 距边 ≥20px；单程距离 64~128；方向取余量大侧；跑动步用对应专行并携带位移。
const area = { x: 0, width: 640 };
const plan = planRoam(100, 96, area, 1, 0.5);
check("规划存在且落在边距内", plan !== null && plan.targetX >= 20 && plan.targetX <= 640 - 96 - 20);
check("单程 64~128（r=0.5 → 96）", plan !== null && Math.abs(plan.targetX - 196) < 1e-9, `target=${plan?.targetX}`);
check(
  "朝右用 running-right 专行",
  plan !== null && plan.step.anim === "running-right" && plan.step.loop === false && plan.step.movement?.direction === "right",
);
// 情况：起点贴右缘（x=500，右界 524）。正确：右侧无空间 → 目标被钳到 ≤504 或规划降级，
// 绝不越界。
const edge = planRoam(500, 96, area, 1, 0.99);
check(
  "贴右缘：钳制或降级，不越界",
  edge === null || (edge.targetX <= 524 && Math.abs(edge.targetX - 500) >= 48),
  `target=${edge?.targetX}`,
);
// 情况：完全无空间（工作区仅比窗宽 4px，已在极值位）。正确：降级为 null（调用方改跳跃）。
check("无空间：降级 null", planRoam(0, 96, { x: 0, width: 100 }, 1, 0.5) === null);

/* ================= 汇总 ================= */
console.log(failed === 0 ? "\n全部通过" : `\n${failed} 项失败`);
process.exit(failed === 0 ? 0 : 1);
