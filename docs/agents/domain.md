# 领域文档

工程 Skill 在探索代码库时，应如何消费本仓库的领域文档。

## 探索前先读这些

- 仓库根的 **`CONTEXT.md`**，或
- 仓库根的 **`CONTEXT-MAP.md`**（若存在）：它指向每个 context 一个 `CONTEXT.md`。读与该话题相关的每一个。
- **`docs/adr/`**：读与你即将下手领域相关的 ADR。在多 context 仓库里，也查 `src/<context>/docs/adr/` 拿到 context 级别的决策。

若这些文件不存在，**静默继续**。不要因为缺失而报错；不要主动建议创建它们。`/domain-modeling` Skill（经由 `/grill-with-docs` 与 `/improve-codebase-architecture` 触发）在术语或决策真正被敲定时，才会懒创建它们。

## 文件结构

单 context 仓库（大多数仓库）：

```
/
├── CONTEXT.md
├── docs/adr/
│   ├── 0001-event-sourced-orders.md
│   └── 0002-postgres-for-write-model.md
└── src/
```

多 context 仓库（仓库根存在 `CONTEXT-MAP.md`）：

```
/
├── CONTEXT-MAP.md
├── docs/adr/                          ← 系统级决策
└── src/
    ├── ordering/
    │   ├── CONTEXT.md
    │   └── docs/adr/                  ← context 级决策
    └── billing/
        ├── CONTEXT.md
        └── docs/adr/
```

## 使用术语表的词汇

当你的输出为一个领域概念命名时（issue 标题、refactor 提案、假设、测试名），使用 `CONTEXT.md` 中定义的那个词。不要漂移到术语表明确避开的同义词。

若你需要的概念还不存在于术语表里，这是一个信号：要么你在发明项目不用的语言（重新考虑），要么确实存在缺口（为 `/domain-modeling` 记下它）。

## 标出 ADR 冲突

若你的输出与既有 ADR 矛盾，明确点出，而不是悄悄覆盖：

> _与 ADR-0007（event-sourced orders）矛盾，但值得重新审视，因为……_