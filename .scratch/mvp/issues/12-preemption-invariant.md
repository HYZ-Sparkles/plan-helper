# 12: 抢占不变式与逐层恢复

**What to build:** 更高优先级计划开始时，用户手头进行中的低优先级计划自动暂停（原因记"自动抢占"）；不存在进行中的更高等级计划时，等级最高的被抢占组整组自动恢复、逐层向下；存在进行中的更高等级时，低等级计划的「开始」按钮禁用。任何时刻进行中的计划必然同等级。

背景：spec 用户故事 15–19；ADR-0006（抢占不变式）；术语 PreemptionInvariant / PauseReason / AutoPauseFeedback / Parallelism。

**Blocked by:** 06 大面板今日分配

**Status:** done（待手动验收）

- [x] 开始约束：存在进行中的更高等级计划时，低等级计划「开始」禁用 + tooltip 说明原因；想开低等级必须先完成/手动暂停高等级（服务层拒绝违反不变式的开始）——`lifecycle.rs#ensure_no_higher_active`（拒绝 `PreemptedByHigher{plan_name}`，带阻挡计划名；同等级并行不受限，CONTEXT Parallelism）；UI：PlanDetailPage `higherActive`（全量计划列表派生）禁用 开始/继续/复制并新建（复制即开始），外层 span 承载 tooltip（disabled 按钮不冒泡 hover），文案与错误提示同源 `labels.ts#preemptedByHigherMessage`；seam_preemption `start_blocked_while_higher_tier_active`
- [x] 自动暂停：更高等级计划开始 → 所有进行中的低等级计划自动暂停，pause_reason = 自动抢占；未开始的计划不受影响——`lifecycle.rs#preempt_lower_tiers`（只针对 status = 进行中；优先级比较在 Rust 侧——priority 是 TEXT 列，SQL 字典序 ≠ 语义序）；清单随 `LifecycleOutcome{paused}` 返回（start/resume/copy 三入口同语义）；seam_preemption `higher_start_auto_pauses_lower_only`
- [x] 逐层恢复：不存在进行中的更高等级时（完成、放弃、**手动暂停**之后均算），恢复等级最高的"自动抢占"组（整组）；更低的组等这组清空才轮到——`lifecycle.rs#auto_resume_top_tier` 接入 pause/complete/abort 三触发点；单次只恢复一组（恢复组自身成为更低组面前的"进行中更高等级"）；seam_preemption `preemption_chain_recovers_layer_by_layer`（High 完成 → 恢复 Medium 组 → Medium 放弃 → 恢复 Low 组）+ `recovery_fires_on_pause_abort_and_resume_repreempts`（三种清空方式都触发）
- [x] 用户主动暂停的计划**永不自动恢复**，只能手动恢复（服务层测试：manual pause 在恢复条件满足时保持暂停）——恢复只认 pause_reason = AutoPreempted；seam_preemption `manual_pause_never_auto_resumes`
- [x] 被恢复的计划再次遇到更高等级开始，再次被自动抢占（原因仍记自动抢占）——resume 走 ensure + preempt 同一路径；seam_preemption `recovery_fires_on_pause_abort_and_resume_repreempts`
- [x] 抢占链场景测试：High 开始 → Medium 组暂停 → Medium 组更早抢的 Low 仍暂停；High 完成 → 恢复 Medium 组；Medium 组再清空 → 恢复 Low 组——`preemption_chain_recovers_layer_by_layer` 全走服务层真实命令（start/complete/abort），不用 SQL 造状态
- [x] AutoPauseFeedback 三层：大面板开着时被暂停计划立刻灰显并移入"暂停"分组——`AllocationBoardView.paused_groups`（`groups_of` 与候选装配同一形态；灰显 + StatusBadge"已暂停"、无勾选框不可选），生命周期变化落库后 PlanDetailPage 发 `main-board:refresh`（src/lib/events.ts 常量）静默重取，开着时立刻可见、显隐不变；控制面板插入一行"计划X 已被自动暂停（高优先级 计划Y 开始）"——PlanDetailPage `preemptNotice`（**落点决策**：CONTEXT 原文写"今日总结面板"，但总结窗是 ADR-0009 纯派生视图、无事件账可查，反馈行落在触发地控制面板详情页；总结侧的账面由工单 11 既有的"已被抢占暂停"标注承载）；桌宠不做额外动作（安静原则，无接线）
- [x] 06 大面板的过滤前提落实：不变式保证进行中计划必然同等级，无需"只显示最高等级"特判（回归测试：抢占后大面板任务集只含进行中计划）——candidates 注释更新；seam_preemption `board_after_preemption_only_shows_active_plans`（抢占后候选只含 High、暂停分组收 Medium、stale 选中集被剔除、恢复后选中集原样回显）；copy_as_new 同受约束（`copy_as_new_respects_invariant_and_reports_preemption`）
