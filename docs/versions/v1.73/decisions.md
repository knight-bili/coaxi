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
