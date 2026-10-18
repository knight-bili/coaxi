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

原生后台阅读及四个User读取入口已编码：注册层持有真实worker名额至结束，关闭/迟到/重试和空闲租约分离于Runtime控制；每次取页重新解析原Core引用，后代保留原root归属。Native/Web名单和闭合Web投影已接。[本批记录](../../../integration/execution/i04-native-reader-result.json)区分源码/组件资格与实际Core/Host/网络/原UI资格，I04仍开发中。

原生完整投影已有私有签名快照及UTF8窗口，整份消息、长工具JSON、嵌套元数据、摘要和统计不逐字段裁剪。引用绑定冻结身份与来源摘要，仅为核实后代签发入口；首次发现/hash及序列化可取消。最低extended编译和精确原文件owner通过，[本批证据](../../../integration/execution/i04-native-snapshot-result.json)不包含该批当时未接的后台任务注册/销毁和每页Core重授权，现已按上方新记录接入源码；其旧分项记录仍不授予原只读UI或实际网络资格，I04全范围不变。

旧 `agent-*.jsonl` 已核为Claude一类的子代理文件，不能混作Codex/Grok Session；Qoder只确认复用部分原解析规则。[来源清单](../../../integration/inventory/native-legacy-agent-source.json)与[本批记录](../../../integration/execution/i04-legacy-agent-source-result.json)保存精确父Agent/Task调用、结构化agentId及对应子文件的私有读取分项。原文件owner实际验证完整长正文、错源/矛盾/遍历/改写与取消；这不是完整Claude parser、生产来源关联或用户查看器完成。现有C/G有界接口和原UI可独立推进，各格式仍在I04统一收口。

原生子会话的工程补丁读取已接Current User服务入口：可信Host最终启动cwd随来源持久保存，原生cwd只选根内位置，后来的绑定/epoch不会改用当前目录；旧v1缺目录仍保留完整原文。两个既有owner验证实际Core/SQLite/文件到原Codex行号的链路及越界、改写、取消和Windows备用流拒绝，[本批记录](../../../integration/execution/i04-native-project-patch-result.json)保留失败修复及资格。没有公共运输或UI变更，Schema161/Migration211和模型输入保持；旧agent来源、分页/后台取消与原查看器继续。

Grok 完整会话统计、显示摘要及原模型窗口缓存→配置→型号回落已接入。两份固定原解析器已实际运行31组受控文件对照：Codex24、Grok7，其中30组完整投影相同，1组仅完整保留原版截断的两段工具正文，未发现未解释差异。[本批证据](../../../integration/execution/i04-native-parser-parity-result.json)与[复现配方](../../../integration/verification/native-parser-reference/README.md)保留源码、依赖和验证边界。该结果不代表完整原App、公共运输、原UI、真实Host/模型或84项正式场景通过；模型输入、Schema161/Migration211、产品依赖不变。

Codex 原消息解析主体及代码模式/MCP/延迟等待、目标/计划、去重/提升、思考/图片与用量已接同一核验文件链路，具体[接续证据](../../../integration/execution/i04-native-codex-content-result.json)区分原版断言与实际受控文件、完整差分及公共查看器资格。新增依赖仅为供体锁定 regex1.12.3 图和已存在的 percent-encoding2.3.2，无其他包升级；模型输入、Schema 和正式场景数量不变。

此前固定 Codeg 的 Grok 消息/工具/答案纯解析、共享材料/归并规则及 Codex cut/seed 历史输入拼接见[内容分项记录](../../../integration/execution/i04-native-content-result.json)。该记录保留当批资格，后续消息解析、统计及受控原parser对照以本页上方接续为准；公共有界运输、原UI及完整应用同场景仍待，没有新增正式场景通过数。

本批启动来源及历史读取沿现有 Core 权威接线，未改模型输入或 Schema161/Migration211；主程序检查与同库/文件分项结果见[来源持久化记录](../../../integration/execution/i04-native-source-origin-result.json)。这不表示真实 provider、公共运输或原查看器已完成。

原生子会话文件来源层已编码：核对 Codex 原生父链/精确历史 rollout 和 Grok 父更新/子元数据，保留完整分块字节与问答历史。实际启动来源捕获、原绑定事务持久化和已记录历史来源读取已编码并分项检查；消息工具归并、签名游标、公共运输和原UI仍待接。分项验证见[原生文件来源记录](../../../integration/execution/i04-native-child-source-result.json)，不得据此称完整查看器或I04完成。

上一版本 [v1.72](../v1.72/README.md) 的历史记录及未完成环境验证保留。
本版本落实客户直接确认的 [I04 独立帮手 r1](model-context-change-native-delegation.md)。
一期完整范围仍为 81 簇、48 包、324 正式场景；本版本不是一期最终 MVP 完成声明。

## 研发状态

