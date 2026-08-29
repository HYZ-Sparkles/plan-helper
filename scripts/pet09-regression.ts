/**
 * 工单 09 确定性回归（Node 直跑，rAF 手动推进——同工单 08 的验证方式）：
 *
 *   node_modules/.bin/esbuild scripts/pet09-regression.ts --bundle --format=esm \
 *     --platform=node --outfile=node_modules/.tmp/pet09-regression.mjs \
 *   && node node_modules/.tmp/pet09-regression.mjs
 *
 * 覆盖：dragBounds 四约束（任务栏禁入/物理边界/跨屏选屏/边缘吸附）、引擎 freeze/
 * unfreeze（帧冻结、时长不累积、在飞位移重锚定）、"return" 回程位移、rAF 叠加回归
 * （08 遗留 bug 修复）、抢占 flick 计数、随机动作 4:3:3 抽取与吃/跳/闲坐编排、拖后
 * Attack 步骤、autoDirection 定向。
 * 每条注释写明：测试的是什么情况，出现什么情况才算正确。
 */
import { clampDragPosition, snapToEdges, type MonitorArea } from "../src/lib/pet/dragBounds";
import { PetEngine, autoDirection, type EngineState, type PetMover } from "../src/lib/pet/engine";
import { afterDragSteps, pickRandomKind, randomSteps } from "../src/lib/pet/actions";

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

/* ---- 模拟 mover：480×220 假想屏 + 64×64 窗口（/dev/anim 同规格） ---- */
function simMover(startX = 100) {
  let pos = { x: startX, y: 100 };
  const mover: PetMover = {
    position: () => ({ ...pos }),
    size: () => ({ width: 64, height: 64 }),
    workArea: () => ({ x: 0, y: 0, width: 480, height: 220 }),
    scaleFactor: () => 1,
    moveTo(x: number, y: number) {
      pos = { x, y };
    },
  };
  return { mover, x: () => pos.x, setX: (v: number) => (pos.x = v) };
}

/* ================= dragBounds：PetDragBounds 四约束 ================= */
console.log("\n[dragBounds]");
// 情况：单屏 1920×1080、底部 40px 任务栏（工作区 1040 高）、64×64 窗口。
// 正确：落点永远在工作区内——任务栏区（y > 976）拖不进去，物理边界外也拖不出去。
const MONO: MonitorArea[] = [
  { x: 0, y: 0, width: 1920, height: 1080, workX: 0, workY: 0, workWidth: 1920, workHeight: 1040 },
];
check("任务栏禁入：y 落在工作区底内", clampDragPosition(100, 1010, 64, 64, MONO).y === 976);
check("物理边界：左侧不出屏", clampDragPosition(-50, 0, 64, 64, MONO).x === 0);
check("物理边界：右侧不出屏", clampDragPosition(1900, 0, 64, 64, MONO).x === 1856);

// 情况：双屏（左屏物理 x∈[-1920,0)）。正确：窗口矩形与哪块屏重叠面积大就钳进哪块的
// 工作区（拖过半自动切屏），跨屏负坐标可用。
const DUAL: MonitorArea[] = [
  { x: -1920, y: 0, width: 1920, height: 1080, workX: -1920, workY: 0, workWidth: 1920, workHeight: 1040 },
  ...MONO,
];
check("跨屏：大半在左屏 → 钳进左屏工作区（全可见）", clampDragPosition(-60, 0, 64, 64, DUAL).x === -64);
check("跨屏：选中的是左屏", clampDragPosition(-60, 0, 64, 64, DUAL).monitor === DUAL[0]);
check("跨屏：大半在右屏 → 钳右屏工作区", clampDragPosition(1900, 0, 64, 64, DUAL).x === 1856);

// 情况：拖动停止时贴近工作区边（< 20px）。正确：贴齐最近边；距离达标不吸附。
const m = MONO[0];
check("左吸附：8px < 20px → 贴左", snapToEdges(8, 500, 64, 64, m, 20).x === 0);
check("右吸附：12px < 20px → 贴右", snapToEdges(1844, 500, 64, 64, m, 20).x === 1856);
check("下吸附：贴工作区底（任务栏上沿）", snapToEdges(500, 961, 64, 64, m, 20).y === 976);
check("不吸附：30px 距离保持原位", snapToEdges(30, 500, 64, 64, m, 20).x === 30);
check("最近边优先：距上 5 < 距左 10 → 吸上", snapToEdges(10, 5, 64, 64, m, 20).y === 0);

