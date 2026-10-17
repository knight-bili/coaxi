---
document_type: protocol-contract
contract: delegation-v1
authority: core-delegation-identity-settings-and-wire
status: accepted
version: 1
last_updated: 2026-10-18
---

# Delegation v1

### 重启后的接受结果与清理

既有启动恢复筛出的无其他阻塞、已接受但结果遗失的输入，必须通过原 Runtime 失败结算事务处理，保留 `accepted_input_outcome_unknown` 原因和接受证据，不自动重发。
父子同时符合条件时按持久委派深度先父后子；受控子树取消、私有投递及 Turn 状态与父结算同事务提交，任一失败整笔回滚。接管切断旧父控制，接管后的 User 排队输入不受旧父恢复影响。
仅派发而未取得接受确认的 `delivery_unknown` 保持等待原请求核对，调度领取必须拒绝。输入摘要、Manifest、原绑定/代次、原生输入编号及任务 Blob 不替换。
Runtime 清理确认是单独的 Adapter 事实；接管或业务失败不等于旧进程已停止，清理尚未完成时原发送/恢复门禁继续生效。

### User 查询与设置运输

`delegations.list` 接 `{parentConversationId,cursor?,limit?}`，默认20、最大100，按确切父 Conversation 和 Thread 读取；按创建时间及稳定ID倒序分页。
签名游标固定父 Conversation、Thread、User、首尾位置，不允许跨父、跨身份、改签名或与其他分页交换。
`delegations.get` 接 `{delegationId}`，返回稳定归属、当前 attempt/Run/epoch、控制版本、原始 Run 状态、接管与清理状态、当前可核定的阻塞摘要。
列表和详情不夹带任务正文、结果全文、原生凭据、Runtime配置或原目录。当前阻塞摘要接原权限请求；问题与计划等其他阻塞类型仍须补齐。

`delegations.history` 和 `delegations.read` 使用批准的模型分页输入 shape，由 Host 的真实 User 身份和持久 child/Thread 关系授权，不能使用模型活跃父 Run 的鉴权替代。
历史只含已发布 User 正文、委派输入、原 Runtime 固定最终回复及安全附件引用，不含草稿、路径、思考片段或传输密钥。
User 能查看接管后的完整会话；父模型原接管水位和 child 自身 CURRENT_INPUT 排除边界不变。
复用原固定结果与UTF-8有界分页算法；User `read` 始终锚定原 attempt 结果，不因接管后有新的 User Run 就把新回复冒充旧任务结果。
User/model游标有独立受众，不能交叉使用。纯查询不写数据库；父执行结束、全局开关关闭或成员历史保留不阻止 User 读取，删除中的 Thread 拒绝。

`delegations.settings.get` 严格接 `{}`。`delegations.settings.update` 接 `{commandId,command:{expectedVersion,patch}}`；业务 patch 只含批准的四字段，外层显式null/未知字段拒绝，嵌套 Runtime preset 保留原 `mode_id` 空值兼容。
使用原全局 User Command Gateway、CAS、同一事务的设置事件与首次回执；只更新出现字段，设置切换不重写任何已冻结 Run。
`commands.reconcile` 对此命令只核对原编号与完整原参数，不派发保存；同号改参数冲突，旧回执零写入，事件或回执存储故障整笔回滚。
公开 `delegation.settings_changed` 仅有版本，不广播设置值。Native/Main和Web只放行已接线方法；Web丢回复后保留未知请求，明确重试须原UUID及参数。
对未升级的旧159来源，查询返回 `delegation.unavailable`，设置写入保存明确拒绝，不做部分DDL安装或假写160/210标记。生产启动现通过完整210迁移进入160后再提供这些入口。
这些是已接线的后端入口；原页面、完整迁移与真实Host/Runtime验证仍按实施队列推进，不能算整项交付。

### 私聊权限版本

普通私聊默认仍使用既有policy2。只有新Run冻结了合法private_parent资格，才使用policy3；旧Run的版本、配置与输入不因当前开关改变。policy3保留原只读Thread/私聊历史及Mission操作，并增加批准的六个delegation操作和四个兼容别名；实际委派仍须经过原Binding、父Run、范围、冻结开关和当前开关检查。

