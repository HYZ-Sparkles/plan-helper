---
name: handoff-zh
description: 把当前会话压缩成交接文档，让另一个 Agent 接手继续干活。
argument-hint: "下一个会话会用来做什么？"
disable-model-invocation: true
---

写一份交接文档，归纳当前会话，让一个全新的 Agent 能接着干这份活。保存到用户操作系统的临时目录——而不是当前工作区。

文档中要包含一个 "suggested skills" 段，列出下一个 Agent 应该用 Skill tool 调用的 skill。

不要重复其他制品（specs、plans、ADRs、issues、commits、diffs）里已经记录的内容；用路径或 URL 引用它们。

脱敏任何敏感信息，例如 API key、密码或个人身份信息（PII）。

如果用户传了参数，把它当作"下一个会话会聚焦什么"的描述，并据此调整文档内容。