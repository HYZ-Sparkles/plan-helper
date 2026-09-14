# 16: codex 契约地基（prefactor）

**What to build:** 桌宠整体换到 codex 契约地基上跑通第一条窄路径：动画数据按契约固定 8 列网格与**逐帧时长表**生成（v1 = 9 行 / v2 = 11 行，格 192×208，替代 Oreo 的 fps + 非透明列段检测），引擎支持逐帧时长步进；**9 个形象资产**（Kiko / 月薪喵 / 邦德·福杰 / 哆啦A梦 / 咕咚 / Toothless / 阿尼亚 / 小悟空 / 扣扣企鹅）随应用打包并建立形象注册表（含 spriteVersionNumber、作者、license 元数据——供环视降级与设置页切换消费）；pet 窗口 64×64 → **96×104**（÷2 整数缩放、像素干净），菜单/小看板摆位锚点与拖拽边界尺寸随新窗口同步调整；调试页与 Node 确定性回归脚本迁移到新词汇表；**Oreo 管线彻底移除**（切帧脚本、素材副本、META/NUDGE/帧序/位移权重表）——本单完成后仓库只存在一套动画词汇表。旧工单 08/09 的引擎机制成果（动作锁、flick、位移插值、freeze/unfreeze、底部锚定）保留复用。

背景：spec 故事 69a、71 与 Implementation Decisions 桌宠条目；术语 CodexPetContract；[ADR-0010](../../../docs/adr/0010-codex-pet-replaces-oreo.md)。资产源在 `resourses/awesome-codex-pet/pets/`（已逐一验证符合契约尺寸）。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] 切帧数据 = 契约固定网格 + 逐帧时长（9 标准动作 + v2 环视行两版本图集都能装载）；9 动作语义命名与 ADR-0010 一致（running ≠ running-right/left）
- [ ] 9 个形象资产入包 + 形象注册表（slug/版本/作者/license），默认形象 = 注册表首项
- [ ] pet 窗口 96×104；菜单、小看板锚点与 dragBounds 尺寸同步；任务栏禁入/不出屏/跨屏/边缘吸附回归通过
- [ ] 调试页按新词汇表展示（逐动作逐帧、时长可查）；Node 回归脚本迁移且全绿（动作锁/flick/位移插值/冻结解冻等引擎不变量保留）
- [ ] Oreo 管线彻底移除，构建与测试无残留引用（`resourses/` 原始素材存档保留，不进代码）
- [ ] 手动验收：默认形象启动 waving→落 idle、窗口比例正确、桌面摆位（右下角锚定）不遮挡不越界
