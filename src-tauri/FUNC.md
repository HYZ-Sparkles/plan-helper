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
  - `task_progress(conn, task_id) -> Result<TaskProgress, PlanError>`（工单 04/07）— 派生进度（ADR-0002）：(已完成分钟: **f64**, 总分钟)，`percent()` 供展示、`is_complete()`（epsilon 容差）供自动完成判定；改耗时/增删未完成子目标后自动缩放（分子不变分母变）；有子目标 = 已完成子目标求和，无子目标 = progress_log 增量求和
  - `TaskView.progress_percent` — 派生进度百分比（load_tasks 装配，列表/详情/小看板候选统一消费）
  - `PlanDraft` / `TaskDraft`（含 `subgoals`、`depends_on` 草稿下标引用）/ `SubGoalDraft` — 创建与编辑共用草稿（id 为 None = 新行）
  - `PlanError` — 结构化领域错误（serde tag=kind/content=payload），前端文案见 `src/lib/labels.ts#planErrorMessage`；工单 05 新增 PlanStatusInvalid{from} / PlanNotTerminal{from} / TasksNotCompleted，工单 06 新增 TaskNotAllocatable{task_id}，工单 07 新增 TaskNotInToday{task_id} / SubGoalOutOfOrder / SubGoalNotCompleted / ProgressLocked / PercentInvalid / PercentOverflow / NotPercentTask
  - `Priority` / `PlanStatus` / `TaskStatus` — 枚举 + `as_db`/`from_db` TEXT 往返；Priority 派生 Ord（Low < Medium < High）
- `lifecycle::LifecycleService`（工单 05；ADR-0001 单向瀑布）
  - `start(conn, plan_id)` — 未开始 → 进行中
  - `pause(conn, plan_id)` — 进行中 → 已暂停，pause_reason = PauseReason::UserInitiated（CONTEXT PauseReason 二值枚举，as_db/from_db 口径同 PlanStatus；AutoPreempted 由工单 12 抢占写入）
  - `resume(conn, plan_id)` — 已暂停 → 进行中，清空 pause_reason
  - `complete(conn, plan_id)` — 进行中 → 已完成（PlanCompletionConfirm 手动确认）；要求全部任务已完成且 ≥1 个任务（删空计划只能放弃），否则 TasksNotCompleted
  - `abort(conn, plan_id)` — 进行中/已暂停 → 已放弃（终态；二级确认在 UI）
  - `sync_task_completion(conn, task_id) -> Result<TaskStatus, PlanError>` — 任务进度到 100% 自动转已完成（幂等；用 `TaskProgress::is_complete` 的 epsilon 判定）；工单 07 的汇报路径已接线（`progress::settle_task` 在每次落账后调用）
  - `copy_as_new(conn, clock, plan_id) -> Result<i64, PlanError>` — 终态计划复制并新建（CopyAsNewPlan）：复制字段/任务/子目标/依赖边、进度归零、名称加"- 副本"、直接进行中；非终态拒绝 PlanNotTerminal
  - 抢占不变式（开始/恢复时自动暂停低等级计划）在工单 12 接入本服务各入口
- `deps::DependencyService`（工单 04）
  - `link(conn, predecessor_id, successor_id) -> Result<(), PlanError>` — 建一条 A→B 边：校验两端存在未删、同计划（DependencyCrossPlan）、不自指、无环（reaches 可达检测，环返回 DependencyCycle）；可事务内调用；草稿路径由 `link_draft_deps` 批量走它
  - `detach_task(conn, task_id)` — 删除任务时双向解除其边
  - `waiting_on(conn, task_id) -> Vec<TaskView>` — 前置中未完成任务列表（`is_unblocked` 的数据源）
  - `is_unblocked(conn, task_id) -> bool` — 依赖就绪判定（06 大面板的候选过滤：被阻塞任务不进列表，2026-08-24 修订"只展示可选任务"）= waiting_on 为空
