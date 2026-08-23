# 08: 桌宠动画引擎与模式

**What to build:** 应用启动时 Oreo Cat 播放启动序列（21 Ear Up → 22 Scan → 23 Ear Down → 24 Jump out the box → 7 Stand Idle），序列期间一切交互禁用；之后用户点击桌宠弹出菜单（控制面板 / 切换模式 / 再见），在工作模式（Sleep Idle 安静陪伴）与休息模式（Stand/Sit Idle 循环）间切换时有过渡动画；「再见」播跳入箱子动画（帧 17）后关闭全部窗口。

背景：spec 用户故事 61–63、68；README 桌宠模块动作映射表；术语 StartupSequence / WorkMode / RestMode / PetMenuActions / PetActionExecution；资源在 `resourses/`（Oreo Cat，Aichan_owo）。

**Blocked by:** 01 骨架与接缝

**Status:** ready-for-agent

- [ ] 帧动画引擎按 README 的"用户动作→桌宠动作"映射播放对应帧序列（Oreo Cat 帧编号为准）
- [ ] 启动序列 21→22→23→24→7 完整播放，期间点击/菜单/拖拽全部忽略；序列完成后进入正常状态，此后 AutoOpenMainBoard 检测接入（06 的启动触发随之激活）
- [ ] 工作模式：Sleep Idle（循环）；休息模式：Stand Idle / Sit Idle 循环；切换走过渡动画（休息→工作 4 Stand to Sleep；工作→休息 6 Sleep to Stand）
- [ ] 单击桌宠弹菜单三项：控制面板（打开独立窗口）/ 切换模式 / 再见；菜单样式遵守 DesignTokens + Phosphor 图标
- [ ] 「再见」：播放帧 17 跳入箱子，动画完成后关闭全部窗口（桌宠、大面板、小看板、控制面板）
- [ ] 动作播放完毕才执行新动作（防割裂）；桌宠像素视觉不被 UI 风格同化（ADR-0005 独立原则）
- [ ] 手动验收：启动序列、双模式切换、菜单三项、再见全流程；启动序列期间交互禁用
