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
  - `list(conn) -> Result<Vec<PlanView>, PlanError>` — 全部计划+嵌套任务（软删除过滤），排序（工单 05 PlanOrdering）：手动序优先（sort_override 非 NULL 按 override 升序在前，手动调整过即接管全局）、其后按默认规则 Priority 降序 → CreatedAt 倒序 → id 倒序；错误统一走 PlanError 通道
  - `set_order(conn, &ordered_ids) -> Result<(), PlanError>`（工单 05）— 手动排序持久化：先全部置 NULL 再按传入顺序写 0..n；前端传全部计划的完整顺序（过滤视图内拖拽由前端并回全量序），一次拖拽即接管全局排序
  - `task_progress(conn, task_id) -> Result<TaskProgress, PlanError>`（工单 04）— 派生进度（ADR-0002）：(已完成分钟, 总分钟)，`percent()` 供展示/断言；改耗时/增删未完成子目标后自动缩放（分子不变分母变）；无子目标任务分子 07 接 ProgressLog
  - `PlanDraft` / `TaskDraft`（含 `subgoals`、`depends_on` 草稿下标引用）/ `SubGoalDraft` — 创建与编辑共用草稿（id 为 None = 新行）
  - `PlanError` — 结构化领域错误（serde tag=kind/content=payload），前端文案见 `src/lib/labels.ts#planErrorMessage`；工单 05 新增 PlanStatusInvalid{from} / PlanNotTerminal{from} / TasksNotCompleted
  - `Priority` / `PlanStatus` / `TaskStatus` — 枚举 + `as_db`/`from_db` TEXT 往返；Priority 派生 Ord（Low < Medium < High）
- `lifecycle::LifecycleService`（工单 05；ADR-0001 单向瀑布）
  - `start(conn, plan_id)` — 未开始 → 进行中
  - `pause(conn, plan_id)` — 进行中 → 已暂停，pause_reason = PauseReason::UserInitiated（CONTEXT PauseReason 二值枚举，as_db/from_db 口径同 PlanStatus；AutoPreempted 由工单 12 抢占写入）
  - `resume(conn, plan_id)` — 已暂停 → 进行中，清空 pause_reason
  - `complete(conn, plan_id)` — 进行中 → 已完成（PlanCompletionConfirm 手动确认）；要求全部任务已完成且 ≥1 个任务（删空计划只能放弃），否则 TasksNotCompleted
  - `abort(conn, plan_id)` — 进行中/已暂停 → 已放弃（终态；二级确认在 UI）
  - `sync_task_completion(conn, task_id) -> Result<TaskStatus, PlanError>` — 任务进度到 100% 自动转已完成（幂等）；07 的汇报路径（子目标勾选/百分比汇报）接线调用
  - `copy_as_new(conn, clock, plan_id) -> Result<i64, PlanError>` — 终态计划复制并新建（CopyAsNewPlan）：复制字段/任务/子目标/依赖边、进度归零、名称加"- 副本"、直接进行中；非终态拒绝 PlanNotTerminal
  - 抢占不变式（开始/恢复时自动暂停低等级计划）在工单 12 接入本服务各入口
- `deps::DependencyService`（工单 04）
  - `link(conn, predecessor_id, successor_id) -> Result<(), PlanError>` — 建一条 A→B 边：校验两端存在未删、同计划（DependencyCrossPlan）、不自指、无环（reaches 可达检测，环返回 DependencyCycle）；可事务内调用；草稿路径由 `link_draft_deps` 批量走它
  - `detach_task(conn, task_id)` — 删除任务时双向解除其边
  - `waiting_on(conn, task_id) -> Vec<TaskView>` — 前置中未完成任务列表（大面板"等待：任务A"数据源；06 消费）
  - `is_unblocked(conn, task_id) -> bool` — 依赖就绪判定（06"是否可选入今日分配"的依赖侧）= waiting_on 为空
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
- 工单 05 生命周期：`start_plan` / `pause_plan` / `resume_plan` / `complete_plan` / `abort_plan` / `copy_plan_as_new(->新计划id)` / `set_plan_order(ordered_ids)` — 前端 `src/lib/api.ts` 同名包装；状态机与校验全在 domain::lifecycle

## 测试先例（tests/）

- `seam_settings.rs` — FirstRun 默认值 / 保存往返 / 快照组装
- `seam_plans.rs` — 创建校验（空任务/缺耗时/空名/子目标必拒）、持久化字段回读、PlanOrdering 排序、文件库重开不丢；工单 03：get/NotFound、编辑字段落库、已开始锁优先级、已完成任务锁定、任务集一致、重排未完成在前已完成沉底、追加任务落位、软删除归档
- `seam_subgoals.rs`（工单 04）— 子目标：填写顺序落库/estimated=求和、行内容与耗时必填、编辑改/增/删未完成行且新行沉最后、已完成子目标锁定（改名/删除/随清空消失都拒）、取消勾选清空、进度按已完成分钟缩放（60%→42.86%→37.5%→75%）；依赖：下标引用落库与等待判定随完成变化、环/自指/越界拒绝、link 跨计划与长环拒绝、编辑全量替换边（含新任务解析）、删任务双向解除后继解锁
- `seam_lifecycle.rs`（工单 05）— 状态机 5×4 全矩阵（合法转换落库、非法拒绝 PlanStatusInvalid 且状态不变、终态重启全拒）、不存在 id 全 NotFound、手动暂停记 UserInitiated/继续清空、完成计划前置（有未完成任务/删空拒 TasksNotCompleted，全完成后成功且终态）、任务 100% 自动完成（部分勾选不完成、全勾转已完成、幂等、无子目标不误判）、复制并新建（字段/任务/子目标/依赖边复制、进度归零、"- 副本"、直接进行中、旧计划保持终态、新计划优先级锁定、非终态拒 PlanNotTerminal）、手动排序（倒序接管默认序、新建计划落其后、重排覆盖、重开库不丢）
- 直接 `db::open_in_memory()` + `FixedClock` 驱动领域服务，断言可观察输出
- 共用测试助手在 `tests/common/mod.rs`（`at` / `plain_task` / `draft_of` / `force_plan_status` / `force_task_status` / `force_subgoal_completed`）——各 seam 二进制 `mod common; use common::*;` 引入，不再逐文件拷贝
- 前置状态用 SQL 直改（`force_task_status`/`force_plan_status`/`force_subgoal_completed`）：汇报与状态机接线前的种子
- 每个测试注明：测试什么情况、什么结果才算正确（仓库规范）
