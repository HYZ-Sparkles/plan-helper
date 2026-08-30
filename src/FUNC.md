# 前端复用方法（src/FUNC.md）

新写前端代码前先查这里，能复用就不新写；新增可复用方法后回来登记（AGENTS.md 复用纪律）。

## 类型与后端调用

- `src/lib/api.ts` — 后端 command 的类型化封装。**组件一律通过这里的包装函数调用后端，不直接 `invoke`**。
  - 类型：`Settings`、`TimeWindow`、`AppStateView`（与 Rust serde 结构一一对应，改后端要同步改这里）
  - 计划域类型：`Priority` / `PlanStatus` / `TaskStatus` / `PauseReason` 枚举、`PlanDraft` / `TaskDraft`（创建与编辑共用入参，TaskDraft.id 缺省 = 新任务；`subgoals` 子目标行、`depends_on` 前置任务的草稿下标引用）、`SubGoalDraft`、`PlanView`（含 `subgoals` / `prerequisite_ids` / `pause_reason`）/ `TaskView`（含 `subgoals` / `prerequisite_ids` / **`progress_percent` 派生进度**，无子目标任务的汇报进度也在其中）/ `SubGoalView`、`PlanErrorShape`
  - `getAppState(): Promise<AppStateView>` — 启动快照（设置 + 服务端时间）
  - `createPlan(draft: PlanDraft): Promise<number>` — 创建计划（含任务/子目标/依赖），失败 reject 结构化错误
  - `listPlans(): Promise<PlanView[]>` — 计划列表（PlanOrdering 默认排序：优先级降序 + 创建倒序，唯一排序；含嵌套任务）
  - `getPlan(planId: number): Promise<PlanView>` — 单个计划详情（工单 03）
  - `updatePlan(planId, draft): Promise<void>` — 整计划编辑保存（编辑态）
  - `deleteTask(taskId): Promise<void>` — 软删除任务进归档（服务端连带解除依赖边；确认弹窗在 UI）
  - 生命周期（工单 05，状态机与抢占不变式在服务层、UI 只按状态展示可得操作）：`startPlan` / `resumePlan` → `Promise<LifecycleOutcome>`（工单 12：paused = 本次操作自动暂停的低等级计划清单〔`PreemptedPlan{id,name}`〕，AutoPauseFeedback 反馈行数据源）、`pausePlan` / `completePlan` / `abortPlan`（二级确认在 UI）、`copyPlanAsNew(planId): Promise<CopyAsNewOutcome>`（new_plan_id + paused——复制即开始，抢占语义同 start）
  - 今日分配（工单 06）：`AllocationTask` / `AllocationGroup` / `AllocationBoardView` 类型、`getAllocationBoard(): Promise<AllocationBoardView>`（候选分组 + 今日回显 + 当日目标 + paused_groups 被抢占暂停分组〔工单 12，灰显不可选〕）、`commitTodayAllocation(taskIds): Promise<void>`（覆盖重选）
  - 进度汇报（工单 07）：`CurrentTaskView`（含 `has_subgoals` / `percent` / `subgoals`）/ `PickerGroup` / `MiniBoardView` 类型、`getMiniBoard()`（当前任务 + 今日完成量 + 当日目标 + 更换候选）、`setCurrentTask(taskId)`（须在今日推进列表内）、`completeSubgoal(subgoalId)`（服务层拒绝乱序）、`undoSubgoal(subgoalId)`（只能撤销最后一个已完成）、`reportPercent(taskId, percent)`（5–100 的 5 倍数增量）、`correctTotalProgress(taskId, percent)`（直接设定 0–100 的 5 倍数，仅无子目标任务）
  - 今日总结（工单 11）：`SummaryTask` / `SummaryPlan` / `DailySummaryView`（date / is_today / is_yesterday / workday / total / target 含结转 / higher_priority_hint / plans 三层嵌套）/ `DailySummaryStatus`（due / last_shown / next_fire_at）类型、`getDailySummary(date)`（每次调用服务端从日志重算——弹出后调出、次日补登都读最新账）、`getDailySummaryStatus()`（启动补登检查 / PetWindow 定时器 / 调出入口共用一次查询）、`markDailySummaryShown(date)`（只弹一次的登记，幂等）
