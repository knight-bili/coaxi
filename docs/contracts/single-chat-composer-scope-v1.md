---
document_type: protocol-contract
contract: single-chat-composer-scope-v1
authority: private-composer-scope-and-material-selection
status: accepted
version: 1
last_updated: 2026-10-15
---

# Single Chat Composer Scope v1

范围和模型输入选择来自客户明确确认的[私聊材料隔离 r1](../versions/v1.72/model-context-change-canvas-private-materials.md)。
本文不替代现有 Single Chat、可信 DraftClient、文件授权、Domain Command Result 或 Accepted Input Recovery。

## 归属类型与兼容编码

卡片描述是闭合对象 `{kind:'canvas',boardId:string,nodeId:string}`。boardId 和 nodeId 使用 Core 创建时的规范小写 UUID；
未知 kind、缺失字段、额外字段及非规范标识拒绝。网络不能用 scopeKey、DraftClient、Owner 或认证证明替代此描述。
Core 对有效描述产生 `canvas:<boardId>:<nodeId>` 闭合存储 key；该值只标识草稿，不授予权限。
原默认草稿 key 为空串，后续存储按 Conversation、Host 验证的 client 与 key 联合定位。

只读 get 的省略与显式 null 不同：省略保留原响应；显式 null 申请默认草稿的 scopeVersion 1 和精确归属回显；
完整描述申请对应卡片回显。`SingleChatComposerScopeRequest` 以 Legacy 和 Explicit 两态保存这一差别。
发送等写命令使用可空 scope，序列化时省略默认值，以维持旧命令规范摘要；不能把 get 的能力协商标记带入旧发送摘要。

## 首次登记的真实 pin 校验

`SingleChatComposerScope::matches_current_pin` 只读精确 Board、pin、Thread 与 Conversation 绑定，要求画板未删、
节点为 conversation、指定私聊未结束、所属 Thread 已激活且未进入删除。公开 pin、同 Thread 的其他私聊、
其他画板或镜像、缺失/已删节点、错绑、结束私聊均不能建立此 scope。
首次写入必须在现有业务写事务内调用，先通过 Host/Owner、可信编辑端和原私聊访问校验，再登记 scope；
函数本身不认证用户、不登记草稿、不 enter、不启动 Runtime，也不删除或重绑既有孤立输入。

现有展开入口使用真实 conversation pin；区域派生 ID 只参与布局，不能冒充 Core pin。
已有 scope 的原 pin 后来失配时保留原 Conversation 并按 r1 标为 orphaned，不用首次 pin 校验清理它或将其绑定到新单聊。

## 实施与证据边界

当前切片只提供类型、缺省/显式协商编码和只读绑定校验基础，尚未接入生产 get/send/上传/quote 或新 scope 登记。
Schema154/Migration204、现有操作数、默认 get 响应和实际模型输入均保持。scopeVersion 不能因仅存在此类型而被前端当作可用。
后续按批准 r1 原子迁移草稿主键并增加 scope/retained 存储，再接材料 CAS、编辑租约、完整移回/交换及原 UUID 恢复。

新增最低层 owner 只拥有闭合 codec/旧省略和当前 pin 关系；Rust 编译/执行待 I03 候选，不宣称功能或 PC01–PC24 已通过。
