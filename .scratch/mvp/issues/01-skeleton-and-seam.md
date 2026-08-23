# 01: 骨架与接缝（prefactor）

**What to build:** `npm run tauri dev` 启动一个能跑的应用骨架：桌宠窗口与控制面板窗口（空壳）能打开；SQLite 初始化；**领域服务层 + 时钟注入**接缝贯通——一条最简单的命令从 UI 经 Tauri command 代理进领域服务并返回，且有 Rust 集成测试证明这条路径是绿的。视觉侧铺好 DesignTokens（ADR-0005）与 Phosphor 图标。这张工单不改任何业务行为，它让后续每张工单都能直接在接缝下写代码。

背景：`.scratch/mvp/spec.md`（Implementation/Testing Decisions）；术语见 `CONTEXT.md`。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] 领域服务层不依赖窗口、时钟以注入方式提供；Tauri command 只做薄代理（spec 接缝决策）
- [ ] 一条示例命令端到端贯通（UI → command → 领域服务 → 返回），对应 Rust 集成测试绿，测试注释按仓库规范写明"测试什么情况、什么结果才算正确"
- [ ] SQLite 初始化（rusqlite），FirstRun 默认值落库：每日工作时间 5h、每周工作日周一至五、均分窗口 7 个工作日（这些同时是全局兜底值）
- [ ] 桌宠窗口与控制面板窗口空壳可打开；桌宠/小看板置顶、无边框，大面板/控制面板普通窗口（CONTEXT「大看板 / 小看板」）
- [ ] DesignTokens（ADR-0005 token 表）以全局样式落地；Phosphor Icons（regular）可正常渲染
- [ ] `src/FUNC.md` 与 `src-tauri/FUNC.md` 建立（AGENTS.md 复用纪律）
- [ ] `npm run tauri dev` 一键启动无报错