- `src/lib/summary.ts` — 今日总结（工单 11）跨窗口编排：事件名常量 + 打开/调出共用的窗口通道（发事件方与监听方不各写魔法串，打开次序只写一份）。`SUMMARY_SHOW_EVENT`（`daily-summary:show`，请求装载并显示指定日期，payload `{date}`）、`SUMMARY_REFRESH_EVENT`（`daily-summary:refresh`，进展落账后打开中的总结重取最新——MiniBoardWindow 的 `act()` 与 PlanDetailPage 的 `applyCorrection` 发射）、`openDailySummaryWindow(date, register)`（先 emit 再 show + setFocus，同大面板重开语义；`register` = 是否登记"只弹一次"——PetWindow 自动触发恒 true，控制面板调出仅在看的就是**待弹**那份时 true）。
- `src/lib/events.ts` — 大/小看板跨窗口事件名常量（发事件方与监听方不各写魔法串，同 summary.ts 模式）：`MAIN_BOARD_REOPEN_EVENT`（重开先发事件再 show）、`MAIN_BOARD_REFRESH_EVENT`（工单 12：生命周期变化落库后静默重取——候选/"暂停"分组刷新、显隐不变）、`MINI_BOARD_REFRESH_EVENT`（分配落定 / 生命周期变化 → 小看板重取，当前任务失效回空态）。
- `src/lib/labels.ts` — 领域枚举的中文文案集中地：
  - `priorityLabel` / `statusLabel`（计划与任务状态合一张表，共有值标签一致）/ `pauseReasonLabel`（用户主动 / 自动抢占）映射表
  - `planErrorMessage(err)` — 后端 PlanError（{kind,payload}）→ 用户可读文案
  - `hoursFromMinutes(minutes)` — 分钟 → **一位小数 round** 的小时展示（与 ProgressGranularity 0.1% 颗粒度对齐），消 IEEE 754 浮点尾巴（例 258.6/60 = 4.3099999... 不再吐成 "4.3099999..." 而是 "4.3"）；整数小时经 round 后 `.toString()` 保持自然位数（120 → "2"、90 → "1.5"），不强制追加 ".0"；同步口径见后端 `domain::plans::round_to_one_decimal`
  - `hoursLabel(minutes)` — 分钟 → **固定一位小数**的小时文案（X.X 口径，180 → "3.0"）；大面板状态条累计/差额、小看板今日总量行与结转标注（"基准 5.0h"）复用。行内任务耗时展示走 `hoursFromMinutes`（同步改为一位小数 round 口径，整数保持自然显示）
  - `carryLabel(targetMinutes, baseMinutes)`（工单 10）— 工时账户结转标注："基准 5.0h + 结转 0.6h"（负结转 = 超额抵扣后的轻松日，如 "结转 -0.4h"）；两数值先各自 round 一位小数再相减，保证"标注两数之和 === 展示目标"；无结转（差额 round 后 0）返回空串不添噪声。大面板状态条与小看板微型条共用
- `src/lib/progress.ts` — 前端进度派生（ADR-0002）：工单 07 起百分比由服务端统一派生（`TaskView.progress_percent`，无子目标任务的汇报进度也来自 ProgressLog），前端只换算展示——
  - `taskProgress(task)` — (已完成分钟, 总分钟)，分钟 = 派生百分比 × 总耗时（**已 round 一位小数**——与后端 `TaskProgress::percent()` 一位小数口径同步，消二次派生引入的浮点尾巴）；展示口径
  - `taskProgressLabel(task)` — "60.3%" / "60%" 百分比文案（后端已 round 一位小数，前端 toString 自然输出：整数不带 ".0"、一位小数带小数点——与 `hoursFromMinutes` 口径一致）；总量为 0 返回空串。详情页对勾了子目标但总量为 0 的任务以 `|| "0%"` 兜底展示。不要二次 `Math.round`——会抹掉一位小数。