工具入口、私聊历史与私聊终态写回共同核对版本和冻结角色：1/2不能携带新角色，3必须是private_parent，未知版本/非法角色拒绝。原输出路由不可变约束继续禁止直接改写权限版本；历史读取仍按本Conversation和原输入序号限定，最终回复只写回该私聊。

新角色的生产准入精确要求v1.73/Schema160及210回执，并以Core完整启动合同准入为前提；旧合同或未来未支持版本不能仅靠schema>=160开通。本批组件夹具中的手动准入标记不构成真实启动迁移验证。

### User 恢复原任务

`delegations.resume` 接 `{commandId,command:{delegationId,expectedAttemptNo,reason?}}`；reason沿批准的非空UTF-8、最多2KiB规则，不接受task、目录、Runtime或伪造父身份。
这是独立全局User命令：先按原编号与完整参数只读恢复首次回执，再查询目标；目标后来关闭或删除不导致重派，也不要求旧模型租约仍有效。
新工作核对确切父Conversation仍开放、Thread/成员/权限有效、原冻结资格与当前开关/深度、展示attempt未过期。父Run可已正常结束或被取消，不能因终态伪造新的活跃父Run。
复用模型恢复的原任务Blob/初始消息摘要、原attempt取消和清理确认、accepted输入、原Runtime/Session/Binding/兼容依据、未结动作/审批/checkpoint/writer与目录检查。
准备和目录观察分开，等待有界worker和观察文件时不持数据库锁；提交前重新核对全部状态及原请求，接管或attempt竞争拒绝。
比较影响原任务的身份与执行事实；父会话无关消息或缓存设置的版本变化本身不使恢复失败，开关/深度及成员资格仍每次重验。

保存仍复用原私有attempt工厂、控制/Conversation CAS、批准的固定继续正文和可选中断说明，不重发原输入或冷建新会话，不改旧Manifest/结果。
User恢复保留上一attempt的持久父Run/epoch作为因果依据，trigger call为 `user-command:原UUID`；User事件与新Run/attempt在同一事务提交，事件只含稳定ID与版本。
候选 `delegation_attempt_insert` 的模型活跃父分支保持原条件；独立User分支必须存在精确对应该Delegation、新Run、attempt、原UUID及User actor的同事务恢复事件，并核对上一attempt原父依据。
缺少或写不下该事件仍拒绝终态父运行，回执写入失败整笔回滚。事件是Core核定的User操作事实，不能用模型输入或运行观察冒充。
成功返回原Delegation/child、当前attempt/control version及新queued Run；公开失效只含Thread/Conversation ID。Web未知结果先核对，明确重试保留原UUID和参数，按具体Delegation分组。
模型的恢复入口继续认证活跃父Run/Binding，不能使用User的终态父分支；实际原生恢复仍必须成功，原有禁止cold fallback的六处执行边界不变。

完整批准边界见[独立变更说明](../versions/v1.73/model-context-change-native-delegation.md)。本合同区分基础实现和待接线能力，不能据此判断一期或 I04 已完成。

## 严格输入基础

delegation.start/status/cancel/resume/read/history 的输入 shape 由已确认 r1 完整机器附件 after.operations 拥有；Core `delegation_contract.rs` 实现相同解码边界。
六个对象都拒绝未知字段。父身份、epoch、Binding 与授权不能由模型提交。必填及已出现可选值不得为空/null；Runtime 只接受既定 21 个标识或 custom 稳定标识，实际已安装/启用/权限准入仍由执行层核对。
working_dir 须为 Host 绝对路径；语法通过不是目录授权。reason 限 2048 UTF-8 字节且不是新任务。status 保持编号顺序及重复项；省略 wait_ms 是快照，0 是无期限，正数上限60000ms。read 默认16384/范围1..65536字节；history 默认20/范围1..100条。
这里只实现参数与等待数值规范化，不实现等待订阅、业务读取/结果、Core API 或 CLI。

## User 设置存储基础

