# 15: 视觉一致性走查

**What to build:** 全部窗口（控制面板、大面板、小看板、桌宠菜单、各类弹窗/toast）按 ADR-0005 与 DesignTokens 逐窗核对：线条 + 暖色基调、Phosphor 图标统一、优先级视觉规范、禁 emoji 扫描。发现偏差当场修复，让整个应用以一致的视觉语言收尾。

背景：ADR-0005（线条风格 + 暖色调，Active）+ CONTEXT DesignTokens / IconFont / PriorityVisuals / PriorityGlyph。

**Blocked by:** 13 工作时间与设置, 14 托盘与窗口关闭语义

**Status:** done（待手动验收）

- [x] 全窗口核对 DesignTokens：1px 边框（default stone-100 / active orange-400）、3px 顶部强调条、极浅填充分组（stone-50 / orange-50）、圆角只用 8/12/16px、阴影仅浮层 shadow-lg
- [x] 主色/状态色/三档文字色严格取 token 值；无硬编码色值散落（抽样式走查）
- [x] Phosphor regular 为主（thin/bold 按规范）；图标优先原则落实，例外保留文字：主操作（创建计划/确认/再删）、优先级文字（高/中/低）、必填说明、语义不明处
- [x] 优先级视觉：边条色 + "文字标签 + CaretUp/Minus/CaretDown"，High 深红/ Medium 蓝/ Low 灰的饱和度在白底不稀释；全应用禁 emoji（🔴/🔵/⚪ 等字符扫描为零）
- [x] 二次确认流（删除任务/放弃计划）样式一致：均为"说明 + 打字「再删」"同一组件
- [x] toast 样式统一（撤销子目标、取消勾选子目标等）
- [x] 桌宠视觉独立未被同化（像素原生、无 UI 化滤镜）
- [x] 产出走查清单（逐窗逐项勾选）附在本工单 Comments

## Comments

**2026-08-30 实施记录（agent，含 2026-08-30 用户反馈「最大化窗口后 UI 保持整齐」重点）**

自动化扫描：全 src 正则扫硬编码色值（hex/rgb/hsl，tokens.css 与帧数据除外）= 零；`var(--x)` 引用与 tokens.css 定义表比对 = 1 处未定义；emoji/几何符号扫描（U+1F300–1FAFF、2600–27BF 等）= 模板渲染文本零命中（仅注释里 →/✕ 箭头词）；`box-shadow` 使用点核对 = 仅浮层（DatePicker 弹层/小看板卡/桌宠菜单）+ 两处 inset 线条（segmented 选中下条、日历"今天"内描边——线条语言，非阴影）。

**修复 4 处：**

1. **未定义 token**：`SettingsPage.vue` `.tag.work` 的 `border-color: var(--color-success)` —— tokens.css 里该 token 叫 `--color-done`，原声明无效（边框色回落 currentColor 碰巧同为绿，视觉未崩但属于脱离 token 体系）。已改 `var(--color-done)`。
2. **大面板最大化整齐**（用户重点反馈）：`MainBoardWindow.vue` 头/体/底内容原为全宽铺开，最大化后任务卡横跨整屏。改法 = **线条与底色通栏、内容限宽居中列**：头部 `.header-inner`、主体 `.content-col`、底部 `.footer-inner` 三段同一 `max-width: 724px`（默认 780 窗口 − 2×28 padding，默认窗口外观零变化），最大化后内容居中、三段同列对齐（状态条与列表左缘对齐、确认居中、差额提示与列表右缘对齐）。
3. **今日总结窗同款**：`DailySummaryWindow.vue` 头部 `.header-inner` + 主体 `.content-col`，`max-width: 564px`（620 − 56）。
4. **调试页图标**：`DevAnimPage.vue` 「◀ 帧 / 帧 ▶」裸字符按钮 → PhCaretLeft/PhCaretRight（同页其余控件已是 Phosphor，仅这两枚漏网）。

**窗口缩放防挤压**：`tauri.conf.json` 给 main-board（min 560×480）与 daily-summary（min 520×480）补最小尺寸（控制面板已有），拉窄到底部三区不重叠。

**走查清单（逐窗逐项）：**

