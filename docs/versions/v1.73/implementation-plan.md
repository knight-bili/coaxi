---
document_type: implementation-plan
version: v1.73
authority: version-implementation-and-acceptance
status: in_progress
last_updated: 2026-10-18
---

# v1.73 实施计划

原生子会话文件层已接标准 Codex/Grok 来源、原生父子核对、精确历史 rollout 和不截断字节分块，全部保持 Core 内部。实际启动捕获/原事务持久来源及已记录历史来源读取已接，当前仍须完整消息工具/答案归并、签名游标、Native/Web及原只读UI；文件检查独立于原数据库身份检查，资格见[来源映射](../../../integration/inventory/native-subagent-viewer-reference.md)。

1. 已完成确认登记、固定附件与新基线核对；基础严格输入和设置存储已编码，分项验证单独留证。
2. 持久身份、冻结资格、三类Profile、格式35、材料与原生投影已有分项接线；现登记v1.73/Schema161/Migration211及两条启动链，完整160/210仍为独立前置迁移。结构、回执、标记同事务，旧v1.72来源显式准入；更早测试沿原降级链保留真实旧来源，旧冻结输入不转成新版本。
3. User命令/原UUID、六模型操作、CLI及四兼容别名已有分项证据；继续完成问题/计划阻塞等剩余投影和实际Host/Runtime协调。Bootstrap/Dynamic Context按批准全文及冻结版本分支，普通私聊默认policy2，新角色才用policy3。
4. 接 Scheduler/Fleet、准备/输入接收、快速终态、审批、递归取消、原 Session 恢复、接管和后续完整草稿消费；每项扩所属 owner，不再建 broker 状态机。
5. 移植 Codeg 原 DelegationSettings、工具卡、分组、委派查看器；User-only 原生子会话读取、另一原生查看器、Grok 进度和后台观察也保留，不能互相替代。
6. 与 I04-T02/T03/T04 共同冻结阶段候选，集中构建/真实 Core 与 Runtime/必要平台验证、修复失败，再作阶段收口。当前不 push/Actions，不消耗模型预算。

当前重启接受输入分项：原启动恢复改用已有 Runtime 失败结算事务，按父先子后取消受控树，接管分支及其后续 User 输入保留。
未知输入仍等待原请求核对并拒绝领取，已正确等待时重复恢复零写；接受凭据、原任务 Blob、冻结配置及绑定保持，清理确认仍由 Adapter 独立负责。
7个原Rust owner及10项文档测试定向通过；Session/ACK/FakeHost明确为夹具，不新增owner或正式场景通过，不作为完整Host/Runtime验证或I04完成声明。
源码 `7c13ccb10` 的[重启结算分项证据](../../../integration/execution/i04-delegation-restart-result.json)固定10个源码Git blob、48份日志及6份批准工件核定，保留测试准备错误和重复写缺陷的失败记录。

## 分项记录

以下按实现先后记录，旧分项中的159/209及未登记状态保留当时事实；当前登记状态以上方实施步骤及最新迁移记录为准。