- `src/lib/validation.ts` — 表单输入校验/归一工具：
  - `normalizeDateString(s)` — 常见日期写法（2026-12-20 / 2026/12/20 / 20261220）归一为 YYYY-MM-DD；非真实日历日期（含 2026-02-30）返回 null。DatePicker 失焦归一用，值要么合法要么回退，调用方免校验。
  - `isValidPercentValue(v, min)` — 百分比数值校验（任意正数 + 最多一位小数 + [min, 100]；汇报 min=0.1 / 修正 min=0——2026-08-24 验收修订砍掉原"5% 倍数"颗粒度）。一位小数判定用容差 1e-9（JS 浮点 0.1/0.3/0.7 等不可精确表示，严格等式会误拒合法输入；与后端 `valid_percent` 对齐）。小看板直填（07）与详情页修正弹窗（07）共用；后端另有权威校验，这里只做即时反馈。
  - `localToday()` — 本地日期 → YYYY-MM-DD（生成"今天"的兜底日期；与服务端日期口径一致）。工单 11 的"调出今日总结"入口在从没弹过任何总结时取它。

## 复用组件（src/components/）

- `CreationForm.vue` — **CreationUI 单页可折叠表单（创建/编辑双模式，工单 03/04/05）**：`mode: "create" | "edit"` + 编辑态 `plan: PlanView`；任务卡默认收起为一行摘要（名称 + 耗时/子目标计数）、点行展开；任务顺序 = 创建顺序**不可拖拽、不加序号**（2026-08-24 决策砍掉——先后语义唯一归依赖边；已完成沉底/追加落位由服务端归一化）、已完成任务锁定、优先级开始后锁定、删已入库任务走打字确认；展开态内含**子目标精简输入行**（内容+耗时、行末 + 与回车加行聚焦、已完成行锁定、取消勾选弹确认清空）与**前置任务多选**（DependencyEditor，次要区域）。emit `saved` / `cancel`。
- `DependencyEditor.vue` — 前置任务多选胶囊（v-model = 草稿内稳定 key 集）：候选仅同计划其它任务，创建与编辑同一控件；勾选集即依赖边全集（所见即所存）。
- `MicroBar.vue` — 微型进度条（4px 细线，`ratio` 0–1 + `reached` 达标转完成色）：大面板状态条与小看板今日总量行共用；非法/负值比例自动归 0%。
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

