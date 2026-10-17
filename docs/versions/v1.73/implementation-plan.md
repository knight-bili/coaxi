---
document_type: implementation-plan
version: v1.73
authority: version-implementation-and-acceptance
status: in_progress
last_updated: 2026-10-18
---

# v1.73 实施计划

1. 已完成确认登记、固定附件与新基线核对；基础严格输入和设置存储已编码，分项验证单独留证。
2. 持久身份、冻结资格、三类Profile、格式35、材料与原生投影已有分项接线；现登记v1.73/Schema160/Migration210及两条启动链。完整结构、回执、标记同事务，旧v1.72来源显式准入；更早测试沿原降级链保留真实旧来源，旧冻结输入不转成新版本。
3. User命令/原UUID、六模型操作、CLI及四兼容别名已有分项证据；继续完成问题/计划阻塞等剩余投影和实际Host/Runtime协调。Bootstrap/Dynamic Context按批准全文及冻结版本分支，普通私聊默认policy2，新角色才用policy3。
4. 接 Scheduler/Fleet、准备/输入接收、快速终态、审批、递归取消、原 Session 恢复、接管和后续完整草稿消费；每项扩所属 owner，不再建 broker 状态机。
5. 移植 Codeg 原 DelegationSettings、工具卡、分组、委派查看器；User-only 原生子会话读取、另一原生查看器、Grok 进度和后台观察也保留，不能互相替代。
6. 与 I04-T02/T03/T04 共同冻结阶段候选，集中构建/真实 Core 与 Runtime/必要平台验证、修复失败，再作阶段收口。当前不 push/Actions，不消耗模型预算。

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
