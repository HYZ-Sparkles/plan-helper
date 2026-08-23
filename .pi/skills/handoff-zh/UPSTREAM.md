# 上游溯源

本文件记录本 skill 的来源、翻译历史与升级方式。每次升级时更新。

## 来源

- **上游仓库**：https://github.com/mattpocock/skills
- **上游路径**：`skills/productivity/handoff/`
- **锁定 commit**：`0ab1b63a410a03d3627979a109c8695de27af954`
- **下载时间**：2025-08-21

## 翻译

- **翻译者**：pi (MiniMax-M3)
- **翻译原则**：严格翻译，逐段对应，不意译、不合并、不删减
- **翻译偏差**：无
  - `argument-hint` 翻译为中文
  - 正文 5 个段落严格 1:1 对应原版 5 个段落
  - 专有名词 `Skill tool`、`specs`、`plans`、`ADRs`、`issues`、`commits`、`diffs`、`PII` 保留原文

## 升级

升级时重新跑 `add-zh-skills`：

1. 拉取上游到缓存目录：

   ```bash
   git -C <upstream-cache> fetch
   ```

2. 比对当前 commit：

   ```bash
   git -C <upstream-cache> log -1 --format="%H %s"
   ```

   与本文件"锁定 commit"对比。

3. 如有更新：

   - 重跑翻译流程
   - 与旧翻译版 diff：保留自定义调整，吸收上游新内容
   - 更新本文件的 commit 与时间

4. 如无更新：

   - 不动文件；提醒用户上游无变更