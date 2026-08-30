# 11: 今日总结

**What to build:** 最晚工作窗口结束那一刻自动弹出今日总结；当天没开应用则次日第一次打开弹"昨日总结"补登。总结只弹一次，内容随后续进展实时重算、可在控制面板随时调出。布局为顶部总览行 + "计划→任务→推进内容"三层嵌套；被抢占暂停的计划照常展示并计入达标判定。

背景：spec 用户故事 50–54、58；ADR-0009（实时重算）；术语 DailySummaryTrigger / DailySummaryLayout / WorkHourLedger。

**Blocked by:** 10 工时账户结算

**Status:** done（待手动验收）

- [x] 触发：**最晚**一个工作时间窗口结束那一刻自动弹出（窗口配置在 13 就绪前，验收用种子窗口数据；时钟注入测试触发逻辑）——`domain/summary.rs`：`latest_window_end`（多段取最晚段结束；跨午夜窗口结束在次日）+ `due`（今天已过未登记→补弹 / 昨天已过未登记→补登）+ `next_fire`（前端 PetWindow 定时排程，触发后重查重排；seam_summary 全部 FixedClock 驱动、窗口经 `SettingsService::save` 种子）
- [x] 当天未开应用（所有工作段已过）：次日第一次打开弹"昨日总结"补登——`due` 只回看今天与昨天两天（基线始终是"昨天"，与星期无关；更早不补登）；补登口径 `DailySummaryView.is_yesterday` 供标题"昨日总结"
- [x] 只弹一次：弹出后新进展不重弹；总结内容从 ProgressLog 实时重算，控制面板"计划管理"页可随时调出看最新版——`summary_shown` 登记表 + `mark_shown`（自动弹出与控制面板补看待弹那份两条路都登记，幂等；手动补看即注销 = "只弹一次"的意图口径）；`summary()` 每次调用全量重算（ADR-0009，无固化总结表）；PlansPage「今日总结」按钮（story 58）随时调出，打开中的总结收到 `daily-summary:refresh`（小看板汇报/详情页修正落账时发射）实时重取。**逐日历史回看不入本工单**——`getDailySummary(date)` 已支持任意日期，需要时补日期选择入口（story 58"回看历史总结"的完整形态）
- [x] 顶部总览行：今日推进 N 计划 / 完成 X 小时 / 目标 Y 小时（**含结转标注**，来自 10）/ 进度条；Phosphor CheckCircle + Target 图标——目标走 `LedgerService::day_target_on(总结归属日)`（补登日按"那天该完成多少"口径，历史修正后重算）；`carryLabel` 结转标注复用；休息日加班态隐藏目标与进度条（与大小面板同口径）
- [x] 下方按"计划 → 任务 → 推进内容"三层：计划 section（名 + 优先级图标 + 今日推进总耗时）、任务行（名 + 推进百分比 + 耗时）、推进内容（有子目标显示完成列表/进行中，无子目标显示百分比进度条）——DailySummaryWindow.vue；计划顺序复用 PlanOrdering（优先级降序）；子目标为当前态快照（✓ 完成列表 + 最早未完成"进行中"，实时重算）；无子目标走 MicroBar
- [x] 展示**所有当日有推进的计划**：被抢占暂停的计划照常出 section、标注"已被抢占暂停"，其推进**计入完成总量与达标判定**；当日零推进的计划不展示——总口径 total_minutes = 当日全部事件净额（与工时账户的 `minutes_by_day` 同源），preempted = Paused + AutoPreempted（测试用 `force_pause_reason` 种子，12 接入前）
- [x] 有更高优先级计划未开始时，总结中提示——`higher_priority_hint`：∃ NotStarted 计划优先级**严格高于**当日推进过的最高优先级（零推进日 = 任何 NotStarted 都算）
- [x] 跨午夜窗口内的汇报与总结归属窗口开始日（13 的归属规则就绪前，服务层按"窗口开始日"口径测试）——事件归属复用 `progress::attributed_date`；触发时刻跨午夜窗口在**次日**结束（偏移 1440 + end 分钟）；seam_summary 断言周一 23:00 + 周二 00:30 两笔都归周一日账
