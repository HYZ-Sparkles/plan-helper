/**
 * 桌宠动作编排（工单 09，CONTEXT PetRandomAction）：休息模式随机动作（吃/跳/闲坐）
 * 与拖拽后用户动作的步骤构造。纯数据构建，无定时器/引擎副作用——调度（300s 间隔、
 * pose 追踪、onSettle 结算）在 PetWindow，动画编号语义见 animations.ts 与 README。
 */
import type { StepSpec } from "./engine";

/** 休息模式常驻姿态（决定闲坐切换方向与拖后是否先起身） */
export type Pose = "stand" | "sit";

/** 随机动作三选一（吃 : 跳 : 闲坐 = 4 : 3 : 3） */
export type RandomKind = "eat" | "jump" | "sit-toggle";

/** 随机动作触发间隔：距上一次（用户或随机）动作结束固定 300 秒 */
export const RANDOM_INTERVAL_MS = 300_000;

/** 吃：起跑距离（显示逻辑 px，超出工作区由引擎钳制截短） */
const EAT_RUN_PX = 150;
/** 跳：单程跳距（显示逻辑 px） */
const JUMP_PX = 48;

/** 权重抽取：r ∈ [0,1) → [0,0.4) 吃 / [0.4,0.7) 跳 / [0.7,1) 闲坐 */
export function pickRandomKind(r: number): RandomKind {
  return r < 0.4 ? "eat" : r < 0.7 ? "jump" : "sit-toggle";
}

/** 随机动作步骤。side = 起跑/起跳方向（调用方用 autoDirection 选屏幕余量大侧）；
 *  坐姿起跑先 3 Sit to Stand（同作者帧自然过渡，与拖后动作同例）；
 *  返回 nextPose 供 onSettle 更新常驻姿态（吃/跳收尾回站立，闲坐站↔坐互换）。 */
export function randomSteps(
  kind: RandomKind,
  pose: Pose,
  side: "left" | "right",
): { steps: StepSpec[]; nextPose: Pose } {
  const flip = side === "left"; // 素材朝右，朝左移动时镜像
  const standUp: StepSpec[] = pose === "sit" ? [{ anim: 3 }] : [];
  switch (kind) {
    case "eat":
      // 朝 side 跑一段 → 原地吃 → 走回动作起点 → 收回站立常驻
      return {
        steps: [
          ...standUp,
          { anim: 10, flip, movement: { direction: side, distance: EAT_RUN_PX } },
          { anim: 8, flip },
          { anim: 9, flip: !flip, movement: { direction: "return" } },
          { anim: 7, loop: true },
        ],
        nextPose: "stand",
      };
    case "jump":
      // 跳过去（14 Jump）再跳回来（14 Jump）→ 收回站立常驻
      return {
        steps: [
          ...standUp,
          { anim: 14, flip, movement: { direction: side, distance: JUMP_PX } },
          { anim: 14, flip: !flip, movement: { direction: "return" } },
          { anim: 7, loop: true },
        ],
        nextPose: "stand",
      };
    case "sit-toggle":
      // 闲坐 = 站↔坐轮换：站立→坐下歇着；已坐→起身站会儿（README 常驻一段时间后的转换）
      return pose === "stand"
        ? { steps: [{ anim: 1 }, { anim: 2, loop: true }], nextPose: "sit" }
        : { steps: [{ anim: 3 }, { anim: 7, loop: true }], nextPose: "stand" };
  }
}

/** 拖拽后（休息模式）：非站立先 3 Sit to Stand，再 15 Attack，收回站立常驻（用户动作占锁） */
export function afterDragSteps(pose: Pose): StepSpec[] {
  const steps: StepSpec[] = [];
  if (pose === "sit") steps.push({ anim: 3 });
  steps.push({ anim: 15 }, { anim: 7, loop: true });
  return steps;
}
