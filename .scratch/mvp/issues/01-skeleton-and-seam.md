# 01: 骨架与接缝（prefactor）

**What to build:** `npm run tauri dev` 启动一个能跑的应用骨架：桌宠窗口与控制面板窗口（空壳）能打开；SQLite 初始化；**领域服务层 + 时钟注入**接缝贯通——一条最简单的命令从 UI 经 Tauri command 代理进领域服务并返回，且有 Rust 集成测试证明这条路径是绿色的。视觉侧铺好 DesignTokens（ADR-0005）与 Phosphor 图标。这张工单不改任何业务行为，它让后续每张工单都能直接在接缝下写代码。

背景：`.scratch/mvp/spec.md`（Implementation/Testing Decisions）；术语见 `CONTEXT.md`。

**Blocked by:** None (can start immediately)

**Status:** done

- [x] 领域服务层不依赖窗口、时钟以注入方式提供；Tauri command 只做薄代理（spec 接缝决策）
- [x] 一条示例命令端到端贯通（UI → command → 领域服务 → 返回），对应 Rust 集成测试绿，测试注释按仓库规范写明"测试什么情况、什么结果才算正确"
- [x] SQLite 初始化（rusqlite），FirstRun 默认值落库：每日工作时间 5h、每周工作日周一至五、均分窗口 7 个工作日（这些同时是全局兜底值）
- [x] 桌宠窗口与控制面板窗口空壳可打开；桌宠/小看板置顶、无边框，大面板/控制面板普通窗口（CONTEXT「大看板 / 小看板」）
- [x] DesignTokens（ADR-0005 token 表）以全局样式落地；Phosphor Icons（regular）可正常渲染
- [x] `src/FUNC.md` 与 `src-tauri/FUNC.md` 建立（AGENTS.md 复用纪律）
- [x] `npm run tauri dev` 一键启动无报错

## Comments

**2026-08-23 实施记录（agent）：**

- 分层：`src-tauri/src/domain/`（settings + app_state 读模型，不 import tauri，时钟经 `&dyn Clock` 注入）、`src-tauri/src/infra/db.rs`（SQLite 连接 + WAL + 建表）、`src-tauri/src/commands.rs`（薄代理）、`src-tauri/src/clock.rs`（`SystemClock` / `FixedClock`）。
- 接缝命令 `get_app_state`：设置页可见快照（每日工作时间/工作日/均分窗口/服务端时间），即 UI → command → 领域服务 → 返回 的活演示。
- 集成测试 `tests/seam_settings.rs` 3 条全绿：FirstRun 默认值落库与幂等、保存覆盖 + 注入时钟写入 updated_at（含跨午夜窗口序列化往返）、快照组装的时钟注入。
- 窗口：四窗口全部在 `tauri.conf.json` 静态预定义（pet / mini-board 置顶无边框透明 + skipTaskbar；main-board / control-panel 普通窗口；后两者 hidden 预建，符合"关闭=隐藏"终态架构）。桌宠启动定位主显示器工作区右下角。
- 视觉：`src/styles/tokens.css` 为 ADR-0005 唯一色值来源（CSS 变量）；`@phosphor-icons/vue@2.2.1`（regular 默认风格）已在控制面板侧边栏使用。
- 验证：`cargo test` 3 passed；`npm run build`（vue-tsc + vite）通过；`cargo build` 通过；`npm run tauri dev` 实际启动无报错（vite ready → cargo run → 进程稳定），验证后已停止。
- 说明：时间窗口（time_windows）默认 09:00–18:00 一段是占位默认（spec FirstRun 未规定），设置页（工单 13）可改；schema 已为 `updated_at` 预留（SettingsEffectiveTime 依赖）。

**code-review 修复（Standards 轴 1 处硬性违反 + 判断题）：**

- 修复 `--bg-group` 色值 typo：`#faf9f9` → `#fafaf9`（stone-50，与 ADR-0005 token 表逐字一致）。
- 重复消除：`.hint` / `.page-title` 提为 tokens.css 全局工具类；label→路由映射收进 `router.ts`（`routeForLabel` + `transparentLabels`），路径字符串不再散落 main.ts。
- `SettingsService::load` 里 `Settings::default()` 四次构造收敛为一次。

**Spec 轴发现的处理说明：**

- 「command 转发层无自动化测试」：spec Testing Decisions 明确「接缝唯一：Rust 集成测试覆盖领域服务层」，command 层不在自动化范围。曾尝试 tauri mock 运行时（`features=["test"]`）补 command 本体测试，在 Windows 上因 WebView2Loader.dll 缺失（STATUS_ENTRYPOINT_NOT_FOUND）不可行，已移除——避免给后续每张工单的 `cargo test` 埋雷。command 路径的证据链：领域组装函数有测试 + dev 启动实测无报错；UI 侧可见验证（设置页显示默认值快照）为 10 秒手动验收项。
- 提前实现（隐藏预建窗口 / position_pet / updated_at / capabilities 权限）：服务于 spec 用户故事 68「关闭只是隐藏」的窗口架构与后续工单，非业务行为变更，保留。
- 控制面板启动即可见：骨架期唯一可打开入口（托盘在 14、桌宠菜单在 08 才有）；工单 14 接管后收敛为隐藏 + 入口打开。