当前源码登记 Core v1.73/Schema161/Migration211。原完整160/210委派迁移保持独立目标，160现在是受支持来源；211单独增加原生User交互存储，不重写210或旧输入。两条启动链按原事务顺序安装，旧v1.72/159仍可沿原链升级。此前160/210的[启动升级分项证据](../../../integration/execution/i04-delegation-startup-result.json)保留原时点，当前211边界见[原生交互合同](../../contracts/delegation-v1.md#原生user交互存储)。不表示用户日常数据库已升级。

原生提问和计划审批已有纯转换、Core 持久请求、User-only 答复/CAS/原回执及可信回调，写端在同一串行锁内重验活跃 prompt；Windows 受控假进程耦合已另有分项证据。安全阻塞摘要、广播和固定 Codeg 问答/计划卡片已接公屏、精确私聊与 Core 委派查看器。Run 取消、绑定失效与启动恢复会封旧请求；Grok 修改意见已保存，原 User 队列尚未接入前仍明确禁止发送。完整 Core main/Host/GUI/provider 往返和平台矩阵仍待，不以组件或假进程替代。没有改变批准 r1 文本、Bootstrap 或父模型历史。

六项模型操作、四个兼容别名、User 查询/设置/恢复及材料操作已接入 Core 与相应运输入口，并有分项证据。设置仍默认关闭，深度 1..8、缓存 512 MiB/0 不限、精确版本更新；原 Codeg 设置界面和完整产品使用流程仍待接入。
持久委派/attempt/control、父链、任务 Blob、原请求回放、独立私有路由、接管序号、取消及原 Session 恢复已有 Core 实现。真实 Host/Fleet 调度、writer/cleanup、输入接受结果核对和进程重启协调仍须完成，不能将分项运输或数据库检查称为完整执行交付。

新 Run 按冻结资格选择帮手 Profile11、公开父 Profile12 或私聊父 Profile13，使用合法的格式35和批准的 Charter；私聊父权限为独立 policy3，普通私聊默认 policy2 保持。帮手使用独立身份、空记忆和空平台技能入口，材料及原生技能/MCP过滤已有分项检查，完整 Runtime 资格另行验证。
旧 Run 不因设置变化而补权；旧选择1、排队选择2、原配置、Blob 正文和摘要的兼容回放已沿原 Context owner 验证。该证据使用真实 SQLite/Blob 和 Core 路径，原生 Session、输入 ACK 与终态仍有明确夹具边界，未证明真实模型运行。

Core 委派查看器已复用 Codeg 原抽屉、卡片、图标和 Rovai 原执行组件；接管后共享聊天与材料、控制回执、隐藏恢复和离开保护已有隔离生产 Renderer 证据，见[本批记录](../../../integration/execution/i04-child-chat-ui-result.json)。尚未完成的产品工作包括完整 Host/Runtime 生命周期、Grok 反馈队列、away/manual 资格、原 Codeg 设置和另一类 Runtime 原生子代理查看器、原完整 App 同场景、三类记忆和自动化等 I04 既定工作，以及阶段候选验证。I04 仍为研发中；全部 21 簇、4 个工作包、84 个正式场景保留。

第二类原生子会话的内部候选解析已编码：Current User、原 Evidence/Run/epoch/Context/Bootstrap/binding 校验，无额外 Session/路径输入；观察 receiver 不自动授予子会话内容读取权。[本批记录](../../../integration/execution/i04-native-child-authority-result.json)只记录内部准入与同库断言；原生来源/父子证明、全文与分页、Native/Web 和原 UI 仍依[有限队列](../../../integration/inventory/native-subagent-viewer-reference.md)继续，不改变模型输入、Schema 或公共方法名单。

## 跨版本文档影响

| 范围 | 结论 | 证据或理由 |
| --- | --- | --- |
| Version lifecycle | 已更新 | 本概览、[实施计划](implementation-plan.md)、[版本索引](../README.md)与旧版 lifecycle 同步；旧实现事实不改写。 |
| Decisions | 已更新 | [V1.73-D01](decisions.md#v1-73-d01) 说明为何复用 Core 而不移植第二套 broker 业务权威，[导航](../../decisions/CURRENT.md)同步。 |
| Contracts | 已更新 | [Delegation v1](../../contracts/delegation-v1.md) 区分已实现基础输入/设置与待接入执行合同；[Builtin Transport v39](../../contracts/builtin-tool-transport-v39.md) 增六项委派、精确租约回放、封闭输出与无限等待，完整Host生命周期资格单独留证。 |
| Architecture | 已更新 | [委派架构](../../architecture/delegation.md)规定单一权威和原生子代理区别；领域词汇在根 CONTEXT.md 同步。 |
| UI | 已更新 | [源码映射](../../../integration/inventory/delegation-viewer-ui-reference.md)与[会话呈现](../../ui/components/conversation-workspace.md#已接管的独立帮手)记录 Codeg 原抽屉/Rovai 原聊天的复用及 child 边界；第二类原生子代理、设置及原完整 App 对照仍待。 |
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

原生查看器接续：已在 Renderer 接通完整分页读取、原生帮手入口和与 Core Delegation 共用的单槽/嵌套只读窗口；日夜宽窄、原列表32项、隐藏/迟到及刷新位置有受控运输的实际 UI 证据。原版参考仅核抽屉/头部/角色容器，完整富内容及真实Core/HTTP/IPC/Runtime仍待，不宣称完整原App对照或I04收口。见[本批结果](../../../integration/execution/i04-native-viewer-ui-result.json)和[原组件映射及未完项](../../../integration/inventory/native-session-viewer-ui-reference.md)。

原生富内容接续：固定来源纯适配层及原计划清单、提案/模式、目标卡片已接产品只读页，182项定向与17组受控运输Renderer检查通过；原图片混合正文、未配对多图及统计保持完整。原NativeDialog/adapter和本批组件参考已运行，但内容路由及部分宿主原语有明确边界，完整原富内容/App与真实Core链路仍待。详见[分项结果](../../../integration/execution/i04-native-rich-content-result.json)及[来源清单](../../../integration/inventory/native-content-ui-port.json)。

原生工具原语接续：原工具外壳、命令输出、JSON/代码展示与完整折叠原语已接产品只读页，修复JSON路径串状态及代码中间内容旧缓存；40项定向、24组实际产品窗口检查通过。这里只资格原语和受控运输UI，完整专用卡片、Core链路及许可文件缺口仍待，I04不提前收口。见[本批记录](../../../integration/execution/i04-native-tool-primitives-result.json)。
