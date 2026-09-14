/**
 * 桌宠帧动画引擎（工单 08 建立、16 改 codex 契约步进）：CONTEXT PetActionExecution
 * 的"当前动作锁"模型。平台无关纯 TS——窗口位移经注入的 PetMover 抽象（PetWindow 注
 * Tauri 实现、/dev/anim 调试页注模拟实现），方向判断与屏幕边界钳制都在引擎内完成。
 *
 * 步进（16 重写）：帧下标由步内单调时长对契约逐帧时长表前缀和查表派生——循环步取模、
 * 一次性步按 repeats 遍数播完进下一步；不再有 fps / 位移权重（契约硬性规定时长，
 * Oreo 的试拍调参管线随其素材一起移除）。
 *
 * 锁规则：用户动作（lock:true）播放期间拒绝一切新动作；系统动作（lock:false，如随机
 * 动作、业务里程碑）仅从稳态发起，可被用户动作立即抢占（装饰性动作让路）。
 * README「先播放完才执行新动作」由锁 + 抢占边界共同保障。
 *
 * freeze/unfreeze（09 建立）：拖拽/挂起期间保持当前帧不动、步内时长不累积，松手继续。
 * flick 计数：抢占未稳态动作 = 帧硬切 → 渲染层快速淡出淡入；ActionSpec.flickOnSwap
 * 让稳态常驻替换也闪一次（模式硬切无过渡动画时的兜底，工单 17）。
 */
import { ANIMATIONS, type PetAnim } from "./animations";
import type { MonitorArea } from "./dragBounds";

/** 窗口位移抽象：一律物理像素（与 Tauri outerPosition / monitor.workArea 同口径）。
 *  moveTo 为原始落位（09 起钳制归调用方：拖拽走 dragBounds、动画位移走 resolveMove）。 */
export interface PetMover {
  position(): { x: number; y: number };
  size(): { width: number; height: number };
  workArea(): { x: number; y: number; width: number; height: number };
  /** 应用绝对位置 */
  moveTo(x: number, y: number): void;
  /** 逻辑→物理缩放因子（位移距离按逻辑像素指定）；缺省 1 */
  scaleFactor?(): number;
  /** 全部显示器几何快照（09 拖拽跨屏/吸附用）；缺省 = 无多屏支持 */
  monitors?(): MonitorArea[];
  /** 刷新屏幕拓扑缓存（显示器热插拔/跨屏 DPI 变化） */
  refresh?(): Promise<void>;
}

/** 动作位移：距离为显示逻辑像素，方向 auto = 朝屏幕余量大的一侧；
 *  "return" = 回到本动作开始时的窗口 x（自主移动往返的回程用）。 */
export type MovementSpec =
  | { direction: "auto" | "left" | "right"; distance: number }
  | { direction: "return" };

/** 屏幕余量定向：右侧可动空间 ≥ 左侧 → "right"（随机动作挑起跑侧与调试页 auto 共用） */
export function autoDirection(posX: number, minX: number, maxX: number): "left" | "right" {
  return maxX - posX >= posX - minX ? "right" : "left";
}

/** 序列中的一步（单动画） */
export interface StepSpec {
  anim: PetAnim;
  /** 覆盖动作天然循环性（如 review 天然循环、一次性演出截断为有限遍） */
  loop?: boolean;
  /** 一次性步的播完遍数（缺省 1；review 业务演出 = 2，见 actions.ts） */
  repeats?: number;
  movement?: MovementSpec;
}

/** 一个动作 = 若干步顺序播放，最后一步通常是循环常驻 */
export interface ActionSpec {
  steps: StepSpec[];
  /** 用户动作占锁（播放中拒绝新动作）；系统动作可被用户动作抢占 */
  lock: boolean;
  /** 稳态替换也 flick（模式硬切无过渡动画时的兜底）；缺省只在抢占未稳态时 flick */
  flickOnSwap?: boolean;
  /** 进入最终稳态时回调：循环末步 = 开始循环那一刻，单次末步 = 播完那一刻。
   *  启动序列完成、一次性演出结束都挂这里。 */
  onSettle?: () => void;
}

/** 引擎对外状态（渲染与 UI 禁用的全部依据） */
export interface EngineState {
  anim: PetAnim;
  frame: number;
  locked: boolean;
  busy: boolean;
  /** 抢占硬切计数：新动作替换了未稳态的旧动作时 +1（flickOnSwap 时稳态替换也 +1）——
   *  渲染层据此快速淡出→淡入避免帧跳变（CONTEXT PetActionExecution 流畅性保障） */
  flick: number;
}

/** 解算后的窗口位移：起点 + 带符号位移（物理像素，已按屏幕边界截短） */
interface ResolvedMove {
  startX: number;
  dx: number;
}