六操作统一执行器 `delegation_dispatch` 已接实际 Core 内核及公开 IPC/CLI：
start/resume 的目录与文件工作在独立线程执行，提交前重新取得原全局调用门并核对完整 CLI 租约，
不能仅凭同一个 Run/Native Binding 继续提交；status 使用原通知快照等待，等待时不占数据库或调用门。
同 Run 换租约会唤醒全部旧等待者，断线只结束等待、不取消帮手。
start/cancel/resume 在新工作鉴权之前通过不暴露身份的证明查询原 Gateway 回执；
退役租约没有记录就拒绝，改变参数仍冲突，read/history/status 每次重新鉴权和查询，不缓存旧页。
原 Context owner 复用同库覆盖完整六操作、同一请求别名回放、旧回执零写入、继续原 Session、
清理前拒绝保留、两种观察后换租约拒绝、双等待者和断线无副作用。
前批Builtin39/36项、六命令/四原别名、封闭输出/安全错误/证据投影元数据与无限等待读取截止已接线；IPC3/Envelope2/AgentOutput10保持。此前159/209原型状态由本批完整160/210启动迁移取代，历史分项证据保留当时状态。
批准的输入/说明/TaskReport和帮助文本使用原全文；原30项黄金结果保持。条件输出规则实际校验，未知结果不能夹带私有字段。
同一 Context owner 借用原数据库验证真实 Windows 管道帧和断线，不启动 Host 或模型；CLI虚拟时钟验证无限读取和原有界超时。
此批不等于真实 Host、全部 Runtime/平台、正式场景或 I04 完成。
旧无资格 Run 仍不能调用新业务操作，Bootstrap/Charter冻结内容不因目录版本补权；完整160/210与旧来源资格仍按原计划推进。
实际159无委派表时先在原 agent_run 核对冻结资格，旧Run的六操作稳定拒绝，不借查新表报内部错误；同一 owner 在候选表安装前验证零写入。
统一执行器的起止已接原 ExecutionEvidenceService/canonical lifecycle，共用实际返回封套；
原回执早返回不记第二笔，每次实际执行尝试独立，避免同号新查询/提交前失败重试污染旧观察。
关联使用原读模型的精确操作/输出摘要载体判定，provider事件夹具与真实Core调用分开标明。
开始记录失败不派任务；结束记录故障保住原提交和回执，started行供后续Host重启核对，不宣称自动补齐。
User list/get/history/read、设置读取/全局CAS保存已接Core与Native/Web方法及原UUID恢复；原分页算法复用，User/model游标隔离，User可读接管后历史而父模型水位不变，原attempt结果不被后续User回复替换。
父Run结束仍可读；设置事件仅版本，旧Run配置不重写；旧159库明确不可用，不装候选表。问题/计划等其他阻塞摘要仍待。
User resume已编码Core/两端运输/原请求恢复：独立User授权支持父Run终态，复用原计划/Session与attempt工厂；候选owned guard凭精确同事务User事件准入，模型活跃父分支不放宽。目录worker锁外、提交重验、事件/回执完整回滚须留定向证据。
接续完整160/210结构分类/升级、执行恢复协调、真实Host/Runtime与原UI；其他I04工作包和全部正式场景保持。
完整delta只读结构分类已编码：settings、全部新增对象、五扩展旧表和十个改动旧guard共用安装转换；旧159来源拒绝部分目标和被删改的旧约束。沿原身份迁移owner补来源/目标篡改、无害格式与额外列兼容、晚期故障回滚验证。接下来补policy3与新Run冻结资格、显式旧v1.72来源和两条升级链，再登记160/210；本项不提前改变现行159/209。
候选delta及额外冲突约束的[结构分项证据](../../../integration/execution/i04-delegation-schema-result.json)记录定向原Rust owner、文档门禁、源码与日志核定；没有新增owner、启动登记或阶段完成声明。
私聊policy3的工具/历史/终态版本与冻结角色检查已接线，普通默认policy2不变；生产准入改为精确v1.73/160和210。原私聊策略owner扩同库新Run创建/只读历史/最终私有回复及角色拒绝，原冻结资格owner补旧/未来合同拒绝；完整启动登记与旧Selection1/2冻结输入/排队兼容仍按下一批合齐，不能据此称160/210已开通。
源码 `596aa7818` 的[私聊权限分项证据](../../../integration/execution/i04-delegation-policy3-result.json)：5项原Rust owner和10项文档测试通过，8个源码文件/31份日志及6份批准工件核定；完整迁移及I04其余工作继续。

当前完整迁移源码 `49e084b38` 的[启动升级分项证据](../../../integration/execution/i04-delegation-startup-result.json)：登记Core v1.73/160/210，两条初始化链、旧159与旧140同文件迁移、结构/回执/标记故障回滚、旧选择1/排队选择2保持及历史回执清理后的技能包能力。13项原Rust owner与10项文档测试通过，19个源码文件/53份日志及6份批准工件核定；不新增owner、不作为完整Host/Runtime或阶段完成证据。

授权票据和中断恢复的[分项证据](../../../integration/execution/i04-delegation-authority-result.json)对应源码 `5491b025d`：
真实159票据升级、210回执失败整步回滚、重新准入/原库重开，以及210前/提交后和六个原历史分支的测试子进程中断恢复通过。
共7个原Rust owner取得定向成功结果；原批4通过/1失败如实保留，失败的进程owner修复后单独复验通过，不重跑全部历史来源。
两个测试准备修正限于私有临时目录/文件创建及无额外定义时恢复原SQL排版；生产安全规则和原库事务迁移架构不变。
没有新建普通升级快照/切换流程，没有完整Host/Fleet/App或模型运行；真实执行、重启未结活动、cleanup、原UI及其他I04工作继续。

