---
document_type: protocol-contract
contract: canvas-references-v1
authority: core-canvas-business-reference-directory
status: accepted
version: 1
last_updated: 2026-10-15
---

# Canvas References v1

为[Canvas Nodes v1](canvas-nodes-v1.md)提供完整、可分页的业务对象目录。
侧栏的[Navigation Read v1](navigation-read-v1.md)继续拥有摘要和窗口，不为画布扩大默认前缀。
本接口从既有 Core 表派生读取，不增加数据库表、迁移、模型输入或第二套业务对象注册表。

## 接口与分页

User 通过 Native Core 或已认证 Host 使用 `canvas.references.page`，参数为
`{afterThreadId?: ThreadId|null, upperThreadId?: ThreadId|null, limit?: number}`。
拒绝未知字段，ThreadId 沿既有解析器校验；limit 默认200并夹在1–200。

响应为 `{schemaVersion:1, throughGlobalSequence, items, nextAfterThreadId, upperThreadId}`。
每个 item 包含 `threadId,title,projectBindingKind,projectPath,updatedAt,version,missionId`，以及：

- `members:[{agentId,displayName,runtimeKind}]`，只含实际 active membership 和未移除队员。
- `privateConversations:[{conversationId,agentId,updatedAt}]`，只含这些队员实际存在且未结束的 single_chat。

`runtimeKind` 是该队员当前配置的产品类型，不代表平台已验收、模型可用或历史 Run 使用的产品。
title 对使命 Thread 使用现行使命标题，missionId 保持实际业务身份；不在普通侧栏创建使命镜像。
字段不含消息、附件、模型输入、技能正文、凭据或 Runtime Session。可见性沿当前请求的可信 DraftClient
核对：active Thread 可见，pending Thread 仅在该编辑端确有草稿/附件或存在记录时可见；删除中的 Thread 排除。
草稿非空条件只用于可见性判断，不把正文投影到目录。Mission-backed Thread 同样纳入画布目录。

第一页省略 upperThreadId 时，从相同可见性范围捕获最高 Thread ID。后续页回传此上界，并以
`id > afterThreadId AND id <= upperThreadId` 按身份升序推进；末页 nextAfterThreadId=null。
活动时间变化不改变分页身份位置，上界之后创建的新对象留待下一轮读取。上界只是读范围，不是访问授权。

每页的 Thread、成员、私有单聊和 throughGlobalSequence 来自同一个只读事务。
整轮不冻结跨页事务：业务仍可变化，客户端遇到相关失效时必须补读；不能把分页上界称为全目录业务快照。
请求沿已有有界独立读取队列运行，SQLite仍使用同一个数据库锁。读取不创建会话、Run、Manifest、草稿或通知。

## 客户端解析与恢复

客户端先订阅再读，固定同一 Host/editor；逐页校验schema、上界、序号、身份递增和私有单聊成员归属。
拒绝循环游标、重复身份、跨上界与迟到旧主机结果。完整读取后才能把缺失引用标为不可用；
读取失败或读取中失效保留已知结果并标记未完整，trailing读取收敛，不应用事件 payload 作为目录状态。
普通画布布局事件不触发目录全量刷新。

队员区域按实际 active membership 过滤，不能只按 Default Lead；agentId和runtimeKind同时指定时必须
匹配同一位成员。私有单聊还须匹配该成员，不能因共用threadId而混入其他队员单聊。
custom 展示保留精确公开/私有引用并按实际活动时间排序；缺失引用明确显示不可用，不在读取时删除。
用户在新建分组时选择的对象若已失效，须重新核对，不能保存时静默丢弃选择。

工程与项目分组必须使用实际目录及父工程/worktree关系；关系或分组偏好尚未取得时显示未完整，
不以字符串前缀猜测父子关系，也不把最近五条导航摘要当全量目录。
真实worktree关系通过[Workspace Worktrees v1](workspace-worktrees-v1.md)读取；项目分组偏好、
成员拖放、展开会话、文件授权与PTY仍按完整I03继续实施。

## 验证边界

`read_model::canvas_references`的最小SQLite owner拥有pending编辑端、删除fence、Mission纳入、
active成员与精确私有身份、keyset分页及只读行为；不启动完整Database或Runtime。
最低命令为`cargo test -p rovai-core --lib read_model::canvas_references`，按阶段批量规则留待I03候选编译。
`application`既有读取队列owner扩展准入断言，不新增重复队列fixture。

TypeScript controller owner拥有完整多页、错误分页/归属、相关失效后的补读和旧Host销毁；
model owner拥有成员交叉过滤、精确引用、目录关系与失效成员保留。
本地额外用Node内存SQLite执行当前Rust原文SQL，只证明这些SQL与最小夹具的行为，不能替代Rust类型检查、
真实Core/Host、实际UI或模型与平台资格。上述实际验证继续留在阶段候选。
