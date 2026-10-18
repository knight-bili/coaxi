---
document_type: version-overview
version: v1.73
lifecycle: current
authority: version-scope-and-status
design_status: confirmed
implementation_status: in_progress
model_context_change: true
last_updated: 2026-10-18
---

# v1.73：Core 独立帮手

上一版本 [v1.72](../v1.72/README.md) 的历史记录及未完成环境验证保留。
本版本落实客户直接确认的 [I04 独立帮手 r1](model-context-change-native-delegation.md)。
一期完整范围仍为 81 簇、48 包、324 正式场景；本版本不是一期最终 MVP 完成声明。

## 研发状态

当前源码登记 Core v1.73/Schema161/Migration211。原完整160/210委派迁移保持独立目标，160现在是受支持来源；211单独增加原生User交互存储，不重写210或旧输入。两条启动链按原事务顺序安装，旧v1.72/159仍可沿原链升级。此前160/210的[启动升级分项证据](../../../integration/execution/i04-delegation-startup-result.json)保留原时点，当前211边界见[原生交互合同](../../contracts/delegation-v1.md#原生user交互存储)。不表示用户日常数据库已升级。

原生提问和计划审批已有纯转换及Core持久请求、User-only答复/CAS/原回执、先标记发送和未知结果不重发的内核。User有界摘要列表、完整读取和答复已接Application、Native IPC准入及Web操作；原UUID未知结果核对和不同问题隔离有运输分项验证。四种原生回调、原Host/输入身份及后台答复驱动已编码，写端在同一串行锁内重验活跃prompt；组件检查通过，完整回调循环仍待耦合验证。Run取消、绑定失效与启动恢复会封旧请求；Grok修改意见已保存，原User队列尚未接入前明确禁止发送。阻塞摘要与原卡片仍待接线，不能称该功能已可实际使用。没有改变批准r1文本、Bootstrap或父模型历史。

六项模型操作、四个兼容别名、User 查询/设置/恢复及材料操作已接入 Core 与相应运输入口，并有分项证据。设置仍默认关闭，深度 1..8、缓存 512 MiB/0 不限、精确版本更新；原 Codeg 设置界面和完整产品使用流程仍待接入。
持久委派/attempt/control、父链、任务 Blob、原请求回放、独立私有路由、接管序号、取消及原 Session 恢复已有 Core 实现。真实 Host/Fleet 调度、writer/cleanup、输入接受结果核对和进程重启协调仍须完成，不能将分项运输或数据库检查称为完整执行交付。

新 Run 按冻结资格选择帮手 Profile11、公开父 Profile12 或私聊父 Profile13，使用合法的格式35和批准的 Charter；私聊父权限为独立 policy3，普通私聊默认 policy2 保持。帮手使用独立身份、空记忆和空平台技能入口，材料及原生技能/MCP过滤已有分项检查，完整 Runtime 资格另行验证。
旧 Run 不因设置变化而补权；旧选择1、排队选择2、原配置、Blob 正文和摘要的兼容回放已沿原 Context owner 验证。该证据使用真实 SQLite/Blob 和 Core 路径，原生 Session、输入 ACK 与终态仍有明确夹具边界，未证明真实模型运行。

尚未完成的产品工作包括完整 Host/Runtime 生命周期、问题/计划阻塞摘要、away/manual 资格、原 Codeg 设置与两类子代理查看器、三类记忆和自动化等 I04 既定工作，以及阶段候选验证。I04 仍为研发中；全部 21 簇、4 个工作包、84 个正式场景保留。

## 跨版本文档影响

| 范围 | 结论 | 证据或理由 |
| --- | --- | --- |
| Version lifecycle | 已更新 | 本概览、[实施计划](implementation-plan.md)、[版本索引](../README.md)与旧版 lifecycle 同步；旧实现事实不改写。 |
| Decisions | 已更新 | [V1.73-D01](decisions.md#v1-73-d01) 说明为何复用 Core 而不移植第二套 broker 业务权威，[导航](../../decisions/CURRENT.md)同步。 |
| Contracts | 已更新 | [Delegation v1](../../contracts/delegation-v1.md) 区分已实现基础输入/设置与待接入执行合同；[Builtin Transport v39](../../contracts/builtin-tool-transport-v39.md) 增六项委派、精确租约回放、封闭输出与无限等待，完整Host生命周期资格单独留证。 |
| Architecture | 已更新 | [委派架构](../../architecture/delegation.md)规定单一权威和原生子代理区别；领域词汇在根 CONTEXT.md 同步。 |
| UI | 确认无需更新 | 本批没有 Renderer 或跨版本 UX 改动；原 Codeg 两类查看器及设置页仍按已确认 r1 待移植。 |
| Runtime Activity | 确认无需更新 | 本批无新活动映射；前两批 User-only 原生观察证据保持。 |
| Runtime compatibility | 确认无需更新 | 没有启动实际 Runtime 或新增适用平台资格。 |
| Documentation routing | 已更新 | 版本、合同及架构索引补委派入口；具体批准记录独立保存。 |
| Root README | 确认无需更新 | 产品定位、公开支持范围和已交付能力未改变。 |

## 验证与收口

User 查询与设置的后端及 Native/Web 接线见[查询设置分项记录](../../../integration/execution/i04-delegation-user-queries-result.json)：12项定向Rust、45项两端运输和10项文档测试通过，父Run停止仍可读、游标受众隔离、设置事件/回执失败整笔回滚。
后续User resume见[用户恢复分项记录](../../../integration/execution/i04-delegation-user-resume-result.json)：13项定向Rust、47项两端运输和10项文档测试通过，父实际正常结束/取消、原Session边界、User同事务证明、并发/接管/闭父及故障回滚均有组件证据；原生接收/终态仍是夹具。原页面、完整启动迁移与真实Host/Runtime仍待，不授予正式场景或阶段收口资格。

检查执行结果在[本轮执行记录](../../../integration/execution/i04-delegation-foundation-result.json)逐项记录。
后续身份约束、候选 DDL 和接管存储分项见[关系执行记录](../../../integration/execution/i04-delegation-identity-result.json)。
冻结资格、Bootstrap与默认路径回归见[冻结输入分项记录](../../../integration/execution/i04-delegation-policy-bootstrap-result.json)。
格式35、最终Blob/Manifest、组件回放及默认路径回归见[输入格式分项记录](../../../integration/execution/i04-delegation-format35-result.json)；测试工厂、实际Core调用和待完整准入的资格分别列明。
本批存储夹具不代表完整 Core 启动迁移或命令回执集成。D01–D32、真实 Gate 与 I04 原 84 正式场景仍按实际运行逐项取证。
专用身份/硬预算、平台条件留册；不新增客户阶段验收，I12 后才交用户使用软件。

原生User交互的[受控进程耦合证据](../../../integration/execution/i04-native-interaction-coupled-result.json)补Windows原Core listener与User分派的真实stdout/stdin往返、重复回执不二发和Host退出关闭。Node假进程不连接provider，此结果不授予真实模型、完整Host启动、GUI或平台资格；Grok修改反馈原队列及原卡片/查看器仍继续。