依赖与客户事项以[执行计划](../../../integration/execution/i04-execution-plan.md)和[客户待办](../../../integration/execution/customer-todos.json)为准。

未结工具记录的[启动核对分项证据](../../../integration/execution/i04-delegation-reconciliation-result.json)对应源码 `92ea14624`：
Core 启动前捕获旧观察 UUID，原后台启动任务按批核对原回执与同事务的实际工具尝试关联，不重派业务命令。
缺少可靠证明时只记工具结果未确认，不改任务状态；退休 epoch 仅可维护精确原记录，普通 Runtime 过期围栏保持。
原 Context owner 通过真实210迁移与SQLite新连接、单行恢复故障/重试、原输出摘要、重复零写及实际rowid重用反例。
I04-TMP-CLEANUP-01 的晚期NotFound与其他IO分类修复及原全局gate不阻塞检查通过；完整进程/writer/清理资格仍按后续生命周期验证。
19个原Rust owner与10项文档测试取得定向成功，15源码Gitblob/40日志/6批准工件核定；不增加owner或84正式场景通过数。
完整Host/Fleet/App/真实Runtime未在此分项运行，原Session、输入unknown、目录/权限/模式/away及原UI与其他I04包继续。

## 原生交互存储接续（2026-10-18）

Core实际指针推进至v1.73/161/211，旧160是受支持来源，210仍独立核对其原160目标。纯转换之后补持久原生请求、User-only原UUID/CAS、安全User视图、发送标记/token、取消/绑定关闭及原启动恢复封旧请求。Grok修改反馈暂以明确围栏阻止丢失，不称已交付原队列。

复用原Context materialization owner的同一SQLite/Blob：真实Grok工厂/领取/绑定/输入投递，验证prepared发送边界、重复/冲突、错归属、User拒绝/CAS、回执失败回滚、精确回放、原生回复、发送未知不重发、计划反馈围栏和取消/重启关闭。Native Session/ACK及可执行文件身份为夹具，无实际模型进程；Cursor仍遵守现有未准入事实，不在测试里放开。原authority进程中断owner增加211前/提交后两分支，210提交后现在继续缺失211而非误称Current。

下一分项固定为可信Native回调及Host失效收口、User读取/决定运输入口、Grok原User队列登记；之后接r1阻塞摘要/原等待广播、原Codeg卡片与两类查看器，再补其他格式及真实Runtime/平台矩阵。I04其他工作包和原84正式场景不删减，阶段候选才集中验证；用户不承担阶段工程测试。

## User交互接口接续（2026-10-18）

完成runtime.interactions.list/get/resolve的Core服务、Application分派/原独立请求通道、CURRENT_USER封闭envelope、Native IPC名单、Web Operation和共享TS DTO。list默认20/上限50且只摘要；get返回原Core显式视图。全局User原UUID可在原目标删除后核对，不依赖UI重交归属。Web未知命令按问题编号隔离，保持同题未知阻挡、不同题独立、完整原Answer和原Promise只读核对结清。

扩原Context同库owner及Web原operation owner定向通过；两文件48项运输/allowlist检查、根类型检查通过。Web首次检查发现原默认操作分组把不同问题互相阻挡，改为interactionId后通过；保留失败日志。Native Session/输入ACK、程序身份及fetch为夹具，未验证实际HTTP、Native管道或GUI，不增加正式场景通过数，也不重复全量1254/平台构建。

下一动作保留为可信process_agent_run_acp_message回调和Host失效封旧、Grok原User队列登记；随后r1安全阻塞摘要、原广播、固定Codeg卡片及两类查看器，再完整Runtime矩阵与I04其他包。三接口已登记不表示Native模型请求可用，阶段仍研发中。

## 原生回调接续（2026-10-18）

四方法从原ACP listener接入持久登记和后台答复，原Cursor直接skipped/rejected分支退出。原Host句柄及确切Native prompt只在可信运输Target中保存；DB业务归属仍由原Core记录拥有。User等待不阻塞reader或持DB锁，原广播加5秒失联回查；答复走原Runtime route admission、持久token和写端重验，不在补回执时重写原生响应。Host退出和单请求失效关闭采用精确Core/Host/Run/epoch。

