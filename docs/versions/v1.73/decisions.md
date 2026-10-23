---
document_type: version-decisions
version: v1.73
authority: decision-rationale
lifecycle: current
last_updated: 2026-10-19
---

# v1.73 重要取舍

<a id="v1-73-d02"></a>
## V1.73-D02：委派主线使用可协商的独立投影

直接改旧 schema3 分组会使旧客户端按原连续区间归并，卡片可能再次被隐藏；把外层 Shell 移入卡片主区间则会改变
分页边界并与穿插的工具冲突。选择保留旧投影，由能力声明启用独立卡片及单独 support；代价是客户端需处理两种
读取模式。同一次持久更新还会改变多个展示块，因此选择成对位置而非突破 limit 或沿用会跳项的单水位。
这增加协议与缓存接线工作，但不建立持久分组/第二业务权威，也不让展示依赖授予来源证明。
规范见 [v52](../../contracts/run-process-detail-surface-v52.md)与[委派架构](../../architecture/delegation.md)。
后端实现和客户端启用分别留证，不以合同接受表示完整链路完成。

<a id="v1-73-d01"></a>
## V1.73-D01：委派复用 Core 执行与持久身份

Codeg broker 已拥有 task、connection 与缓存，直接保留它并与 Rovai 同步也是可行移植路径；但它会使同一次取消、恢复、审批和重启具有两个终态/身份来源。已确认 r1 选择以 Core DelegationId 和现有 Conversation/AgentRun/Binding/Command receipt 为唯一业务权威，只复用供体协议、界面和纯缓存设计。代价是必须补 Core 持久父链与子执行 policy，不能直接调用原 broker 假装完成融合。原生提供方创建的子代理仍是观察事实，不伪造 Core 调度记录。

规范由[委派架构](../../architecture/delegation.md)和[Delegation v1](../../contracts/delegation-v1.md)拥有；完整批准快照与确认另存[变更说明](model-context-change-native-delegation.md)。该取舍不表示执行实现已经完成。

User 接管后的 UI 继续遵循同一权威：复用 Codeg 抽屉和 Rovai 聊天组件，并以明确 child 操作族连接 Core，避免另一份会话/材料状态。代价是原单聊组件需适配固定 owner、共享输入恢复及嵌入几何；这使原任务、后续 User 聊天和已有输入保护沿同一投影工作。控制回执可持久关闭恢复，输入交接未结仍围栏离开，不靠 Drawer 的开关推断业务结束。具体行为和资格由当前合同与分项证据拥有。

Runtime 原生查看器同样不能直接移植 Codeg 的 agentType/Session 泛读取入口：被报告为工具 receiver 不一定已证明是该父的子代理。选择先由 Core 原证据和绑定构造不可反序列化候选，再由原生来源核对父子记录后读取；代价是多一层来源验证，并须明确补全历史绑定来源。它不引入 Core 子任务，也不能把候选解析测试称为全文查看器完成。

原生来源也沿 Core 的原绑定事务持久记录。改用当前成员环境推断旧目录会让同一历史证据随设置改变；保存完整启动环境则会扩大凭据持久化和兼容负担。选择从实际 Host 最终启动快照只提取所需目录/Host 身份，warm reuse 沿原值，并把源记录与绑定和回执一起提交。代价是旧未记录来源不能自动补猜，完整内容与原UI仍须按独立链路补齐。当前行为由原生子会话合同拥有。


<a id="v1-73-d03"></a>
## V1.73-D03：可选会话工具使用独立冻结与原业务权威

客户确认r1要求保留Codeg四项工具的可用能力，同时保留Rovai原Built-in授权。直接替换原目录或把所有同范围授权注入每次输入会改变旧任务和资料边界，因此选择独立可选stdio、Run级策略冻结及本次User明确引用选择。问题使用原调用outbox而不是重发新消息；使命与自动化复用原领域事务和唯一调度器。代价是需要补四端设置、授权选择、原编号恢复及目录兼容，不能以某一端页面已存在宣称完整实现。
规范由[会话工具架构](../../architecture/conversation-model-tools.md)、[会话模型工具v1](../../contracts/conversation-model-tools-v1.md)及[Runtime Launch v61](../../contracts/runtime-launch-and-verification-v61.md)拥有。原Runtime60已接受，批准快照对目标编号的表述不覆盖它；编号递进不改变此次已批准字段，冻结文件保持原字节。
