# 19: 休息随机 + home/away 自主移动

**What to build:** 休息模式随机动作池上线：从上一次（用户或随机）动作结束算起**固定 300 秒后抽签，50% 触发**；触发后池子**跳跃 jumping ： 自主移动 = 1:1**。**自主移动 = home / away 交替往返**：在 home 时跑出去（单程 64~128 逻辑像素随机、方向取屏幕余量大侧、目标点钳进工作区且距边 ≥20px）**留在 away**，下次触发跑回 home；**用户拖拽 = 重置 home（拖拽落点）、清除 away 态**；away 方向无足够空间时本次**降级为跳跃**；移动期间环视挂起（若工单 21 已落地则接线，否则预留挂起点）；工作模式不触发（PetActionPolicy 白名单）。

背景：spec 故事 62；术语 PetRandomAction / PetActionPolicy（随机动作列为白名单第四来源）；[ADR-0010](../../../docs/adr/0010-codex-pet-replaces-oreo.md) 修订节（home/away 取舍：位置控制权归用户 + 接受拖拽/移动共用动画的双语义；引擎位移插值 / return 回程 / autoDirection 机制直接复用，取代工单 09 的吃/闲坐/Attack 编排）。

**Blocked by:** 17 模式常驻与生命周期编排

**Status:** done（待手动验收）

- [x] 随机调度：间隔 300s、概率 50%、权重 1:1；用户/随机动作结束重新计时；全量守卫复查（模式/阶段/拖拽中）被拒自愈重排
- [x] home/away 交替：跑出留下、下次跑回原位；方向动画与位移方向一致；目标钳制与 ≥20px 边距成立
- [x] 拖拽落点 = 新 home，旧 away 作废；下次触发从新 home 出发
- [x] 降级路径：无空间时本次变跳跃，不报错不越界
- [x] 移动期间环视挂起点就绪（21 接线或预留）
- [x] 手动验收：临时调小间隔观察跳跃/移动/回位、拖拽重置、贴边降级

实现注记：homeX = 启动落位或拖拽落点、awayX 标记在外；出程 planRoam（64~128px 随机/余量大侧/边距钳制/行程不足 48px 降级）、回程 planReturn（目标钳当前工作区防拓扑漂移）；环视挂起点 = 环视只在常驻 idle 显示（跑动动画天然排除，21 已按此接线）；被拖拽抢占的自主移动不欠账（onSettle 丢失由拖拽重置 home/away 兜底）。

审查修正（code-review-zh Standards）：planRoam/planReturn 参数序统一（(fromX, …, area, winW, factor) 一致旅行）+ roamStep 共享构造；Spec 轴补 fireRandom 的 boardOpen 守卫（面板开着 = waiting 循环演出，随机顺延下个间隔——随工单 20 修正落地）。
