# Issue tracker：本地 Markdown

本仓库的 issue 与 spec 存在 `.scratch/` 下的 markdown 文件里。

## 约定

- 一个 feature 一个目录：`.scratch/<feature-slug>/`
- spec 在 `.scratch/<feature-slug>/spec.md`
- 实施工单按 ticket 一文件，放在 `.scratch/<feature-slug>/issues/<NN>-<slug>.md`，从 `01` 起编号，绝不合并到一个 tickets 文件
- 分诊状态在每个工单文件顶部用一行 `Status:` 记录（角色字符串见 `triage-labels.md`）
- 评论与讨论历史追加到文件底部的 `## Comments` 标题下

## 当 Skill 说「发布到 issue tracker」

在 `.scratch/<feature-slug>/` 下建一个新文件（必要时建目录）。

## 当 Skill 说「取回相关 ticket」

读指定路径的文件。用户通常直接给路径或 issue 编号。

## 寻路操作

由 `/wayfinder` 使用。**地图** 是一个文件，**子工单** 是每个 ticket 一文件。

- **地图**：`.scratch/<effort>/map.md`（Notes / Decisions-so-far / Fog 三段正文）。
- **子工单**：`.scratch/<effort>/issues/NN-<slug>.md`，从 `01` 起编号，正文里写问题。一行 `Type:` 记录工单类型（`research`/`prototype`/`grilling`/`task`）；一行 `Status:` 记录 `claimed`/`resolved`。
- **阻塞**：在顶部一行 `Blocked by: NN, NN`。它列出的每个文件都 `resolved` 时，该工单即被解锁。
- **前沿**：扫 `.scratch/<effort>/issues/` 找开放、未阻塞、未认领的文件；按编号小的优先。
- **认领**：动手前先把 `Status: claimed` 设上并保存。
- **解决**：在 `## Answer` 标题下追加答案，把 `Status` 设为 `resolved`，再向 `map.md` 的 Decisions-so-far 追加一条上下文指针（gist + 链接）。