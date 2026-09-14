# 22: 设置页形象切换 + 署名

**What to build:** 控制面板设置页「桌宠偏好」区上线形象切换：**9 个形象**的选择器（带预览——idle 帧或短循环）、**切换即时生效**（帧网格同构，常驻动画就地换装、生命周期不重播）、选择**持久化**（重启记忆，首次默认 = 注册表首项）；同页「关于」段落落**署名表**：9 个形象的名称 / 作者 / 来源仓库 / license（Kiko CC BY-NC / 月薪喵 CC BY-NC / 邦德·福杰·哆啦A梦·Toothless·阿尼亚·小悟空 个人非商用同人 / 咕咚 CC BY 4.0 / 扣扣企鹅 CC BY-NC）。

背景：spec 故事 69a；术语 Skin / PetConfig / IconFont（选择器遵守图标优先与 DesignTokens）；[ADR-0010](../../../docs/adr/0010-codex-pet-replaces-oreo.md)（署名落设置页"关于"的授权决策）。形象清单与元数据来自工单 16 的形象注册表。

**Blocked by:** 16 codex 契约地基

**Status:** done（待手动验收）

- [x] 9 个形象可选，预览正确（能区分 v1/v2 与不同画风）
- [x] 切换即时生效：常驻动画连续不跳变、不重播启动序列；正在播的一次性动作安全处理（换装不中断播放亦可，验收注明实际行为）
- [x] v1 形象选择后环视自动降级（若 21 已落地则联动验证）
- [x] 重启后记住选择；默认形象稳定
- [x] 署名表 9 条完整（形象名 / 作者 / 来源 / license），样式遵守线条 + 暖色基调
- [x] 手动验收：切换、持久化、重启、署名完整性

实现注记：偏好走 app_prefs KV 表（get_pref/set_pref 命令）+ PET_SKIN_PREF_KEY 常量共用；SkinPreview 组件按契约逐帧时长循环播 idle（实际尺寸、复用 PetSprite）；pickSkin = setPref 落库 + pet:skin-changed 事件（选择即生效不走保存按钮）；PetWindow 换装 = 换 sheet URL 就地继续（一次性动作不中断、播完照常结算），v1/v2 切换同步起停环视轮询；署名表落「关于」区（作者链作者页、来源链 awesome-codex-pet）。

审查修正（code-review-zh Standards）：偏好存储下沉 infra::prefs（commands 薄代理，不内联 SQL）；"pet-skin" 魔法串收编 skins.ts 常量。

验收修订（2026-09-14 用户反馈）：形象收缩 9→7——Toothless / 咕咚 删除（图效果不好），选择器与署名表随注册表自动变 7 条；邦德·福杰保留但忽略环视（选择它后环视自动关闭，gazeIgnored 标记）；已删形象的持久化选择经 skinBySlug 回落默认 Kiko（回归覆盖）。
