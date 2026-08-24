# 前端复用方法（src/FUNC.md）

新写前端代码前先查这里，能复用就不新写；新增可复用方法后回来登记（AGENTS.md 复用纪律）。

## 类型与后端调用

- `src/lib/api.ts` — 后端 command 的类型化封装。**组件一律通过这里的包装函数调用后端，不直接 `invoke`**。
  - 类型：`Settings`、`TimeWindow`、`AppStateView`（与 Rust serde 结构一一对应，改后端要同步改这里）
  - 计划域类型：`Priority` / `PlanStatus` / `TaskStatus` / `PauseReason` 枚举、`PlanDraft` / `TaskDraft`（创建与编辑共用入参，TaskDraft.id 缺省 = 新任务；`subgoals` 子目标行、`depends_on` 前置任务的草稿下标引用）、`SubGoalDraft`、`PlanView`（含 `subgoals` / `prerequisite_ids` / `pause_reason`）/ `TaskView`（含 `subgoals` / `prerequisite_ids`）/ `SubGoalView`、`PlanErrorShape`
  - `getAppState(): Promise<AppStateView>` — 启动快照（设置 + 服务端时间）
  - `createPlan(draft: PlanDraft): Promise<number>` — 创建计划（含任务/子目标/依赖），失败 reject 结构化错误
  - `listPlans(): Promise<PlanView[]>` — 计划列表（PlanOrdering：手动序优先于默认排序，含嵌套任务）
  - `getPlan(planId: number): Promise<PlanView>` — 单个计划详情（工单 03）
  - `updatePlan(planId, draft): Promise<void>` — 整计划编辑保存（编辑态）
  - `deleteTask(taskId): Promise<void>` — 软删除任务进归档（服务端连带解除依赖边；确认弹窗在 UI）
  - 生命周期（工单 05，状态机在服务层、UI 只按状态展示可得操作）：`startPlan` / `pausePlan` / `resumePlan` / `completePlan` / `abortPlan`（二级确认在 UI）、`copyPlanAsNew(planId): Promise<number>`（返回新计划 id）、`setPlanOrder(orderedIds)`（传全部计划的完整顺序）
- `src/lib/labels.ts` — 领域枚举的中文文案集中地：
  - `priorityLabel` / `statusLabel`（计划与任务状态合一张表，共有值标签一致）/ `pauseReasonLabel`（用户主动 / 自动抢占）映射表
  - `planErrorMessage(err)` — 后端 PlanError（{kind,payload}）→ 用户可读文案
  - `hoursFromMinutes(minutes)` — 分钟 → 小时展示
- `src/lib/progress.ts` — 前端进度派生（ADR-0002，与服务层 `task_progress` 同公式）：
  - `taskProgress(task)` — (已完成分钟, 总分钟)，仅子目标任务有效
  - `taskProgressLabel(task)` — "60%" 百分比文案；无子目标/零总量返回空串。详情页对勾了子目标但总量为 0 的任务以 `|| "0%"` 兜底展示
- `src/lib/validation.ts` — 表单输入校验/归一工具：
  - `normalizeDateString(s)` — 常见日期写法（2026-12-20 / 2026/12/20 / 20261220）归一为 YYYY-MM-DD；非真实日历日期（含 2026-02-30）返回 null。DatePicker 失焦归一用，值要么合法要么回退，调用方免校验。

## 复用组件（src/components/）