- `PlanDetailPage.vue` — 生命周期按钮矩阵（未开始→开始；进行中→完成计划〔任务全完成后亮起〕/暂停/放弃；已暂停→继续/放弃；终态→复制并新建）；放弃走一级确认（保留多少历史）+ 二级打字「再删」；`runLifecycle` 统一 busy 互斥与错误文案（修正总进度也复用它；成功后统一广播 events.ts 三个重取事件——大/小看板与总结随之刷新），工单 12 抢占不变式 UI 面：`higherActive`（由全量计划列表派生"存在进行中的更高优先级"）禁用 开始/继续/复制并新建（复制即开始；外层 span 承载 tooltip——disabled 按钮不冒泡 hover），`preemptNotice` 出"「X」已被自动暂停（高优先级「Y」开始）"反馈行（AutoPauseFeedback 第二层）；查看态依赖**双向行**（`depNames`"前置：A" + `waitingFor`"被等待：B"——顺序关系的诚实载体是边不是序号，2026-08-24 复审砍掉序号标识）；任务行右侧**修正总进度** icon-btn（无子目标且未完成的任务，`openCorrection`/`applyCorrection`，0–100 的 5 倍数，服务层以差额事件落账）。
- `PlansPage.vue` — 纯展示列表（无拖拽；2026-08-24 决策砍掉计划手动排序，排序全在服务端默认规则）；标题行右侧「今日总结」（工单 11 story 58：`openSummary` 日期 = 待弹的 > 最近已弹的 > `localToday()`，走 `openDailySummaryWindow(date, date === st.due)`——补看的是**待弹**那份才登记已弹，自动触发不再重弹）与「打开大面板」按钮（`openMainBoard`：先 `emitTo("main-board", "main-board:reopen")` 再 show + setFocus——重开必须带回最新数据）。
- `MainBoardWindow.vue` — 大面板今日分配（工单 06）：分组候选列表（checkbox 数组绑定 `selected`；被依赖阻塞的任务**不进列表**——只展示可选任务，2026-08-24 修订）、底部状态条三区布局（左累计+MicroBar / 中确认 / 右黄色差额软提示）、确认后持久化分配并 `emitTo("mini-board", "mini-board:refresh")` + 显示小看板（工单 07 联动）再 `win.hide()`；休息日态（`workday=false` 显示"今日不在工作日"、隐藏列表与状态条）；**关闭请求拦截为隐藏**（`onCloseRequested` preventDefault + hide，完整关闭语义归工单 14）；监听 `main-board:reopen` 事件重新 load（回显服务端选中集，本地未提交勾选被重置——仅重开瞬间发生）+ `main-board:refresh`（工单 12：生命周期变化静默重取）；候选分组之后渲染 `paused_groups`（被抢占暂停计划灰显、附 StatusBadge"已暂停"、无勾选框不可选——AutoPauseFeedback 第一层，自动恢复后随刷新回到候选）。
- `MiniBoardWindow.vue`（工单 07）— 小看板：头部计划名 + 更换任务 icon-btn（PhArrowsLeftRight）；主体四态——空态（选任务）/ 停留态（任务完成，PhCheckCircle，不自动切换）/ 有子目标（已完成行可点撤销〔toast"进度已重算"〕+ 当前待完成行点亮勾选 + "还有 N 项"）/ 无子目标（PercentAdjustControl：[-]±10% 钳制 5–100 / 点数字直填 5% 倍数 / 推进 +X% 主按钮，**会话级不持久化**——`watch(task_id)` 重置 5%）；任务行百分比旁附耗时口径注脚（ADR-0002）；更换任务选择器 = 今日推进列表按计划分组（服务端已把同计划排最前）；底部常驻今日总量（`hoursLabel` + MicroBar）；`act()` 统一 busy 互斥 + toast 反馈 + 成功后 `emitTo("daily-summary", SUMMARY_REFRESH_EVENT)`（11：总结开着时实时重算）；监听 `mini-board:refresh` 重载；关闭拦截为隐藏（归 14）。初始坐标在 `lib.rs#position_pet_and_board`（组合体锚工作区右下角：小看板在桌宠**下方**右对齐、桌宠居上——2026-08-24 验收要求；启动亮板条件 = 有当前任务**且此刻是工作时间**）；拖拽刚性跟随与按模式显隐在 PetWindow（09）。
- `DailySummaryWindow.vue`（工单 11）— 今日总结窗（带边框、启动隐藏、620×680 居中）：头部标题（is_today→"今日总结"/is_yesterday→"昨日总结"/更早→"M月D日总结"）+ 归属日期；总览行（推进 N 计划 / CheckCircle 完成 X 小时〔`hoursLabel`〕/ Target 目标 Y 小时 + `carryLabel` 结转标注 / MicroBar 总进度条）——休息日加班态隐藏目标与进度条（与大小面板同口径）；`higher_priority_hint` 琥珀提示行（PhCaretUp，同大面板差额提示语气）；计划 section（名 + PriorityLabel + "已被抢占暂停"标注〔preempted〕+ 当日总耗时）→ 任务行（名 + percent% + 耗时）→ 推进内容（有子目标 = `✓ 完成列表` + 最早未完成"进行中"；无子目标 = MicroBar）；当日零推进的计划不出现（服务端过滤）。装载路径三合一：mount 时按 status 预载（due ?? last_shown——自动触发的「先 emit 再 show」在窗口 webview 未就绪时事件可能丢失，预载自愈）、`SUMMARY_SHOW_EVENT` 带 date、`SUMMARY_REFRESH_EVENT` 重取当前日期；关闭拦截为隐藏（归 14）。**无任何本地账**——每次都重拉视图（ADR-0009）。
- `CreationForm.vue`（页面级要点）— 任务/子目标顺序 = 创建顺序不可拖、不加序号（2026-08-24 决策，拖拽机器与序号标识已删）：偏序任务贴全序编号必然误导，顺序由依赖边展示承载（详情页前置/被等待行、DependencyEditor）；任务卡 key 稳定标识（`nextKey` 发号器）支撑依赖引用与行删除，与库中 id 无关。