表 delegation_settings 为 singleton=1，version>=1；enabled 默认 false、depth_limit 默认1且范围1..8、per_runtime_defaults 默认空对象、completed_cache_max_mb 默认512且0不限。每个目标配置沿用 Codeg mode_id/config_values 形状：mode_id 可省略或 null（表示无覆盖），config_values 可省略且空映射不序列化；值是配置字符串，不是凭据或执行授权。嵌套供体默认配置的 null 规则不同于外层严格 patch，不能混用。
User API 更新输入为 {expectedVersion,patch}，patch 仅 enabled/depthLimit/perRuntimeDefaults/completedCacheMaxMb。缺字段保持原值，显式 null/未知字段拒绝。深度/缓存非法时零写入。每次成功更新精确比较版本并递增；开关请求只携带 enabled，不覆盖其他配置。
更新原语接受调用者事务；随后回执/事件失败应回滚同一事务。读取只读，不懒建表、不重设配置。SCHEMA 已由完整 Migration210 登记到新库/旧库启动链；User-only Command Gateway、幂等回执/事件与 Host 两端入口有分项实现，产品页面及完整执行资格仍按实施计划推进。业务路径不自行执行建表字符串。

## 持久身份与接管存储

Core `delegation.rs` 已实现身份登记与精确父查询原语，`delegation_schema.sql` 定义 delegation/attempt/control。原请求由绑定 ID/generation、父 Run/epoch/tool-call 固定；同身份同 task 字节、目标和原始 working_dir 回放原身份，不调用创建回调，也不重新冻结变化后的配置。换参数拒绝。新登记才核对当前启用状态、成员/父会话/执行/绑定、从持久父链派生的 root/parent/depth；深度同时受1..8和当前设置限制。
子 Conversation、消息、私有 Turn 和 queued Run 由 Core 调用者在同一事务创建；身份/initial attempt/control 一起提交。原始任务继续由 managed_blob 外键持有，初始消息正文 SHA 必须与该 Blob 一致，失败由调用者回滚同一事务。查询只接受当前可信父 Conversation 的直接委派，错父返回缺失；这里没有开放网络读取 API。

候选身份 DDL 只扩展精确 child kind、delegation/delegation_user invocation 和 delegation_child_v1 policy。子执行不能退回公屏成员 policy，task_id 必须为空，输出必须送回同一个 child，Turn/触发消息/成员/Thread 同属；每个 child 仅一个 queued/running/waiting Run。原公屏/活跃私聊索引不扩进 child，父与子不能复用绑定或同安装的原生 Session。新身份、attempt、Conversation/Run/Turn 归属不可改绑，不增加 AgentProfile。

control 原语按精确版本记录接管，要求旧 Run 已经终态，冻结该事务的消息序号；后续用户消息不能抬高旧父的可见边界。同原命令可回放，其他旧版本拒绝，已接管的控制事实不可改回。该原语不执行递归取消、进程清理、用户命令鉴权、权限校验或后续新输入准入；这些仍须由完整 Core 服务在同一命令链中实现，不能仅记录 control 就宣布接管完成。

`db_delegation::install_identity_schema` 是待合并迁移的一部分，只在隔离 Core 夹具执行。新对象检查核对完整 DDL，不以同名触发器存在代替约束；该检查尚不等于完整数据合同分类器。现有表重建复用通用 helper；中途失败回滚与已有行保留已分项验证。设置、旧表扩展和格式版本还要统一完整准入。

## 完整执行待接入

### User 委派控制运输

`delegations.takeover` 接 `{commandId,command:{delegationId,expectedControlVersion}}`，`delegations.cancel` 接 `{commandId,command:{delegationId,attemptNo}}`。未知身份字段、任意 Thread、路径或原生 Session 均拒绝；Core 从持久关系派生 Thread，以本地 User 身份封装命令。原 UUID 通过 `commands.reconcile` 只读恢复，Web 未确认请求按操作和委派编号分组，明确重试保持完整原参数。

接管在同一 Gateway 事务内核对 control 版本、调用原 Run 取消结算关闭受控树的活跃输出、冻结 child 的当前消息序号并持久保存 control 与回执；任一后续写入失败整体回滚。已经终态的原 Run 保持原结果，接管仍关闭它下面尚受控的后台子树。单项取消仅接受当前 attempt，终态根为 no-op，不借此停止后台孙会话；父 Conversation 整体关闭另行接线。

