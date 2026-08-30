# 后端复用方法（src-tauri/FUNC.md）

新写 Rust 代码前先查这里；新增可复用方法后回来登记（AGENTS.md 复用纪律）。

## 分层约定（spec 接缝决策）

- **domain/**：领域服务层，不 import tauri；需要"现在"一律取 `&dyn Clock`（时钟注入）。全部自动化测试打这层（`tests/`）。
- **infra/**：外部资源接入（目前只有 SQLite）。
- **commands.rs**：Tauri command 薄代理——只解包 `State<AppState>` 转发 domain，不写业务判断。
- `app_state.rs`：Tauri 托管状态 `AppState { db: Mutex<Connection>, clock: Box<dyn Clock> }`。

## 时钟（src/clock.rs）

- `trait Clock { fn now(&self) -> DateTime<Local> }`
- `SystemClock` — 生产用（lib.rs 装配时注入）
- `FixedClock(DateTime<Local>)` — 测试用，构造即固定"现在"

## 数据库（src/infra/db.rs）

- `db::open(&Path) -> Connection` — 打开/建库 + WAL + 建表（schema 见 `SCHEMA`）；init 幂等补列（`backfill_columns`，工单 05 起 pause_reason——开发期旧库不删重建也能升列）
- `db::open_in_memory() -> Connection` — 集成测试用内存库

## 领域服务（src/domain/）

- `plans::PlanService`（工单 02/03/04/05）
  - `create(conn, clock, &PlanDraft) -> Result<i64, PlanError>` — 事务写入计划+任务+子目标+依赖边；简述留空回退名称（`default_text`）；校验：名称非空、≥1 任务、无子目标任务必填耗时、勾子目标须 ≥1 子目标且每行内容/耗时必填
  - `update(conn, clock, plan_id, &PlanDraft) -> Result<(), PlanError>`（工单 03/04）— 整计划编辑（CreationUI 编辑态）。不变量：计划开始后优先级锁定（PriorityLocked）；已完成任务内容锁定（TaskLocked，position 除外）；提交任务集必须与库一致（TaskSetMismatch，删除显式走 delete_task）；落库顺序 = 未完成按提交序在前、已完成按库内序沉底（新任务自然落在所有未完成之后）；子目标全量替换（`update_subgoals`：已完成锁定 SubGoalLocked、未完成可改可删、新行追加在后、取消勾选=清空）；依赖边全量替换（草稿所见即所存）
  - `get(conn, plan_id) -> Result<PlanView, PlanError>` — 单计划详情（含任务+子目标+前置 id，按 position）；不存在 NotFound
  - `delete_task(conn, clock, task_id) -> Result<(), PlanError>` — 软删除进归档（deleted_at 落删除时刻）并自动解除依赖边（后继解锁）；不存在/已删 NotFound
  - `list(conn) -> Result<Vec<PlanView>, PlanError>` — 全部计划+嵌套任务（软删除过滤），按 PlanOrdering 默认规则（唯一排序，2026-08-24 决策砍掉手动覆盖）：Priority 降序 → CreatedAt 倒序 → id 倒序；错误统一走 PlanError 通道
  - `task_progress(conn, task_id) -> Result<TaskProgress, PlanError>`（工单 04/07）— 派生进度（ADR-0002）：(已完成分钟: **f64**, 总分钟)，`percent()` 供展示（**保持原始 f64 精度**——这是领域语义值，"分子不变分母变"缩放后 60/140 = 42.857142857142854 必须如实保留）、`is_complete()`（epsilon 容差）供自动完成判定；改耗时/增删未完成子目标后自动缩放（分子不变分母变）；有子目标 = 已完成子目标求和，无子目标 = progress_log 增量求和；存储层 ProgressLog 写入时 round delta 到 1e-9（边界消 IEEE 754 浮点尾巴，不破坏领域语义），跨边界输出（CurrentTaskView.percent / completed_minutes）时 round 1e-1 / 1e-9（消 UI 浮点尾巴）
  - `TaskView.progress_percent` — 派生进度百分比（load_tasks 装配，列表/详情/小看板候选统一消费）
  - `PlanDraft` / `TaskDraft`（含 `subgoals`、`depends_on` 草稿下标引用）/ `SubGoalDraft` — 创建与编辑共用草稿（id 为 None = 新行）
  - `PlanError` — 结构化领域错误（serde tag=kind/content=payload），前端文案见 `src/lib/labels.ts#planErrorMessage`；工单 05 新增 PlanStatusInvalid{from} / PlanNotTerminal{from} / TasksNotCompleted，工单 06 新增 TaskNotAllocatable{task_id}，工单 07 新增 TaskNotInToday{task_id} / SubGoalOutOfOrder / SubGoalNotCompleted / ProgressLocked / PercentInvalid / PercentOverflow / NotPercentTask，工单 11 新增 InvalidDate（总结 command 边界的日期解析，`summary::parse_date`），工单 12 新增 PreemptedByHigher{plan_name}（抢占不变式的开始约束，plan_name 供 tooltip/文案）
  - `Priority` / `PlanStatus` / `TaskStatus` — 枚举 + `as_db`/`from_db` TEXT 往返；Priority 派生 Ord（Low < Medium < High）
- `lifecycle::LifecycleService`（工单 05；ADR-0001 单向瀑布；工单 12 接入 ADR-0006 抢占不变式——任何时刻进行中的计划必然同等级）
  - `start(conn, plan_id) -> Result<LifecycleOutcome, PlanError>` — 未开始 → 进行中；存在进行中的更高等级计划时拒绝 `PreemptedByHigher{plan_name}`（想开低等级先完成/手动暂停高等级），成功则自动暂停所有进行中的低等级计划并随结果返回（AutoPauseFeedback 数据源）
  - `pause(conn, plan_id)` — 进行中 → 已暂停，pause_reason = PauseReason::UserInitiated（CONTEXT PauseReason 二值枚举，as_db/from_db 口径同 PlanStatus）；手动暂停使高等级清空 → 触发逐层恢复
  - `resume(conn, plan_id) -> Result<LifecycleOutcome, PlanError>` — 已暂停 → 进行中，清空 pause_reason；抢占约束同 start（恢复的高等级再遇低等级进行中 → 再次抢占，原因仍记自动抢占）
  - `complete(conn, plan_id)` — 进行中 → 已完成（PlanCompletionConfirm 手动确认）；要求全部任务已完成且 ≥1 个任务（删空计划只能放弃），否则 TasksNotCompleted；完成使高等级清空 → 触发逐层恢复
  - `abort(conn, plan_id)` — 进行中/已暂停 → 已放弃（终态；二级确认在 UI）；放弃进行中的高等级同样触发逐层恢复
  - `sync_task_completion(conn, task_id) -> Result<TaskStatus, PlanError>` — 任务进度到 100% 自动转已完成（幂等；用 `TaskProgress::is_complete` 的 epsilon 判定）；工单 07 的汇报路径已接线（`progress::settle_task` 在每次落账后调用）
  - `copy_as_new(conn, clock, plan_id) -> Result<CopyAsNewOutcome, PlanError>` — 终态计划复制并新建（CopyAsNewPlan）：复制字段/任务/子目标/依赖边、进度归零、名称加"- 副本"、直接进行中；非终态拒绝 PlanNotTerminal；复制即开始 → 抢占约束与自动暂停同 start
  - 抢占内部件（私有；优先级比较在 Rust 侧——priority 是 TEXT 列，SQL 字典序 High<Low<Medium 与语义序 Low<Medium<High 不同）：`plan_priority` / `ensure_no_higher_active`（开始约束，同等级并行不受限）/ `preempt_lower_tiers`（自动暂停，只针对进行中，未开始不受影响）/ `auto_resume_top_tier`（逐层恢复：完成/放弃/手动暂停使高等级清空后，恢复等级最高的"自动抢占"组——整组、只恢复 AutoPreempted（用户主动暂停永不自动恢复）、单次只恢复一组，恢复组自身成为更低组面前的"进行中更高等级"）。输出结构 `LifecycleOutcome{paused}` / `CopyAsNewOutcome{new_plan_id, paused}` / `PreemptedPlan{id, name}`
- `deps::DependencyService`（工单 04）
  - `link(conn, predecessor_id, successor_id) -> Result<(), PlanError>` — 建一条 A→B 边：校验两端存在未删、同计划（DependencyCrossPlan）、不自指、无环（reaches 可达检测，环返回 DependencyCycle）；可事务内调用；草稿路径由 `link_draft_deps` 批量走它
  - `detach_task(conn, task_id)` — 删除任务时双向解除其边
  - `waiting_on(conn, task_id) -> Vec<TaskView>` — 前置中未完成任务列表（`is_unblocked` 的数据源）
  - `is_unblocked(conn, task_id) -> bool` — 依赖就绪判定（06 大面板的候选过滤：被阻塞任务不进列表，2026-08-24 修订"只展示可选任务"）= waiting_on 为空
- `allocation::AllocationService`（工单 06；术语 MainBoardTodayAllocation / TodayLoadCommitment / AutoOpenMainBoard）
  - `board(conn, clock) -> Result<AllocationBoardView, PlanError>` — 大面板一次装配：候选分组（复用 `PlanService::list` 的 PlanOrdering；过滤 PlanStatus=进行中、剔除已完成任务、`is_unblocked` 依赖就绪过滤——被阻塞的不进列表）+ 今日回显（库存选中集与当前可选集求交，计划暂停等 stale 选择被剔除）+ 当日目标（**工单 10 起含结转**：`LedgerService::day_target` 实时派生，target_minutes 为 f64 + base_minutes 供结转标注）+ `workday` 周循环标记（非工作日 UI 走加班态——只看累计、无目标/差额提示，2026-08-29 用户决策，CONTEXT WorkingHours）
  - `commit(conn, clock, &[task_id]) -> Result<(), PlanError>` — 提交当日分配，再次提交**整行覆盖**（重开重选）；选中集必须 ⊆ 当前可选集（进行中 + 未完成 + 依赖就绪），否则 `TaskNotAllocatable{task_id}`（UI 失步的后端兜底）。存储为 `today_allocations` 单行表（id=1，date + task_ids JSON）——分配只属于它的日期，隔日读视为未分配；历史分配无消费方（总结从 ProgressLog 派生），不按日留行
  - `should_auto_open(conn, clock, work_mode, manual) -> Result<bool>` — AutoOpenMainBoard 判定：工作模式 && 今日未分配 &&（manual || 今日在周循环工作日）。manual = 手动切入工作模式（主动加班，2026-08-29 用户决策）旁路工作日条件；自动触发（启动序列后检测 / 13 的工作窗口开始）传 false。接线在 08 已落（启动 false / 手动切入 true）
  - `AllocationBoardView`（date / workday / target_minutes / selected_task_ids / groups / **paused_groups**——工单 12：被抢占暂停的分组，`preempted_groups` 装配，只含 AutoPreempted，灰显不可选的 AutoPauseFeedback 数据源）、`AllocationGroup`（plan_id / plan_name / priority / tasks）、`AllocationTask`（id / name / estimated_minutes）— serde 结构与前端 `src/lib/api.ts` 类型一一对应
- `progress::ProgressService`（工单 07；ADR-0002/0009，术语 CurrentTask / ProgressGranularity / PercentAdjustControl / SubGoalUndo）
  - `domain::progress::round_delta_minutes(v: f64) -> f64` — pub(crate) 边界辅助（round 到 1e-9），`report_percent` / `correct_total` 写日志前 + CurrentTaskView.completed_minutes 装配 + ledger 调整后目标跨边界输出复用，消 IEEE 754 浮点尾巴；前端 `src/lib/labels.ts#hoursFromMinutes` 与 `src/lib/progress.ts#taskProgress` 各自实现对齐同一口径
  - `domain::progress::round_to_one_decimal(v: f64) -> f64` — pub(crate) 辅助（边界 round 到一位小数），CurrentTaskView.percent 与 summary 的 SummaryTask.percent（工单 11）装配复用，UI 展示与 ProgressGranularity 0.1% 颗粒度对齐
  - `domain::progress::attributed_date(at, own_windows, prev_windows) -> NaiveDate` — pub(crate) 事件归属日（ADR-0009 跨午夜口径：按事件所在日窗口判同段/跨午夜今晚段、按**前一日**窗口判凌晨尾巴〔t < end 归前一日〕，两处都不在窗内归自身日期）
  - `domain::progress::attributed_date_on(at, cal) -> NaiveDate` — 归属日的按日解析版（own/prev 窗口取自 SettingsCalendar），ledger 按日聚合与 11 总结装配共用同一入口
  - `board(conn, clock) -> Result<MiniBoardView, PlanError>` — 小看板一次装配：当前任务（单行表 id 过三重校验：存在未删 ∧ 计划进行中 ∧ 在今日推进列表内；失效回 None、已完成仍展示为"任务完成"停留态）+ 今日完成量（`day_minutes`：ledger 的 `minutes_by_day` 按日聚合取今天一档）+ 当日目标（**工单 10 起含结转**：`LedgerService::day_target`，target f64 + base_minutes + `workday` 加班态标记）+ 更换候选分组（复用 `PlanService::list` 的 PlanOrdering，当前任务同计划提到最前）
  - `set_current_task(conn, clock, task_id)` — 指定当前任务（覆盖式单行表 current_task）；校验 ∈ 今日推进列表（TaskNotInToday）∧ 计划进行中（PlanStatusInvalid）∧ 任务未完成（ProgressLocked）
  - `complete_subgoal(conn, clock, subgoal_id) -> TaskStatus` — 按序勾选：必须是最早未完成项（乱序/重复 SubGoalOutOfOrder），落 + 分钟账（source=SubGoal），`settle_task` 首次推进转 Active + 到 100% 自动完成
  - `undo_subgoal(conn, clock, subgoal_id)` — 撤销：必须已完成（SubGoalNotCompleted）且是最晚已完成项（SubGoalOutOfOrder，保住"已完成是前缀"不变式——编辑态 update_subgoals 依赖它），落 - 分钟补偿账（source=SubGoalUndo）；已完成任务锁定（ProgressLocked）
  - `report_percent(conn, clock, task_id, percent) -> TaskStatus` — 无子目标任务增量汇报：任意正数、最小 0.1%、最多一位小数、上限 100%（PercentInvalid；2026-08-24 验收修订砍掉原"5 倍数"颗粒度）、累计不超 100%（PercentOverflow）、有子目标拒绝（NotPercentTask）；落 + 分钟账（source=Percent，原始精度存 ProgressLog）
  - `correct_total(conn, clock, task_id, percent) -> TaskStatus` — 修正总进度（直接设定 0–100 任意正数、最多一位小数——同汇报颗粒度 2026-08-24 修订）：差额（可正可负）以一条事件落账（source=Correction）；仅无子目标任务、已完成锁定；无计划状态门槛（暂停计划的历史也可对账）；归零修正不产生"首次推进"语义
  - 表：`progress_log`（task_id / at / delta_minutes REAL / source）追加式账本 + `current_task`（单行 task_id）——schema 在 `infra/db.rs`，两张都 CREATE IF NOT EXISTS（旧开发库自动补表）
  - `MiniBoardView` / `CurrentTaskView`（has_subgoals / percent / subgoals…）/ `PickerGroup` / `PickerTask` — serde 结构与前端 `src/lib/api.ts` 类型一一对应
- `settings::SettingsService`（工单 13 起：SettingsEffectiveTime 版本历史 + DateOverride）
  - `load(conn) -> Settings` — 读取**最新保存值**（设置页展示口径；均分窗口即生效值）；FirstRun 自动落默认值（300 分钟/天、周一至五、均分 7 工作日；窗口默认 09:00–18:00 一段），并保证版本历史至少一条"生效日极早"的种子版本（新库初始化与旧开发库升级共用）
  - `save(conn, clock, &Settings) -> Result<NaiveDate, PlanError>` — 覆盖保存（updated_at 取注入时钟），返回延时字段生效日：每日工作时间/每周工作日/时间窗口相对**今天生效的配置**有变化时追加一条 `settings_versions`（effective_from = 按保存前配置"今天之后的第一个工作日"——当日维持原值），无变化不追加版本；均分窗口与日期例外**立即生效**。三条写（设置行/版本/例外替换）一个事务（中途失败不留下半份保存）；校验先于变更（InvalidSettings：窗口起止相同〔会把跨午夜判定变成无限窗〕/ 均分窗口或每日工作时间 < 1 / 例外日期不是将来——"提前标注、将来生效"，不提供对当日与历史的追溯改写）
  - `SettingsService::calendar(conn) -> SettingsCalendar` — 按日解析器（一次性装载最新值 + 版本历史，免逐日查询）；ledger 逐日回放 / summary 逐日触发 / is_work_time 共用
  - `SettingsCalendar::for_date(date) -> Settings` — 某日期的生效配置：三个延时字段取 effective_from <= date 的最新版本（历史日与当日不被未来的新配置追溯改写），均分窗口与日期例外取全局最新值
  - `Settings::is_workday_on(date) -> bool` — 纯判定：日期例外命中优先（DateOverride 双向覆盖），否则周循环
  - `SettingsService::is_workday(conn, clock) -> bool` / `is_workday_on(conn, date) -> bool` — "现在"/指定日期的工作日判定（按日解析）；allocation 的面板 workday 标记 / should_auto_open / 小看板加班态共用
  - `is_work_time(conn, clock) -> bool` — 现在是否处于工作时间（端点左闭右开）；跨午夜窗口按 story 54 归属窗口开始日：今晚段按**今天**的配置判、凌晨段（t < end）按**昨天**的配置判（窗口开始日拥有整段窗口）。桌宠初始模式判定（08）与 13 的窗口触发共用
  - `next_window_start(conn, clock) -> Option<DateTime<Local>>` — 下一个工作窗口**开始**时刻（多段窗口取当日最早的未来段；跨午夜的凌晨尾巴不算新开始；None = 未配置窗口或一年内无工作日）——AutoOpenMainBoard 的"工作窗口开始时"触发排程（13）
  - `merge_time_windows(&[TimeWindow]) -> Result<Vec<TimeWindow>, PlanError>` — pub(crate) 时间窗口归一化（2026-08-30 用户反馈）：保存时合并重叠或**首尾相接**的段（跨午夜窗口拆线性段参与合并再穿午夜重组，按 start 升序）；并集覆盖全天返回 InvalidSettings（TimeWindow 无法表达 start==end，用户决策拒绝保存而非改语义）。save 落库前调用，设置行与版本行都写合并结果
  - `updated_at(conn) -> Option<DateTime<Local>>` — 最近保存时刻
- `settings::Settings / TimeWindow / DateOverride` — serde 结构，与前端 `src/lib/api.ts` 类型一一对应（DateOverride {date: NaiveDate〔serde = "YYYY-MM-DD" 字符串〕, working}：这天工作=调休 / 这天不工作=假期；独立表 date_overrides、全量替换——例外天然锚定具体日期，不进版本历史）
- `ledger::LedgerService`（工单 10；ADR-0007 对称均分、ADR-0009 单一事实源，术语 WorkHourLedger / DailyCompletionTolerance / LoadSmoothing；工单 13 起逐日按生效配置回放——基准与"是否工作日"取该日期的解析结果，历史与当日不被新配置改写）
  - `day_target(conn, clock) -> Result<DayTarget, PlanError>` — 注入时钟"今天"的**调整后目标**（DayTarget { base_minutes, target_minutes: f64 }）：从账户锚定日（最早一条进度事件的归属日；之前的日子无从谈起，之后的空闲工作日照样欠全额——债跟着走）逐日回放到昨天，滚动窗口（定长 smoothing_workdays 的 VecDeque，O(天数)）摊派各日差额到后续工作日。规则：±10% 容差带**以调整后目标为基数、对称豁免带内差额**（差几分钟的未达标与多干几分钟的超额都不入账）；带外差额 = 目标 - 实际（正上调/负下调，对称抵扣，量按 ADR"5.0+0.6"示例从目标量起算）；**只读 ProgressLog**（选择层面超额不计）；非工作日不消费窗口、不接收结转、推进全额按超额并入；历史修正后再次调用即重算（无日结冻结）。allocation / progress 两视图装配共用；11 的今日总结"目标 Y 小时"复用
  - `ledger::minutes_by_day(conn, cal) -> Result<BTreeMap<NaiveDate, f64>, PlanError>` — pub(crate) 全量扫 progress_log 按事件归属日聚合每日完成分钟数（`attributed_date` 口径；归属窗口按事件所在日与其前一日的生效配置解析——工单 13 后历史事件的跨午夜归属仍由当时的窗口决定）；ledger 回放与 progress 今日完成量共用同一聚合，不建汇总表
  - `day_target_on(conn, date) -> Result<DayTarget, PlanError>`（工单 11 拆出）— 指定日期的调整后目标（按**总结归属日**取口径——补登昨日时目标仍是"那天该完成多少"）；`day_target(conn, clock)` = `day_target_on(clock 今天)`，回放主体共用一份
- `summary::SummaryService`（工单 11；ADR-0009 单一事实源，术语 DailySummaryTrigger / DailySummaryLayout）
  - `due(conn, clock) -> Result<Option<NaiveDate>, PlanError>` — DailySummaryTrigger 判定：今天的最晚窗口结束已过而未登记 → 补弹"今日总结"；否则昨天已过而未登记 → 补登"昨日总结"（次日首开）。只看今天与昨天两天（更早不补登——隔夜旧账只添噪声）；只弹一次由 `summary_shown` 登记表判定；逐日按生效配置取窗口（工单 13——设置变更不追溯改写当日/历史触发点）
  - `next_fire(conn, clock) -> Result<Option<DateTime<Local>>, PlanError>` — 下一次自动触发时刻（今天向前找第一个"最晚窗口结束 > 现在"的工作日；None = 未配置窗口）。前端 PetWindow 定时器据此排程，触发后重查重排、设置保存（settings:changed）后重排
  - `status(conn, clock) -> Result<DailySummaryStatus, PlanError>` — due + last_shown + next_fire_at 一次查全；一个 command 喂三处（启动补登检查 / 前端定时器 / 控制面板"调出总结"入口的默认日期）
  - `mark_shown(conn, clock, date)` — 登记某日总结已弹出（幂等 UPSERT）；自动弹出与控制面板补看待弹总结两条路都走它
  - `summary(conn, clock, date) -> Result<DailySummaryView, PlanError>` — 总结装配（每次调用从 ProgressLog 重算，无任何固化总结表）：当日净推进按任务聚合（JOIN tasks **不过滤 deleted_at**——已删任务的推进计入计划与总量，只是渲染不出任务行；归档不是抹账）→ 计划净额 > 阈值才展示（当日零推进不展示）→ 复用 `PlanService::list` 的 PlanOrdering 过滤出展示计划（优先级降序）+ 每计划子目标快照随 TaskView 带出 → total = 当日全部事件净额（与工时账户达标判定同口径）→ 目标 = `LedgerService::day_target_on(date)` 含结转 → `higher_priority_hint` = ∃ NotStarted 计划优先级**严格高于**当日推进过的最高优先级（零推进日 = 任何 NotStarted 都算）。`preempted` 标注 = Paused + PauseReason::AutoPreempted（12 接入前测试用 force_pause_reason 种子）
  - `summary::parse_date(s) -> Result<NaiveDate, PlanError>` — YYYY-MM-DD 边界解析（command 薄代理用，InvalidDate 通道）
  - `latest_window_end(settings, date)`（私有）— 工作日 D 的触发时刻 = 当日最晚窗口的结束；跨午夜窗口（end < start）结束在**次日**（偏移 1440 + end 分钟）；settings 为该日期的生效配置（调用方按日解析后传入）；due / next_fire 共用
  - 表：`summary_shown`（date PRIMARY KEY / shown_at）——CREATE IF NOT EXISTS（旧开发库自动补表）
- `app_state::app_state_view(conn, clock) -> AppStateView` — 启动快照组装（示例接缝读模型）

## Tauri command（src/commands.rs）

- `get_app_state` — 返回 `AppStateView`（设置 + 服务端时间）；前端包装 `src/lib/api.ts#getAppState`
- `create_plan(new: PlanDraft) -> Result<i64, PlanError>` — 前端 `createPlan`（子目标/依赖随草稿一并落库）
- `list_plans() -> Vec<PlanView>` — 前端 `listPlans`
- `get_plan(plan_id) -> Result<PlanView, PlanError>` — 前端 `getPlan`（工单 03 详情页）
- `update_plan(plan_id, draft: PlanDraft) -> Result<(), PlanError>` — 前端 `updatePlan`（编辑态保存）
- `delete_task(task_id) -> Result<(), PlanError>` — 前端 `deleteTask`（软删除 + 解除依赖，确认弹窗在 UI）
- 工单 05 生命周期：`start_plan` / `pause_plan` / `resume_plan` / `complete_plan` / `abort_plan` / `copy_plan_as_new(->新计划id)` — 前端 `src/lib/api.ts` 同名包装；状态机与校验全在 domain::lifecycle（原 `set_plan_order` 已随手动排序砍掉移除，2026-08-24）
- 工单 06 今日分配：`get_allocation_board(-> AllocationBoardView)` / `commit_today_allocation(task_ids)` — 前端 `src/lib/api.ts` 同名包装；装配与校验全在 domain::allocation
- 工单 07 进度汇报：`get_mini_board(-> MiniBoardView)` / `set_current_task(task_id)` / `complete_subgoal(subgoal_id)` / `undo_subgoal(subgoal_id)` / `report_percent(task_id, percent)` / `correct_total_progress(task_id, percent)` — 前端 `src/lib/api.ts` 同名包装；账本与派生全在 domain::progress；`lib.rs` 启动时**有有效当前任务且此刻是工作时间**（= 初始工作模式，`SettingsService::is_work_time`）才显示小看板——休息时段启动进休息模式不亮（2026-08-29 反馈，显隐跟着模式走）、`position_pet_and_board` 把「桌宠+小看板」组合体锚到工作区右下角（小看板在桌宠**下方**、右对齐 40px 边距，桌宠居其上方水平居中——2026-08-24 验收要求；拖拽刚性跟随与模式显隐归 09）。工单 10 无新 command：两视图的 target_minutes 升级为含结转 f64 + base_minutes（+ MiniBoardView.workday），前端同步
- 工单 08 桌宠：`should_auto_open_main_board(work_mode, manual) -> bool` — AutoOpenMainBoard 判定的触发接线（06 预留；启动序列完成后传 manual=false、手动切入工作模式传 true=主动加班）；`is_work_time() -> bool` — 桌宠初始模式判定（启动时工作日+窗口内 = 工作模式，否则休息）；`exit_app()` — 再见/托盘退出的统一通道（跳箱动画播完后由前端调用，`app.exit(0)` 关闭全部窗口）。窗口配置新增 `pet-menu`（无边框透明置顶小窗，桌宠菜单）；`control-panel` 改为启动隐藏（spec「启动序列播完直接上桌面」，入口 = 桌宠菜单/托盘）；capabilities 补 `core:window:allow-current-monitor` / `allow-scale-factor`（菜单定位与 mover 缓存用）
- 工单 11 今日总结：`get_daily_summary(date -> DailySummaryView)` / `get_daily_summary_status(-> DailySummaryStatus)` / `mark_daily_summary_shown(date)` — 前端 `src/lib/api.ts` 同名包装；触发判定与装配全在 domain::summary。窗口配置新增 `daily-summary`（带边框、启动隐藏，弹出方先 emit `daily-summary:show` 带 date 再 show——与前两个面板同款重开语义）；capabilities 的 windows 列表同步补 `daily-summary`
- 工单 13 设置：`save_settings(settings -> 生效日 YYYY-MM-DD)` / `get_next_window_start(-> Option<DateTime<Local>>)` — 前端 `src/lib/api.ts` 同名包装；生效时机（版本历史）与校验全在 domain::settings。schema 新增 `settings_versions`（版本历史）与 `date_overrides`（日期例外）两表，CREATE IF NOT EXISTS + load 懒种子（旧开发库自动补表补种子）
- 工单 14 托盘与退出语义：无新 command。`setup_tray`（lib.rs setup 末尾装配，Cargo `tray-icon` feature）——图标 = 打包默认图标 + tooltip「任务帮手」，`show_menu_on_left_click(false)` 左键让给开面板（`on_tray_icon_event` 左键**抬起** = `reveal_control_panel`），右键菜单「打开控制面板」/「退出」（`on_menu_event` 按 id 分发）；`reveal_control_panel` = unminimize→show→set_focus（与前端 `revealWindow` 同序，Windows 上 show 对最小化窗口无效）；退出不直接退进程——`request_exit` 只向 pet 发 `pet:tray-exit`（与 `src/lib/pet/menu.ts` TRAY_EXIT_EVENT 跨端互指），跳箱动画播完由前端走 `exit_app` 统一退出，两条退出路径（托盘/桌宠菜单「再见」）的等价性由同一通道保证

## 测试先例（tests/）

- `seam_settings.rs` — FirstRun 默认值 / 保存往返（含生效日返回）/ 快照组装；工单 13：延时字段下一个工作日生效（周一改每日工作时间 → 当日解析仍 300、周二起 400）、均分窗口与例外立即生效（不追加版本）、日期例外双向覆盖周循环、非法设置全拒且库不变（窗口起止相同/均分 0/每日 0）、next_window_start（窗前/段间/段内/周五晚跳周末/例外标休顺延/未配置窗口 None）、保存时窗口合并（重叠/首尾相接/跨午夜相接各归一段、不相接保持原序、并集覆盖全天拒绝且库不变——2026-08-30 验收反馈）、is_work_time 按日生效配置判定（周一改窗口当日仍按旧窗、周二按新窗）
- `seam_plans.rs` — 创建校验（空任务/缺耗时/空名/子目标必拒）、持久化字段回读、PlanOrdering 排序、文件库重开不丢；工单 03：get/NotFound、编辑字段落库、已开始锁优先级、已完成任务锁定、任务集一致、重排未完成在前已完成沉底、追加任务落位、软删除归档
- `seam_subgoals.rs`（工单 04）— 子目标：填写顺序落库/estimated=求和、行内容与耗时必填、编辑改/增/删未完成行且新行沉最后、已完成子目标锁定（改名/删除/随清空消失都拒）、取消勾选清空、进度按已完成分钟缩放（60%→42.86%→37.5%→75%）；依赖：下标引用落库与等待判定随完成变化、环/自指/越界拒绝、link 跨计划与长环拒绝、编辑全量替换边（含新任务解析）、删任务双向解除后继解锁
- `seam_lifecycle.rs`（工单 05）— 状态机 5×4 全矩阵（合法转换落库、非法拒绝 PlanStatusInvalid 且状态不变、终态重启全拒）、不存在 id 全 NotFound、手动暂停记 UserInitiated/继续清空、完成计划前置（有未完成任务/删空拒 TasksNotCompleted，全完成后成功且终态）、任务 100% 自动完成（部分勾选不完成、全勾转已完成、幂等、无子目标不误判）、复制并新建（字段/任务/子目标/依赖边复制、进度归零、"- 副本"、直接进行中、旧计划保持终态、新计划优先级锁定、非终态拒 PlanNotTerminal）；手动排序测试已随功能砍掉移除（2026-08-24，默认排序断言在 seam_plans）
- `seam_allocation.rs`（工单 06）— 大面板分组与依赖过滤（进行中计划才进候选、高优先在前、被阻塞任务不进列表且解锁当日回归、已完成任务剔除、子目标任务耗时=求和）、提交覆盖往返（再提交整行覆盖、被拒提交不污染已有分配）、不可选拒绝（被阻塞/已完成/非进行中/不存在 id 全 TaskNotAllocatable）、自动打开三条件（休息模式/周日/已分配都不触发、齐备才 true、workday 标记随周循环翻转）、隔日失效（8/25 回显空、判定回 true）
- `seam_progress.rs`（工单 07）— 增量汇报与日志派生（多次 +X% 进度=求和/首报转 Active/到 100% 自动完成并锁定/停留态仍展示）、颗粒度与溢出（非 5 倍数 PercentInvalid、95%+10% 溢出拒绝、+5% 恰好补满）、**边界 round 消 IEEE 754 浮点尾巴**（反复 +0.1% 累计 current_percent = 干净 43.1 而非 43.0999999...；ProgressLog sum 接近 258.6 干净；TaskProgress::percent() 原始精度 60/140 = 42.857142857142854 不变——领域语义保护）、子目标按序（跳序/重复 SubGoalOutOfOrder、百分比通道 NotPercentTask、全勾自动完成）、撤销重算（非末尾撤销拒绝保前缀、末尾撤销今日净量实时重算、已完成任务锁定）、修正落账（设定绝对值、差额正负事件、来源序列断言、有子目标拒绝、100% 自动完成）、当前任务生命周期（不在今日列表拒/暂停回空态恢复回来/移出列表回空态/隔日失效/同计划候选排最前）、跨午夜归属（20:00–01:00 窗口凌晨段归前一日、窗口外归自身日期）、归属看窗口开始日的配置（工单 13：跨午夜窗口"一直如此"，周一深夜改窗口周二生效后，周二凌晨的事件仍归周一——两集合归属判定的存在证明）
- `seam_ledger.rs`（工单 10；工单 13 补例外回归）— 工时账户结算：空历史=基准（大/小面板视图字段同步）、缺口按 7 工作日窗口均分（ADR"目标 5.6h（基准 5.0h + 结转 0.6h）"示例 + 第 8 工作日窗口耗尽）、±10% 容差带（4.5h 恰达标/4h 带外、基数=**调整后**目标——被上调日 300min 仍判未达标再均分；空闲工作日照样欠整日）、超额对称抵扣（384min 下调后续、324min 带内不抵扣）、多日差额滚动叠加、递归再均分（连续欠债日债滚债）、周末不消费窗口但加班推进按超额入账（跨周末逐工作日落账）、历史修正实时重算（correct_total / undo_subgoal 改口昨日→今日目标随之变）、"选 14h 推 5h"选择层面超额不计。注意：验证窗口后段时中间工作日要用**带内汇报**中和（否则空闲日的递归欠债会叠上来——那是 respread 测试的职责）；修正/撤销事件用注入时钟落在受修正日。例外回归（工单 13）：工作日被标休 → 该日推进全额按超额入账且不占窗口位（顺延到周三起均分）、休息日被标工（补班）→ 自身有目标义务并计入均分窗口（与无例外的超额抵扣成对照）
- `seam_summary.rs`（工单 11）— 触发时刻（两段窗口只认**最晚**段结束 18:00、12:00 与 17:59 都不触发、18:00 整触发——端点左闭右开的"外"侧；登记后不再触发=只弹一次）、次日补登（周一该弹未弹 → 周二首开 due=周一且 is_yesterday；基线始终是"昨天"，与星期无关——周二未登记则周三补登周二）、跨午夜窗口（20:00–01:00 触发在**次日** 01:00；周一 23:00 + 周二 00:30 两笔都归周一日账、周二总结为 0）、非工作日（周六是周五的"次日"——周五该弹未弹则周六首开补登；登记后周六自身无触发点；next_fire 跳周末落周一 18:00）、next_fire 口径（多段取最晚、已过落明天、跨午夜落次日 01:00）、三层装配（优先级降序、零推进计划缺席、子目标快照、一位小数百分比、总览 90/300 无结转）、被抢占暂停照常展示并计入总量（AutoPreempted 标注、UserInitiated 不标）、更高优先级提示（严格高于——推 Medium 有 High 未开始才提示、零推进日任何 NotStarted 都提示）、目标含结转（周二 = 300 + 150/7；周一自己的总结目标仍 300——当日缺口如实呈现为未达标）、已删任务的推进计入计划与总量（无任务行）、status 三字段、parse_date 拒垃圾、设置变更不追改当日触发（工单 13：周一 10:00 把窗口改到 12:00 结束〔周二生效〕→ 当日 next_fire 仍 18:00、周二起 12:00）、例外改触发点（周一标休无触发点、周六标工有触发点）。注意：种子窗口走 `force_settings`（save 的延时字段会把当日留在旧配置——工单 13 起）
- 直接 `db::open_in_memory()` + `FixedClock` 驱动领域服务，断言可观察输出
- 共用测试助手在 `tests/common/mod.rs`（`at` / `plain_task` / `draft_of` / `force_plan_status` / `force_task_status` / `force_subgoal_completed` / `force_pause_reason` / `force_settings`）——各 seam 二进制 `mod common; use common::*;` 引入，不再逐文件拷贝
- `force_settings(conn, &Settings)`（工单 13）— 把设置固定为"自从有设置以来一直如此"：直改存储（设置行 UPDATE + 例外全量替换 + 版本历史重置为生效日极早的种子）。`SettingsService::save` 的延时字段自下一个工作日生效，会把当日判定留在旧值——验收种子要的是"配置早已稳定"这条生产前提，必须走它而不是 save
- 前置状态用 SQL 直改（`force_task_status`/`force_plan_status`/`force_subgoal_completed`/`force_pause_reason`）：汇报与状态机接线前的种子；pause_reason 种子供工单 11 总结的"已被抢占暂停"标注与工单 12 抢占路径
- 每个测试注明：测试什么情况、什么结果才算正确（仓库规范）