新增唯一最低层extended内存AsyncWrite owner，覆盖旧六身份、排队后换运行、Completed拒绝、JSON整帧、部分写超时/flush失败及隔离后不续写；原Context同库owner扩范围关闭/重复零写/回滚和单请求不动其他请求，原Grok分类owner回归。未创建新SQLite/Runtime fixture，未启用Cursor平台资格。自审修正writer坏与进程退出的含义分离，不用alive=false伪造清理；错误响应后的reap交原任务集合，不让reader回调长期持permit。

完整Native桥循环仍待可控Host耦合验证；组件通过不作为真模型、实际Host进程、HTTP/GUI或正式84场景通过。Grok修改反馈原User队列目前仍有明确发送围栏，必须随后接完；再接r1安全阻塞摘要、原卡片和两类查看器，以及其他Runtime格式/平台矩阵。I04其余工作包和阶段候选验证保持。

## 原生交互受控进程耦合接续（2026-10-18）

[本轮记录](../../../integration/execution/i04-native-interaction-coupled-result.json)补上前批明确未验的连接处：复用原隔离Core fixture，经真实Grok子会话工厂/领取/Binding/Context/输入dispatch标记、原ACP listener、受管Node假进程及Core.handle User三入口完成往返。两个请求先于任何User答案同时登记；完整原RPC字符串及问题/计划结果到达原stdin；原UUID重放不二次写入；第三个未答请求由原Host Exited封闭并核对closeReason。此为本机Windows的耦合验证，不是完整Core main/Host/HTTP/IPC、GUI、真实模型或逐平台资格。

新增一个slow-tests seam owner，转换/CAS/写端围栏仍归原较低层测试，不复制输入矩阵。测试准备中发现默认委派开关、Windows脚本执行身份/长路径及受控认证握手缺项，均修正假进程或使用原设置/文件身份登记入口；未放宽产品准入、伪造Ready或使用真实账号。失败日志保留。假进程脚本从精确源码位置复制到隔离目录，后续只改协议夹具无需重编Rust。

Grok修改反馈原User队列、安全blocked_summary及原广播、固定Codeg卡片/两类查看器仍是下一有限队列；其余I04工作包、21簇和84正式场景不减少，本批正式场景新增通过0。完整一期81簇/48包/324场景及I12后客户使用验收保持。

## 原生请求阻塞摘要与等待接续（2026-10-18）

[本批记录](../../../integration/execution/i04-native-interaction-blocked-result.json)接通r1允许的question/plan_approval安全blocked_on及User详情blockedOn；原permission输出和优先级保持。最早仍live的pending请求可见，决定后消失，题目/计划正文/答案/Native ID不进入父模型摘要。沿原status cursor与广播，无关唤醒和同一问题不重复返回，队首处理后下一请求使等待结束；Native和User路径共用仅ID/版本的两类失效通知。

扩原Context与受控ACP耦合两个owner，没有新增完整fixture或Rust测试函数。真实父Binding鉴权、原status等待、SQLite可取得、User详情、原Core事件输出，以及问题→计划→新问题的等待链路已验证；默认栈复验通过。测试栈溢出与Windows旧产物占用诊断保留，未提高默认栈或终止进程来取得最终通过。完整1256、真实模型/GUI/Host全链及平台矩阵未由这两个owner替代，正式84场景不计新增通过。

Grok修改意见仍未完成。核对固定Codeg的lifecycleSend/mqEnqueue和已确认r1的显式接管/三种Context mode，发现不能直接把Native计划答案塞进普通接管后消息队列。具体[核查记录](../../../integration/execution/i04-grok-feedback-boundary.json)保留原来源、必需结果和设计待核定项；尚未改相关输入/Schema或发起新的客户确认。先核定完整衔接，再按仓库治理决定是否需要独立revision，同时继续已授权原卡片/查看器和其余I04包，不缩减该功能。

## 接管后聊天界面接续（2026-10-18）

此前已验证的 child 快照和共享输入控制器现挂到生产 SingleChatPanel：固定原 child/发起队员，清理完成开放编辑，原任务说明和 Run 不丢。附件、引用、排队及保留输入复用 child 归属；普通私聊未绑定草稿不迁入。控制入口持久保存 UUID/命令并核对原回执，结束保留历史；抽屉和 Thread 离开保护贯通嵌套输入，隐藏恢复保持编辑器与阅读位置。

