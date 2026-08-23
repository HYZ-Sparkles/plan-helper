# 视觉基调反转 v3：线条风格 + 暖色调，禁用 emoji

**Status: Active**

替代 [ADR-0003](0003-ui-modern-pet-independent.md)（暗色 + 玻璃拟态，已 superseded by 0004）、[ADR-0004](0004-visual-tone-warm-light.md)（暖白 + 阴影系统，已 superseded by 本 ADR）。这是第三次基调反转。

## 决策

桌面应用整体走**线条风格 + 暖色调**：

- **线条为骨**：1px solid 边框 + 几何元素（圆点、线段、矩形徽章）作为主要视觉语言。
- **浅色填充为肉**：极浅色块（stone-50 / orange-50）做信息分组、hover/选中状态。
- **暖色主调**：强调色为暖橙 `#FB923C`（orange-400），主色系保留暖橙/琥珀。
- **状态色三档**：完成（暖绿）/ 进行（暖黄）/ 警示（红）—— 都往暖色方向倾斜。
- **阴影退为辅助**：层次感由"线 + 浅填充"承担，不再依赖 shadow 系统（shadow 仅用于浮层）。
- **禁用 emoji**（🔴/🔵/⚪ 等符号）—— 改用"文字标签 + 几何符号"或"图标字体 + 文字"组合，**具体形态待后续 grill 敲定**。

### 桌面应用线条 token（更新自 ADR-0004）

| Token | 值 |
|---|---|
| 背景（base） | `#FFFFFF` |
| 背景（分组） | `#FAFAF9`（stone-50）/ `#FFF7ED`（orange-50 强调分组） |
| 表面（surface） | `#FFFFFF` |
| 边框（default） | `1px solid #F5F5F4`（stone-100） |
| 边框（active） | `1px solid #FB923C`（orange-400） |
| 强调条（active bar） | `3px solid #FB923C`（顶部选中条） |
| 主色（primary） | `#FB923C`（orange-400）；hover `#F97316`（orange-500） |
| 状态色（完成） | `#16A34A`（green-600） |
| 状态色（进行） | `#D97706`（amber-600） |
| 状态色（警示） | `#DC2626`（red-600） |
| 文字 primary | `#1C1917`（stone-800） |
| 文字 secondary | `#57534E`（stone-600） |
| 文字 muted | `#A8A29E`（stone-400） |
| 阴影 lg（仅浮层） | `0 10px 25px rgba(0,0,0,0.1), 0 4px 6px rgba(0,0,0,0.05)` |
| 圆角 | 8px / 12px / 16px（与 ADR-0004 一致） |
| 字体栈 | `Inter, "SF Pro", -apple-system, sans-serif`（中文 fallback：`"PingFang SC", "Microsoft YaHei"`） |

### 桌宠视觉语言保持独立

像素 Oreo Cat 仍走自己的视觉语言，不被 UI 的线条风格同化。与 [ADR-0003](0003-ui-modern-pet-independent.md) 的核心原则一致。

## 考虑过的备选

- **A. 保留暖浅色 + 阴影系统**（即 ADR-0004 原方案）：否决——阴影系统让卡片显得"软"和"轻浮"，与线条的"利落"调性冲突；用户希望视觉更具"线条感"。
- **B. 走多色系（任务分类用不同颜色）**：否决——与线条的"克制"调性冲突，多色容易让线条极简崩掉。
- **C. 全黑白灰（零色）**：否决——桌宠的暖色氛围需要主色呼应，纯黑白会与桌宠割裂。

## 后果

- ADR-0004 的所有 prototype 样式需要重写——从"阴影 + 浅渐变"切到"线条 + 浅填充"。
- 优先级 / 状态等视觉表达**禁用 emoji**，前端代码里所有 🔴/🔵/⚪ 字符移除。
- 浅色 UI 的层次感改由"线条粗细 + 填充深浅"承担——`shadow-sm` 从卡片样式中移除，保留 `shadow-lg` 给浮层。
- 桌宠资源收敛：Natori 被移除（grill 决策），只保留 Oreo Cat 一种。
- 桌面应用整体观感从"白天生产力"切到"线条工程美学 + 暖色陪伴"——与桌宠的可爱氛围形成"刚柔对照"。

## 待决（已解决）

原待决项——几何符号的具体形态——已在后续 grill 中敲定：

- **图标库**：**Phosphor Icons**（regular 风格）
- **优先级几何**：High `CaretUp` / Medium `Minus` / Low `CaretDown`
- 详见 `CONTEXT.md` 的 `PriorityGlyph` 段、`PriorityVisuals` 表、`IconFont` 段

Phosphor 同时作为整套应用的图标体系（菜单、按钮、状态），不仅是优先级。