| 窗口/组件 | token 色值 | 圆角/阴影 | 图标 | 优先级视觉 | emoji | 布局（最大化） |
|---|---|---|---|---|---|---|
| 控制面板壳 | ✓ 全 token | ✓ 侧栏线 1px | ✓ 侧栏 PhListBullets/PhPlus/PhGear | — | ✓ | ✓ 侧栏固定宽 + 页面级内容列居中（840/720/560px） |
| 计划管理页 | ✓ | ✓ 卡片线 + 4px 优先级边条 | ✓ 标题行 icon-btn 双钮 | ✓ PriorityLabel + 边条 | ✓ | ✓ 840px 居中列 |
| 创建/详情页 | ✓ | ✓ 分段选中=下缘 3px 主色条 | ✓ PhPlay/PhPause/PhXCircle/PhCopySimple/PhPencilSimple 等 | ✓ segmented 内嵌 PriorityLabel | ✓ | ✓ 720px 居中列 |
| 设置页 | ✓（修复 --color-success） | ✓ 分组 stone-50 | ✓ 五组标题图标 | — | ✓ | ✓ 560px 居中列 |
| 大面板 | ✓ | ✓ | ✓ PhFlag/PhCheck/PhCoffee | ✓ 分组头 PriorityLabel | ✓ | ✓ **724px 居中列（本次修复）** |
| 今日总结窗 | ✓ | ✓ | ✓ PhCheckCircle/PhTarget/PhCaretUp/PhFlag | ✓ 计划头 PriorityLabel | ✓ | ✓ **564px 居中列（本次修复）** |
| 小看板 | ✓ | ✓ 浮层 shadow-lg | ✓ PhArrowsLeftRight/PhX/PhCircle(Dashed)/PhCheckCircle/PhFlagBanner | — | ✓ | 固定 240×260 不缩放 |
| 桌宠菜单/气泡 | ✓ | ✓ 浮层 shadow-lg | ✓ PhSlidersHorizontal/PhCoffee/PhBriefcase/PhHandWaving | — | ✓ | 固定 168×152 不缩放 |
| 弹窗（Confirm/TypeConfirm） | ✓ 共用全局骨架 | ✓ radius-lg + shadow-lg | ✓（危险流打字「再删」同一组件） | — | ✓ | min(420px, 100vw−48) 自适应 |
| toast（小看板） | ✓ 深底浅字全 token | ✓ radius-md | — | — | ✓ | 固定宽自适应卡 |
| 桌宠本体 | 独立像素渲染（pixelated、无 UI 滤镜/边框/圆角） | — | — | — | ✓ | 64×64 固定 |
| /dev/anim 调试页 | ✓ 全 token | ✓ | ✓（修复 ◀▶ 裸字符 ×2） | — | ✓ | dev-only 不验收 |

**验证**：`npm run build`（vue-tsc + vite）通过；`cargo test` 9 套件 95 测全绿。手动验收建议：`npm run tauri dev` 后把大面板/今日总结/控制面板逐一最大化——内容应保持居中列、头底线条通栏、状态条与列表对齐；设置页「这天工作」标签边框应为绿色实线。

**code-review（两轴）修订：**

- **（Standards：Duplicated Code）限宽块收编全局类**：`.header-inner`/`.content-col`/`.footer-inner` 的"max-width + width + margin auto + border-box"四件套在两窗重复 5 处 → 收进 tokens.css：`.content-col`（限宽块，列宽经窗口根 `--col-max` 变量注入：大面板 724px / 总结 564px）、`.content-col-scroll`（主体档：滚动收进列自身）、`.thin-scrollbar`（6px 细滚动条；小看板 picker-list 的 scoped 同款一并收编）；两窗 scoped 只留 flex/grid 结构与 `--col-max` 一行。
- **（Spec：滚动条对齐破绽）滚动收进内容列**：原方案主体 `overflow:auto` 在内容超高出现经典滚动条时主体变窄 ~17px、头/体/底三段错位（默认 780 窗口任务多时即可复现）——改 `.content-col-scroll` 把滚动移到列自身，滚动条贴列右缘，三段对齐与滚动状态无关；滚动条本身用 `.thin-scrollbar` 消掉默认粗条。
- **（Spec 报备的越界项）**：tauri.conf 给 main-board（560×480）/daily-summary（520×480）补 minWidth/minHeight 属"防拉窄挤压"的预防性约束，验收项未直接要求——保留，理由是用户反馈「整齐」在任意窗口尺寸下成立；FUNC.md 大面板条目顺带回填了 2026-08-29 已落地的休息日加班态描述（文档滞后修正）。