树遍历使用当前 attempt 的触发父 Run，独立接管的后代切断整个分支，后续 User Run 不属于旧 delegation_attempt 树。清理门禁与树取消共用同一范围；取消业务终态和真实清理确认分开，未确认清理、恢复或未知输入时不得准入新的 User 发送。接口响应保存当次 `cleanupPending` 事实，原 UUID 回放不根据当前状态重写首次回执。User 控制及下述父级因果入口已接 Core；真实进程/writer/Fleet 与完整恢复仍须接续验证。

### 父级因果取消与私有完成

父 Run 停止、失败、派发失败或未知输入终结，在原事务内按当前 attempt 的 `trigger_parent_run_id + trigger_parent_epoch` 选取直接委派，再关闭仍归其控制的子树。不同轮次、已独立接管的分支和后续 User Run 不属于旧父运行；终态中间节点不阻挡父运行或父 Conversation 整体关闭时对后台后代的处理。Single Chat 结束使用父 Conversation 范围，成员离队使用该 Thread 内归属成员的父 Conversation 范围；成员失效本身仍有权结算其新的 User Run，不能把这种权限与旧父运行权限混用。

正常成功不触发因果取消。`delegation` 与 `delegation_user` 的成功结果沿原私有完成算法保存到自己的 Conversation，原消息、Run 终态、回执同事务提交，禁止走公开回复或业务 Task fallback；旧 Single Chat 的 policy1/2 分支不变，policy3 完整准入仍待合齐。子会话完成后保留后台帮手，待明确停止、所属会话关闭或成员失效时按相应范围处理。Thread 聚合删除须先释放 delegation/attempt/control 关系，以及已发布私有排队输入对消息/Turn/Run 的引用，再删除其 Run/Conversation；旧库不存在排队表时沿原路径，仍由原外键及事务保证完整性。这里不代表真实进程/文件清理和完整启动迁移已经通过。

### 新工作父身份与目录观察

`delegation_admission` 从原 Team Tool attested Binding 验证当前 Run/epoch、实际成员和 Thread，不接收模型自报的父身份。新工作必须具有匹配 Conversation/invocation 的冻结角色、完整配置摘要、未结束的绑定、在场成员，以及同时有效的冻结/当前开关和深度。旧私聊没有资格时不能因当前开关开启而获得委派工具；接管前的原 attempt 和接管后的 User Run 使用各自当前控制状态。内部证明携带短期凭据，不可调试输出或序列化。

目录观察在数据库锁外进行，默认取该 Run 冻结的实际 executionRoot；显式目录须存在且为目录，按真实 canonical 路径判定范围，父目录跳转、前缀相似目录及目录链接不能扩大范围。观察保持原 access/isolation，不授予额外写权限。原范围内登记在同一事务重验父身份、设置、配置、请求编号、原目录请求及计划，之后接原生产工厂。观察 token 不能跨运行或原 tool call 使用。原 Core 准备/claim 对 child 的目录改为核对同归属 delegation 中冻结的完整 workspace，而不是要求等于当前 Camp 根；Run/关系的目录、access 或 isolation 错配拒绝。旧 public/SingleChat 的原 Camp/Mission 目录校验保持；完整 Host 在各边界的观察与权限重验仍需接齐。

超出父范围只能得到“需要 User 权限”的观察结果，不能从观察直接构造可登记 Workspace。本批没有创建或消费持久审批；真实权限请求/决定/停止与重启协调仍是必需后续。该组件只用于新的工作准入；完整模型入口必须先按原请求身份恢复首次结果，再做新准入，不能用新资格校验替代既有请求恢复。

### 执行前的外部目录审批

已认证的父请求可以在同一事务内登记外部目录 child、初始排队 Run 与原 `action_execution` / `approval` 请求。
`delegation_workspace_permission` 只保存不可变的归属及初始配置摘要，批准/拒绝仍由原审批表和 User 命令决定；不增加另一套审批状态，不伪造 Runtime 原生请求。
请求冻结原任务摘要、目标 Runtime、原目录参数、真实目录与访问范围、初始配置、父 Run/epoch/Binding generation/tool call 和 child 身份。
外部目录登记必须使用真实父身份与锁外目录观察的同一计划；失败时全部回滚，原请求重试不新增审批。
审批按精确 child Conversation 读取；公开 Thread 的审批、待审批计数和动作列表排除 child，目录摘要不进入新建审批事件的公共通知载荷。旧 SingleChat 路径保持。

