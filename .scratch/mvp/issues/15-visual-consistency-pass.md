# 15: 视觉一致性走查

**What to build:** 全部窗口（控制面板、大面板、小看板、桌宠菜单、各类弹窗/toast）按 ADR-0005 与 DesignTokens 逐窗核对：线条 + 暖色基调、Phosphor 图标统一、优先级视觉规范、禁 emoji 扫描。发现偏差当场修复，让整个应用以一致的视觉语言收尾。

背景：ADR-0005（线条风格 + 暖色调，Active）+ CONTEXT DesignTokens / IconFont / PriorityVisuals / PriorityGlyph。

**Blocked by:** 13 工作时间与设置, 14 托盘与窗口关闭语义

**Status:** ready-for-agent

- [ ] 全窗口核对 DesignTokens：1px 边框（default stone-100 / active orange-400）、3px 顶部强调条、极浅填充分组（stone-50 / orange-50）、圆角只用 8/12/16px、阴影仅浮层 shadow-lg
- [ ] 主色/状态色/三档文字色严格取 token 值；无硬编码色值散落（抽样式走查）
- [ ] Phosphor regular 为主（thin/bold 按规范）；图标优先原则落实，例外保留文字：主操作（创建计划/确认/再删）、优先级文字（高/中/低）、必填说明、语义不明处
- [ ] 优先级视觉：边条色 + "文字标签 + CaretUp/Minus/CaretDown"，High 深红/ Medium 蓝/ Low 灰的饱和度在白底不稀释；全应用禁 emoji（🔴/🔵/⚪ 等字符扫描为零）
- [ ] 二次确认流（删除任务/放弃计划）样式一致：均为"说明 + 打字「再删」"同一组件
- [ ] toast 样式统一（撤销子目标、取消勾选子目标等）
- [ ] 桌宠视觉独立未被同化（像素原生、无 UI 化滤镜）
- [ ] 产出走查清单（逐窗逐项勾选）附在本工单 Comments
