# 20: 业务里程碑动作

**What to build:** PetActionPolicy 白名单的四个低频业务里程碑接线（桌宠从"装饰"升级为业务状态的活指示器）：

1. **waiting**：大面板**打开期间**循环演出（规则就是"面板开着 = 等你分配"——不区分今日是否已确认、不区分当前模式），面板关闭即回当前常驻；
2. **review**：小看板每发生一次**推进型**进度汇报（子目标勾选或 +X% 增量）演一次（约 2.4s）回 running；**修正总进度、撤销子目标等"往回改"的日志事件不触发**；连续快速汇报合并反馈、播完补一次不逐条排队；
3. **failed**：今日总结弹出且当日未达标（低于容差带）时演一次回 running；
4. **jumping（庆祝）**：今日任务**全部完成**时演一次——与休息随机跳跃共用动作、触发语义不同；达标但未全完不给。

抢占暂停等其余业务事件不驱动动作（AutoPauseFeedback 第 3 条维持"不做额外动作"）。

背景：spec 故事 62a、62b、62c；术语 PetActionPolicy / MiniBoard / CurrentTask / DailySummaryTrigger / ProgressLog（推进型事件的判定源）/ TodayLoadCommitment；[ADR-0010](../../../docs/adr/0010-codex-pet-replaces-oreo.md)。事件接线沿用既有窗口事件模式（桌宠窗口是唯一编排持有者）。

**Blocked by:** 17 模式常驻与生命周期编排

**Status:** done（待手动验收）

- [x] waiting 起止 = 大面板开关；确认分配后重开面板仍 waiting；休息模式开着面板也 waiting；关闭即回该模式常驻
- [x] review 只由推进型日志事件触发（子目标勾选 / +X%）；修正总进度、撤销子目标不触发；密集汇报合并不排队
- [x] failed 未达标播一次回 running，达标不播；庆祝 jumping 仅今日任务全部完成时一次
- [x] 里程碑动作不占用户动作锁（系统动作无锁规则），播放中用户交互仍按 PetActionExecution 裁决
- [x] 手动验收：四个触发场景 + 三个不触发场景（确认后重开 / 撤销子目标 / 修正总进度）

实现注记：waiting = MainBoard 亮出/隐藏发 main-board:visibility、PetWindow boardOpen 状态 + chassisAnim 裁决（自己亮面板直接置位）；review = MiniBoard 推进型动作（complete/report）成功后发 pet:milestone，撤销/修正/换任务不发，密集到达转 pending 播完补一次；failed = DailySummary show 事件装载后按服务端 met_target（±10% 容差带下沿，ledger::day_met 与工时账户同口径）判定；庆祝 = 后端 today_tasks_all_complete（今日已分配且非空且全部未删除 Completed，seam 五态覆盖）+ MiniBoard false→true 跃迁、mount 首查只置基态；afterOneShot 统一结算一切一次性演出（随机/里程碑/被拖拽抢占）。

审查修正（code-review-zh）：Spec 轴——fireRandom 补 boardOpen 守卫（休息+面板开着随机不打断 waiting）；Standards——failed 容差判定后端化（DailySummaryView.met_target，前端不再自推 0.9 魔法数，seam_summary 补容差带/休息日用例）。