- `allocation::AllocationService`（工单 06；术语 MainBoardTodayAllocation / TodayLoadCommitment / AutoOpenMainBoard）
  - `board(conn, clock) -> Result<AllocationBoardView, PlanError>` — 大面板一次装配：候选分组（复用 `PlanService::list` 的 PlanOrdering；过滤 PlanStatus=进行中、剔除已完成任务、`is_unblocked` 依赖就绪过滤——被阻塞的不进列表）+ 今日回显（库存选中集与当前可选集求交，计划暂停等 stale 选择被剔除）+ 当日目标（暂 = 基准 `daily_minutes`，工单 10 升级为含结转并加标注）+ `workday` 周循环标记（非工作日 UI 走休息日态、分配不加载，CONTEXT WorkingHours）
  - `commit(conn, clock, &[task_id]) -> Result<(), PlanError>` — 提交当日分配，再次提交**整行覆盖**（重开重选）；选中集必须 ⊆ 当前可选集（进行中 + 未完成 + 依赖就绪），否则 `TaskNotAllocatable{task_id}`（UI 失步的后端兜底）。存储为 `today_allocations` 单行表（id=1，date + task_ids JSON）——分配只属于它的日期，隔日读视为未分配；历史分配无消费方（总结从 ProgressLog 派生），不按日留行
  - `should_auto_open(conn, clock, work_mode) -> Result<bool>` — AutoOpenMainBoard 判定：工作模式 && 今日在周循环工作日 && 今日未分配。**触发接线留给工单 08**（启动序列后检测 / 切入工作模式）**与 13**（工作窗口开始），工作模式状态在 08 落地
  - `AllocationBoardView`（date / workday / target_minutes / selected_task_ids / groups）、`AllocationGroup`（plan_id / plan_name / priority / tasks）、`AllocationTask`（id / name / estimated_minutes）— serde 结构与前端 `src/lib/api.ts` 类型一一对应
- `progress::ProgressService`（工单 07；ADR-0002/0009，术语 CurrentTask / ProgressGranularity / PercentAdjustControl / SubGoalUndo）
  - `board(conn, clock) -> Result<MiniBoardView, PlanError>` — 小看板一次装配：当前任务（单行表 id 过三重校验：存在未删 ∧ 计划进行中 ∧ 在今日推进列表内；失效回 None、已完成仍展示为"任务完成"停留态）+ 今日完成量（`day_minutes`：全量扫 progress_log 按**事件归属日**求和，`attributed_date` 实现跨午夜窗口归前一日、窗口外归自身日期）+ 当日目标（暂 = 基准，工单 10 升级含结转）+ 更换候选分组（复用 `PlanService::list` 的 PlanOrdering，当前任务同计划提到最前）
  - `set_current_task(conn, clock, task_id)` — 指定当前任务（覆盖式单行表 current_task）；校验 ∈ 今日推进列表（TaskNotInToday）∧ 计划进行中（PlanStatusInvalid）∧ 任务未完成（ProgressLocked）
  - `complete_subgoal(conn, clock, subgoal_id) -> TaskStatus` — 按序勾选：必须是最早未完成项（乱序/重复 SubGoalOutOfOrder），落 + 分钟账（source=SubGoal），`settle_task` 首次推进转 Active + 到 100% 自动完成
  - `undo_subgoal(conn, clock, subgoal_id)` — 撤销：必须已完成（SubGoalNotCompleted）且是最晚已完成项（SubGoalOutOfOrder，保住"已完成是前缀"不变式——编辑态 update_subgoals 依赖它），落 - 分钟补偿账（source=SubGoalUndo）；已完成任务锁定（ProgressLocked）
  - `report_percent(conn, clock, task_id, percent) -> TaskStatus` — 无子目标任务增量汇报：5–100 的 5 倍数（PercentInvalid）、累计不超 100%（PercentOverflow）、有子目标拒绝（NotPercentTask）；落 + 分钟账（source=Percent）
  - `correct_total(conn, clock, task_id, percent) -> TaskStatus` — 修正总进度（直接设定 0–100 的 5 倍数）：差额（可正可负）以一条事件落账（source=Correction）；仅无子目标任务、已完成锁定；无计划状态门槛（暂停计划的历史也可对账）；归零修正不产生"首次推进"语义
  - 表：`progress_log`（task_id / at / delta_minutes REAL / source）追加式账本 + `current_task`（单行 task_id）——schema 在 `infra/db.rs`，两张都 CREATE IF NOT EXISTS（旧开发库自动补表）
  - `MiniBoardView` / `CurrentTaskView`（has_subgoals / percent / subgoals…）/ `PickerGroup` / `PickerTask` — serde 结构与前端 `src/lib/api.ts` 类型一一对应
- `settings::SettingsService`
  - `load(conn) -> Settings` — 读取设置；FirstRun 自动落默认值（300 分钟/天、周一至五、均分 7 工作日；窗口默认 09:00–18:00 一段）
  - `save(conn, clock, &Settings)` — 覆盖保存，updated_at 取注入时钟
  - `updated_at(conn) -> Option<DateTime<Local>>` — 最近保存时刻（SettingsEffectiveTime 依赖）
