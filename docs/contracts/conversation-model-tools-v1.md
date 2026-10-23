---
document_type: contract
name: Conversation Model Tools
version: v1
status: accepted
source_version: v1.73
last_updated: 2026-10-23
---

# Conversation Model Tools v1

客户已明确确认“会话模型工具补齐 r1”。本合同拥有该可选服务的权限、作用域、输入输出、幂等、回答投递与自动化所有权语义。
完整字段和闭集 Schema 见[实现合同](../../crates/rovai-core/src/conversation_tools/contract.json)；批准时的原始快照见[冻结方案](../versions/v1.73/model-context-change-conversation-model-tools.md)及[确认记录](../../integration/proposals/conversation-model-tools-r1-confirmation.json)。合同接受不表示软件已编译、测试或可供模型调用。

## 服务和原权限

服务名固定 `rovai-conversation-tools`，只有四个模型工具：

| 工具 | 功能 | 副作用 |
| --- | --- | --- |
| `ask_user_question` | 在原公开、私聊或帮手讨论里请求真实 User 回答 | 问题意图、首份答案、原连接投递记录 |
| `get_session_info` | 读取该 Run 明确选择且仍有效的来源授权 | 无；未授权或无法核对返回 `found:false` |
| `create_automation` | 创建原 Core 自动化定义 | 定义、时区、动作及 enabled 在同一事务保存 |
| `create_work_task` | 创建未启动的 Mission | 完整内容、工程和实际队员在同一领域事务保存 |

全局四开关与目标队员四项明确分配取交集，再与实际 Runtime 能力及作用域相交。四开关默认 false。
原 Built-in 工具的能力与授权独立，关闭此服务不撤销已独立授予的原工具。`feedbackGate:null` 保留原反馈行为，false 关闭新执行的反馈选择，true 仍不能扩大原队员权限。
迁移只处理明确选择的 Codeg 五项总开关：`question.enabled`、`session_info.enabled`、`chat_authoring.automations_enabled`、`chat_authoring.work_tasks_enabled`、`feedback.enabled`。不按名称自动授予接收端队员权限。

## 冻结和原生恢复

可选 `FrozenAgentRuntimeConfig.conversationTools` 在 None 时不序列化，保留旧配置字节和摘要路径。
非空冻结值绑定策略修订、摘要、真实 Scope、实际工具集合、目录摘要及来源/工程授权。正文、凭据和私有 locator 不进入模型目录。
公开输入只从本批实际 User 消息选取来源引用；私聊及 User 接管帮手只从本条真实 User 正文选取。Agent/System 任务不能复制父讨论的来源授权。
独立帮手在新建的真实子 Conversation 上冻结目标 Runtime 能力，不复用父 Run/Session/Binding/租约。
恢复保留原 Run 冻结和原请求身份。不能证明原目录、租约和原生调用仍可安全续接时，保留历史，按新合同建立新原生会话；不能重放 accepted/unknown 输入。
实际启动与恢复边界见 [Runtime Launch v61](runtime-launch-and-verification-v61.md)。

## 来源授权和有界投影

实时来源使用真实 Core Conversation UUID；导入来源获得新的 Core UUID，绑定受管 manifest/transcript 摘要，不能把原生 Session 编号当成当前 Core 身份。
旧数字别名只有已证明来自原 Codeg 数据库时才可使用，不由原生编号推断。
预览有效 600 秒，确认后的授权最多 7200 秒；撤销、作用域结束或来源边界改变后不能继续读取。私人来源不能授权给公开接收范围。
授权后引用以普通 User 消息中的 `rovai-source:<UUID>` 进入现有正文，不增加 Context 章节。
只读已经保存的可见文字和工具名称；图片、附件字节、隐藏思考、工具参数/结果、令牌和路径不进入该投影。
默认最近 20 条，最多 200 条，每条文字最多 4096 字符，完整结果最多 64 KiB。始终返回真实 total/included/truncated。
实时工具名称限于已发布消息原 Run 在该消息边界之前的可见证据；每消息最多 512 份证据、128 个名称。用量最多覆盖最近 512 个已发布运行中已结束且没有越过消息时刻的实际计数；只报告已知字段，coverage 为 partial 或 unavailable。
前缀授权不能读到之后的完整摘要、当前统计或新的消息。工具与统计达到边界时明确说明覆盖限制。

## 状态和幂等

