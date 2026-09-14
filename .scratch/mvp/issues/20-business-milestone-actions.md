# 20: 业务里程碑动作

**What to build:** PetActionPolicy 白名单的四个低频业务里程碑接线（桌宠从"装饰"升级为业务状态的活指示器）：

1. **waiting**：大面板**打开期间**循环演出（规则就是"面板开着 = 等你分配"——不区分今日是否已确认、不区分当前模式），面板关闭即回当前常驻；
2. **review**：小看板每发生一次**推进型**进度汇报（子目标勾选或 +X% 增量）演一次（约 2.4s）回 running；**修正总进度、撤销子目标等"往回改"的日志事件不触发**；连续快速汇报合并反馈、播完补一次不逐条排队；
3. **failed**：今日总结弹出且当日未达标（低于容差带）时演一次回 running；
4. **jumping（庆祝）**：今日任务**全部完成**时演一次——与休息随机跳跃共用动作、触发语义不同；达标但未全完不给。

抢占暂停等其余业务事件不驱动动作（AutoPauseFeedback 第 3 条维持"不做额外动作"）。

背景：spec 故事 62a、62b、62c；术语 PetActionPolicy / MiniBoard / CurrentTask / DailySummaryTrigger / ProgressLog（推进型事件的判定源）/ TodayLoadCommitment；[ADR-0010](../../../docs/adr/0010-codex-pet-replaces-oreo.md)。事件接线沿用既有窗口事件模式（桌宠窗口是唯一编排持有者）。

**Blocked by:** 17 模式常驻与生命周期编排

**Status:** ready-for-agent

- [ ] waiting 起止 = 大面板开关；确认分配后重开面板仍 waiting；休息模式开着面板也 waiting；关闭即回该模式常驻
- [ ] review 只由推进型日志事件触发（子目标勾选 / +X%）；修正总进度、撤销子目标不触发；密集汇报合并不排队
- [ ] failed 未达标播一次回 running，达标不播；庆祝 jumping 仅今日任务全部完成时一次
- [ ] 里程碑动作不占用户动作锁（系统动作无锁规则），播放中用户交互仍按 PetActionExecution 裁决
- [ ] 手动验收：四个触发场景 + 三个不触发场景（确认后重开 / 撤销子目标 / 修正总进度）