界面沿 Codeg 原抽屉与 Rovai 原聊天组件，只有 child 模式、容器嵌入和具体归属适配；日夜宽窄图及生产 Renderer 运输夹具结果见[接管聊天记录](../../../integration/execution/i04-child-chat-ui-result.json)。当前没有 Rust、Schema、依赖或模型输入变化，不因本切片重编 Core 或触发远程 Actions；原完整 App 对照、第二类原生子代理、实际 Core/Host/GUI/模型往返及其他 I04 包仍必需，正式场景新增通过 0。

## 原生子会话可信目标接续（2026-10-18）

第二类原查看器先接 Core 内部身份准入：从既有真实 Evidence/Run/Context/Bootstrap/binding 推导只读原生目标，Current User-only，不收 UI Native ID/路径/Runtime override；Grok 无显式 Session 不回退短编号，父运行终态停止旧状态动画。扩同一 Context 和原 Evidence 投影两个 owner，无新 SQLite 或测试 owner；[分项记录](../../../integration/execution/i04-native-child-authority-result.json)区分内部准入与未接的文件/运输/UI。没有新模型输入、Schema 或新公共 method，完整 I04 范围继续。

## 原生内容归并接续（2026-10-18）

已移植固定 Codeg Grok 的本层消息/思考/图片、promptIndex、累计工具和空完成、MCP/TaskOutput、已知问答答案、压缩与用量；直接长中文与工具 JSON 完整保留，显式后代文件再核父链。逐行消费核验块，记录坏行/尾帧且允许解析取消；稳定 fallback 时间消除重读漂移。Codex cut/seed 历史输入按精确来源拼接，保留原头部和回退替换。来源范围及有依据的适配见[移植清单](../../../integration/inventory/native-grok-content-port.json)。

复用原 source 文件 owner 和受控目录，无新 Rust owner、数据库或进程。extended-tests 库编译及精确列册1项/实际1项通过，10项现有警告保持；首次编译的重复derive边界错误已修，原失败保留。本批没有启动 main、公共运输、GUI 或真实 provider，不增加84正式场景通过数。具体证据见[本批记录](../../../integration/execution/i04-native-content-result.json)。

下一有限队列：Codex 原完整消息/代码模式/目标/用量解析；Grok 完整会话统计/模型上下文窗口目录及原完整 parser 差分；有界完整内容与签名快照/后代引用、可取消后台快照打开；Native/Web 共用 User-only 读取；固定 Codeg 原只读卡片/抽屉及同场景验证。旧无来源导入、真实 Host/Runtime/平台和完整 I04 四包/21簇/84正式场景继续，未完成项不因本层通过删除。

## Codex 原消息主体接续（2026-10-18）

原消息主体及其纯依赖现接核验记录：去重/promotion、思考/图片、计划确认/目标开闭与首问、代码模式/MCP/延迟 wait/shell、collab 与累计花费/占用保持原规则。直接输出完整保留，后代预览保持；原读取路径替换为 Core 后代/工程源行回调，未结后代统计按来源顺序稳定。显示摘要去掉原生路径/身份/启动字段，流式指纹覆盖实际投影与 patch 几何。原列表专用和 live 控制未用代码不为只读路径保留，也不加 dead-code 抑制。

沿原文件 owner 扩固定 Codeg 的22组断言和受控文件→wrapper场景，无新 Rust owner/数据库/进程；原版断言只换输入夹具适配，未改期望。新增正则依赖固定供体1.12.3及原图，percent-encoding复用已锁2.3.2，无其他依赖升级。实际编译、定向结果、失败历史和边界见[本批记录](../../../integration/execution/i04-native-codex-content-result.json)，不是原完整 App 差分、公共运输/GUI/真实模型或84正式场景资格。

下一有限队列：Grok 完整会话统计/上下文窗口目录和两原 parser 差分；可信工程文件回调与旧 agent 文件显式父链；完整有界内容/签名快照和后代引用、首次 snapshot 发现/hash后台取消；User-only Native/Web 与固定 Codeg 原只读 UI/同场景，再阶段 Runtime/平台和其余工作包集中候选。完整 I04 四包/21簇/84和一期81簇/48包/324不减，不新增客户工程测试事项。

## Grok 统计与固定原 parser 对照分项

Grok标题/型号/分支/时间、原统计和上下文窗口cache→config→id回落已接Core核验文件；缓存/配置只提供显示数值，不输出原路径或凭据。辅助源初次不可读可回落，已捕获源中途改写及取消继续拒绝。Codex原统计/标题规则移入共享实现，函数正文不变，模型推断/窗口合并也保持固定供体正文。

