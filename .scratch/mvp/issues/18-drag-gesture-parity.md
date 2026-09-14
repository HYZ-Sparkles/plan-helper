# 18: 拖拽手势照抄原版

**What to build:** 拖拽反馈照抄 codex 原版 playground 手势语义：**拖起瞬间 jumping**（被提起的反应）→ 拖动中按 **140ms 滑动采样窗口**的主方向演 running-right / running-left（方向反转实时跟切；纯竖直 → jumping；轴向模糊/斜向不清 → 保持当前动作）→ **松手立即回当前常驻动画、无任何后续动作**（"拖后 Attack"编排废除）。PetDragBounds 四约束（任务栏禁入 / 不出物理边界 / 跨屏 / 边缘吸附）与小看板刚性耦合（PetBoardCoupling：位置联动、动作独立）原样保留。

背景：spec 故事 65、66；术语 PetDragBounds / PetBoardCoupling / PetActionExecution；[ADR-0010](../../../docs/adr/0010-codex-pet-replaces-oreo.md)（取代工单 09 的"拖动保持当前帧 + 拖后起身 Attack"——freeze/unfreeze 机制仅保留给轴向模糊瞬间，不再承担整段拖拽）。

**Blocked by:** 17 模式常驻与生命周期编排

**Status:** done（待手动验收）

- [x] 拖起瞬间 jumping；水平拖动方向动画正确（右→running-right、左→running-left）且中途反转实时跟切
- [x] 纯竖直拖动 jumping；轴向模糊（斜向不足 1.12 偏差比）保持当前动作不抖动
- [x] 松手立即回常驻（工作 running / 休息 idle），无遗留后续编排
- [x] 拖拽期间小看板刚性随动（一个被挡全体一起停）、四条边界约束回归通过
- [x] 手动验收：四方向拖拽、中途反转、竖直、松手回常驻、看板随动

实现注记：classifyDrag 纯函数（140ms 滑窗首尾位移主方向、1.12 偏差比，Node 回归覆盖含反转跟切与窗口淘汰）；拖起与方向切换共用 setDragFeedback（dragLoopAction 用户锁 + 单循环步即刻稳态支撑实时替换、可抢占一次性系统动作）；松手 afterOneShot 统一结算。

审查修正（code-review-zh Standards）：startDragFeedback 并入 setDragFeedback("jumping")（函数体重复）。
