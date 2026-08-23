# 02: 创建无子目标计划

**What to build:** 用户能在控制面板「创建」页用单页可折叠表单建出一个"无子目标"计划并持久化：Plan 段默认展开（名称、简述默认同名、详细内容纯文本 textarea、优先级默认 Medium、可选截止日期仅作展示），Task 列表段默认折叠（任务卡：名称、简述默认同名、详细内容、"是否需要子目标"切换默认不勾——不勾则**必填预计耗时**）。保存后「计划管理」页的 4-tab 壳（进行中/已暂停/已完成/已放弃）里能看到它（状态"未开始"），重启应用数据不丢。

背景：spec 用户故事 1–3、4、8、55–56；术语 CreationUI / PlanOrdering / PlanArchiveView。

**Blocked by:** 01 骨架与接缝

**Status:** ready-for-agent

- [ ] 表单为单页可折叠三段（Plan 默认展开、Task/SubGoal 默认折叠），按段独立校验，无向导步骤条
- [ ] 简述留空默认取名称；详细内容为纯文本自适应 textarea（最少 3 行，8 行起滚动）
- [ ] 无子目标任务必填预计耗时，否则该任务卡校验不过
- [ ] 保存校验：计划至少 1 个任务（领域服务层集成测试覆盖校验规则）
- [ ] 保存持久化到 SQLite；计划管理 4-tab 壳可见，重启不丢
- [ ] 计划列表默认排序：Priority 降序 + CreatedAt 倒序（PlanOrdering 默认规则）
- [ ] 视觉遵守 DesignTokens：优先级显示用"文字标签 + Phosphor 图标"（CaretUp/Minus/CaretDown），禁 emoji