/* ================= engine：freeze / unfreeze ================= */
console.log("\n[engine freeze]");
{
  // 情况：7 Stand Idle 循环播放中冻结（拖拽）。正确：冻结期间不再产生新帧（emit 停止），
  // 解冻后继续出帧——"保持当前帧不动，移动完了才继续"。
  const sim = simMover();
  const eng = new PetEngine(sim.mover);
  let emits = 0;
  eng.subscribe(() => emits++);
  eng.request({ lock: false, steps: [{ anim: 7, loop: true }] }); // 5 帧 @4fps
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
  // 情况：多步序列（21→7）在 21 播到一半时冻结很久再解冻。正确：冻结时长不累积
  // （不会瞬移到末步），解冻后按剩余时长播完 21 再进 7；锁随稳态释放。
  const sim = simMover();
  const eng = new PetEngine(sim.mover);
  let settles = 0;
  eng.request({ lock: true, steps: [{ anim: 21 }, { anim: 7, loop: true }], onSettle: () => settles++ });
  flush(2, 100); // 200ms → 21 的第 1 帧（共 2 帧 @6fps≈167ms/帧）
  eng.freeze();
  flush(100, 100); // 冻结 10 秒
  eng.unfreeze();
  flush(1, 100);
  check("冻结时长不快进（仍在 21 中途）", eng.state().anim === 21, `anim=${eng.state().anim}`);
  flush(30, 100);
  check("解冻后播完进末步 7", eng.state().anim === 7);
  check("稳态锁释放", eng.state().locked === false);
  check("onSettle 恰好一次", settles === 1);
}
{
  // 情况：在飞位移（Run 向右 150px）途中冻结，拖拽把窗口挪到别处再解冻。
  // 正确：以当前位置重锚定、仍落原目标（窗口不回跳、不超目标）。
  const sim = simMover(100);
  const eng = new PetEngine(sim.mover);
  eng.request({
    lock: false,
    steps: [{ anim: 10, movement: { direction: "right", distance: 150 } }, { anim: 7, loop: true }],
  });
  flush(2, 100); // Run 571ms 的前 200ms
  eng.freeze();
  sim.setX(60); // 用户拖拽期间窗口被挪到 60
  eng.unfreeze();
  flush(30, 100);
  check("解冻重锚定：仍落原目标 250", Math.round(sim.x()) === 250, `x=${sim.x()}`);
}

/* ================= engine："return" 回程位移 ================= */
console.log("\n[engine return]");
{
  // 情况：吃动作（跑 150 → 吃 → 走回）。正确：结束后回到动作起点 x=100。
  const sim = simMover(100);
  const eng = new PetEngine(sim.mover);
  eng.request({
    lock: false,
    steps: [
      { anim: 10, movement: { direction: "right", distance: 150 } },
      { anim: 9, movement: { direction: "return" } },
      { anim: 7, loop: true },
    ],
  });
  flush(600, 100);
  check("吃：跑出去又走回原位", Math.round(sim.x()) === 100, `x=${sim.x()}`);
}
{
  // 情况：起点贴近右缘（x=400，右界 416）：去程被工作区钳短。
  // 正确：回程目标仍是动作起点 400（钳短不影响回程落点）。
  const sim = simMover(400);
  const eng = new PetEngine(sim.mover);
  eng.request({
    lock: false,
    steps: [
      { anim: 10, movement: { direction: "right", distance: 150 } },
      { anim: 9, movement: { direction: "return" } },
      { anim: 7, loop: true },
    ],
  });
  flush(600, 100);
  check("吃：去程钳短后仍回原位", Math.round(sim.x()) === 400, `x=${sim.x()}`);
}

/* ================= engine：rAF 叠加回归（08 遗留 bug） ================= */
console.log("\n[engine rAF]");
{
  // 情况：反复替换运行动作（稳态轮换/抢占都走 begin）。修复前每替换一次叠一个 tick
  // 循环、动画倍速；修复后 tick 单一。正确：替换 3 次后固定窗口内的帧变化次数正常。
  const sim = simMover();
  const eng = new PetEngine(sim.mover);
  eng.request({ lock: false, steps: [{ anim: 7, loop: true }] });
  flush(10, 16);
  eng.request({ lock: false, steps: [{ anim: 2, loop: true }] });
  flush(10, 16);
  eng.request({ lock: false, steps: [{ anim: 7, loop: true }] });
  flush(10, 16);
  eng.request({ lock: false, steps: [{ anim: 2, loop: true }] });
  flush(10, 16);
  let emits = 0;
  eng.subscribe(() => emits++);
  flush(120, 16); // 1920ms，4fps ≈ 每 250ms 一帧 → 正确约 7 次；双倍速 ≥ 14 次
  check("多次替换后动画不倍速", emits < 10, `emits=${emits}`);
}

