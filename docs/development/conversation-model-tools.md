# 会话模型工具开发入口

本功能已经客户确认 r1，当前仍是生产源码编写阶段，未编译、测试或宣称可供模型调用。
字段和状态先读[当前合同](../contracts/conversation-model-tools-v1.md)，启动恢复读[Runtime Launch v61](../contracts/runtime-launch-and-verification-v61.md)。

服务源码位于 `crates/rovai-core/src/conversation_tools/`：策略、Run冻结、资料投影和授权、问题/outbox、创建领域命令、原调度器、stdio/broker 分别由对应模块拥有。
Host 可信 User 入口位于 `application/conversation_tools_access.rs`；模型只走该服务的原生 stdio 桥接，不得调用 User RPC。
UI 复用原 Codeg 问题卡、原 Rovai 队员设置和自动化编辑器；移动端使用原生 AskQuestionCard、List/Section/GlassRow，沿原滚动容器嵌入。

用户 CLI 形式为：

```text
rovai app conversation-tools <action> --input <private-json-file> --json
```

action 为14项 User 方法中 `conversationTools.` 后的部分，或 `source.list`、`source.prepare`、`automation.details`。
输入文件为对应 Core API JSON对象，最多128KiB。每次写入由调用方提供稳定commandId；断线后使用原编号核对，不创建另一个编号试探。
此入口使用已运行的 App 用户认证连接，不启动 App，不是模型工具通道。答案、授权材料和密钥不要进入命令行参数或共享日志。
原始 source 列表通过 `history.library` 的 sources 操作获取，准备历史引用使用 source.prepare；准备来源不等于授权，授权不等于发送消息。
自动化详情使用 `automations.get`，生成使命按 queuedMissionAfter 翻页；打开真实 Thread 即进入原使命工作区。

依用户要求，步骤1不运行软件 Review、测试、格式/类型/文档门禁、构建、实际 App/Runtime/模型或 Actions，不安装新增依赖。
新增时区依赖的 lockfile 解析、编译兼容与所有跨端运行证据留到统一 Review/后续工程验证，不把源码提交当通过证据。
