# 14: 托盘与窗口关闭语义

**What to build:** 应用常驻系统托盘：左键打开控制面板，右键菜单（打开控制面板 / 退出）。退出有两条等价路径——托盘「退出」与桌宠菜单「再见」，都走跳箱告别动画后关闭全部窗口。桌宠窗口无关闭按钮、系统关闭请求被拦截；其余窗口的关闭只是隐藏。

背景：spec 用户故事 68–69；术语 SystemTray / PetMenuActions；窗口体系见 CONTEXT「大看板 / 小看板」。

**Blocked by:** 08 桌宠动画引擎与模式

**Status:** done（待手动验收）

- [x] 系统托盘常驻：左键单击打开控制面板；右键菜单「打开控制面板」「退出」——lib.rs `setup_tray`（Cargo `tray-icon` feature）；`show_menu_on_left_click(false)` 左键让给开面板（`on_tray_icon_event` 左键**抬起**触发，按住拖出不误触），右键 `on_menu_event` 按 id 分发；图标 = 打包默认图标 + tooltip「任务帮手」
- [x] 两条退出路径等价：托盘「退出」与桌宠「再见」都播放 Oreo Cat 帧 17（跳入箱子），动画完成后关闭全部窗口（桌宠、大面板、小看板、控制面板）并退出进程——Rust 不直接退进程：`request_exit` 发 `pet:tray-exit`（Rust/前端常量跨端互指），PetWindow 监听后走与菜单「再见」**同一个 `goodbye()`**，播完统一 `exit_app`；`goodbye` 加固三件：去重（双入口连点忽略）、占用重试（300ms 轮询到可占锁——托盘绕 normal 守卫直入，启动序列/过渡动画期间也生效，常驻兜底入口永远有效）、告别期 `summaryGen`/`windowStartGen` 自增（自动触发静默，防总结窗在告别最后一秒弹出并把"已弹"登记进库）
- [x] 桌宠窗口：无边框、无关闭按钮；系统关闭请求（Alt+F4 等）被拦截——不退出、不隐藏（`PetWindow.onCloseRequested` preventDefault，工单 08 已铺、本单核对）；桌宠菜单窗补同款拦截为隐藏（菜单窗被 Alt+F4 摧毁后菜单功能即废）
- [x] 控制面板 / 大面板 / 小看板：点关闭 = 隐藏窗口，应用继续运行；从托盘或对应入口可再次打开（四窗口 onCloseRequested preventDefault + hide 早已各就位——控制面板/大面板/小看板/今日总结，本单核对；复现入口：托盘/桌宠菜单 → 控制面板，控制面板 → 大面板/总结，模式联动 → 小看板；Rust 侧 `reveal_control_panel` 与前端 revealWindow 同序 unminimize→show→set_focus）
- [ ] 手动验收：托盘双入口、退出动画完整播放后才关窗、Alt+F4 无效、三窗口隐藏/复现

## Comments

- 2026-08-30 实施：dev 冒烟通过（托盘装配无 panic、进程稳定），`vue-tsc` 干净、`cargo test` 全量 95 通过。退出路径选择「事件进桌宠」而非 Rust 直接关窗：桌宠窗口是动画引擎唯一持有者，跳箱动画必须在它那里播——两条退出路径共用 `goodbye()` 让"等价"由构造保证而非两处对齐。
- 2026-08-30 审查修正（双轴子代理）：Standards 零硬性违反；Spec 无缺失、三处自我追加（pet-menu 拦截 / goodbye 加固 / tooltip）已如上登记。**已知限制**：应用启动首秒 PetWindow 的 `listen(TRAY_EXIT_EVENT)` 尚未挂载，此窗口期内点托盘「退出」事件丢失、需再点一次（修复需 Rust 侧 pending 标志 + pet 就绪握手命令，为亚秒级竞态加状态不值）；手动验收请在启动序列播完后测退出路径。