## 路由与多窗口

- `src/router.ts` — hash 路由；窗口 label → 路由的映射在 `src/main.ts` 的 `routeByLabel`。
  - 新增窗口：在 `tauri.conf.json` 定义窗口 + `routeByLabel` 加一项 + `router.ts` 加路由。
  - 控制面板子页：`/control-panel/{plans|plans/:id|create|settings}`，新页面挂 children。
  - 窗口 label 常量：`pet` / `pet-menu` / `mini-board` / `main-board` / `daily-summary` / `control-panel`（与 `tauri.conf.json` 一致）。
  - dev 调试路由：`/dev/anim`（桌宠动画调试页，工单 08）。
- 无边框透明窗口（pet / pet-menu / mini-board）需要 `body.transparent-root`（`main.ts` 已按 label 自动加）。

## 样式

- `src/styles/tokens.css` — DesignTokens（ADR-0005）唯一色值/圆角/字体来源。
  - **组件样式只引用 CSS 变量（`var(--primary)` 等），禁止硬编码色值**。
  - 圆角只用 `--radius-sm/md/lg`（8/12/16px）；阴影只有 `--shadow-lg`（浮层）；弹窗遮罩用 `--overlay-dim`。
- 图标：`@phosphor-icons/vue`，默认 regular 风格（如 `<PhGear :size="18" />`）；优先级图标 `PhCaretUp` / `PhMinus` / `PhCaretDown`（CONTEXT PriorityGlyph）。

## 待积累（随工单推进登记）

## 桌宠动画（工单 08/09）

