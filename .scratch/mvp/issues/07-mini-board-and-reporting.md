# 07: 小看板与进度汇报

**What to build:** 用户在桌宠旁的小看板上看到当前任务（所属计划、任务名、当前待完成子目标或百分比控件），按序勾子目标或用百分比控件做增量汇报；每次汇报落 ProgressLog，一切进度从日志派生；任务到 100% 自动完成，小看板空态停留等用户换下一个。当前任务完成后小看板显示"✓ 任务完成"，不自动切换。

背景：spec 用户故事 30–39；ADR-0002（进度=耗时完成度）、ADR-0009（ProgressLog 单一事实源）；术语 MiniBoard / CurrentTask / SwitchCurrentTask / ProgressGranularity / SingleRecommendedPercent / PercentAdjustControl / SubGoalUndo / PetBoardCoupling。

**Blocked by:** 06 大面板今日分配

**Status:** done（待手动验收）

- [x] 小看板为置顶独立窗口（tauri.conf 已有 alwaysOnTop/无边框/跳过任务栏），初始摆在桌宠正上方共享坐标系（`lib.rs#position_mini_board`，完整拖拽交互在 09 就绪后接入）；点击区域只响应桌宠本体，不响应小看板（两窗口不重叠、各自独立命中）
- [x] 大面板确认分配后联动显示小看板（emitTo refresh + show；重启时已有有效当前任务也直接显示），用户从今日推进列表指定当前任务（`set_current_task`：不在今日列表/计划不在进行中/已完成均服务层拒绝）；「更换任务」从今日列表挑，同计划备选排前（picker_groups 把当前任务所属计划分组提到最前）
- [x] 有子目标任务：显示当前待完成子目标（第一个未完成项点亮，其余折叠为"还有 N 项"），按序勾选推进（最小单位 1 个、不可跳序——服务层 SubGoalOutOfOrder 拒绝乱序与重复勾选）
- [x] 无子目标任务：PercentAdjustControl（[-] -10%、点数字直填 5% 倍数、[+] +10%，默认 5%，范围 5–100%），会话级不持久化（watch task_id 重置），切换任务重置
- [x] 汇报为增量语义（+X%），每次汇报追加 ProgressLog 事件（任务、时刻、增量分钟、来源 SubGoal/SubGoalUndo/Percent/Correction）；任务当前进度（task_progress：有子目标=子目标求和、无子目标=日志求和）、今日完成量（day_minutes 按工作窗口开始日归属，跨午夜凌晨段归前一日）全部从日志派生（服务层测试）
- [x] 任务进度到 100% 自动转已完成并锁定（sync_task_completion 换 epsilon 容差 is_complete）；小看板空态停留"任务完成"（PhCheckCircle）+「更换任务」，不自动切换（current_task 存储不清，视图带 Completed 状态）
- [x] 当前任务所属计划被暂停/放弃、或任务被移出今日推进列表时，小看板回空态（board 装配三重校验派生，隔日分配失效同样回空态）
- [x] 撤销已完成子目标的勾选可点（已完成行整行可点），toast"已撤销「X」的完成，进度已重算"，进度按日志实时重算（撤销落负分钟补偿账；只能撤销最后一个已完成项，保住"已完成是前缀"不变式）
- [x] 任务详情提供"修正总进度"（PlanDetailPage 无子目标任务行 icon-btn + 确认弹窗直接设定当前值、0–100 的 5 倍数、差额以 Correction 事件落账）；已完成任务锁定不可修正（服务层 ProgressLocked）
- [x] 小看板底部常驻今日总量微型进度条（今日推进 X / 目标 Y，目标暂用基准，10 就绪后升级含结转；MicroBar 组件与大面板共用）
- [x] 休息模式隐藏小看板、切回工作模式恢复——随 08 的模式状态生效（本工单不做显隐接线）
- [x] 服务层测试：增量汇报、派生进度、自动完成、乱序拒绝、撤销重算、修正落账（seam_progress.rs：7 测，另含颗粒度/溢出、当前任务生命周期、跨午夜归属；全套 48 测回归通过）