三个有状态工具要求 UUID `request_id`。`operation:status` 是原工具只读分支，不增加第五个模型工具。
请求 UUID 同 Run/执行代次/工具名/完整输入摘要绑定；先保存加密意图，再执行副作用。同编号不同输入拒绝，未知原请求阻挡新编号绕过。
状态为 prepared、awaiting_answer、answered、applied、rejected、cancelled、outcome_unknown、not_seen；User 处置只允许取消剩余未发生动作或撤销原 Run 可选服务租约。
取消先保存墓碑，已 applied 的业务保留。未知不能当未发生，也不能自动重发。
Mission/自动化创建、成功结果及领域命令回执在同一事务保存。失败返回闭集 `ToolErrorV1`，不能回显原异常或秘密。
成功 structuredContent 和规范 JSON 文本表示同一对象；每个工具含标准 outputSchema 和完整局部 `$defs`。
请求最多 128 KiB、结果最多 256 KiB、帧最多 8 MiB；整数遵守安全范围。

## 问题和原回答通道

每个 Run 同时最多一组问题，一组 1–4 题，每题 2–4 个唯一选项，header 最长 12、问题最长 4096 字符。
每题由 Core 分配稳定 UUID。User 明确选择或明确拒答，停止、过期、取消均不制造选择。
首份有效答案与 outbox 同事务保存；后来的重复提交只能核对原结果。客户端恢复只保存原操作和编号，不保存第二份答案或授权材料。
答案只能由原请求的 Run/epoch、原生 Binding/generation、bridge 和调用 locator 领取。
领取后的结果只能写到原 stdout 调用；只有实际 flush 成功后记录写回。写回不确定时保留 outcome_unknown，回执重试不能重新发送答案。
Core 重启、原连接关闭、Run 停止与迟到回执分别处理，已明确写回的历史不能被后来的停止改成未发送。

## 自动化、Mission 和兼容

`create_work_task` 创建未启动 Mission，不创建旧 Task，不自动执行。名称最长 120、完整说明最长 20000 字符；过长拒绝，不截断。
复用原使命说明权威；新来源附表用于区分较长输入，旧入口的限制和旧数据语义保留。
自动化仍由唯一 Core 调度器管理。动作仅 launch_session/enqueue_task，后者按发生时间生成等待 User 启动的 Mission。
Cron 只接受五字段，星期日 0/7；未填时区固定主机 IANA 时区，无法确定时使用 UTC 并说明。夏令时重复时刻仅一次，不存在时刻跳过。
按 automation + scheduled UTC 去重；enabled=false 与定义同事务保存暂停，未提供 cron 时为手动计划。
工程只接受 Core 授权 UUID，重新核对实际目录、工作树、关系和修订；队员选择必须唯一且已配置，不能猜测、替换或自动安装。
`conversation_tool_automation_owner` 保存动作和时区。User 修改须同时匹配 owner revision 和 automation version；旧模型入口不能隐式修改外地时区或 enqueue_task 语义，返回 timezone_aware_edit_required。
用户自动化详情补充 owner 和分页 queuedMissions；旧模型工具结果仍由原合同拥有，不能拼入新 User 字段。已生成使命保留原计划 UTC、唯一发生编号和真实 Thread/Mission 身份。

## User API、运输和版本轴

User API 为 14 项：policy.get/set，questions.list/answer/decline/reconcile，grants.preview/confirm/revoke/list，requests.get/resolve，automationOwner.get/set；可信 Host 注入 User 和编辑端身份，不能接受模型伪造的 User。
来源列表和显式准备沿既有 `history.library`，计划补充详情沿 `automations.get`，不增加模型工具。
stdio 是唯一模型运输，自有服务保留名称冲突拒绝；用户 CLI、Web 和原生客户端均通过认证 User 入口，不能借模型通道调用。
数据库追加 Migration217/Schema167；保留旧216/166识别路径。User 公开操作登记为300项。
选中服务的 MCP 文件与 exposure 为3，未选中仍为2。Bootstrap5、Formatter5、Charter22、非批次Context29、公开Context34、Delegation35、Built-in39、IPC3、Envelope2、Output10 不因本服务改变。
Runtime Launch 当前合同递进为61：既有60已拥有原生恢复语义，批准快照中“60”的目标编号不覆盖它；本次只将已批准的增量登记为继承60的下一版，不修改冻结提案或改变批准字段。
