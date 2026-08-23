# 视觉基调反转：暖浅色（白 + 浅橙 + 阴影系统）

**Status: superseded by [ADR-0005](0005-visual-tone-line-warm.md)**

替代 [ADR-0003](0003-ui-modern-pet-independent.md)（已 superseded）。基调从**暗色 + 玻璃拟态**反转成**暖浅色 + 阴影系统**。

## 决策

走**纯白卡片 + 浅橙渐变背景 + 阴影系统**：

- **背景**：`linear-gradient(135deg, #FFF7ED, #FFFFFF)`（暖白到白）
- **卡片**：`#FFFFFF` + 阴影 `0 1px 3px rgba(0,0,0,0.05), 0 1px 2px rgba(0,0,0,0.06)`
- **主色**：`#FB923C`（orange-400）；hover `#F97316`（orange-500）
- **警示**：`#DC2626`（red-600）；hover `#B91C1C`（red-700）
- **文字**：primary `#1C1917`（stone-800）/ secondary `#57534E`（stone-600）/ muted `#A8A29E`（stone-400）
- **边框**：`1px solid #F5F5F4`（stone-100）

**弃用**玻璃拟态（毛玻璃在浅背景上效果差），改 shadow system：卡片用 shadow-sm、浮层（pill 切换器、模态）用 shadow-lg。

**桌宠视觉语言保持独立**：像素 Oreo Cat / Live2D Natori 仍走自己的视觉，不被 UI 同化。这与 [ADR-0003](0003-ui-modern-pet-independent.md) 的核心原则一致。

## 考虑过的备选

- **B. 暖色玻璃**：保留毛玻璃、底色改橙渐变。否决——浅色背景上毛玻璃效果差，且与"白色"方向冲突；用户明确说"白色或浅橙"，不是"全橙"。
- **C. 浅米色 + 一点紫**：温暖但不张扬。否决——偏保守，"阳光感"不如 A 纯粹。

## 后果

- 三个 prototype 变体（VariantA/B/C）的样式要全部重写。
- 优先级配色在白底上加深饱和度（High `#DC2626`、Medium `#3B82F6`、Low `#94A3B8`），避免被白色稀释。
- 浅色 UI 的层次感靠阴影不是模糊——卡片必须有 shadow-sm，浮层必须有 shadow-lg。
- 桌面应用整体观感从"夜间模式"切到"白天生产力"调性，与 README 里"鼓励和监督"的角色氛围一致。