调度候选在分页前排除未批准的目录请求；即使持有旧候选，目录准备与领取仍在原事务中核对精确授权。
User 沿用 `action.approval.resolve` 回答，核对本地身份、Thread、审批版本、排队 child/epoch、当前成员和原父绑定。
父正常完成不单独撤销已登记的后台工作；父停止、非正常结束、成员失效、Conversation 关闭或 child 取消仍走原因果取消和审批收尾。
批准只完成本次目录授权这一 Core 动作，允许后续原执行流程；不投递原生许可回包，不运行文件操作或进程，不提升目标 Runtime 权限。
拒绝复用原 Run 取消结算；重复原 UUID 回放原结果，停止后的迟到批准不能启动后来运行。通用动作执行器不能领取这种目录授权。
该分支目前在候选160/210隔离 Core 验证；完整模型入口、Host 启动前文件系统重验、原界面及完整应用重启恢复仍须接入，不代表已开通生产执行。

### 初始子运行工厂

目标 Runtime 的默认设置来自已认证父请求所观察的 User 设置，登记时核对精确 `runtime_preset_json`，不接受另填一份参数。
工厂在原 Registry 解析前选择模型和原生权限；原 Registry 继续核对安装、模型、权限 schema 与支持值，未知值不会改用默认值。
新模型选择不沿用另一个模型的选项或 DSH 来源；同一明确模型保留原来源。原生模式只按目标已登记的权限选项值匹配，歧义或无法映射时明确拒绝。
非 ACP 的其他模型选项要求明确模型，并沿原模型校验与执行路径；ACP 其他选项保留给真实 Session catalog 核对，不静默丢弃。

执行、User 后续 Run 与恢复只读此委派固定的 preset，不重读当前全局设置；ACP Host 兼容摘要同时冻结这份设置。
原 ACP Session 建立/恢复后先应用尚需原生协议设置的 mode，再沿原模型/权限准备流程选择模型，最后按稳定键序设置其余 config，均在会话就绪和首条 prompt 前完成。不能在明确模型已选好后再用新增 mode 设置改变它；无法映射的 mode 类型 config 也不能在末尾悄悄改模式。
Host 只更新自己已知 Session 的模式/参数目录缓存，其他 Session 或请求帧不能写入；模式切换后必要时有界等待目录更新，不把目录通知当作任务已接受。
核对实际模式、select 选项、合法值与配置回执；设置失败、返回值不符或只读目录下试图改变权限相关选项时停止准备，不发送任务。
没有设置时沿用原调用和字节；原生 Runtime 与供体模式 ID 的完整别名兼容、各目标真实执行及完整恢复资格仍需补齐，不能以明确拒绝代替已支持。

`delegation_factory::create_child` 在原关系登记事务中按显式列创建独立 Conversation、完整任务消息、私有 Turn 与 queued Run。它从 Core 选择的安装和目标配置调用原 Adapter Registry 重新冻结目标，只应用一次 child 角色摘要；不复制父 Run 的 Session、Lease、接收证据、终态、恢复或未知未来列。21个内置 wire 名称映射到原 closed Adapter；custom 稳定编号必须精确对应已登记的 custom 安装，并保留其真实 Adapter，不能猜另一个产品作为运行载体。平台未准入、安装缺失/禁用、Adapter/权限配置错配、非法 workspace/policy 拒绝，不回退目标。

该工厂不是模型鉴权或目录授权入口；父 Run/epoch/Binding、当前与冻结深度、工作范围审批、User 默认模式及 config 的实际投递仍由完整 start/Host 链路拥有。现有 typed custom 安装映射也不等于已接入任意 generic ACP 程序。外层命令必须传播错误并回滚全部关系/子行，或者在保存拒绝回执前先回滚其 savepoint；工厂和原 `register` 都不启动进程。原请求回放先返回固定身份，不重新选择或再创建目标。

