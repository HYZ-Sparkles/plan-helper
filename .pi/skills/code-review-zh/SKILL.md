---
name: code-review-zh
description: "沿两轴审查自某个固定点（commit、branch、tag、或 merge-base）以来的改动：Standards（代码是否遵守仓库文档化的编码规范？）与 Spec（代码是否与发起 issue/spec 所要求的一致？）。两个审查并行跑在子代理里，并排报告。用户想审查一个分支、一个 PR、进行中的改动、或说「审查自 X 以来」时使用。"
---

对 `HEAD` 与用户给定的某个固定点之间的 diff 做两轴审查：

- **Standards**：代码是否符合本仓库文档化的编码规范？
- **Spec**：代码是否忠实地实现了发起 issue / spec？

两轴作为**并行子代理**运行，所以互不污染上下文，然后这个 skill 汇总它们的发现。

issue tracker 应该已经提供给你。若 `docs/agents/issue-tracker.md` 缺失，告诉用户去跑 `/setup-matt-pocock-skills`。

## 流程

### 1. 钉住固定点

用户说的就是固定点（commit SHA、branch 名、tag、`main`、`HEAD~5` 等）。若他们没说，问他们要。

只捕一次 diff 命令：`git diff <fixed-point>...HEAD`（三点，让比较的是 merge-base）。同时记下提交列表：`git log <fixed-point>..HEAD --oneline`。

继续之前先确认固定点能解析（`git rev-parse <fixed-point>`）且 diff 非空。坏的 ref 或空 diff 应该在这里就失败，而不是在两个并行子代理内部。

### 2. 识别 spec 源

按以下顺序找发起 spec：

1. commit 信息里的 issue 引用（`#123`、`Closes #45`、GitLab `!67` 等），通过 `docs/agents/issue-tracker.md` 里的工作流抓取。
2. 用户作为参数传进来的路径。
3. `docs/`、`specs/` 或 `.scratch/` 下与分支名或 feature 对得上的 spec 文件。
4. 找不到时问用户 spec 在哪。他们说没有，**Spec** 子代理就跳过并报告"无 spec 可用"。

### 3. 识别规范源

仓库里任何记录"代码该怎么写"的东西，比如 `CODING_STANDARDS.md` 或 `CONTRIBUTING.md`。

除了仓库自己文档化的内容之外，Standards 轴总是带着下面的 **smell 基线**：一份固定的 Fowler 代码味道集（_Refactoring_, ch.3），即使仓库啥也没文档化也适用。两条规则约束它：

- **仓库覆盖。** 文档化的仓库规范永远赢；当它认可基线本会标记的某事时，压制该 smell。
- **永远是判断题。** 每个 smell 都是带 label 的启发（"可能 Feature Envy"），不是硬性违反。和这里的任何标准一样，跳过工具已经强制执行的部分。

每个 smell 都是 *它是什么* → *怎么修*；拿它对 diff：

- **Mysterious Name（神秘命名）**：函数、变量或类型的名字不揭示它在做什么或装着什么。→ 重命名它；如果取不出诚实的名字，设计就是混沌的。
- **Duplicated Code（重复代码）**：同一段逻辑形态在改动的多个 hunk 或文件里出现。→ 把共享形态抽出来，两边都调它。
- **Feature Envy（依恋情结）**：一个方法伸手摸另一个对象的数据多于自己的。→ 把方法搬到它依恋的那个数据上。
- **Data Clumps（数据泥团）**：同样的几个字段或参数总是一起旅行（一个想被生出来的类型）。→ 把它们捆成一个类型，传递它。
- **Primitive Obsession（基本类型偏执）**：原始类型或字符串代替了本该有自己的类型的领域概念。→ 给那个概念一个小类型。
- **Repeated Switches（重复 switch）**：同样的 `switch`/`if` 级联在同一类型上反复出现。→ 换成多态，或两边共用一张表。
- **Shotgun Surgery（散弹手术）**：一个逻辑变化迫使 diff 里多处文件都被改。→ 把总是一起变的东西收到一个模块里。
- **Divergent Change（发散式变化）**：一个文件或模块因多种无关原因被改。→ 拆分，让每个模块因一个原因变。
- **Speculative Generality（投机性通用）**：为 spec 不需要的需求加上的抽象、参数或钩子。→ 删掉它；等到真正需要再 inline 回来。
- **Message Chains（消息链）**：调用方不该依赖的长 `a.b().c().d()` 导航。→ 把走的这步藏在第一个对象的一个方法后。
- **Middle Man（中间人）**：一个类或函数主要只是把活转手下去。→ 砍掉它，直接调真正的目标。
- **Refused Bequest（被拒绝的遗赠）**：一个子类或实现者忽略或覆写了大半它继承来的东西。→ 丢掉继承，改用组合。

### 4. 并行派发两个子代理

**Standards 子代理 prompt** 应包含：

- 完整的 diff 命令与 commit 列表。
- 第 3 步里找到的规范源文件清单，**加上第 3 步的 smell 基线原文**（子代理没有其他访问它的途径）。
- 简报："按文件/hunk 报告（若相关）：(a) diff 里违反文档化规范的每一处：引用规范（文件 + 规则）；(b) 你看出的任何基线 smell：叫出名字并引用 hunk。区分硬性违反与判断题：文档化规范违背可以是硬性，但基线 smell 永远是判断题，且文档化仓库规范覆盖基线。跳过工具已经强制执行的部分。400 字以内。"

**Spec 子代理 prompt** 应包含：

- diff 命令与 commit 列表。
- spec 的路径或抓取到的内容。
- 简报："报告：(a) spec 要求但缺失或仅部分实现的需求；(b) diff 里没被要求的行为（scope creep）；(c) 看起来实现了但实现看着不对的需求。每条发现引用 spec 原文。400 字以内。"

若 spec 缺失，跳过 Spec 子代理，并在最终报告里标注。

### 5. 汇总

把两份报告分别放在 `## Standards` 和 `## Spec` 标题下，原样或略作清理。**不要**合并或重新排序发现，因为两轴是刻意分开的（见 _为什么是两轴_）。

以一行摘要结尾：每轴的总发现数，以及**每轴内**最严重的问题（若有）。不要跨轴挑唯一一个赢家：那正是分离要阻止的重新排序。

## 为什么是两轴

一次改动可以过一条轴、败另一条：

- 遵守每条规范但实现错了东西 → **Standards 过、Spec 败。**
- 完全按 issue 要求做但破坏了项目惯例 → **Spec 过、Standards 败。**

分开报告防止一条轴掩盖另一条。