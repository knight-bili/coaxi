---
document_type: protocol-contract
contract: builtin-tool-transport-v39
authority: builtin-tool-transport
status: accepted
version: 39
last_updated: 2026-10-18
---

# Built-in Tool Transport v39

继承 [v38](builtin-tool-transport-v38.md)，按已确认的
[I04 独立帮手 r1](../versions/v1.73/model-context-change-native-delegation.md)增加六个操作。
Transport/CLI 39、capability `builtin_cli.transport.v39`；IPC 3、Envelope 2、Agent Output 10 不变。
目录共36项，原30项的输入、结果和投影保持，默认/旧无委派资格 Run 不自动补权。
Schema160/Migration210 与完整角色准入仍单独控制；运输代码版本不等于数据升级或阶段交付。

## 操作与身份

`rovai delegation start|status|cancel|resume|read|history` 使用同一 Core 执行器。
完整输入、说明和 TaskReport 来自已冻结 r1；完整清单见 [Delegation v1](delegation-v1.md)。
参数使用 `--agent-type`、`--working-dir`、可重复 `--task-id`、`--wait-ms`、`--after-cursor`、
`--before-cursor`、`--limit-bytes`。重复 task ID 保留原顺序，不去重。
原四工具名 `delegate_to_agent`、`get_delegation_status`、`cancel_delegation`、`resume_delegation`
在请求边界规范化为同一个 canonical 操作，不能绕过认证、调用另一套任务权威或制造另一个回执。

Core 从当前进程租约和原 Native Binding 派生 Run、epoch、Thread、成员及父 Conversation。
JSON 不接受父身份、epoch、凭据或权限。实际业务准入复核原冻结角色、当前授权与范围。
锁外目录/文件工作完成后，在原调用门内再次认证原完整 CLI 租约，再进入短数据库提交；
同一个 Run/Native Binding 不能替代租约代次、id 和 token 的精确核对。

## 回放与等待

start/cancel/resume 在新工作鉴权之前，只读查原身份和冻结参数的原 Gateway 回执。
退役租约凭据摘要最多1024份且只在进程期保留；匹配证明不暴露 Run/Binding/tmp 或配置，
没有原记录则停止，不能派发新工作。参数改变返回 `builtin_tool.idempotency_conflict`。
Core 重启/凭据淘汰的恢复资格须由完整生命周期验证证明，不能声称靠内存索引跨进程恢复。

read/history/status 每次重新鉴权和查询，不复用旧结果缓存。status 省略 wait_ms 是快照；
正数最多等待60000ms，0无限等待。连接和发送保持原有界超时；正数结果读取留出 Core 等待时间，
0不设置读取截止时间。CLI 重试保留同一 UUID、参数、认证帧，不重新读取身份或另发任务。
等待用原业务通知和租约撤销通知；登记在快照/认证检查之前，等待期间不占数据库或调用门。
断线只结束等待，不自动取消帮手；同 Run 换租约仍唤醒全部旧等待者。

## 封闭结果、错误与证据

三类变更返回 TaskReport，包括业务拒绝；unknown 仅返回 task_id/status。
状态页、固定结果 UTF-8 页和清洗历史采用各自封闭 Schema，不包含其他父、草稿、凭据、路径或原始思考。
TaskReport 的条件约束实际执行：文本/固定引用只能属于 completed、blocked_on 只能属于 running、
cleanup_pending 只能属于 canceled；Runtime 目标复用严格内置/custom 类型验证。

身份和输入错误使用稳定错误封套，存储/worker 错误只给安全通用消息，不输出原 SQL、路径、正文或凭据。
cursor_invalid/page_limit_too_small 要求修正输入，result_unavailable 要求重读状态后判断；
result_changed、access_denied、失效连接和断线等待均停止当前调用。
完整输入/结果投影经过原 CLI 输出校验；契约不匹配停止，不盲目重发已执行变更。

原执行证据投影增加委派元数据：身份、状态、审批请求标识、内容长度/摘要与游标存在性；
投影不复制任务全文、目录路径、结果/历史正文或凭据。
原 Host 执行证据起止记录、provider 工具调用关联及 UI 活动完整接线仍须单独工程验证，
不能用投影函数或管道夹具替代真实 Host/Runtime 资格。