### User 结束子会话

`delegations.conversation.end` 仅接受 `{commandId,command:{conversationId}}`，Thread 由 Core 的确切 child 派生。User 必须已经接管该会话；旧 `singleChat.end` 仍只接受 single_chat，两类命令不能相互升级。复用原私有结束事务：标记确切 Conversation 结束，取消其后续 User Run 与仍受该 Conversation 控制的全部后台子树，清空各端草稿、保留输入和编辑租约，将未发布队列改为 cancelled，保留历史消息、原 attempt 结果与接管水位。已独立接管的后代和其他会话不随旧父关闭。

原 UUID 回放与 `commands.reconcile` 只返回首次结果，不重新结束或发失效通知；事务失败不留下部分关闭或回执。新 UUID 再次结束原 Conversation 返回既有结束状态，不能影响其他 Conversation。结束后发送、发布和编辑仍走原 ended gate。Native/Web 仅运输该 User 操作与安全失效事件，实际进程及输入清理由原 Runtime 管理。生产私有完成路由也用于接管后的 delegation_user 正常完成，其后台帮手继续保留。

### User 子输入运输

`delegations.message.send` 使用 `{commandId,command:{conversationId,composerScope?,expectedRevision,body}}`；Core 从确切 child 派生 Thread、从可信 Request 派生编辑端，以本地 User 身份调用已经存在的整份私有材料消费算法。拒绝额外 Thread、actor、draftClient、sourcePath 或另一组附件字段；不能把其他私聊升级为 child。忙碌时整份输入进入该 child 的原 FIFO，失败保留草稿材料。

`delegations.pendingInputs.get` 只接 `{conversationId,composerScope?}`，返回安全队列及该编辑端在该位置的工作副本；`delegations.pendingInputs.edit` 使用 `{commandId,command:{conversationId,pendingInputId,expectedRevision,editToken,composerScope?,action}}`，沿原 begin/takeover/save/remove/reorder/quote/cancel/delete/return 动作。这里的 takeover 仅接管排队编辑租约，不是原委派执行的控制权接管。

两项写操作通过 `commands.reconcile` 重建同一密封命令并只读核对原 UUID；查询不补发。Web 分组按操作、child 和输入位置隔离，明确重试须保持原编号和完整参数；Native Main 与 Web Operation 只放行这三项，不开放内部 pending publish。源文件观察沿原有界 worker 在数据库锁外执行，事务内再核对原 owner/CAS/租约；首次成功或拒绝回执仍由 Core Gateway 持久保存。新写入成功后发 `delegation.changed`，回放不另发；Web 原失效通知流会要求重新读取授权快照。

这些接口不激活候选160/210迁移，不代替原委派真实接管、取消清理、目标 Runtime 初始准入、完整 Context 投递、原界面或阶段集成验证。

新Run的`delegationPolicy`冻结schemaVersion=1、enabled、depthLimit和public_parent/private_parent/child角色。缺失即旧资格；null、未知字段、非法版本/深度拒绝，不能从当前全局开关补给旧Run。新的start/resume还要求当前设置启用；既有关系的status/cancel/read/history不因关开关消失。该判断只是资格原语，完整工具网关仍要校验当前执行、绑定、直接父及控制权。

Profile11为child的publicHistory/taskAwareness/memoryEntrypoint三个none；Profile12保留公共maxSelfActiveTasks=8；Profile13沿用原私聊数值边界。格式35只接受(public batch/direct/a2a/gather_completion,12,9)、(single_chat,13,6)和(delegation/delegation_user,11,10)元组。默认Profile7/10、格式29/34和既有摘要不改。

父/私聊/child Charter使用批准r1全文。child按实际冻结Runtime种类展示独立名称和稳定短编号，不读取父人设；无Memory入口，Bootstrap v5仍存空入口Blob及摘要，平台技能索引为空。新证据不查询父Mission/渠道；旧child证据拒绝带Memory、旧v4或非空平台技能。此处不代表原生Runtime目录中的技能/MCP投影已经过滤完成。

