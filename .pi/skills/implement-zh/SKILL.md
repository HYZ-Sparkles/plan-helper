---
name: implement-zh
description: "基于 spec 或一组工单实施一项工作。"
disable-model-invocation: true
---

按 spec 或工单里描述的内容实施工作。

能使用 `/skill:tdd-zh` 时就用，用在事先约定的接缝处。

定期跑类型检查、单个测试文件，最后跑一次完整测试套件。

完成后，用 `/skill:code-review-zh` 审查工作。

把工作提交到当前分支。