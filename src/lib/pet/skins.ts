/**
 * 桌宠形象注册表（工单 16，spec 69a / ADR-0010）：内置 codex 契约形象，
 * 元数据来自 awesome-codex-pet 仓库 pets.json（license 全部干净、已逐一验证契约尺寸）。
 * 契约统一（形象只是皮肤）：帧网格与时长对所有形象成立，切换 = 换 sheet URL。
 * 2026-09-14 验收收缩：Toothless / 咕咚 删除（图效果不好，9 → 7）；邦德·福杰保留
 * 但忽略环视（v2 转向效果不好）——环视能力不再只由 spriteVersion 决定。
 */

/** 一个形象的注册项 */
export interface SkinDef {
  slug: string;
  /** 展示名（中文优先，同仓库 localized_names） */
  name: string;
  author: string;
  /** 形象来源仓库（署名表"来源"列） */
  sourceUrl: string;
  /** license 摘要（署名表用；商用一律不允许，见 ADR-0010 授权决策） */
  license: string;
  /** 契约图集版本：2 = 带 16 向环视行，1 = 无（环视自动降级） */
  spriteVersion: 1 | 2;
  /** v2 图集但环视被忽略（资产转向效果不好，2026-09-14 用户反馈）；
   *  环视能力 = spriteVersion === 2 且未忽略（skinCanGaze） */
  gazeIgnored?: boolean;
  /** 打包后的雪碧图 URL（public/pet/skins/） */
  sheet: string;
}

/** 资产来源仓库（全部形象的出处） */
export const SKIN_SOURCE_REPO = "https://github.com/legeling/awesome-codex-pet";

/** 7 个形象（README 桌宠模块清单顺序；默认形象 = 首项 Kiko） */
export const SKINS: SkinDef[] = [
  {
    slug: "kiko--untko",
    name: "Kiko 壁虎",
    author: "untko",
    sourceUrl: "https://github.com/untko",
    license: "CC BY-NC 4.0",
    spriteVersion: 2,
    sheet: "/pet/skins/kiko--untko.webp",
  },
  {
    slug: "salary-cat--zuochunjie",
    name: "月薪喵",
    author: "Zuochunjie",
    sourceUrl: "https://github.com/Zuochunjie",
    license: "CC BY-NC 4.0（衍生自 Einswen/SalaryCat，Apache-2.0）",
    spriteVersion: 2,
    sheet: "/pet/skins/salary-cat--zuochunjie.webp",
  },
  {
    slug: "bond-forger--legeling",
    name: "邦德·福杰",
    author: "Legeling",
    sourceUrl: "https://github.com/legeling",
    license: "个人非商用同人",
    spriteVersion: 2,
    gazeIgnored: true, // 2026-09-14 验收：v2 转向效果不好，忽略环视
    sheet: "/pet/skins/bond-forger--legeling.webp",
  },
  {
    slug: "doraemon--xueshi",
    name: "哆啦A梦",
    author: "xueshi",
    sourceUrl: "https://codex-pets.net/users/xueshi",
    license: "个人非商用同人（创作者授权仓库再分发）",
    spriteVersion: 1,
    sheet: "/pet/skins/doraemon--xueshi.webp",
  },
  {
    slug: "anya--chenxin-dlut",
    name: "阿尼亚",
    author: "Xin Chen",
    sourceUrl: "https://github.com/chenxin-dlut",
    license: "个人非商用同人",
    spriteVersion: 1,
    sheet: "/pet/skins/anya--chenxin-dlut.webp",
  },
  {
    slug: "kid-goku--julianhuang",
    name: "小悟空",
    author: "JulianHuang",
    sourceUrl: "https://codex-pets.net/users/julianhuang",
    license: "个人非商用同人（创作者授权仓库再分发）",
    spriteVersion: 1,
    sheet: "/pet/skins/kid-goku--julianhuang.webp",
  },
  {
    slug: "koukou-penguin--hoody",
    name: "扣扣企鹅",
    author: "hoody",
    sourceUrl: SKIN_SOURCE_REPO,
    license: "CC BY-NC 4.0",
    spriteVersion: 2,
    sheet: "/pet/skins/koukou-penguin--hoody.webp",
  },
];

/** 默认形象 = 注册表首项（spec 69a：首次启动的记忆值） */
export const DEFAULT_SKIN = SKINS[0];

/** 形象偏好的存储键（工单 22 getPref/setPref 用——写读两侧共用一份，不各写魔法串） */
export const PET_SKIN_PREF_KEY = "pet-skin";

/** 环视能力：v2 图集且未被忽略（v1 无环视行天然 false；bond-forger 显式 false） */
export function skinCanGaze(s: SkinDef): boolean {
  return s.spriteVersion === 2 && !s.gazeIgnored;
}

/** 按 slug 查形象；未知 slug 回落默认（存储值指向已删除形象时防炸——如 Toothless） */
export function skinBySlug(slug: string | null | undefined): SkinDef {
  return SKINS.find((s) => s.slug === slug) ?? DEFAULT_SKIN;
}