export class PetEngine {
  private mover: PetMover | null = null;
  private action: ActionSpec | null = null;
  private stepIdx = 0;
  /** 渲染状态持久保留：动作结束后停在最后一帧，编排间隙不闪空 */
  private anim: PetAnim = "idle";
  private frame = 0;
  /** 当前步的单调时长（ms）——帧查表与位移插值共用；不能用会归零的累积量 */
  private stepElapsed = 0;
  private lastT = 0;
  private raf = 0;
  private move: ResolvedMove | null = null;
  /** 末步稳态回调是否已触发（循环末步在开始时触发、单次末步在播完时触发，各一次） */
  private settled = false;
  /** 本动作开始时的窗口 x（"return" 位移的回程目标） */
  private actionStartX = 0;
  /** 拖拽冻结：帧与位移都不推进，但步内时长不累积（松手无缝继续） */
  private frozen = false;
  /** 抢占硬切计数（EngineState.flick） */
  private flick = 0;
  private listeners = new Set<(s: EngineState) => void>();

  constructor(mover?: PetMover) {
    if (mover) this.mover = mover;
  }

  /** 运行期注入位移实现（等 Tauri 窗口 API 就绪后） */
  setMover(mover: PetMover): void {
    this.mover = mover;
  }

  /** 发起动作；被拒时返回 false（UI 据此提示"桌宠正在执行动作"）。
   *  锁的边界：**过渡中**（settled=false）用户动作拒绝一切、系统动作仅可被用户动作抢占；
   *  **稳态循环**（settled=true，末步循环开始后）视为"空闲"——任何新动作可直接替换，
   *  否则启动序列末尾的常驻循环会永久占锁，之后一切动作都被拒。 */
  request(action: ActionSpec): boolean {
    if (action.steps.length === 0) return false;
    if (this.action && !this.settled) {
      if (this.action.lock || !action.lock) return false; // 用户过渡在播拒绝一切；系统过渡只能被用户抢占
    }
    this.begin(action);
    return true;
  }

  state(): EngineState {
    return {
      anim: this.anim,
      frame: this.frame,
      /** 锁只在过渡期持有；末步循环开始（settled）即释放，菜单恢复可用 */
      locked: (this.action?.lock && !this.settled) ?? false,
      busy: this.action != null,
      flick: this.flick,
    };
  }

  /** 拖拽冻结（工单 09）：保持当前帧不动、在飞位移暂停；播放状态（锁/步进）不受影响 */
  freeze(): void {
    this.frozen = true;
  }

  /** 松手继续。在飞位移以**当前窗口位置**重锚定、原目标为终点（拖拽期间窗口被用户
   *  挪走时不回跳）；重锚后无行程则位移作废。 */
  unfreeze(): void {
    if (!this.frozen) return;
    this.frozen = false;
    if (this.move && this.mover) {
      const target = Math.min(
        Math.max(this.move.startX + this.move.dx, this.clampMinX()),
        this.clampMaxX(),
      );
      const startX = this.mover.position().x;
      const dx = target - startX;
      this.move = dx === 0 ? null : { ...this.move, startX, dx };
    }
  }

  /** 停止当前动作、停在最后一帧（调试页暂停用；编排层的常驻接管不走这里） */
  stop(): void {
    this.action = null;
    this.stopLoop();
    this.emit();
  }

