---
name: to-tickets-zh
description: 把一份 plan、spec 或当前对话切成一套示踪子弹（tracer-bullet）工单，每张声明自己的阻塞边（blocking edges），发布到配置好的 tracker（在本地以"每个工单一个文件、阻塞边写成文本"形式，或在真 tracker 上以原生阻塞链接形式）。
disable-model-invocation: true
---

# 切工单

把一份 plan、spec，或当前对话切成一套**工单**：示踪子弹式的纵向切片，每张声明阻塞它的那些工单。

issue tracker 和分诊 label 词汇表应该已经提供给你。如果没有，告诉用户去跑 `/setup-matt-pocock-skills`。

## 流程

### 1. 收集上下文

以当前会话上下文里已有的内容为输入。如果用户把一个引用（spec 路径、issue 编号或 URL）作为参数传进来，把它抓来读全文和评论。

### 2. 探索代码库（可选）

如果还没探索过代码库，现在去做以了解代码的当前状态。工单标题和描述要用项目的领域术语表词汇；尊重你所触及领域的 ADR。

找机会做 **prefactor** 让实现更容易。"让变化容易，再做容易的变化。"

### 3. 草拟纵向切片

把工作切成**示踪子弹**工单。

<vertical-slice-rules>

- 每个切片穿过每一层（schema、API、UI、测试）走一条窄但**完整**的路径——是纵向的，**不是**某一层的横向切片
- 完成的切片本身可 demo 或可验证
- 每个切片要能放进一个全新的上下文窗口
- 任何 prefactor 应该先做

</vertical-slice-rules>

给每个工单标上它的**阻塞边**：必须在它之前完成的那些工单。没阻塞工的工单可立即开工。

**宽重构是纵向切片的例外。** **宽重构**是一类机械变化（重命名列、给共享符号换类型），其**爆炸半径**横扫整个代码库，一次改动同时破坏成千上万的调用点，没有纵向切片能单独落绿。不要硬塞进示踪子弹；按 **expand–contract** 来排序。先 expand：在旧形式旁边加上新形式，这样什么都不破。然后按爆炸半径分批（每个 package、每个目录）把调用点迁过去，每批自己一张工单、被 expand 阻塞，每批之间 CI 保持绿，因为旧形式还在。最后 contract：在没有调用者时删掉旧形式，作为一张被所有 migrate 批阻塞的工单。当连批之间都没法各自保持绿时，保留顺序但让它们共享一个集成分支，所有批都阻塞一张最终的 integrate-and-verify 工单；绿只在那里承诺。

### 4. 拷问用户

把拟好的拆分以编号列表呈现。对每个工单，展示：

- **标题**：简短具描述性的名字
- **阻塞于**：哪些其他工单（若有）必须先完成
- **它交付什么**：这张工单让端到端的什么行为工作起来

问用户：

- 粒度感觉对吗？（太粗 / 太细）
- 阻塞边是否正确：每个工单是否只依赖于真正卡它的那些工单？
- 是否需要再合并或再切分？

迭代到用户批准拆分。

### 5. 把工单发布到配置好的 tracker

发布已批准的工单。**怎么**发布取决于 `/setup-matt-pocock-skills` 配置的 tracker；工单本身一样，只有阻塞边的形态不同：

- **本地文件** → 在 `.scratch/<feature-slug>/issues/<NN>-<slug>.md` 下为每张工单写一个文件，按依赖顺序编号（阻塞工单在前）。每个文件的"Blocked by"列出它依赖的编号/标题。用下面的"每工单一文件"模板：**一文件一工单**，绝不合并到一个总文件。
- **真 issue tracker（GitHub、Linear…）** → 按依赖顺序（阻塞工单在前）每张工单发一个 issue，让每张的阻塞边能引用真实标识符。平台有原生 blocking / sub-issue 关系的就用；否则把"Blocked by"设为阻塞的 issue。除非另有要求，否则打上 `ready-for-agent` 分诊 label；按构造这些工单就是 agent 可拿的。

按**前沿**推进：所有阻塞都已 done 的工单。纯线性链就是自顶向下。

不要关闭或修改任何父 issue。

<local-ticket-template>

# <NN>: <Ticket title>

**What to build:** 这张工单让端到端的什么行为工作起来，从用户视角——不是逐层的实现清单。

**Blocked by:** 卡住这张工单的工单编号/标题，或 "None (can start immediately)"。

**Status:** ready-for-agent

- [ ] 验收标准 1
- [ ] 验收标准 2

</local-ticket-template>

<issue-template>

## Parent

对 tracker 上父 issue 的引用（如果源头是已有 issue；否则省略这一节）。

## What to build

这张工单让端到端的什么行为工作起来，从用户视角——不是逐层实现。

## Acceptance criteria

- [ ] 标准 1
- [ ] 标准 2

## Blocked by

- 对每个阻塞工单的引用，或 "None (can start immediately)"。

</issue-template>

无论哪种形式，都避免具体文件路径或代码片段：它们很快过时。例外：如果某个原型产出的片段把某个决策编码得比散文更精确（状态机、reducer、schema、type 形态），把它内联并简注"来自一个原型"。裁剪到"决策密集"的部分，不是能跑的 demo，只留关键点。