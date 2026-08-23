# 02: 创建无子目标计划

**What to build:** 用户能在控制面板「创建」页用单页可折叠表单建出一个"无子目标"计划并持久化：Plan 段默认展开（名称、简述默认同名、详细内容纯文本 textarea、优先级默认 Medium、可选截止日期仅作展示），Task 列表段默认折叠（任务卡：名称、简述默认同名、详细内容、"是否需要子目标"切换默认不勾——不勾则**必填预计耗时**）。保存后「计划管理」页的 4-tab 壳（进行中/已暂停/已完成/已放弃）里能看到它（状态"未开始"），重启应用数据不丢。

背景：spec 用户故事 1–3、4、8、55–56；术语 CreationUI / PlanOrdering / PlanArchiveView。

**Blocked by:** 01 骨架与接缝

**Status:** done

- [x] 表单为单页可折叠三段（Plan 默认展开、Task/SubGoal 默认折叠），按段独立校验，无向导步骤条
- [x] 简述留空默认取名称；详细内容为纯文本自适应 textarea（最少 3 行，8 行起滚动）
- [x] 无子目标任务必填预计耗时，否则该任务卡校验不过
- [x] 保存校验：计划至少 1 个任务（领域服务层集成测试覆盖校验规则）
- [x] 保存持久化到 SQLite；计划管理 4-tab 壳可见，重启不丢
- [x] 计划列表默认排序：Priority 降序 + CreatedAt 倒序（PlanOrdering 默认规则）
- [x] 视觉遵守 DesignTokens：优先级显示用"文字标签 + Phosphor 图标"（CaretUp/Minus/CaretDown），禁 emoji

## Comments

**2026-08-23 实施记录（agent）：**

- Rust：`domain/plans.rs`（Priority/PlanStatus/TaskStatus 枚举 + TEXT 往返；NewPlan/NewTask 入参；PlanView/TaskView 出参；PlanError 统一 {kind,payload} 序列化）；`PlanService::create` 事务写入 + 权威校验（名称非空 / ≥1 任务 / 无子目标必填耗时 / 勾子目标须有子目标——03 前必拒）；`PlanService::list` PlanOrdering 默认排序；简述留空回退名称（`default_text` 计划/任务共用）。schema 新增 plans/tasks 表（含软删除列、sort_override 预留）。
- 集成测试 `tests/seam_plans.rs` 6 条全绿：创建回读字段/默认回退、≥1 任务拒绝（含事务不落库断言）、缺耗时/空名拒绝、勾子目标无子目标拒绝、排序（High→M2→M1→L）、文件库重开不丢。
- 前端：CreatePage 单页可折叠三段（计划默认展开 / 任务列表、子目标默认折叠）、按段独立校验 + 服务端错误透传；PlansPage 4-tab + 数量徽章 + 空状态 CTA（story 59）；新复用组件 PriorityLabel / StatusBadge / CollapsibleSection / AutoTextarea / FormField（FUNC.md 登记）；labels.ts 文案集中。
- 设计决策：未开始计划归入「进行中」tab（工作区语义——该 tab 收纳 NotStarted+Active），否则新计划无处可见；工单文字"4-tab 壳里能看到它（状态'未开始'）"由此满足。
- 验证：cargo test 9 条全绿（含 01 的 3 条）；npm run build 通过；tauri dev 启动无报错。
- 手动验收建议：创建页建计划 → 保存跳转计划管理「进行中」tab 可见 → 重启应用仍在。

**code-review 修复：**

- 真 bug：labels.ts EmptyName 分支 payload 形状不匹配（读 index 而后端发 task_index），任务序号恒显示"第 1 个"——已修。
- 复用：`.input` / `.primary-btn` 提升为 tokens.css 全局类（消除两页重复 + 硬编码 #ffffff）；PlansPage 改用 `hoursFromMinutes`。
- 一致性：PlanService::list 错误通道统一为 PlanError；planStatusLabel/taskStatusLabel 合并为 statusLabel（共有值标签一致）；StatusBadge 死兜底移除。
- 结构：补齐 CreationUI 第三段（子目标段空壳 + 说明），满足验收项 1 的"三段"字面；分段控件强调条改 3px 对齐 token；AutoTextarea 行高常量与 CSS 显式一致；列表加载失败显示错误行（不与空态混淆）。
- 保留项：sort_override 列、SubGoalsRequired 校验为 03/05 铺路（已在 FUNC.md/注释披露）；"进行中"tab 含未开始为产品裁量，如需改由用户定夺。