- `CreationForm.vue` — **CreationUI 单页可折叠表单（创建/编辑双模式，工单 03/04）**：`mode: "create" | "edit"` + 编辑态 `plan: PlanView`；任务卡默认收起为一行摘要（拖拽手柄 + 名称 + 耗时/子目标计数）、点行展开、HTML5 拖拽排序、已完成任务锁定、优先级开始后锁定、删已入库任务走打字确认；展开态内含**子目标精简输入行**（内容+耗时、行末 + 与回车加行聚焦、已完成行锁定、取消勾选弹确认清空）与**前置任务多选**（DependencyEditor，次要区域）。emit `saved` / `cancel`。
- `DependencyEditor.vue` — 前置任务多选胶囊（v-model = 草稿内稳定 key 集）：候选仅同计划其它任务，创建与编辑同一控件；勾选集即依赖边全集（所见即所存）。
- `TypeConfirmDialog.vue` — 打字确认的危险操作弹窗（默认打「再删」）；父组件 v-if 控制显隐，emit `confirm` / `cancel`。删任务（03）与放弃计划（05）共用。
- `ConfirmDialog.vue` — 轻量二选一确认弹窗（标题 + 插槽内容 + 取消/确认）；子目标删除/取消勾选（04）、生命周期确认（05）等非打字场景共用。
- `DatePicker.vue` — 轻量日期选择（自制月历弹层，零依赖）：v-model 为 `""` 或合法 YYYY-MM-DD（组件自身保证合法性，调用方免校验）；日历点选 + 手打归一（失焦非法回退）。工单 13（设置-日期例外）复用。
- `PriorityLabel.vue` — 优先级标签（文字 + PhCaretUp/PhMinus/PhCaretDown + 标签底色，PriorityVisuals）。全应用优先级展示统一走它。
- `StatusBadge.vue` — 计划/任务状态徽章（中文 + 状态色边框）。
- `CollapsibleSection.vue` — 可折叠区块（title + 可选 badge + defaultOpen），CreationUI 段落容器。
- `AutoTextarea.vue` — 自适应 textarea（min 3 行、8 行起滚动），计划/任务详细内容共用；disabled 经 attr 透传。
- `FormField.vue` — 表单字段外壳（label + required 标记 + error 行）。

## 全局工具类（tokens.css，样式复用优先级：token 变量 > 全局类 > scoped）

- `.hint` / `.page-title` — 次级提示文字 / 页标题。
- `.input` — 单行输入与 textarea 的统一形态（边框/圆角/focus）。
- **按钮体系（工单 05 归一，全应用唯一形态来源）**：`.primary-btn` 主操作、`.ghost-btn` 次级（取消/暂停，与主按钮严格等高 39px）、`.danger-btn` 危险主操作（打字确认弹窗的执行按钮）、`.danger-ghost-btn` 危险次操作（放弃等破坏性入口，hover 红底）、`.icon-btn` 图标按钮（删除类加 `danger` 变体，hover 红）、`.link-btn` 文字链接型轻操作；全部带 `:focus-visible` 键盘聚焦环，原生 `button` 也有兜底聚焦（覆盖 filter/add-task 等特型入口——特型按钮样式可 scoped 自定，但不再自造文字/图标按钮形态）。
- **弹窗骨架**：`.dialog-overlay` / `.dialog` / `.dialog-title` / `.dialog-body` / `.dialog-actions` — ConfirmDialog 与 TypeConfirmDialog 共用（组件只留自己的内容样式）。

## 页面级实现要点（不易从文件名看出）

- `PlanDetailPage.vue` — 生命周期按钮矩阵（未开始→开始；进行中→完成计划〔任务全完成后亮起〕/暂停/放弃；已暂停→继续/放弃；终态→复制并新建）；放弃走一级确认（保留多少历史）+ 二级打字「再删」；`runLifecycle` 统一 busy 互斥与错误文案。
- `PlansPage.vue` — 计划列表指针拖拽排序（半透明 + 3px 主色条、±20px 磁吸、他卡让位）；过滤视图内拖拽由 `persistMove` 并回全量顺序再 `setPlanOrder` 持久化。

## 路由与多窗口

- `src/router.ts` — hash 路由；窗口 label → 路由的映射在 `src/main.ts` 的 `routeByLabel`。
  - 新增窗口：在 `tauri.conf.json` 定义窗口 + `routeByLabel` 加一项 + `router.ts` 加路由。
  - 控制面板子页：`/control-panel/{plans|plans/:id|create|settings}`，新页面挂 children。
  - 窗口 label 常量：`pet` / `mini-board` / `main-board` / `control-panel`（与 `tauri.conf.json` 一致）。
- 无边框透明窗口（pet / mini-board）需要 `body.transparent-root`（`main.ts` 已按 label 自动加）。

## 样式

- `src/styles/tokens.css` — DesignTokens（ADR-0005）唯一色值/圆角/字体来源。
  - **组件样式只引用 CSS 变量（`var(--primary)` 等），禁止硬编码色值**。
  - 圆角只用 `--radius-sm/md/lg`（8/12/16px）；阴影只有 `--shadow-lg`（浮层）；弹窗遮罩用 `--overlay-dim`。
- 图标：`@phosphor-icons/vue`，默认 regular 风格（如 `<PhGear :size="18" />`）；优先级图标 `PhCaretUp` / `PhMinus` / `PhCaretDown`（CONTEXT PriorityGlyph）。

## 待积累（随工单推进登记）

- （暂无）