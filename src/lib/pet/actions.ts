/**
 * 桌宠动作编排（工单 09 建立、16 起改 codex 词汇表）：常驻/随机/里程碑动作的步骤
 * 构造与纯函数规划。纯数据构建，无定时器/引擎副作用——调度（间隔、概率、home/away
 * 记账、onSettle 结算）在 PetWindow；动作语义见 animations.ts 与 ADR-0010。
 * 一次性动作不自带常驻收尾步：settle 后由 PetWindow 的 applyChassis 按当下状态
 * （模式/大面板开关）动态落常驻，避免动作播放期间条件变化导致的过期收尾。
 */
import { autoDirection, type ActionSpec, type StepSpec } from "./engine";
import type { PetAnim } from "./animations";

/** 模式（CONTEXT WorkMode / RestMode；menu.ts 的 PetMode 同源） */
export type Mode = "work" | "rest";

/* ---- 常驻（chassis）：PetActionPolicy 白名单外的默认态 ---- */

/** 当前应有的常驻动作（工单 17/20）：大面板开着 = waiting（不区分模式与是否已确认），
 *  否则工作 running / 休息 idle。flick = 模式硬切的淡出淡入兜底（换常驻来自不同
 *  动作族、无过渡帧可衔接，ADR-0010：codex 无入睡/醒来动画）。 */
export function chassisAction(anim: PetAnim, flick = false): ActionSpec {
  return { lock: false, flickOnSwap: flick, steps: [{ anim, loop: true }] };
}

/** 常驻动作裁决（纯函数，回归覆盖）：boardOpen 优先于模式 */
export function chassisAnim(mode: Mode, boardOpen: boolean): PetAnim {
  return boardOpen ? "waiting" : mode === "work" ? "running" : "idle";
}

/* ---- 休息随机（工单 19，PetRandomAction）：跳跃 : 自主移动 = 1:1 ---- */

/** 随机动作触发间隔：距上一次（用户或随机）动作结束固定 300 秒 */
export const RANDOM_INTERVAL_MS = 300_000;
/** 触发概率：50%（另一半保持 idle，下个间隔再抽） */
export const RANDOM_PROBABILITY = 0.5;

/** 随机动作二选一（1:1）：r ∈ [0,1) → [0,0.5) 跳跃 / [0.5,1) 自主移动 */
export type RandomKind = "jump" | "roam";
export function pickRandomKind(r: number): RandomKind {
  return r < 0.5 ? "jump" : "roam";
}

/** 随机跳跃（也承载业务"今日任务全部完成"庆祝，工单 20——共用动作、触发语义不同） */
export function jumpSteps(): StepSpec[] {
  return [{ anim: "jumping" }];
}

/** 自主移动单程距离（显示逻辑像素，随机区间）与边界边距（PetRandomAction） */
export const ROAM_MIN_PX = 64;
export const ROAM_MAX_PX = 128;
export const ROAM_MARGIN_PX = 20;
/** 钳制后行程低于此值 = 无足够空间，本次降级为跳跃（取最小单程的四分之三） */
export const ROAM_MIN_ACTUAL_PX = 48;

/** 自主移动规划结果：跑动步 + 落点（物理像素，供 home/away 记账） */
export interface RoamPlan {
  step: StepSpec;
  targetX: number;
}

/** 规划一次自主移动单程（纯函数，回归覆盖）：从 fromX 出发、方向取屏幕余量大侧、
 *  单程 64~128 逻辑像素随机（r）、目标钳进工作区且距边 ≥20px；无足够空间返回 null
 *  （调用方降级为跳跃）。跑动用契约 running-right/left 单遍承载（左右各专行、无需镜像）。 */
export function planRoam(
  fromX: number,
  winW: number,
  area: { x: number; width: number },
  factor: number,
  r: number,
): RoamPlan | null {
  const minX = area.x;
  const maxX = area.x + area.width - winW;
  const side = autoDirection(fromX, minX, maxX);
  const dist = (ROAM_MIN_PX + r * (ROAM_MAX_PX - ROAM_MIN_PX)) * factor;
  const margin = ROAM_MARGIN_PX * factor;
  // 边距区间可能为空（工作区比窗口宽不足 2×边距）：先保证不出工作区，边距尽力满足
  const lo = Math.max(minX, Math.min(minX + margin, maxX));
  const hi = Math.max(lo, Math.min(maxX - margin, maxX));
  const target = Math.min(Math.max(fromX + (side === "right" ? dist : -dist), lo), hi);
  const dx = target - fromX;
  if (Math.abs(dx) < ROAM_MIN_ACTUAL_PX * factor) return null;
  const direction = dx > 0 ? "right" : "left";
  return {
    step: {
      anim: direction === "right" ? "running-right" : "running-left",
      loop: false,
      movement: { direction, distance: Math.abs(dx) / factor },
    },
    targetX: target,
  };
}

/** 规划跑向指定点（纯函数，回归覆盖）：away → home 的回程用——目标钳进当前工作区
 *  （显示器拓扑可能已变），位移方向 = 落点方向；已在目标点返回 null。 */
export function planReturn(
  fromX: number,
  targetX: number,
  area: { x: number; width: number },
  winW: number,
  factor: number,
): RoamPlan | null {
  const target = Math.min(Math.max(targetX, area.x), area.x + area.width - winW);
  const dx = target - fromX;
  if (Math.abs(dx) < 1) return null;
  const direction = dx > 0 ? "right" : "left";
  return {
    step: {
      anim: direction === "right" ? "running-right" : "running-left",
      loop: false,
      movement: { direction, distance: Math.abs(dx) / factor },
    },
    targetX: target,
  };
}

/* ---- 业务里程碑（工单 20，PetActionPolicy 第 4 来源；系统动作不占锁） ---- */

/** review 演出遍数：契约一圈 1030ms × 2 ≈ spec"约 2.4s"（整圈数取最接近档） */
export const REVIEW_REPEATS = 2;

/** review：小看板推进型汇报的即时反馈（修正/撤销等"往回改"不触发） */
export function reviewSteps(): StepSpec[] {
  return [{ anim: "review", loop: false, repeats: REVIEW_REPEATS }];
}

/** failed：今日总结弹出且当日未达标时演一次（契约一圈 1240ms ≈ "约 1.2s"） */
export function failedSteps(): StepSpec[] {
  return [{ anim: "failed" }];
}

/** 庆祝：今日任务全部完成时演一次（与休息随机跳跃共用 jumping） */
export function celebrateSteps(): StepSpec[] {
  return [{ anim: "jumping" }];
}

/* ---- 拖拽反馈（工单 18，PetDragGesture）：拖起/竖直 = jumping、水平 = 专行跑 ---- */

/** 拖拽反馈循环（用户动作占锁；单循环步即刻稳态 → 方向反转可实时跟切） */
export function dragLoopAction(anim: PetAnim): ActionSpec {
  return { lock: true, steps: [{ anim, loop: true }] };
}
