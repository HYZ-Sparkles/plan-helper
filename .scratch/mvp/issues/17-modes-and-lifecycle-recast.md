# 17: 模式常驻与生命周期编排

**What to build:** 桌宠全生命周期换 codex 语义：**工作模式常驻 running**（陪你伏案，语义翻转自 Sleep Idle）；**休息模式常驻 idle**；**启动** = waving（约 0.7s，期间交互禁用）→ 按初始状态落位——工作且大面板即将自动弹出 → 直接 waiting，工作 → running，休息 → idle；**模式切换**硬切常驻（codex 无入睡/醒来过渡动画，无法衔接的帧序列走 flick 快速淡出淡入兜底）；**退出**（桌宠「再见」/托盘「退出」两条路径）= 桌宠窗口**直接淡出**后关闭全部窗口（无告别仪式），goodbye 的去重 / 占锁重试 / 告别期静默机制保留；休息模式左键气泡文案形象无关化「右键打开菜单」。

背景：spec 故事 61、63、68；术语 StartupSequence / WorkMode / RestMode / PetMenuActions / SystemTray / PetActionExecution；[ADR-0010](../../../docs/adr/0010-codex-pet-replaces-oreo.md)（取代工单 08 的 Oreo 编排与工单 14 的跳箱动画段）。

**Blocked by:** 16 codex 契约地基

**Status:** done（待手动验收）

- [x] 工作模式 running 循环、休息模式 idle 循环；切换硬切不卡顿，无法衔接时 flick 兜底
- [x] 启动 waving 完成后正确分流三种落位（工作+面板将弹→waiting / 工作→running / 休息→idle）；waving 期间点击/菜单/拖拽禁用；大面板在 waving 完成后才弹出（面板打开期间 waiting 的完整规则在工单 20 落地，本单只需"面板将弹→waiting"这一分流）
- [x] 两条退出路径 = 淡出→关闭全部窗口并退出进程；行为等价由共用通道保证；启动首秒托盘退出的已知竞态维持现状
- [x] 气泡文案无形象专属措辞（去"喵"）
- [x] 手动验收：双模式切换、启动三分流（改工作时间设置构造三场景）、双路径退出、气泡文案

实现注记：三分流 = onStartupSettled 先 await checkAutoOpen(false) 再 applyChassis（落位前停在挥手末帧等检测结果）；模式硬切 + flickOnSwap 淡出淡入兜底；退出 = goodbye() 淡出（280ms CSS，不依赖引擎状态——取代旧跳箱的占锁重试）→ exitApp；旧机制保留：双入口去重、告别期静默、托盘绕 normal 守卫。