[本批证据](../../../integration/execution/i04-native-parser-parity-result.json)包含实际31组对照、参考程序原主体与102包锁图、普通及启用参考进程的精确原owner结果，以及明确长正文差异。参考配方入库，不依赖未提交的临时修改。没有新Rust owner/数据库，只有测试专用原parser进程夹具；不授予完整App/Host/运输/UI/模型/平台或84正式场景资格。Schema161/Migration211、产品依赖及6份批准r1不变。

下一项补可信工程patch行/旧agent显式父链，再推进有界签名读取、首次发现/hash后台取消、原版只读UI及同场景。Grok修改意见r1衔接、旧来源显式导入、Runtime/平台和其余I04工作包保持，阶段集齐再集中验证。上方旧检查点“完整统计/差分未接”保留当批事实，本节覆盖其当前状态。

## 可信Host工程目录与补丁源行接续

可信最终Host启动cwd随原来源事件持久保存为可选workingDirectory；旧v1没有字段仍读完整正文，只有另一份同Host/Run/epoch/绑定的明确来源记录才能补足，互相矛盾的目录拒绝选择。不从当前Camp、Run workspace或原生cwd补授旧权限。原User-only准入后统一读取Codex/Grok，Codex用根内实际核验文件提供原补丁几何，文件失败保留原patch；完整UTF8、单次读取缓存、快照改写及取消保持，Windows备用数据流拒绝。

[本批记录](../../../integration/execution/i04-native-project-patch-result.json)区分实际Core初始绑定写入、SQLite/Blob/真实文件读取，以及新旧历史记录兼容夹具；后者不当作完整Host复用。原source owner与原slow Context owner分层实际执行，没有新Rust owner/数据库/进程或正式场景通过；首轮误选feature、检查脚本和嵌套事务夹具错误保留，修夹具不放宽生产事务。Schema161/Migration211、依赖和模型输入不变。

下一项保留旧agent显式父链/旧来源导入、有界签名内容与后代引用、首次原生来源发现/hash后台取消和资源回收，再两端公共读取及原Codeg只读UI/同场景。完整Host/Runtime/平台、Grok反馈r1衔接及I04其他包继续；不重做已提交的工程回调，不将本内部读取入口当公共API或产品查看器完成。

## 旧Claude代理文件来源分项

固定Codeg的agent-*.jsonl来自Claude类代理日志；Qoder有部分共享解析但自身根/producer独立。Core以单独ClaudeAgentLink表达父Session、父工具和agentId；只在可信根内读取唯一父文件，核对原Agent/Task调用及结构化确认，再取精确父目录下的子文件。消息parentUuid不当Session，子文件sessionId也不创建独立Session授权；改写、遍历、矛盾/缺确认及重名父文件拒绝，完整UTF8不裁剪。复用原JsonRecords和快照，仅在runtime_subagent_session内部扩大可见性；目录扫描新增可选取消，现有无取消调用保持。

[分项记录](../../../integration/execution/i04-legacy-agent-source-result.json)保存锁定源码、初次缺测试导入的编译错误和修复后最低extended编译/精确原owner。无新Rust owner/数据库/进程、无模型输入/Schema/依赖变化，Source层不是Core User grant或完整Claude parser。

下一队列优先让已接C/G完整解析进入有界签名读取、后台取消和原只读UI；Claude完整消息/用量/生命周期、生产捕获与证据关联、Qoder独立来源和旧源明确导入并行列入本I04剩余清单。四包21簇84及一期81簇48包324保持，不把本次分类纠正当作删除能力或阶段完成。

## 原生完整签名快照与可取消分页

Core User解析后生成整份不可变投影，私有NativeSnapshot按4至32768原始UTF8字节输出窗口，保留所有消息、工具正文/JSON、嵌套元数据、摘要和统计。客户端收齐、按偏移及完整摘要核对后再解析；单窗口不是独立JSON。快照/游标签名绑定精确身份、来源摘要和冻结内容，仅核实后代有引用；状态/live沿当前Core目标，不在冻结内容内伪造活跃状态。

首次目录扫描、源文件hash与Grok父关联完整解码及投影序列化使用同一停止标记。自审纠正write_all会重试Interrupted的取消隐患，改为终止错误，避免取消后无限重试。原Grok大于1MiB的父更新或meta不再因旧预览上限被跳过。复用既有安全打开/完整记录解码和原parser，无依赖/Schema/模型输入变更。

