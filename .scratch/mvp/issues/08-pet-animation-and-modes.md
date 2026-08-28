# 08: 桌宠动画引擎与模式

**What to build:** 应用启动时 Oreo Cat 播放启动序列（21 Ear Up → 22 Scan → 23 Ear Down → 24 Jump out the box → 7 Stand Idle），序列期间一切交互禁用；之后用户点击桌宠弹出菜单（控制面板 / 切换模式 / 再见），在工作模式（Sleep Idle 安静陪伴）与休息模式（Stand/Sit Idle 循环）间切换时有过渡动画；「再见」播跳入箱子动画（帧 17）后关闭全部窗口。

背景：spec 用户故事 61–63、68；README 桌宠模块动作映射表；术语 StartupSequence / WorkMode / RestMode / PetMenuActions / PetActionExecution；资源在 `resourses/`（Oreo Cat，Aichan_owo）。

**Blocked by:** 01 骨架与接缝

**Status:** ready-for-review

- [x] 帧动画引擎按 README 的"用户动作→桌宠动作"映射播放对应帧序列（Oreo Cat 帧编号为准）
- [x] 启动序列 21→22→23→24→7 完整播放，期间点击/菜单/拖拽全部忽略；序列完成后进入正常状态，此后 AutoOpenMainBoard 检测接入（06 的启动触发随之激活）
- [x] 工作模式：Sleep Idle（循环）；休息模式：Stand Idle / Sit Idle 循环；切换走过渡动画（休息→工作 4 Stand to Sleep；工作→休息 6 Sleep to Stand）
- [x] 单击桌宠弹菜单三项：控制面板（打开独立窗口）/ 切换模式 / 再见；菜单样式遵守 DesignTokens + Phosphor 图标
- [x] 「再见」：播放帧 17 跳入箱子，动画完成后关闭全部窗口（桌宠、大面板、小看板、控制面板）
- [x] 动作播放完毕才执行新动作（防割裂）；桌宠像素视觉不被 UI 风格同化（ADR-0005 独立原则）
- [x] 手动验收：启动序列、双模式切换、菜单三项、再见全流程；启动序列期间交互禁用

## Comments

**2026-08-28 实施记录（工单 08）**

- **帧数据程序化生成**：`scripts/gen-pet-frames.mjs` 从 sprite sheet 切帧（规则：帧间透明间隙 ≥4px = 边界；帧内 ≤3px 断裂吸附；>36px 宽段谷底分割——行 17 的两帧粘连即此修复），产物 `src/lib/pet/animations.ts`（24 动画全量）。用户补充的两条硬约束已落实：**帧底部锚定统一地面线**（蹲/坐/站/睡同高）；**引擎支持水平翻转 + 窗口位移**（方向 auto = 屏幕余量大的一侧，工作区边界钳制不出屏）——08 未触发移动动作，09 的 Run/Eat/Walk 直接配 `movement` 即用。
- **视觉自查（/dev/anim + 浏览器自动化）**：7 / 17 / 21 / 24 / 4 / 5 / 6 逐帧核对通过；行 6 帧 4"尾部贴边"经像素复核为紧致裁剪正常形态（边界两侧全透明列，无内容损失）。作者标注图与自动切帧在 12/14/15/19/24 行相差 ±1 帧（视觉模型对 32px 像素图计数不可靠），以节距均匀性为准取自动结果；**验收时如发现某动画出现"半只猫闪帧/双猫粘连"，在 /dev/anim 全帧平铺里定位后改脚本重生成**。
- **修复的实际 bug**：位移插值误用帧内模累积（每帧归零 → 窗口锯齿移动），改为单调步内时长 + 终点钉死；Node 确定性复现验证线性 + 终值精确。
- **fps 为实现侧默认值**（资源包无时序元数据）：idle 5、过渡 8、跑跳 10-12；在 `/dev/anim` 试拍后改 `scripts/gen-pet-frames.mjs` 的 META 重生成。
- **接的线**：`should_auto_open_main_board` command（06 预留启动触发）；`exit_app` command（再见 = 17 播完 → 关全部窗口，托盘退出（14）走同一通道）；pet 窗口拦截系统关闭请求；`pet-menu` 独立菜单窗（64×64 装不下）；control-panel 改启动隐藏（spec「序列播完直接上桌面」，入口 = 菜单）；休息模式隐藏/恢复小看板（67）；`main.ts` 浏览器直开兜底（#/dev/anim 依赖）。
- **留给 09 的钩子**：拖拽完整形态（看板跟随/任务栏禁入/边缘吸附——08 已实现带钳制的手动拖拽，复用 tauriMover）；随机动作（吃:跳:闲坐 4:3:3、300s 间隔，替换 08 的 40s 站坐轮换 stand-in）；Attack（拖拽后休息模式 15）。
- **验收方式**：`npm run tauri dev` 走手动验收清单；动画细节核对/调参用浏览器直开 `http://localhost:1420/#/dev/anim`。

**2026-08-28 code-review（两轴：Standards + Spec）后修正**

- **关键缺陷 ×2（已修复 + Node 确定性回归）**：(1) 多步序列第一步播完后 `advanceStep` 未续排 rAF → 启动序列会卡死在 21 之后（单步播放不触发，浏览器验证没暴露）；(2) 锁语义错误——末步 idle 循环 + lock:true 会永久占锁，稳态后一切动作（睡眠过渡/菜单/再见）被拒。修正为**锁只在过渡期持有**：末循环步开始（settled）即释放、稳态循环可被任何新动作替换（回归断言：settle 恰好一次、稳态 locked=false、系统过渡不可被另一系统动作抢占、用户动作可抢占并持锁、17 播完触发退出回空闲）。
- `goodbye()` / `switchMode()` 改为先试占锁再改状态（被拒时不污染 phase/mode）。
- 控制面板补「关闭 = 隐藏」拦截（spec 68；08 起启动隐藏、菜单为主要入口，不拦截则点 X 销毁窗口后入口失效）。
- 复用修正：`openMainBoard` 复用 `openWindow`；`PetMode`/`MenuAction`/事件名常量收敛到 `src/lib/pet/menu.ts`（两窗口不再各写一份）；菜单项数据驱动渲染。
- DevAnimPage 样式违规修正（rgba 硬编码 → `--surface`、2px 圆角移除）；Rust 侧 `cargo fmt` 归一。
- 遗留判断题（记录不修）：/dev/anim 进生产包（有意保留作验收调参工具）；前端无测试基建（spec Testing Decisions 明确桌宠动画不在自动化范围，锁/位移用 Node 确定性回归脚本验证过）。
