---
document_type: version-decisions
version: v1.73
authority: decision-rationale
lifecycle: current
last_updated: 2026-10-17
---

# v1.73 重要取舍

<a id="v1-73-d01"></a>
## V1.73-D01：委派复用 Core 执行与持久身份

Codeg broker 已拥有 task、connection 与缓存，直接保留它并与 Rovai 同步也是可行移植路径；但它会使同一次取消、恢复、审批和重启具有两个终态/身份来源。已确认 r1 选择以 Core DelegationId 和现有 Conversation/AgentRun/Binding/Command receipt 为唯一业务权威，只复用供体协议、界面和纯缓存设计。代价是必须补 Core 持久父链与子执行 policy，不能直接调用原 broker 假装完成融合。原生提供方创建的子代理仍是观察事实，不伪造 Core 调度记录。

规范由[委派架构](../../architecture/delegation.md)和[Delegation v1](../../contracts/delegation-v1.md)拥有；完整批准快照与确认另存[变更说明](model-context-change-native-delegation.md)。该取舍不表示执行实现已经完成。

User 接管后的 UI 继续遵循同一权威：复用 Codeg 抽屉和 Rovai 聊天组件，并以明确 child 操作族连接 Core，避免另一份会话/材料状态。代价是原单聊组件需适配固定 owner、共享输入恢复及嵌入几何；这使原任务、后续 User 聊天和已有输入保护沿同一投影工作。控制回执可持久关闭恢复，输入交接未结仍围栏离开，不靠 Drawer 的开关推断业务结束。具体行为和资格由当前合同与分项证据拥有。

Runtime 原生查看器同样不能直接移植 Codeg 的 agentType/Session 泛读取入口：被报告为工具 receiver 不一定已证明是该父的子代理。选择先由 Core 原证据和绑定构造不可反序列化候选，再由原生来源核对父子记录后读取；代价是多一层来源验证，并须明确补全历史绑定来源。它不引入 Core 子任务，也不能把候选解析测试称为全文查看器完成。
