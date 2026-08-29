# 10: 工时账户结算

**What to build:** 每个工作日结束后，实际推进与当日实际目标的差额（未达标上调、超额下调）按均分窗口向后续工作日均分；多日差额滚动叠加、未达标的均分日递归再均分；历史修正后一切实时重算。大面板状态条与小看板微型条升级为显示含结转的调整后目标并透明标注。

背景：spec 用户故事 40–46；ADR-0007（工时账户对称均分）、ADR-0009（单一事实源实时派生）；术语 WorkHourLedger / DailyCompletionTolerance / TodayLoadCommitment / LoadSmoothing 窗口（以工作日为单位）。

**Blocked by:** 06 大面板今日分配, 07 小看板与进度汇报

**Status:** done（待手动验收）

- [x] 结算全部从 ProgressLog + 设置实时派生，不固化汇总表；时钟注入使任意"今天"可确定性测试（`domain/ledger.rs`：`LedgerService::day_target` 从最早事件归属日逐日回放到昨天，`minutes_by_day` 按事件归属日聚合——与 progress 今日完成量共用同一聚合；seam_ledger 全部用 FixedClock 驱动）
- [x] 对称抵扣：差额为负（未达标）上调后续目标，为正（超额）下调；**只有实际完成的超额抵扣**，选择层面超额不计（服务层测试用"选 14h 推 5h"场景验证：`selection_level_surplus_not_credited`——结算只读 ProgressLog，不碰 today_allocations）
- [x] 滚动叠加：当日实际目标 = 基准 + Σ(各历史差额均分到今天的份额)（多日差额并存场景测试：`rolling_sum_of_multiple_deficits`，周三 = 300 + 252/7 + 288/7）
- [x] 递归均分：均分目标日当天是工作日却未达标，其新差额继续向后均分（测试覆盖：`deficit_day_respreads_recursively` / `tolerance_band_uses_adjusted_target` 第三段——空闲工作日照样欠整日、债滚债）
- [x] ±10% 达标容差以**调整后的当日目标**为基数（5h → 4.5h 达标、4h 未达标的边界测试：`tolerance_band_uses_adjusted_target`——4.5h 恰在带沿不均分；被上调日（目标 342.857）推 300 仍 < 0.9×342.857 判未达标再均分。**实施口径**：带对称豁免两个方向——带内超额同样不抵扣（`surplus_symmetric_band`），带外差额按 |目标-实际| 计量，已补记 CONTEXT 工时账户节）
- [x] 均分窗口以工作日为单位：非工作日不计入窗口、不从历史扣除；非工作日的推进按超额并入账户（`weekend_never_consumes_window_but_surplus_enters`：周五缺口 + 周六加班跨周末逐工作日落到同一 7 工作日窗口，第 7 个工作日仍在窗内；周六自身无目标义务）
- [x] 历史修正（撤销子目标、修正总进度）后，受影响日期差额与未来全部目标自动重算——不存在日结冻结（测试：修正昨日数据后今日目标变化：`correction_recomputes_future_targets`——correct_total / undo_subgoal 改口昨日净量，今日目标随之实时变；无任何固化汇总可脱节）
- [x] 慢性赤字不设上限、无清账操作；化解路径只有调大均分窗口（13 的设置项）——实现上无任何封顶/清账代码路径，`smoothing_workdays` 全量即时生效（重放口径，无按日 W 快照）
- [x] 大面板底部状态条与小看板微型条显示"目标 Y 小时（基准 + 结转）"并标注，如"目标 5.6h（基准 5.0h + 结转 0.6h）"；06/07 的占位目标升级替换（两视图 target_minutes 升级为 f64 含结转 + base_minutes；`labels.ts#carryLabel` 共享标注（负结转 = 超额抵扣的轻松日；无结转不加噪声）；小看板另加 workday 标记——休息日加班态无目标义务，显示"休息日加班并入账户"不显示目标/进度条）