完整160/210开通之前，生产者不生成新角色。格式35现已接原装配、Manifest/Blob和回放：child只渲染原Additional Skills、Skill Packs、DELEGATION_CONTEXT、RUN_FACTS10、CURRENT_INPUT；父指南在Skill Packs后，原父段与预算算法保留。child不查询/投影父公屏历史、成员协作、Task、Mission或跨Thread历史；两种恢复入口都使用独立身份。初始正文与原任务Blob的SHA一致；resume只使用批准的固定继续提示及可选中断说明；User后续正文按本child和接管序号校验。超限抛出原context_payload_too_large错误，不截断任务。

候选DDL扩展context_manifest/agent_run_input的35及Profile11/12/13、Facts10闭集；35元组不能写到旧版本，child Manifest无公屏/跨Thread历史证据，角色必须匹配Run/Conversation。材料来源、原生技能/MCP过滤及执行资格已有分项接线，完整启动迁移资格还须与旧来源升级、policy3和执行协调合齐；各分项结果不代替真实Host/Runtime验证。

候选结构检查覆盖settings、新增表/索引/guard、conversation/camp_turn/agent_run/context_manifest/agent_run_input五表，以及格式4个和材料6个改动旧guard。约束比较保留完整CHECK表达式和字符串值，核对变动字段的类型、非空、默认值、主键、隐藏属性和比较规则；不以字段名存在或SQL片段出现代替完整约束。安装器与验证器共用转换，兼容空白、注释、标识符引号及不相关旧扩展列；未改变的旧结构继续由原Core分类器核对。

旧159来源必须保留本次将修改的完整旧约束且不存在新增保留对象（表名大小写不构成绕过）；部分settings/identity安装、同名空壳guard和放宽约束拒绝升级。重建、对象安装、210回执和v1.73/160标记使用同一事务；收尾重验完整目标合同后提交，失败完整回滚并恢复外键ON。重复完整迁移零写；旧159固定209完成记录，当前210缺失、未来记录或错误标记均不能准入。
变动字段上额外的非合同CHECK同样拒绝，避免所有新约束都存在却被另一条条件禁止子会话；不相关旧扩展字段及其原有CHECK继续保留。

v1.73/Schema160/Migration210已登记两条启动链，原v1.72来源经既有历史迁移推进到159，再安装同一完整210事务。当前技能包能力允许由完整新合同证明，不因已清理203历史回执而停用；不得补写伪造的历史回执。旧Run的配置、选择、输入和已保存正文不转换，已冻结版本优先于当前开关。

测试专用159来源从真实历史启动链的210之前取得；原更早来源的测试降级链只允许在无委派数据/新冻结Run时恢复159，保留旧行和无关扩展列，不提供产品降级入口。实际Host/Runtime调度、输入接受/清理、原Session恢复及原界面仍按工程计划验证，Run仍是唯一终态权威，正式场景不按这些组件测试记为完成。

### 工具记录的启动核对

三项模型写操作的开始记录只增加原命令编号与完整请求摘要的观察信息，不保存凭据或任务正文。
真正执行的尝试在原 Gateway 事务内追加 `delegation.tool_receipt_confirmed`，与业务变化及首次回执一起提交或回滚。
重启核对须同时匹配这份尝试关联与原回执的操作、成员、Thread、Run、epoch 和摘要；仅找到同请求回执不够。
查询、未提交调用、旧观察缺少可靠关联时，工具记录以 `builtin_tool.result_unconfirmed` 结束并要求核对原任务，不补发操作、不重发模型输入，不把观察失败改写为任务失败。
开始时捕获未结 Evidence 的精确 UUID 集合，后台按64条处理并在批次间释放数据库锁；不把可被重用的 SQLite rowid 当作启动边界，新调用不纳入旧记录扫描。
维护入口只更新精确已有、未终结的 Core 观察及其原 revision；允许处理退休 epoch，不放宽 Runtime 事件或业务命令授权，也不生成新的工具调用。
投影复用原输入摘要与脱敏元数据，按原生命周期存储省略 null 的规则恢复摘要验证，不复制原始任务或生成另一份结果正文。
此为工程接线；完整 Host/Fleet/Runtime 的重启、执行、writer 与清理资格仍须阶段验证。