- `scripts/gen-pet-frames.mjs` — **帧清单生成器**（资产更新可重跑）：从 `resourses/` 的 Oreo Cat sprite sheet 程序化切帧 → 生成 `src/lib/pet/animations.ts`。切分规则（对 sheet 像素实测）：帧间透明间隙 ≥4px = 帧边界；帧内容内部 ≤3px 断裂（抬爪与身体间的全透明列）吸附回同帧；>36px 的宽段 = 两帧粘连，在列覆盖低谷处分割。`--inspect DIR` 额外导出每行条带与逐帧裁剪 PNG 供人工核对。fps 默认值在脚本 `META` 表（资源无时序元数据，试拍后改——**用户已在验收中调过一轮，改动以 META 现值为准**）。**验收调整三张表**（都在 /dev/anim 调试页拨草稿粘进来重跑落盘）：`NUDGE` 帧摆放微调（动画号 → { 帧号 1-based: [ox, oy] 素材像素，正 = 右/下 }）只调摆放不动切分；`FRAME_ORDER` 帧序覆写（动画号 → 播放顺序，元素 = 素材从左数第几帧 1-based，缺省 = 素材从左到右）；`MOVE_WEIGHTS` 每帧位移权重（动画号 → 与帧数等长数组，第 i 帧行程占比 = w[i]/Σw，缺省全 1 均匀）。NUDGE/MOVE_WEIGHTS 的帧号一律指**重排后的播放位**；FRAME_ORDER/MOVE_WEIGHTS 长度与切分帧数不符时脚本报错拒绝生成（防手误）。
- `src/lib/pet/animations.ts` — **生成物，勿手改**：`ANIMATIONS`（24 个动画的帧矩形 + fps + loop；键 = 作者标注编号）、`PET_SHEET_URL`（public/pet/oreo-sheet.png）、`SHEET_SCALE=2`、`SHEET_WIDTH/HEIGHT`。frames 数组顺序 = **播放顺序**（可经 FRAME_ORDER 重排、切分矩形跟着帧走）；`AnimationDef.moveWeights?` 为每帧位移权重（运动类动画用）。
- `src/lib/pet/engine.ts` — **PetEngine 动画引擎**（CONTEXT PetActionExecution 当前动作锁）：`request({steps, lock, onSettle})` 发起动作；steps = 顺序步（`{anim, loop?, fps?, flip?, movement?}`），末循环步开始/单次末步播完触发 `onSettle`（启动序列完成、模式过渡完成、再见退出都挂这里）；`stop()` 停在最后一帧（调试页用）；`subscribe` 订阅渲染状态。**锁的边界**：只在过渡期持有——末步 idle 循环开始（settled）即释放、任何新动作可替换稳态循环（否则启动序列的 7 循环会永久占锁）；过渡中用户动作拒绝一切、系统动作仅可被用户动作抢占（`state().locked` 同口径）。**步进续排**：`advanceStep` 非末步分支必须重排 rAF，否则多步序列第一步播完就停摆（启动序列曾卡死于 21 之后）。**窗口位移**：`MovementSpec` 为 `{direction: "auto"|"left"|"right", distance}`（距离逻辑像素，`auto` = `autoDirection` 朝屏幕余量大侧）或 `{direction: "return"}`（回到**本动作开始时的窗口 x**——跑去吃饭走回原位的回程），经注入的 PetMover 应用、按工作区边界截短不出屏；位移插值用**单调步内时长**（`stepElapsed`），不能用帧内模累积 `acc`（每帧归零会锯齿）；动画配了 `moveWeights` 时插值走累积权重曲线（`moveProgress` 分段线性采样）。**工单 09 增补**：`freeze()/unfreeze()`（拖拽期间帧与位移停推、时长不累积，松手无缝继续；unfreeze 把在飞位移**以当前位置重锚定、原目标为终点**，窗口不回跳）；`EngineState.flick`（抢占硬切计数，替换未稳态动作时 +1，稳态替换=同作者衔接链不触发——渲染层据此淡出淡入）；修复 08 两个隐患——`begin()` 先 `stopLoop()` 取消在排 tick（否则每次替换叠一个 rAF 循环、动画倍速），`advanceStep()` 顶部统一钉死位移终值 p=1（原只钉序列末步，中间步结束采样停在 <1 让回程起点带偏差）。引擎纯 TS 无 DOM/Tauri 依赖，可 Node 直跑复现。
- `src/lib/pet/dragBounds.ts`（工单 09）— **PetDragBounds 四约束纯函数**：`MonitorArea`（物理边界 + 工作区几何，物理像素）；`monitorForRect`（窗口矩形与各屏**重叠面积最大**者选屏，跨屏拖过半自动切换）；`clampDragPosition`（选屏 + `clampIntoWorkArea` 钳进工作区——任务栏禁入与全可见一并成立）；`snapToEdges`（距工作区最近边 < 阈值贴齐）。拖拽的每帧 moveTo 与小看板随动都走这里；无 DOM/Tauri 依赖。
- `src/lib/pet/actions.ts`（工单 09）— **PetRandomAction 编排**：`pickRandomKind(r)`（吃:跳:闲坐 = 4:3:3 权重抽取，边界 0.4/0.7）、`RANDOM_INTERVAL_MS = 300_000`；`randomSteps(kind, pose, side)`（吃 = 10 Run→8 Eat→9 Walk(return)→7 循环；跳 = 14 Jump×2（去 + return 回）→7；闲坐 = 站↔坐轮换 1→2 / 3→7；坐姿起跑吃/跳先 3 Sit to Stand——同作者帧自然过渡，与拖后动作同例；返回 `nextPose` 供结算）；`afterDragSteps(pose)`（拖后用户动作：坐着先 3 Sit to Stand 再 15 Attack→7）。纯数据构建，调度在 PetWindow。
- `src/lib/pet/menu.ts` — 桌宠菜单共享词汇：`PetMode` / `MenuAction` / `PetMenuState` 类型 + pet↔pet-menu 事件名常量（两窗口通信不各写一份魔法串）。
- `src/lib/pet/tauriMover.ts` — `createTauriMover(): PetMover` 的 Tauri 实现：物理坐标驱动桌宠窗口；缓存显示器快照（`monitors()` 供 dragBounds、`refresh()` 拖拽前刷新热插拔/DPI），`workArea()` = 桌宠中心所在屏的工作区；**moveTo 是原始落位**（钳制归调用方：拖拽走 dragBounds、动画位移走引擎 resolveMove），坐标取整。
- `src/components/PetSprite.vue` — **桌宠帧渲染器**：单图 background-position 切帧；底部锚定（帧矩形含整行高，行底 = 地面线，蹲/坐/站/睡同高——用户验收硬约束）；每帧水平居中；整数倍缩放 + `image-rendering: pixelated`；`flip` 水平镜像（素材朝右，朝左移动用）。**帧摆放微调**：帧级 `ox/oy`（素材像素，来自清单或 `nudge` prop 实时覆写）叠加在锚定之上，translate 写在 scaleX 之前 → 偏移是屏幕空间（翻转不镜像偏移方向）。**硬切衔接（09）**：`fadeSignal` prop 计数变化时快速淡出→淡入一次（220ms，接引擎 flick）。容器负责 flex 底对齐。
- `src/pages/DevAnimPage.vue` — **/dev/anim 动画调试页**（dev-only，浏览器直开 `http://localhost:1420/#/dev/anim`）：动画列表/播放/暂停/逐帧步进/调 fps（播放中实时生效）/循环/翻转/缩放（2×/4×/8× 检视切帧）/全帧平铺终审/位移模拟预览（模拟 mover 在 480×220 假想屏内滑动）。**逐帧调整三件套**（草稿不持久化，同 fps 滑杆；页底三表合一草稿一键复制，粘进生成脚本对应表重跑即落盘）：平铺格上排 ◁▷ 播放位移位 + −/＋ 位移权重步进（0.5 步，权重跟素材帧走）、下排 ◀▲▼▶ 摆放微调（素材像素，↺ 恢复本帧默认）；帧序/权重直接改写 def（引擎每 tick 取 def，**播放中立即可见**）；「恢复原序」「权重归一」「清空草稿」按需复原。**帧边界，节奏，顺序，位移，摆放问题都先来这里核对**。
- `src/windows/PetWindow.vue` — 桌宠窗口编排：启动序列 21→22→23→24→7（期间 pointer-events 关闭，点击/拖拽全禁用）→ `onSettle` 进正常态；**初始模式按工作时间判定**（启动时 `isWorkTime()`：工作日 + 时间窗口内 = 工作，否则休息；后端不可达默认工作）——工作模式走 `shouldAutoOpenMainBoard(false)` 自动检测（06 的启动触发接线；非工作日不弹）+ 700ms 站立节拍后系统动作 4→5 进工作睡眠（守卫判 `locked` 不判 `busy`——稳态循环 action 不清空、busy 恒真，误判会让转睡眠永不触发），休息模式直接从 7 Stand Idle 排随机动作计时、不弹面板；模式切换（菜单用户动作占锁）：休息→工作 4→5、工作→休息 6→7 + 隐藏小看板（67），切回工作恢复小看板（有有效当前任务才显示）+ `shouldAutoOpenMainBoard(true)` 检测（手动切入 = 主动加班，休息日也弹，06 加班态）；**随机动作调度（09，PetRandomAction）**：`armRandom()`（每个用户/随机动作的 settle 各重排一次 300s 计时、gen 自弃旧计时；离开休息模式/再见的 gen 自增取消），触发时守卫全量复查 + `randomSteps`（side 用 `autoDirection` 选余量大侧）+ 被拒自愈重排；pose 追踪（stand/sit）决定闲坐切换方向与拖后是否先起身；**拖拽（09）**：位移过 slop 即 `engine.freeze()`、松手 `unfreeze()`（帧冻结/继续、播放状态不断），休息模式拖后 `afterDragSteps`（用户动作占锁）；**刚性组合拖拽（2026-08-29 反馈）**：小看板可见（`boardAttached` 挂靠跟踪）时与桌宠成**刚性组合体**——拖拽每帧与松手吸附都按整体矩形（`unitOrigin` 并集）走 `clampDragPosition`/`snapToEdges`，一个成员被边界挡住全体一起停、相对位置固定不重叠；隐藏时桌宠单体自由拖，桌宠动作不带动看板（PetBoardCoupling 位置联动、动作独立）；**小看板显隐跟着模式走**：启动 `syncBoardAtStartup`（休息兜底隐藏，抹平 Rust/前端判定的毫秒级漂移）、`attachBoardRigidly`（以桌宠为锚重摆：正下方 12px 水平居中同启动摆位，下方放不下则组合体整体上移；工作模式切入恢复与 `mini-board:refresh`〔大面板确认点亮〕都走它），休息模式收到 refresh 也坚持隐藏（67）；再见 = 17 播完 → `exitApp()`；系统关闭请求拦截（Alt+F4 无效）；点击 = 位移 <4px 的 pointerup；菜单失焦关闭与点击切换的竞态用 400ms 时间戳防抖；**今日总结触发（11，DailySummaryTrigger）**：`armDailySummary()`（常驻心跳负责定时——mount 即查一次：`status.due` 非空立即 `openDailySummary`〔补登/补弹〕，否则 `scheduleSummaryFire` 定时到服务端给的 `next_fire_at`〔setTimeout 上限 2^31-1 ms 钳制，超长空档短睡重排〕；到点重查状态由服务端裁决再重排下一次；gen 自弃旧计时与随机动作调度同构）；`openDailySummary(date)` = `openDailySummaryWindow(date, true)`（打开即登记"只弹一次"，失败静默——下次启动补登自愈）。
- `src/windows/PetMenuWindow.vue` — 桌宠菜单独立小窗（桌宠窗口 64×64 装不下菜单）：三项 PetMenuActions（控制面板 PhSlidersHorizontal / 切换模式 PhCoffee↔PhBriefcase，按当前模式显示目标 / 再见 PhHandWaving）；播放用户动作期间项禁用 + muted 色 + not-allowed 光标 + tooltip「桌宠正在执行动作」（菜单可打开查看，拖拽不受影响）；`tauri://blur` 失焦自隐藏；选择后 `emitTo("pet", "pet-menu:action")`。
- `scripts/pet09-regression.ts` — **工单 09 Node 确定性回归**（esbuild bundle 后 node 直跑；rAF 手动推进）：dragBounds 四约束、freeze/unfreeze（帧冻结、时长不累积、在飞位移重锚定）、"return" 回程、rAF 叠加回归（08 遗留 bug）、抢占 flick、4:3:3 抽取与编排、拖后 Attack 步骤。命令见文件头注释。
- `main.ts` — 浏览器直开兜底：无 `__TAURI_INTERNALS__` 时不按窗口 label 落位路由、沿用 URL hash（#/dev/anim 依赖此行为；此前浏览器直开任何页面都会在挂载前崩）。