[本批记录](../../../integration/execution/i04-native-snapshot-result.json)保存最低extended编译、精确原文件owner及自审结果；一个既有owner扩同一目录，检查完整长内容重组、UTF8/转义、签名篡改/混用/错误身份和来源/越界、后代隔离和取消后零追加。未新增Rust owner或数据库/进程夹具，84正式场景新增通过0。后台任务注册/销毁、每页Core重新授权、Native/Web运输、原Codeg只读UI及同场景、真实Runtime/平台仍须继续；组件对象不构成公共授权或I04完成。

## 原生后台阅读与四个User入口

先前完整快照检查点的后台/重授权待接项，本批已进入源码。进程内Registry、独立std线程和4个工作名额负责首次发现/hash/解析；16缓存及按访问清理的5分钟空闲租约、关闭墓碑/饱和保护、Core退出及迟到结果丢弃保持阅读独立。每poll/page重做原Core resolve，后代沿原root核对，不接受UI自报原生Session/目录。Core原运行创建时间作为缺原生时间戳的固定回落，不用打开时的当前时间。

Native renderer和Web加入start/poll/page/close四方法；Web闭合过滤字段并保留完整fragment，User actor由Core封闭派生，Builtin/模型目录不增加方法。沿原source owner实际受控线程/文件、原Web projection owner和原Desktop/Web client owner检查，不新建数据库或外部进程；名单和受控运输不能当实际Core handle/Host HTTP/IPC/GUI资格。[本批记录](../../../integration/execution/i04-native-reader-result.json)保留首次通过后自审时间回落修正及最终验证证据。下一项接实际Core/两端调用与固定Codeg原只读UI/同场景；Claude/Qoder旧格式、Grok反馈r1及完整I04四包84仍保持。

## 原生只读查看器客户端与共享窗口

NativeChildReader 已接完整分页/摘要核对、关闭/迟到围栏、只在 live 刷新、失败保留正文和后代父快照租约。公开执行区及私聊原执行记录接结构化原生帮手入口，已知截断列表沿现有证据完整读取 API 保留原 receiverIndex。原生与 Core Delegation 每层共用一个 Host，保留原操作的离开保护；原 Codeg collab-tool 纯辅助文件加来源头后原文移植，未增加依赖、Schema 或模型输入。

[本批记录](../../../integration/execution/i04-native-viewer-ui-result.json)区分29项新读取/模型检查、82项既有检查、9项离开/动作检查、实际生产 Renderer 的原生场景与两项既有委派场景。Renderer 使用受控运输；参考运行只包含原生 Drawer/header/角色外壳，正文适配器明确为标记夹具，不等于完整原 App 对照。实际发现并修复错误提示插入正文造成阅读位置跳动，关闭点击区域按原32px修正。完整原富内容仍按[有限清单](../../../integration/inventory/native-session-viewer-ui-reference.md)继续，真实Core/两端/Runtime、Claude/Qoder和其他I04工作包保持，84正式场景新增通过0。

## 原生纯内容适配与原计划目标组件

固定 Codeg ai-elements-adapter 及必要纯依赖已移植，原类型按实际引用抽取，不导入原应用 store、工具执行或Tauri。原计划/模式/提案、目标组件与溢出测量主体仅换import，接Rovai安全Markdown、语言和只读界面原语；样式用供体已锁Tailwind4.1.18生成并隔离，不增加生产Tailwind或全局preflight。仅新增供体已用的tailwind-merge3.4.0解决原class冲突；781个旧package/snapshot及原importers保持，MIT通知保留。

[本批证据](../../../integration/execution/i04-native-rich-content-result.json)记录原适配器148项、原生边界5项及既有读取/配对29项共182项定向通过，实际生产Renderer17组检查与原组件参考。供体原测试中只有1904–1959的live producer参数化用例未移入；只读数据不来自该store，其他期望不改。自审补齐重复ID/跨turn结果，以及图片伴随完整工具文字、未配对结果多图/统计；原适配器通过默认false内部选项保持原调用结果。参考运行原NativeDialog/adapter/计划目标JSX，部分宿主原语显式适配、其他内容路由为标记子集，不授予完整原ContentParts/App/主题或真实Core/两端/Runtime资格。具体工具/代码模式/搜索/Diff、图片交互、旧格式及I04其余包保持，84正式场景新增通过0。