  subscribe(fn: (s: EngineState) => void): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }

  private begin(action: ActionSpec): void {
    // 抢占未稳态动作 = 帧硬切，flickOnSwap 让稳态常驻硬切也闪（模式切换无过渡动画）
    if (this.action && (!this.settled || action.flickOnSwap)) this.flick++;
    this.stopLoop(); // 取消在排 tick：替换运行动作时 rAF 会叠加（08 遗留——轮换/抢占后动画倍速）
    this.frozen = false; // 新动作即播：拖拽冻结不跨动作延续
    if (this.mover) this.actionStartX = this.mover.position().x;
    this.action = action;
    this.stepIdx = 0;
    this.settled = false;
    this.startStep();
    this.lastT = 0;
    this.raf = requestAnimationFrame(this.tick);
  }

  private startStep(): void {
    const action = this.action!;
    const step = action.steps[this.stepIdx];
    const def = ANIMATIONS[step.anim];
    this.anim = step.anim;
    this.frame = 0;
    this.stepElapsed = 0;
    this.move = this.resolveMove(step);
    // 循环末步：开始循环即最终稳态（如启动后的常驻循环）
    if (this.stepIdx === action.steps.length - 1 && (step.loop ?? def.loop) && !this.settled) {
      this.settled = true;
      action.onSettle?.();
    }
    this.emit();
  }

  private clampMinX(): number {
    const a = this.mover!.workArea();
    return a.x;
  }

  private clampMaxX(): number {
    const a = this.mover!.workArea();
    return a.x + a.width - this.mover!.size().width;
  }

  /** 解算本步窗口位移：auto/显式定向按工作区边界截短（不出屏），return 回到动作起点 */
  private resolveMove(step: StepSpec): ResolvedMove | null {
    if (!step.movement || !this.mover) return null;
    const pos = this.mover.position();
    const spec = step.movement;
    if (spec.direction === "return") {
      const target = Math.min(Math.max(this.actionStartX, this.clampMinX()), this.clampMaxX());
      const dx = target - pos.x;
      return dx === 0 ? null : { startX: pos.x, dx };
    }
    const dir = spec.direction === "auto" ? autoDirection(pos.x, this.clampMinX(), this.clampMaxX()) : spec.direction;
    const dist = spec.distance * (this.mover.scaleFactor?.() ?? 1);
    const target = Math.min(
      Math.max(pos.x + (dir === "right" ? dist : -dist), this.clampMinX()),
      this.clampMaxX(),
    );
    const dx = target - pos.x;
    return dx === 0 ? null : { startX: pos.x, dx };
  }

  /** 步的有效总时长：循环步 = 一圈；一次性步 = 一圈 × 遍数（位移插值的分母） */
  private stepDuration(step: StepSpec): number {
    const def = ANIMATIONS[step.anim];
    const one = def.cum[def.cols];
    const loop = step.loop ?? def.loop;
    return loop ? one : one * (step.repeats ?? 1);
  }

  /** 帧下标查表：t 落进 durations 前缀和的第几格；一次性步播完 reps 遍返回 done */
  private frameIndex(def: (typeof ANIMATIONS)[PetAnim], t: number): number {
    let i = 0;
    while (i < def.cols && def.cum[i + 1] <= t) i++;
    return i;
  }

  private tick = (t: number): void => {
    const action = this.action;
    if (!action) return;
    if (this.lastT === 0) this.lastT = t;
    const dt = t - this.lastT;
    this.lastT = t;
    // 拖拽冻结：丢弃 dt（时长不累积，松手继续不快进），只续排循环
    if (this.frozen) {
      this.raf = requestAnimationFrame(this.tick);
      return;
    }
    const step = action.steps[this.stepIdx];
    const def = ANIMATIONS[step.anim];
    const loop = step.loop ?? def.loop;
    const reps = step.repeats ?? 1;

    // 窗口位移按单调时长线性插值（比帧率平滑）；目标已在 resolveMove 钳进工作区
    if (this.move && this.mover) {
      const progress = Math.min(this.stepElapsed / this.stepDuration(step), 1);
      this.mover.moveTo(this.move.startX + this.move.dx * progress, this.mover.position().y);
    }

    this.stepElapsed += dt;
    if (!loop && this.stepElapsed >= def.cum[def.cols] * reps) {
      this.advanceStep();
      return; // advanceStep 内部负责续排 rAF（末步完成则不续）
    }
    // 帧下标从步内时长派生：循环取模一圈、一次性在遍内取模（repeats>1 时每圈从头）
    const t2 = this.stepElapsed % def.cum[def.cols];
    const frame = this.frameIndex(def, t2);
    if (frame !== this.frame) {
      this.frame = frame;
      this.emit();
    }
    this.raf = requestAnimationFrame(this.tick);
  };

  private advanceStep(): void {
    const action = this.action!;
    // 位移终值钉死：末帧采样可能停在 <1，每步结束时刻补一次 p=1——中间步不钉死会让
    // 下一步的起点（含"return"回程目标）带着小数偏差（09 随机动作实测暴露）
    if (this.move && this.mover) {
      this.mover.moveTo(this.move.startX + this.move.dx, this.mover.position().y);
      this.move = null;
    }
    if (this.stepIdx === action.steps.length - 1) {
      const onSettle = action.onSettle;
      this.action = null;
      this.stopLoop();
      // 单次末步播完 = 稳态（如「再见」淡出前的最后一帧）；settled=false 说明循环分支没触发过
      if (!this.settled) onSettle?.();
      this.emit();
      return;
    }
    this.stepIdx++;
    this.startStep();
    // 步进后必须续排循环，否则多步序列在第一步播完就停摆（启动序列曾卡死于中途）
    this.raf = requestAnimationFrame(this.tick);
  }

  private stopLoop(): void {
    cancelAnimationFrame(this.raf);
    this.lastT = 0;
  }

  private emit(): void {
    const s = this.state();
    for (const fn of this.listeners) fn(s);
  }
}