- `settings::Settings / TimeWindow` — serde 结构，与前端 `src/lib/api.ts` 类型一一对应
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
- 工单 07 进度汇报：`get_mini_board(-> MiniBoardView)` / `set_current_task(task_id)` / `complete_subgoal(subgoal_id)` / `undo_subgoal(subgoal_id)` / `report_percent(task_id, percent)` / `correct_total_progress(task_id, percent)` — 前端 `src/lib/api.ts` 同名包装；账本与派生全在 domain::progress；`lib.rs` 启动时已有有效当前任务则显示小看板、`position_pet_and_board` 把「桌宠+小看板」组合体锚到工作区右下角（小看板在桌宠**下方**、右对齐 40px 边距，桌宠居其上方水平居中——2026-08-24 验收要求；拖拽跟随归 09、模式显隐归 08）

## 测试先例（tests/）

- `seam_settings.rs` — FirstRun 默认值 / 保存往返 / 快照组装
- `seam_plans.rs` — 创建校验（空任务/缺耗时/空名/子目标必拒）、持久化字段回读、PlanOrdering 排序、文件库重开不丢；工单 03：get/NotFound、编辑字段落库、已开始锁优先级、已完成任务锁定、任务集一致、重排未完成在前已完成沉底、追加任务落位、软删除归档
- `seam_subgoals.rs`（工单 04）— 子目标：填写顺序落库/estimated=求和、行内容与耗时必填、编辑改/增/删未完成行且新行沉最后、已完成子目标锁定（改名/删除/随清空消失都拒）、取消勾选清空、进度按已完成分钟缩放（60%→42.86%→37.5%→75%）；依赖：下标引用落库与等待判定随完成变化、环/自指/越界拒绝、link 跨计划与长环拒绝、编辑全量替换边（含新任务解析）、删任务双向解除后继解锁
- `seam_lifecycle.rs`（工单 05）— 状态机 5×4 全矩阵（合法转换落库、非法拒绝 PlanStatusInvalid 且状态不变、终态重启全拒）、不存在 id 全 NotFound、手动暂停记 UserInitiated/继续清空、完成计划前置（有未完成任务/删空拒 TasksNotCompleted，全完成后成功且终态）、任务 100% 自动完成（部分勾选不完成、全勾转已完成、幂等、无子目标不误判）、复制并新建（字段/任务/子目标/依赖边复制、进度归零、"- 副本"、直接进行中、旧计划保持终态、新计划优先级锁定、非终态拒 PlanNotTerminal）；手动排序测试已随功能砍掉移除（2026-08-24，默认排序断言在 seam_plans）
- `seam_allocation.rs`（工单 06）— 大面板分组与依赖过滤（进行中计划才进候选、高优先在前、被阻塞任务不进列表且解锁当日回归、已完成任务剔除、子目标任务耗时=求和）、提交覆盖往返（再提交整行覆盖、被拒提交不污染已有分配）、不可选拒绝（被阻塞/已完成/非进行中/不存在 id 全 TaskNotAllocatable）、自动打开三条件（休息模式/周日/已分配都不触发、齐备才 true、workday 标记随周循环翻转）、隔日失效（8/25 回显空、判定回 true）
- `seam_progress.rs`（工单 07）— 增量汇报与日志派生（多次 +X% 进度=求和/首报转 Active/到 100% 自动完成并锁定/停留态仍展示）、颗粒度与溢出（非 5 倍数 PercentInvalid、95%+10% 溢出拒绝、+5% 恰好补满）、子目标按序（跳序/重复 SubGoalOutOfOrder、百分比通道 NotPercentTask、全勾自动完成）、撤销重算（非末尾撤销拒绝保前缀、末尾撤销今日净量实时重算、已完成任务锁定）、修正落账（设定绝对值、差额正负事件、来源序列断言、有子目标拒绝、100% 自动完成）、当前任务生命周期（不在今日列表拒/暂停回空态恢复回来/移出列表回空态/隔日失效/同计划候选排最前）、跨午夜归属（20:00–01:00 窗口凌晨段归前一日、窗口外归自身日期；注意 `SettingsService::save` 是 UPDATE-only——先 `load` 落默认行再 save，应用启动 get_app_state 已保证）
- 直接 `db::open_in_memory()` + `FixedClock` 驱动领域服务，断言可观察输出
- 共用测试助手在 `tests/common/mod.rs`（`at` / `plain_task` / `draft_of` / `force_plan_status` / `force_task_status` / `force_subgoal_completed`）——各 seam 二进制 `mod common; use common::*;` 引入，不再逐文件拷贝
- 前置状态用 SQL 直改（`force_task_status`/`force_plan_status`/`force_subgoal_completed`）：汇报与状态机接线前的种子
- 每个测试注明：测试什么情况、什么结果才算正确（仓库规范）
