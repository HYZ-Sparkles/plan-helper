---
name: to-spec-zh
description: "把当前对话压成一份 spec，并发布到项目 issue tracker：不采访你，只合成已经讨论过的内容。"
disable-model-invocation: true
---

这个 skill 接管当前会话上下文和代码库理解，产出 spec。不要采访用户；只合成你已经知道的东西。

issue tracker 和分诊 label 词汇表应该已经提供给你。如果没有，告诉用户去跑 `/setup-matt-pocock-skills`。

## 流程

1. 探索仓库以了解代码库的当前状态（如果还没了解过）。整篇 spec 用项目的领域术语表词汇；尊重你所触及领域的任何 ADR。

2. 草拟出你打算验证这道 feature 的接缝。优先用已有的接缝；只在必要时才新增。把接缝放在能放到的最高位置。代码库跨过的接缝越少越好——理想数量是一个。

   和用户确认这些接缝是否符合预期。

3. 用下面的模板写 spec，然后发布到项目 issue tracker。打上 `ready-for-agent` 分诊 label——不需要再分诊。

<spec-template>

## Problem Statement

用户面对的问题，从用户视角。

## Solution

这个问题的解法，从用户视角。

## User Stories

一个**很长的**、编号的用户故事列表。每个用户故事用以下格式：

1. As an <actor>, I want a <feature>, so that <benefit>

<user-story-example>
1. As a mobile bank customer, I want to see balance on my accounts, so that I can make better informed decisions about my spending
</user-story-example>

这个用户故事列表要极其详尽，覆盖 feature 的所有方面。

## Implementation Decisions

一系列已敲定的实现决策。可以包括：

- 将要新建/修改的模块
- 这些模块将被修改的接口
- 来自开发者的技术澄清
- 架构决策
- schema 变更
- API 契约
- 具体交互

**不要**包含具体文件路径或代码片段。它们很快就会过时。

例外：如果某个原型产出的片段把某个决策编码得比散文更精确（状态机、reducer、schema、type 形态），把它内联进相关决策里，并简注"来自一个原型"。裁剪到"决策密集"的部分，不是能跑的 demo，只留关键点。

## Testing Decisions

一系列已敲定的测试决策。包括：

- 什么是好测试的描述（只测外部行为、不测实现细节）
- 哪些模块会被测
- 测试的先例（即代码库里类似类型的测试）

## Out of Scope

不在本 spec 范围内的东西的描述。

## Further Notes

关于这道 feature 的任何其他备注。

</spec-template>