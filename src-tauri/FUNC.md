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

- `db::open(&Path) -> Connection` — 打开/建库 + WAL + 建表（schema 见 `SCHEMA`）
- `db::open_in_memory() -> Connection` — 集成测试用内存库

## 领域服务（src/domain/）

- `plans::PlanService`（工单 02/03）
  - `create(conn, clock, &PlanDraft) -> Result<i64, PlanError>` — 事务写入计划+任务；简述留空回退名称（`default_text`）；校验：名称非空、≥1 任务、无子目标任务必填耗时、勾子目标须有子目标（04 前必拒）
  - `update(conn, clock, plan_id, &PlanDraft) -> Result<(), PlanError>`（工单 03）— 整计划编辑（CreationUI 编辑态）。不变量：计划开始后优先级锁定（PriorityLocked）；已完成任务内容锁定（TaskLocked，position 除外）；提交任务集必须与库一致（TaskSetMismatch，删除显式走 delete_task）；落库顺序 = 未完成按提交序在前、已完成按库内序沉底（新任务自然落在所有未完成之后）
  - `get(conn, plan_id) -> Result<PlanView, PlanError>` — 单计划详情（含任务按 position）；不存在 NotFound
  - `delete_task(conn, clock, task_id) -> Result<(), PlanError>` — 软删除进归档（deleted_at 落删除时刻）；不存在/已删 NotFound
  - `list(conn) -> Result<Vec<PlanView>, PlanError>` — 全部计划+嵌套任务（软删除过滤），PlanOrdering 默认排序：Priority 降序 → CreatedAt 倒序 → id 倒序；错误统一走 PlanError 通道
  - `PlanDraft` / `TaskDraft` — 创建与编辑共用草稿（TaskDraft.id 为 None = 新任务）
  - `PlanError` — 结构化领域错误（serde tag=kind/content=payload），前端文案见 `src/lib/labels.ts#planErrorMessage`
  - `Priority` / `PlanStatus` / `TaskStatus` — 枚举 + `as_db`/`from_db` TEXT 往返；Priority 派生 Ord（Low < Medium < High）
- `settings::SettingsService`
  - `load(conn) -> Settings` — 读取设置；FirstRun 自动落默认值（300 分钟/天、周一至五、均分 7 工作日；窗口默认 09:00–18:00 一段）
  - `save(conn, clock, &Settings)` — 覆盖保存，updated_at 取注入时钟
  - `updated_at(conn) -> Option<DateTime<Local>>` — 最近保存时刻（SettingsEffectiveTime 依赖）
- `settings::Settings / TimeWindow` — serde 结构，与前端 `src/lib/api.ts` 类型一一对应
- `app_state::app_state_view(conn, clock) -> AppStateView` — 启动快照组装（示例接缝读模型）

## Tauri command（src/commands.rs）

- `get_app_state` — 返回 `AppStateView`（设置 + 服务端时间）；前端包装 `src/lib/api.ts#getAppState`
- `create_plan(new: PlanDraft) -> Result<i64, PlanError>` — 前端 `createPlan`
- `list_plans() -> Vec<PlanView>` — 前端 `listPlans`
- `get_plan(plan_id) -> Result<PlanView, PlanError>` — 前端 `getPlan`（工单 03 详情页）
- `update_plan(plan_id, draft: PlanDraft) -> Result<(), PlanError>` — 前端 `updatePlan`（编辑态保存）
- `delete_task(task_id) -> Result<(), PlanError>` — 前端 `deleteTask`（软删除，确认弹窗在 UI）

## 测试先例（tests/）

- `seam_settings.rs` — FirstRun 默认值 / 保存往返 / 快照组装
- `seam_plans.rs` — 创建校验（空任务/缺耗时/空名/子目标必拒）、持久化字段回读、PlanOrdering 排序、文件库重开不丢；工单 03：get/NotFound、编辑字段落库、已开始锁优先级、已完成任务锁定、任务集一致、重排未完成在前已完成沉底、追加任务落位、软删除归档
- 直接 `db::open_in_memory()` + `FixedClock` 驱动领域服务，断言可观察输出
- 前置状态用 SQL 直改（`force_task_status`/`force_plan_status`）：汇报与状态机接线前的种子
- 每个测试注明：测试什么情况、什么结果才算正确（仓库规范）
