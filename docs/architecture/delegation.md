---
document_type: architecture
authority: core-delegation-ownership
last_updated: 2026-10-17
---

# 独立帮手

Core 是委派业务与执行权威。DelegationId 关联真实发起成员、父/根 Conversation、原 Run/epoch/Binding/tool-call 和独立 child Conversation；恢复增加 attempt，不替换逻辑编号。它不是 Task、Mission、正式成员或提供方原生子代理。
现有 Scheduler/Fleet、Runtime Adapter、输入接收、清理和命令回执承担执行；不能另运行具有业务权威的 Codeg broker。原生提供方的子代理通过只读证据/引用投影，缺可靠来源时显示未知，不伪造 Core 子 Run。

当前[输入、设置和持久身份原语](../contracts/delegation-v1.md)已编码，候选身份 DDL 在隔离 Core 夹具验证；启动迁移尚未登记。下一步按[实施计划](../versions/v1.73/implementation-plan.md)合齐格式/冻结资格/严格迁移与执行全链路；既有 Host 或 Runtime 尚不会因为这些模块而获得新能力。
新Run从可信Conversation和完整合同开通标记冻结角色及资格；Runtime刷新沿用该Run的冻结资格，不读取当前设置授予旧Run新能力。角色绑定摘要保留原Runtime模型、权限及无原生Session恢复能力的事实。子Bootstrap按已批准文本使用独立身份、空记忆/平台技能，不查询父Mission或渠道事实；模型输入和原生技能/MCP的完整隔离仍须格式35与执行准入共同实现。
完整行为、上下文隔离和原界面职责在[已确认 r1](../versions/v1.73/model-context-change-native-delegation.md)，取舍理由见[V1.73-D01](../versions/v1.73/decisions.md#v1-73-d01)。
