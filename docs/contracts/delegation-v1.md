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

新Run的`delegationPolicy`冻结schemaVersion=1、enabled、depthLimit和public_parent/private_parent/child角色。缺失即旧资格；null、未知字段、非法版本/深度拒绝，不能从当前全局开关补给旧Run。新的start/resume还要求当前设置启用；既有关系的status/cancel/read/history不因关开关消失。该判断只是资格原语，完整工具网关仍要校验当前执行、绑定、直接父及控制权。

Profile11为child的publicHistory/taskAwareness/memoryEntrypoint三个none；Profile12保留公共maxSelfActiveTasks=8；Profile13沿用原私聊数值边界。格式35只接受(public batch/direct/a2a/gather_completion,12,9)、(single_chat,13,6)和(delegation/delegation_user,11,10)元组。默认Profile7/10、格式29/34和既有摘要不改。

父/私聊/child Charter使用批准r1全文。child按实际冻结Runtime种类展示独立名称和稳定短编号，不读取父人设；无Memory入口，Bootstrap v5仍存空入口Blob及摘要，平台技能索引为空。新证据不查询父Mission/渠道；旧child证据拒绝带Memory、旧v4或非空平台技能。此处不代表原生Runtime目录中的技能/MCP投影已经过滤完成。

完整160/210开通之前，不生成新角色。格式35装配未接通时显式阻止新角色进入旧Dynamic Context格式；不得降级到公屏/普通私聊来绕过隔离。完整35分支、Manifest/Blob和重放必须在开通前实现并验证。

目标 v1.73/Schema160/Migration210 尚未登记启动链。必须集齐格式35、父冻结工具资格、子 Bootstrap、数据合同分类/旧来源升级，然后接 User/模型/CLI、回执事件、调度与输入接收、取消清理、原 Session 恢复、接管发送与原界面。Run 仍是唯一执行终态权威；这些存储原语不构成完整身份认证或 Runtime 准入，不得据此启动真实进程。当前运行数据和旧模型输入保持，正式场景不据分项测试记为完成。
