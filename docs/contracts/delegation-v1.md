---
document_type: protocol-contract
contract: delegation-v1
authority: core-delegation-identity-settings-and-wire
status: accepted
version: 1
last_updated: 2026-10-17
---

# Delegation v1

完整批准边界见[独立变更说明](../versions/v1.73/model-context-change-native-delegation.md)。本合同区分基础实现和待接线能力，不能据此判断一期或 I04 已完成。

## 严格输入基础

delegation.start/status/cancel/resume/read/history 的输入 shape 由已确认 r1 完整机器附件 after.operations 拥有；Core `delegation_contract.rs` 实现相同解码边界。
六个对象都拒绝未知字段。父身份、epoch、Binding 与授权不能由模型提交。必填及已出现可选值不得为空/null；Runtime 只接受既定 21 个标识或 custom 稳定标识，实际已安装/启用/权限准入仍由执行层核对。
working_dir 须为 Host 绝对路径；语法通过不是目录授权。reason 限 2048 UTF-8 字节且不是新任务。status 保持编号顺序及重复项；省略 wait_ms 是快照，0 是无期限，正数上限60000ms。read 默认16384/范围1..65536字节；history 默认20/范围1..100条。
这里只实现参数与等待数值规范化，不实现等待订阅、业务读取/结果、Core API 或 CLI。

## User 设置存储基础

表 delegation_settings 为 singleton=1，version>=1；enabled 默认 false、depth_limit 默认1且范围1..8、per_runtime_defaults 默认空对象、completed_cache_max_mb 默认512且0不限。每个目标配置沿用 Codeg mode_id/config_values 形状：mode_id 可省略或 null（表示无覆盖），config_values 可省略且空映射不序列化；值是配置字符串，不是凭据或执行授权。嵌套供体默认配置的 null 规则不同于外层严格 patch，不能混用。
User API 更新输入为 {expectedVersion,patch}，patch 仅 enabled/depthLimit/perRuntimeDefaults/completedCacheMaxMb。缺字段保持原值，显式 null/未知字段拒绝。深度/缓存非法时零写入。每次成功更新精确比较版本并递增；开关请求只携带 enabled，不覆盖其他配置。
更新原语接受调用者事务；随后回执/事件失败应回滚同一事务。读取只读，不懒建表、不重设配置。SCHEMA 只在隔离 SQLite 存储夹具执行；启动迁移、Command Gateway User-only 鉴权、幂等回执/事件及 Host 两端入口仍待集齐，不允许生产路径自行执行建表字符串。

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

### 初始子运行工厂

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

候选DDL扩展context_manifest/agent_run_input的35及Profile11/12/13、Facts10闭集；35元组不能写到旧版本，child Manifest无公屏/跨Thread历史证据，角色必须匹配Run/Conversation。完整旧表真结构分类、User后续材料的delegation_child来源及原生技能/MCP过滤尚未合齐，当前不是完整启动迁移资格。格式分项检查不证明User/模型鉴权或真实取消/恢复/接管已完成。

目标 v1.73/Schema160/Migration210 尚未登记启动链。格式35/冻结资格/子Bootstrap组件接线后，继续合齐材料/原生投影、完整数据合同分类及旧来源升级，再接 User/模型/CLI、回执事件、调度与输入接收、取消清理、原 Session 恢复、接管发送与原界面。Run 仍是唯一执行终态权威；这些组件不构成完整身份认证或 Runtime 准入，不得据此启动真实进程。当前运行数据和旧模型输入保持，正式场景不据分项测试记为完成。
