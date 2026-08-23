# 前端复用方法（src/FUNC.md）

新写前端代码前先查这里，能复用就不新写；新增可复用方法后回来登记（AGENTS.md 复用纪律）。

## 类型与后端调用

- `src/lib/api.ts` — 后端 command 的类型化封装。**组件一律通过这里的包装函数调用后端，不直接 `invoke`**。
  - 类型：`Settings`、`TimeWindow`、`AppStateView`（与 Rust serde 结构一一对应，改后端要同步改这里）
  - 计划域类型：`Priority` / `PlanStatus` / `TaskStatus` 枚举、`NewPlan` / `NewTask`（创建入参）、`PlanView` / `TaskView`（列表视图）、`PlanErrorShape`
  - `getAppState(): Promise<AppStateView>` — 启动快照（设置 + 服务端时间）
  - `createPlan(plan: NewPlan): Promise<number>` — 创建计划（含任务），失败 reject 结构化错误
  - `listPlans(): Promise<PlanView[]>` — 计划列表（PlanOrdering 排序，含嵌套任务）
- `src/lib/labels.ts` — 领域枚举的中文文案集中地：
  - `priorityLabel` / `statusLabel`（计划与任务状态合一张表，共有值标签一致）映射表
  - `planErrorMessage(err)` — 后端 PlanError（{kind,payload}）→ 用户可读文案
  - `hoursFromMinutes(minutes)` — 分钟 → 小时展示

## 复用组件（src/components/）

- `PriorityLabel.vue` — 优先级标签（文字 + PhCaretUp/PhMinus/PhCaretDown + 标签底色，PriorityVisuals）。全应用优先级展示统一走它。
- `StatusBadge.vue` — 计划/任务状态徽章（中文 + 状态色边框）。
- `CollapsibleSection.vue` — 可折叠区块（title + 可选 badge + defaultOpen），CreationUI 段落容器。
- `AutoTextarea.vue` — 自适应 textarea（min 3 行、8 行起滚动），计划/任务详细内容共用。
- `FormField.vue` — 表单字段外壳（label + required 标记 + error 行）。

## 全局工具类（tokens.css，样式复用优先级：token 变量 > 全局类 > scoped）

- `.hint` / `.page-title` — 次级提示文字 / 页标题。
- `.input` — 单行输入与 textarea 的统一形态（边框/圆角/focus）。
- `.primary-btn` — 主操作按钮（带文字的主按钮，IconFont 例外项）。

## 路由与多窗口

- `src/router.ts` — hash 路由；窗口 label → 路由的映射在 `src/main.ts` 的 `routeByLabel`。
  - 新增窗口：在 `tauri.conf.json` 定义窗口 + `routeByLabel` 加一项 + `router.ts` 加路由。
  - 控制面板子页：`/control-panel/{plans|create|settings}`，新页面挂 children。
- 窗口 label 常量：`pet` / `mini-board` / `main-board` / `control-panel`（与 `tauri.conf.json` 一致）。
- 无边框透明窗口（pet / mini-board）需要 `body.transparent-root`（`main.ts` 已按 label 自动加）。

## 样式

- `src/styles/tokens.css` — DesignTokens（ADR-0005）唯一色值/圆角/字体来源。
  - **组件样式只引用 CSS 变量（`var(--primary)` 等），禁止硬编码色值**。
  - 圆角只用 `--radius-sm/md/lg`（8/12/16px）；阴影只有 `--shadow-lg`（浮层）。
- 图标：`@phosphor-icons/vue`，默认 regular 风格（如 `<PhGear :size="18" />`）；优先级图标 `PhCaretUp` / `PhMinus` / `PhCaretDown`（CONTEXT PriorityGlyph）。

## 待积累（随工单推进登记）

- （暂无）