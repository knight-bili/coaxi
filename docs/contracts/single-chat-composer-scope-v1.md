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

Core 类型、编码、只读 pin 校验、新存储和材料业务服务已实现。新服务提供精确归属读取及七个闭合动作：登记、
移除/排序附件、引用操作、交换/丢弃保留输入、显式迁入旧默认材料。Host 注入可信编辑端；读取不分配 scope。
写操作先核对当前材料 revision，保留输入另核对自己的 revision。交换同时搬动正文、附件、引用及撤销材料，
任一写入或回执失败回滚双方；原 UUID 回放首次结果。旧材料只移动材料，目标已有正文或材料时另建完整保留项。
明确拒绝也保存结果；普通变更事件只含 Conversation 标识，完整私有结果仍由原命令回执的编辑端边界保护。

现有 Core 发送命令识别可选 composerScope；省略时沿默认路径及原规范序列化。卡片必须先登记，发送才按相同
Conversation/client/scope 读取、验证和消费材料。所有 scope 仍进入原 Conversation 单一 FIFO，不增加执行队列。
引用仍复用原捕获和模型投影；只引用而无问题正文的卡片发送明确拒绝并保留输入。

Core recovery 仅列出当前可信编辑端在指定 Thread/Conversation 的 scope 元数据和完整保留项，每页默认50、最大100。
签名游标绑定上下文、原始状态摘要和遍历位置；所有元数据参与一致性判断，只有本页保留项加载正文及材料。
翻页期间状态变化时 complete 为 false；缺页、未完整或游标失效不能解释成删除。进程重启会使进程期签名游标失效，
客户端重新只读枚举，原输入不受影响。游标不替代 Host 认证；孤立输入仍按原归属保留，不自动绑定或发送。

Application 已接 `singleChat.get`、`singleChat.composerDraft.mutate` 和 `.recovery` 的同一封闭入口；
可信 Request 注入 DraftClient，网络或 Renderer 不能自报编辑端。Web 枚举、Desktop Main 与 TS 准入同步增加两项，
current 操作数180，历史来源161保持。新增材料命令也进入原只读 `commands.reconcile`：原 UUID/完整请求核对原回执，
结果未知时不派发；执行与回查复用同一闭合解析器。重放不再发布变更提示，首次提示只含 Thread/Conversation 标识。

get 省略 composerScope 保留旧四字段 draft 和原 pendingInputs 响应；显式 null 或完整描述才返回 scopeVersion 1、
scopeKey 与完整描述回显，并在 pendingInputs 增加 editingElsewhere 布尔值。draft 和编辑会话都按同一可信 client/scope 读取，
其他位置不返回 editToken、未保存正文或 working quotes。公开历史/原私聊 FIFO 仍由原 snapshot 拥有。
共享只读客户端核对 Thread、Conversation、完整描述、key 和版本；缺失/错误能力回显明确失败，不登记、不发送、不自动降级。

排队编辑已识别可选 composerScope，默认 None 序列化省略，旧规范请求形状保持。单 Conversation 租约继续唯一，
ownership 按可信 client 加 scope；Begin 不能覆盖已有租约，显式 Takeover 可接管相同排队项/版本，
从已保存 Pending 重开并旋转 token，原位置迟到的保存、附件和引用操作均拒绝。工作副本 quote/trash 和附件写入按同一归属保护；
即使已有 scope，也拒绝正在删除或非活跃 Thread 的新编辑。

return_to_composer 必须携带冻结的 targetBody 并核对目标材料 revision；缺字段的新请求持久拒绝 capability_required，
已存旧 UUID 仍沿原 Gateway 回执规则回放。排队项有未完成编辑时须先保存或取消，不用 working 副本覆盖持久版本。
空目标把正文、附件和引用作为一份活动输入返回；目标正文或材料/撤销材料非空时，创建完整 retained 输入并返回 placement=retained，
当前 draft/revision 保持。两条路径与原 Pending 取消、事件和回执同事务，失败全回滚；不创建新 FIFO。
成功 payload 保留 pendingInputId/body/draft，增加 active/retained placement，retained 时带完整 retainedInput；外部变更提示只含 Thread/Conversation。

上传 prepare UUID/源摘要、资源授权、Renderer 新移回参数/完整保留选择、本机意图恢复继续实施。
现有 SingleChatPanel 尚未接 targetBody 与 placement，按钮新请求会明确拒绝；这是本阶段尚未接通的客户端项，不能宣称 UI 可用。
卡片 UI 尚未启用新材料操作；最终 Manifest Blob、实际 Runtime 和完整隔离仍待验收。

本轮因主键及迁移安全影响后续接线，按阶段策略例外做本地定向编译/迁移 owner，完整跨平台、实际 Runtime 与 PC01–PC24 仍待验。
具体命令、源码和失败记录见[迁移执行证据](../../integration/execution/i03-private-scope-storage-result.json)，不宣称完整功能或整套一期通过。

材料服务与按 scope 发送的17项本地定向检查、自审修正和未验边界见
[业务服务执行证据](../../integration/execution/i03-private-composer-service-result.json)。完整 PC01–PC24 仍待全链路验证。

Application/可信编辑端接线、默认形状/编辑令牌隔离、回执只读核对及客户端能力检查见
[应用入口执行证据](../../integration/execution/i03-private-composer-api-result.json)。实际 Host HTTP/Native IPC 与完整平台随阶段候选验证。

排队租约/完整移回、尾部回执失败回滚、原请求回放和旧默认形状的本地证据见
[排队编辑执行记录](../../integration/execution/i03-private-pending-edit-result.json)。本地分项不替代完整 PC01–PC24 或实际客户端验收。
