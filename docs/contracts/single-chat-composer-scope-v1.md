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

Schema155/Migration205 从准确 Schema154 来源原子升级：草稿主键加入 scope_key，旧行保持为空 scope；
新 scope 注册和完整 retained input 存储、恢复索引、八个归属约束及排队编辑 scope 字段与 marker/receipt 同事务。
旧正文仍沿原本机存储，不在迁移中复制或自动分配附件、quotes、trash；旧 revision、原始 JSON 和时间值原样保留。

scope 注册必须对应真实活跃 pin；其 Conversation、client、key 和节点身份不可变。新草稿、保留输入和编辑租约
的非默认 scope 必须已登记。Scope 与 retained 仅级联引用原 Conversation，不级联画板或节点，因此删除 pin 不删除输入；
永久删除 Conversation 则清理其数据，结束私聊按原全私聊清理语义清除材料、保留项和 scope，不删除外部源文件。
排队编辑仍以 Conversation 为唯一主键，不增多条 FIFO 或并行租约。

旧默认读取、CAS、消费、引用和 owner-scoped 附件资源明确限定空 scope；同一编辑端的其他 scope 不得到
旧默认编辑 token、working body 或材料。Scope 全部迁移完成并经必需结构/索引/归属约束准入后才可启动业务工作区。
失败回滚同一原库事务，不替换原库、不初始化空库。旧 marker 与已加入新布局的混合来源在迁移入口拒绝。

Core 类型、编码、只读 pin 校验与新存储已实现；新 scope 的生产 get/send/上传/quote/登记操作尚未接通，
默认 get 响应、现有操作数和实际模型输入保持。scopeVersion 不能因类型或表存在而被前端当作可用。
后续按批准 r1 接材料 CAS、编辑租约、完整移回/交换及原 UUID 恢复。

本轮因主键及迁移安全影响后续接线，按阶段策略例外做本地定向编译/迁移 owner，完整跨平台、实际 Runtime 与 PC01–PC24 仍待验。
具体命令、源码和失败记录见[迁移执行证据](../../integration/execution/i03-private-scope-storage-result.json)，不宣称完整功能或整套一期通过。