/* ================= engine：锁矩阵 + flick 衔接 ================= */
console.log("\n[engine lock/flick]");
{
  // 情况：用户过渡（4→5）播放中。正确：用户/系统新动作都被拒（当前动作锁）；
  // 稳态后用户动作可替换。
  const sim = simMover();
  const eng = new PetEngine(sim.mover);
  eng.request({ lock: true, steps: [{ anim: 4 }, { anim: 5, loop: true }] });
  check("用户过渡中：拒新用户动作", eng.request({ lock: true, steps: [{ anim: 15 }] }) === false);
  check("用户过渡中：拒系统随机动作", eng.request({ lock: false, steps: [{ anim: 8 }] }) === false);
  flush(100, 100);
  check("稳态后：用户动作可替换", eng.request({ lock: true, steps: [{ anim: 15 }, { anim: 7, loop: true }] }) === true);
  flush(100, 100);
}
{
  // 情况：系统动作未稳态时被用户动作抢占（帧硬切）；稳态循环被替换（同作者衔接链）。
  // 正确：前者 flick +1（渲染层淡出淡入），后者不 flick。
  const sim = simMover();
  const eng = new PetEngine(sim.mover);
  eng.request({ lock: false, steps: [{ anim: 8 }, { anim: 2, loop: true }] });
  flush(1, 16); // 未稳态
  const before = eng.state().flick;
  eng.request({ lock: true, steps: [{ anim: 15 }, { anim: 7, loop: true }] });
  check("抢占硬切：flick +1", eng.state().flick === before + 1);
  flush(60, 100); // Attack 播完进 7 循环（稳态）
  const mid = eng.state().flick;
  eng.request({ lock: false, steps: [{ anim: 1 }, { anim: 2, loop: true }] });
  check("稳态替换（衔接链）：不 flick", eng.state().flick === mid);
}

/* ================= actions：随机动作抽取与编排 ================= */
console.log("\n[actions]");
// 情况：权重 吃:跳:闲坐 = 4:3:3（累积边界 0.4 / 0.7）。正确：边界两侧各归其类。
check(
  "权重 4:3:3 边界",
  pickRandomKind(0) === "eat" &&
    pickRandomKind(0.399) === "eat" &&
    pickRandomKind(0.4) === "jump" &&
    pickRandomKind(0.699) === "jump" &&
    pickRandomKind(0.7) === "sit-toggle" &&
    pickRandomKind(0.999) === "sit-toggle",
);

// 情况：吃（朝左）。正确：10 Run→8 Eat→9 Walk→7 循环；朝左镜像、回程朝右 + return。
const eat = randomSteps("eat", "stand", "left");
check("吃 = 10→8→9→7", eat.steps.map((s) => s.anim).join(",") === "10,8,9,7");
check("吃回程 = return", eat.steps[2].movement?.direction === "return");
check("吃朝左镜像 / 回程反向", eat.steps[0].flip === true && eat.steps[2].flip === false);
check("吃收尾站立", eat.nextPose === "stand");
// 情况：吃（坐姿起跑）。正确：先 3 Sit to Stand 再跑（同作者帧自然过渡）。
check("吃（坐姿）先起身", randomSteps("eat", "sit", "left").steps.map((s) => s.anim).join(",") === "3,10,8,9,7");

// 情况：跳（朝右）。正确：14 Jump 两次（去 + 回）→7 循环；翻转相反、回程 return。
const jump = randomSteps("jump", "stand", "right");
check("跳 = 14→14→7", jump.steps.map((s) => s.anim).join(",") === "14,14,7");
check("跳往返翻转相反", jump.steps[0].flip === false && jump.steps[1].flip === true);
check("跳回程 = return", jump.steps[1].movement?.direction === "return");

// 情况：闲坐。正确：站立→1 Stand to Sit→2 循环（nextPose 坐）；已坐→3 Sit to Stand→7 循环。
const sitDown = randomSteps("sit-toggle", "stand", "left");
const standUp = randomSteps("sit-toggle", "sit", "left");
check("闲坐（站→坐）= 1→2", sitDown.steps.map((s) => s.anim).join(",") === "1,2" && sitDown.nextPose === "sit");
check("闲坐（坐→站）= 3→7", standUp.steps.map((s) => s.anim).join(",") === "3,7" && standUp.nextPose === "stand");

// 情况：拖拽后（休息模式）。正确：坐着先 3 Sit to Stand 再 15 Attack 再 7；站立直接 15。
check("拖后（坐）= 3→15→7", afterDragSteps("sit").map((s) => s.anim).join(",") === "3,15,7");
check("拖后（站）= 15→7", afterDragSteps("stand").map((s) => s.anim).join(",") === "15,7");

// 情况：autoDirection 定向。正确：右余量大 → right；左余量大 → left。
check("autoDirection 余量大侧", autoDirection(100, 0, 416) === "right" && autoDirection(350, 0, 416) === "left");

/* ================= 汇总 ================= */
console.log(failed === 0 ? "\n全部通过" : `\n${failed} 项失败`);
process.exit(failed === 0 ? 0 : 1);
