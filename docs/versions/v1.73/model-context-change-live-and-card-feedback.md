---
document_type: model-context-change
version: v1.73
change_id: live-and-card-feedback
revision: 1
confirmed_revision: 1
confirmation_status: confirmed
confirmed_by: user
confirmed_at: 2026-10-22T05:03:37+00:00
authority: proposed-model-input-change-statement
implementation_status: proposal_only
last_updated: 2026-10-22
---

# 队员工作时怎样收到你的补充和卡片选择

Codeg 与 Rovai 第一期融合 客户确认版 r1 待确认

这份方案请你决定一种使用方式：你可以把补充说明或卡片里的选择，明确交给正在做事的那位队员。系统先让你看见将发送的完整内容，再提交到你选定的会话。它不会因为你填了卡片，就替你广播、替你开新任务。

## 场景一 队员还在工作 你临时改了主意

你在私聊里让甲队员修改首页。它正在工作时，你补充“按钮用蓝色，先不要加动画”。你选择“交给当前队员”，界面会写明接收的是哪张会话、哪位队员和哪次任务，再显示这句话供你确认。

能使用反馈工具的智能体，会在自己检查新反馈时读到这句话。因此它可能不是在你点击后立即改变动作。界面只在连接确实写出结果后显示“已送到智能体连接”，不会声称模型已经理解。

这句话只交给甲。乙队员、其他私聊和独立帮手收不到。原来的群聊发送、私聊发送和附件入口照常使用。接收实时反馈的开关默认关闭；开启后从新的任务绑定生效，不往已经冻结的任务中途塞入新工具。

<!--PAGE-->

## 场景二 队员给了一张选择卡片

队员在回复里给你一张首页样式卡片，你选择“紧凑布局”，勾选“显示文件树”。卡片下面由应用提供“把填写内容交给队员”入口。点开后，你先看到这次选择对应的卡片、接收队员和将发送的完整内容，再确认提交。

只提交这张卡片中选中的可编辑字段。密码框、隐藏状态、整份网页、其他卡片和应用配置都不会一并收走。模型在卡片里画的按钮也不能绕过这次确认。普通切换标签、展开面板或本地计算，仍然只影响卡片本身。

旧版 Codeg 的卡片主要在卡片内部改变显示状态；本次把用户确认后的内容接回 Core，是一期原计划中的回传接线。原卡片组件、内置动作和 ui-render 技能原文保持。原生导入历史仍只读，不能借一张旧卡片直接控制现在的任务。

<!--PAGE-->

## 场景三 点击以后断线 或任务已经结束

你确认提交后，界面会保留这份原文和编号。重复点击只核对同一份提交，不另造一份。如果连接断开，系统显示“结果待核对”；它不假装反馈已被理解，也不偷偷把反馈转给后来启动的任务。

任务已经结束、停止、换队员或内部连接换代时，尚未送出的反馈保留为“需要你重新选择”。你可以把原反馈放回同一会话的普通输入框，检查原文后按原发送方式继续。这会是一轮新的处理，界面明确说明，不冒充上一轮仍在运行。

没有反馈工具的智能体也可以走普通输入框；它会按已有排队规则等待下一轮，不会声称当前运行已收到插话。原生即时推送、会话分叉和历史继续属于其他待实施范围，这一版不借此确认改变它们。

## 需要你确认的处理方式

请确认是否按“文字和卡片反馈 r1”实施：先看完整内容，再提交到明确的队员和任务；卡片只带选中的可编辑字段；断线保留原文和编号；旧任务失效后不自动改挂新任务，普通输入框续办需要你再次确认。

确认授权这一版说明中的模型工具、输出和回传实现。它不包含付费模型测试、main 合入或发布。研发自审、测试和交付仍由我负责，你的真实用户验收仍在 I12 之后。

<!--PAGE-->
# 研发附录

## 变更前

本工程当前没有向模型提供 check_user_feedback，没有本次反馈提交合同。JSON 卡片已移植原呈现和内置状态动作，自定义 actions 为 {}。ui-render 原技能文字保持固定供体原文。这份方案不把只读呈现代码称为模型回传已完成。

固定 Codeg 原仓的反馈工具是独立的中途文字指导通道，并非原 JSON 卡片的外部提交动作。以下分别列出原工具及结果呈现函数，再给出本工程的完整候选改动；来源功能与融合新接线不混写。

## 变更后

新增独立 stdio MCP 服务 rovai-ui-feedback，仅提供下述一个工具。每位队员有 enabled 布尔开关，默认 false；关闭不投递，不启动本服务。开启只影响新 Run 绑定。工具列表在绑定内固定，tools.listChanged=false。JSON 卡片的 actions 仍为 {}，不增加 submitToAgent、任意脚本、网络请求或 watch 自动发送动作。

模型参数不能填写 Core 身份。Core 绑定真实 Thread、Conversation、队员、Run、epoch、原生绑定和成员代次，调用前、排队提升前及结果写出前重新核对。公开、私聊、帮手范围不继承。队员版本或权限变动、离队、停止、过期和换代后拒绝旧绑定。

真实运行中的反馈通过本工具读取；没有可靠 MCP 通道的 Runtime 使用普通输入框续办。Pi 在原 Codeg 中未投递 MCP，保持不可用事实。Cursor 保持独立候选，未经真实会话隔离与调用验证不能标可用。其他 Runtime 沿现有 external MCP 与 stdio 能力逐项验证，源码分支不等于实际准入。

## C 卡片与普通文字的用户提交

应用只为 Core 自己保存的已完成 assistant 回复提供卡片回传入口。Core 根据 messageId、会话种类、cardOrdinal 和原文摘要重新定位卡片，不接受前端自称的来源正文或另一个队员。来自工具输出、用户消息、外部导入历史、原生只读帮手转录和仍在流式变化的卡片不提供此入口。

应用在原组件外登记已呈现的可编辑控件绑定，支持 Input、Textarea、Select、Checkbox、Radio、Switch 及 repeat 中的实际索引；提交字段必须能回溯到原 spec 中的这些绑定。password 控件强制排除。只发送用户在应用预览中勾选的字段，不提交其他 state、隐藏项、文件内容、附件或应用设置。数值必须有限，JSON 深度最多 8，最多 128 叶值，数组最多 32 项，对象最多 64 个键，禁止 __proto__/constructor/prototype。

反馈先在 Core 创建十分钟有效的预览，返回完整 text 与 previewDigest。确认只引用 previewId 和该摘要，不能同时替换正文。普通文字原样保存，前后空白不偷偷修改；卡片 text 是 kind/title/fields 的完整规范 JSON，kind 固定 generated_ui_feedback，fields 按 statePath、name 排序，value 保留原 JSON 类型。整个 text 最多 4096 个 Unicode 字符，超限拒绝且保留编辑内容，不截断后发送。

提交使用 UUID commandId 和原摘要幂等。相同编号不同内容拒绝；重复点击回读原结果。Core SQLite 事务拥有反馈、预览和提交回执。模型侧只有读取工具，没有创建预览、确认或续办的工具入口；这些操作只在可信 Native 所有者或已认证 Web 用户界面开放，不加入 Builtin CLI 的 39 项合同。

## D 投递 冻结和未知状态

每次原生绑定独立领取私有反馈凭据，MCP 配置只在应用私有目录。服务命令为当前 Core 二进制，参数依次为 --ui-feedback-mcp、--config、当次私有配置路径；cwd 是当前 Run 已核定执行目录，env 为空。不写全局智能体配置，不借其他 Run 的连接。新服务追加在已有 MCP 定义之后，既有服务的相对顺序与字段保持；同名冲突拒绝覆盖。

一份工具响应只选同一精确 Run 与 epoch 的 pending 反馈，按创建时间和编号稳定排序。最多 256 条待发反馈，超量拒绝新提交并保留本机输入。每条最多 4096 字符，完整渲染结果以 UTF8 序列化不得超过 65536 字节；按完整输出大小取可放入的前缀，不按估算字符数越过帧上限。未取出的仍 pending。读取不先消费，响应实际写出后才记 connection_written。

同一连接相同 JSON RPC id 与参数摘要回读冻结批次和原输出；编号相同但参数不同拒绝。未知的 nativeToolCallId 为 null，不按时间或参数猜测。断线或写出状态无法确定时记 unknown，并保留稳定反馈编号；模型会在文本和结构化结果中同时看到编号，必须按编号避免重复动作。不能据此承诺模型侧恰好执行一次。

任务停止或换代后，未投递项转 requires_user_review，绝不自动转到新 Run。用户选择普通输入框续办时，只能选择从未进入不明投递的 pending 项，并再次查看完整正文；沿现有 User 消息、附件、排队和预算规则发送，状态与 continuedMessageId 在 Core 命令事务内绑定。已通过 MCP 投递或 unknown 的项不能悄悄再作为普通输入发送；需要先核对或由用户另写新反馈。

旧原生绑定能否按新 Run 重新绑定 MCP 沿单独验证矩阵；没有可靠证据则回收旧进程、新建内部会话并沿既有 Core 历史范围准备，原记录保留，界面说明内部连接重建。旧冻结 Run 不热升级。此处不改变历史范围、预算、Skill 选择、Bootstrap 正文或旧 Manifest；新的独立投递证据只证明本次新增服务的配置及结果写出。

## E 版本 数据和兼容

新反馈目录版本、工具输出版本、私有传输协议、反馈记录与投递证据均为 1。新增数据库表与可回退迁移待确认后实施；当前 schema162 不在提案阶段改写。反馈随原 Core 数据目录备份，恢复后所有旧运行凭据失效，未确认投递保留待用户核对。旧表和旧输入不迁改，不把历史 User 消息自动转成反馈。

Builtin Tool Contract 与 CLI 39、IPC 3、Envelope 2、Agent Output 10 均不变；MCP Projection 继续 2，只在新冻结定义列表中增一条服务。Bootstrap、Formatter、Manifest、公共批处理、独立帮手及 Skill 配对轴全部维持下面冻结的声明，不能只看版本数字 29 判兼容。旧技能包 r1、私聊隔离 r1、独立帮手 r1 和浏览器电脑 r1 的已确认语义保持。

## F 完整字段约定

下面所有对象为闭集，列出的字段全部必填。nullable 字段允许显式 null；其余不允许省略或写 null。Core 身份字段按数据库核对，字符串不是授权证明。SHA 字段采用 sha256: 加 64 位小写十六进制。JSON 字段值不允许非有限数字。机器附件给出对应完整 Schema 和基线摘要。

Scope { threadId:string, conversationId:string|null, scopeKind:public|single_chat|delegation_child }。public 的 conversationId 必须 null，其余必须是本 Thread 当前队员的真实 Conversation。

CardOrigin { messageId:string, conversationId:string|null, messageKind:public|single_chat|delegation_child, cardOrdinal:0..127, sourceDigest:sha256 }。来源必须属于当前 Scope，原回复的责任队员等于接收队员，卡片原文摘要一致。

FeedbackField { name:string, label:string, statePath:string, value:JSON }。name 最多 128 字符，label 最多 200，statePath 最多 512。字段与值在预览时冻结，不能在确认请求中修改。值仅是已选择控件的数据，不是指令权限。

PreviewRequest { schemaVersion:1, scope:Scope, agentId:string, agentRunId:string, executionEpoch:integer, source:{kind:live_note}|{kind:generated_card,origin:CardOrigin}, text:string, title:string, fields:FeedbackField[] }。普通文字的 title 为空且 fields 为空；卡片的 text 为空，由 Core 生成完整规范 JSON。两类不能混合冒充。最终 text 必须非空且不超过 4096 字符。

PreviewResult { schemaVersion:1, previewId:UUID, previewDigest:sha256, scope:Scope, agentId:string, agentRunId:string, executionEpoch:integer, text:string, expiresAt:UTC time }。确认界面展示这里返回的全部 text，不能只显示摘要。

SubmitRequest { schemaVersion:1, commandId:UUID, previewId:UUID, previewDigest:sha256 }。Core 重新检查预览所有者、期限、消息来源、运行资格和原权限修订；复用编号的回执不重新派发。

Binding { schemaVersion:1, bridgeInstanceId:UUID, coreInstanceId:UUID, scope:Scope, agentId:string, agentRunId:string, executionEpoch:integer, nativeBindingId:string, nativeBindingGeneration:integer, nativeSessionId:string|null, membershipGeneration:integer, policyRevision:integer, catalogDigest:sha256, expiresAt:UTC time }。nativeSessionId 仅建立原生会话之前可 null；tools/call 前须核实实际值。

MemberPolicy { schemaVersion:1, revision:integer, members:[{agentId:string,enabled:boolean}] }。采用 expectedRevision 的独立 CAS 配置，默认全部关闭。该表不替代 Camp 成员、Runtime、MCP 或权限权威。

ModelNote { id:UUID, text:string, created_at:UTC time }。模型可见结果是 {count:integer,feedback:ModelNote[]}，count 必须等于数组长度。created_at 保留原 Codeg 的 snake_case 拼写。只有三个字段，不包含内部 commit id、路径、凭据或其他会话数据。

Receipt { schemaVersion:1, feedbackId:UUID, scope:Scope, agentId:string, sourceAgentRunId:string, sourceExecutionEpoch:integer, sourceKind:live_note|generated_card, textDigest:sha256, status:pending|response_prepared|connection_written|unknown|requires_user_review|continued_as_message, deliveryChannel:none|mcp|normal_message, bridgeInstanceId:UUID|null, transportRpcId:string|integer|null, nativeToolCallId:string|null, resultDigest:sha256|null, writtenAt:UTC time|null, continuedMessageId:string|null }。connection_written 只代表写到连接，不能称模型已读或已理解；每条投递尝试另留原绑定和摘要，不覆盖历史尝试。

失败结果固定 content 中一段文字 Feedback tool is unavailable for this run.，isError 为 true，structuredContent 为 {schemaVersion:1,status:unavailable|rejected|unknown,code:固定错误码}。固定错误码是 feature_disabled、runtime_unavailable、scope_changed、run_not_active、epoch_changed、binding_changed、member_changed、policy_changed、result_too_large、original_request_pending、transport_failed、feedback_unknown。失败不会消费 pending 反馈。

私有 ProcessConfig { schemaVersion:1, binding:Binding, bindingDigest:sha256, coreEndpoint:string, authorizationToken:string, maxFrameBytes:65536 }。endpoint 必须是本次应用实例的本机私有 IPC，不接受远程 URL；token 为每个绑定独立随机凭据，不进入模型、日志或共享配置。bindingDigest 是完整冻结 Binding 的规范 JSON 摘要。配置文件只允许当前用户访问，撤权或退出后回收，重启重新领取。

私有 prepare 请求 { schemaVersion:1, bindingDigest:sha256, transportRpcId:string|integer, argumentsDigest:sha256 }。Core 由连接凭据定位真实 Binding，不能用请求中的字段换身份。参数摘要按完整原 arguments 的规范 JSON 计算，原工具无参且额外键不用于身份。RPC id 与参数摘要只用于在该绑定内重放冻结结果。

私有 prepare 返回 { schemaVersion:1, attemptId:UUID, bindingDigest:sha256, feedbackIds:UUID[], resultDigest:sha256, result:完整 MCP result }。result 只允许本文完整成功呈现函数或固定失败结果；resultDigest 对原序列化结果字节计算，feedbackIds 与成功结果逐项一致。进程不得自行拼入其他反馈。空结果也记录同一请求的冻结结果，重试不能换成后来新增的反馈。

私有写出回执 { schemaVersion:1, attemptId:UUID, bindingDigest:sha256, resultDigest:sha256, writeStatus:written|unknown }。Core 校验原请求和摘要后更新结果；回执只可确认本绑定已准备的结果，不能确认另一批、删除原文或转派给新任务。写出前再核对资格；准备后撤权则以固定失败回应并把原批次留为需要核对。写出后撤权不会把已经发生的连接写出抹成从未投递。

成员策略更新 { schemaVersion:1, expectedRevision:integer, members:[{agentId:string,enabled:boolean}] }。失败返回当前修订，界面要求刷新；不能用过期列表覆盖他人的选择。预览、反馈原文、命令回执与每次投递尝试分别存入本次新增的 Core 表。预览到期可清理；最终反馈随所在会话保留，不能因为完成一次工具读取就删除用户原文。到期绑定只失去执行资格，不删除已留存的回执。

模型只看 tool 的 name、description、inputSchema 和完整 MCP result。上面的私有配置、预览、策略与回执不是额外模型参数，不把内部身份或凭据嵌入工具说明。所有私有操作属于新增独立传输协议，不更改既有 Builtin IPC 3 的 shape。

## G 完整普通输入框续办文本

```text
请根据下面这份由我确认的反馈继续处理，仍遵守当前会话的权限和审批设置。

反馈编号：{feedbackId}
原任务：{sourceAgentRunId}
反馈原文：
{text}
```

## H 原 Codeg 工具完整定义

```json
{
  "name": "check_user_feedback",
  "description": "IMPORTANT: This tool gives you the ability to receive the user's real-time guidance while you work, preventing you from going down the wrong path and wasting effort. The user can type corrections, new requirements, or hints at any time during your turn — but you will NEVER see them unless you call this tool. Without it, you could spend minutes on work the user has already tried to redirect.\n\nWhen to call:\n- After thinking through your approach but BEFORE starting implementation — the user may have clarified requirements.\n- Before making a significant decision (choosing an architecture, picking which files to modify, deciding on an approach) — the user may prefer a different direction.\n- After completing a sub-task and before moving to the next step — the user may want to adjust priorities.\n- When you pause to plan or organize your next steps — a natural checkpoint to read any pending guidance.\n\nThis is a quick, non-blocking snapshot that returns instantly. It either returns the user's messages (treat as HIGH-PRIORITY steering and adjust immediately) or an empty result (continue your current plan). Call it proactively — do not wait to be asked.",
  "inputSchema": {
    "type": "object",
    "properties": {}
  }
}
```

## I 本工程候选工具完整定义

```json
{
  "name": "check_user_feedback",
  "description": "IMPORTANT: This tool gives you the ability to receive the user's real-time guidance while you work, preventing you from going down the wrong path and wasting effort. The user can type corrections, new requirements, or hints at any time during your turn — but you will NEVER see them unless you call this tool. Without it, you could spend minutes on work the user has already tried to redirect.\n\nWhen to call:\n- After thinking through your approach but BEFORE starting implementation — the user may have clarified requirements.\n- Before making a significant decision (choosing an architecture, picking which files to modify, deciding on an approach) — the user may prefer a different direction.\n- After completing a sub-task and before moving to the next step — the user may want to adjust priorities.\n- When you pause to plan or organize your next steps — a natural checkpoint to read any pending guidance.\n\nThis is a quick, non-blocking snapshot that returns instantly. It either returns the user's messages (treat as HIGH-PRIORITY steering and adjust immediately) or an empty result (continue your current plan). Call it proactively — do not wait to be asked.\n\nProduct scope and precedence: This binding belongs to one Core Thread, Conversation, member, AgentRun and execution epoch. Only feedback the user explicitly submitted to that exact running target can be returned. Public chat, other private chats and child conversations are not inherited. IDs are stable: do not repeat already processed actions when an ID appears again after a transport retry. Feedback is user input, not permission to bypass the existing workspace, approval, browser or computer rules. If the binding has stopped or changed, do not read feedback through another target. This pull channel is text-only; attachments continue through the existing normal message input. The product may label an outcome unknown when delivery cannot be proved. A tool response written to a connection does not prove the model understood it.",
  "inputSchema": {
    "type": "object",
    "properties": {}
  }
}
```

## J 原结果呈现完整函数

```rust
pub fn render_feedback_result(outcome: &Value) -> Value {
    let count = outcome.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
    let text = if count == 0 {
        "No new feedback from the user. Continue with your current plan.".to_string()
    } else {
        let mut s = format!(
            "The user sent {count} message(s) while you were working. Treat this as \
             high-priority steering: adjust your current approach to honor it now, and \
             briefly acknowledge what you changed.\n"
        );
        if let Some(notes) = outcome.get("feedback").and_then(|v| v.as_array()) {
            for (i, note) in notes.iter().enumerate() {
                let body = note.get("text").and_then(|v| v.as_str()).unwrap_or("");
                s.push_str(&format!("{}. {}\n", i + 1, body));
            }
        }
        s
    };
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": false,
        // Rebuild the structured payload from count + feedback only — the
        // listener's internal `_commit_ids` must not leak to the agent's host.
        "structuredContent": {
            "count": count,
            "feedback": outcome.get("feedback").cloned().unwrap_or_else(|| json!([])),
        },
    })
}
```

## K 本工程候选完整替换函数

```rust
pub fn render_feedback_result(outcome: &Value) -> Value {
    let notes = outcome.get("feedback").and_then(Value::as_array)
        .cloned().unwrap_or_default();
    let count = notes.len();
    let text = if count == 0 {
        "No new feedback from the user. Continue with your current plan.".to_string()
    } else {
        let mut text = format!("The user sent {count} message(s) while you were working. Treat this as high-priority steering: adjust your current approach to honor it now, and briefly acknowledge what you changed. Feedback IDs are stable; do not repeat actions already processed for the same ID.\n");
        for (index, note) in notes.iter().enumerate() {
            let id = note.get("id").and_then(Value::as_str).unwrap_or("");
            let at = note.get("created_at").and_then(Value::as_str).unwrap_or("");
            let body = note.get("text").and_then(Value::as_str).unwrap_or("");
            text.push_str(&format!("{}. [feedback_id={id} created_at={at}] {body}\n", index + 1));
        }
        text
    };
    json!({"content":[{"type":"text","text":text}],"isError":false,
        "structuredContent":{"count":count,"feedback":notes}})
}
```

## L 保持不变的上下文版本声明

```rust
use serde_json::{Value, json};

pub const NATIVE_SESSION_BOOTSTRAP_CONTRACT_VERSION: &str = "native_session_bootstrap_v5";
pub const BOOTSTRAP_FORMATTER_VERSION: i64 = 5;
pub const SESSION_CHARTER_REVISION: i64 = 22;
const NATIVE_BINDING_CHARTER_COMPATIBILITY_REVISION: i64 = 16;
pub const AGENT_RUN_CONTEXT_FORMATTER_VERSION: i64 = 29;
pub const CONTEXT_MANIFEST_VERSION: i64 = 29;
pub const PUBLIC_CAMP_BATCH_CONTEXT_FORMATTER_VERSION: i64 = 34;
pub const PUBLIC_CAMP_BATCH_CONTEXT_MANIFEST_VERSION: i64 = 34;
pub const DELEGATION_CONTEXT_VERSION: i64 = 35;
pub const DELEGATION_SESSION_CHARTER_REVISION: i64 = 23;
pub const DELEGATION_BINDING_REVISION: i64 = 17;

pub(crate) const PACK_CONTEXT_MANIFEST_VERSION: i64 = 29;
pub(crate) const PACK_PUBLIC_CONTEXT_MANIFEST_VERSION: i64 = 34;

/// Numeric version 29 belongs to two different frozen contracts. Callers must
/// retain the invocation/profile/facts tuple instead of classifying by number.
pub(crate) fn skill_pack_context_format(
    invocation_kind: &str,
    manifest_version: i64,
    profile_version: i64,
    facts_version: i64,
) -> anyhow::Result<bool> {
    if manifest_version == DELEGATION_CONTEXT_VERSION {
        anyhow::ensure!(
            matches!(
                (invocation_kind, profile_version, facts_version),
                ("delegation" | "delegation_user", 11, 10)
                    | ("single_chat", 13, 6)
                    | ("batch" | "direct" | "a2a" | "gather_completion", 12, 9)
            ),
            "Delegation Context format/profile/Run Facts pairing is invalid"
        );
        return Ok(true);
    }
    let batch = invocation_kind == "batch";
    anyhow::ensure!(
        batch
            || matches!(
                invocation_kind,
                "direct" | "a2a" | "single_chat" | "gather_completion"
            ),
        "Unknown Context invocation kind"
    );
    match (batch, manifest_version, profile_version, facts_version) {
        (true, PACK_PUBLIC_CONTEXT_MANIFEST_VERSION, 10, 9)
        | (false, PACK_CONTEXT_MANIFEST_VERSION, 7, 6) => Ok(true),
        (true, 29, 9, 7)
        | (true, 30, 10, 7)
        | (true, 31, 10, 8)
        | (true, 32 | 33, 10, 9)
        | (false, 26, 6, 5)
        | (false, 27, 7, 5)
        | (false, 28, 7, 6) => Ok(false),
        _ => anyhow::bail!("Context format/profile/Run Facts pairing is invalid"),
    }
}


```

## M 可执行验证计划 尚未执行

FB-01 默认关闭时 tools/list 不出现反馈工具，也不创建子进程。

FB-02 开启仅作用新绑定，旧 Run 的冻结 tools/list 与既有输入逐字节保持。

FB-03 甲私聊的文字和卡片反馈不能被公开对话、乙队员或独立帮手读取。

FB-04 已离队、角色版本变化、Run停止、epoch变化、原生绑定变化逐一拒绝。

FB-05 前端替换 messageId、cardOrdinal、sourceDigest、接收队员或会话时全部拒绝。

FB-06 用户消息、工具输出、导入历史、原生只读转录和流式卡片不能回传。

FB-07 六类输入控件、嵌套 state、repeat 实际索引及 JSON Pointer 转义逐项核对。

FB-08 password、隐藏 state、未选字段、原型键、文件与二进制内容不投递。

FB-09 预览必须逐字展示完整最终 text；确认请求改摘要或过期不得提交。

FB-10 Unicode 4096 字符边界、UTF8多字节、转义膨胀和65536字节完整响应上限。

FB-11 256条待发上限与超过批次大小的尾项保持 pending；不能静默截断。

FB-12 空结果精确返回 count=0、feedback=[] 和完整固定文字。

FB-13 有反馈时文本与 structuredContent 均含同一原编号、原文和时间。

FB-14 同 commandId 同内容回读；同编号改内容拒绝；双击不新增第二份。

FB-15 同 RPC id 同参数回读原批次；相同编号不同参数拒绝。

FB-16 读取后未写出、写一半断线、写出后提交回执丢失分别保留正确未知状态。

FB-17 nativeToolCallId 缺失保持 null，不从时间或参数猜测。

FB-18 未知状态不自动重发物理或模型动作，不自动换绑新 Run。

FB-19 正常结束、停止、重启和备份恢复后待发项进入需要用户核对。

FB-20 普通输入框续办只迁入明确未投递项，不能与 MCP 双通道消费同一项。

FB-21 续办排队、发送失败、输入超预算沿旧机制保留完整原文。

FB-22 本地控件 state/标签切换只改变卡片，不自动创建 Core 反馈。

FB-23 卡片程序化事件不能绕过应用预览和单独确认。

FB-24 Native、Web、移动浏览器同一数据与状态显示，Web 不取得私有桥凭据。

FB-25 所有21个既有Runtime按MCP能力矩阵记录支持、候选或具体不可用原因。

FB-26 Pi保持原无MCP事实，Cursor真实非空独立MCP隔离证据前不标准入。

FB-27 两个可用Runtime的真实文字反馈与卡片反馈证明模型实际收到编号和内容。

FB-28 新旧完整 Bootstrap/DynamicContext、技能选择及预算对照；旧冻结记录不变。

FB-29 MCP同名冲突不覆盖用户服务，原31浏览器电脑工具与五个开关不变。

FB-30 大反馈队列/数据库锁/并发撤权/有序关闭与恢复，不丢用户原文。

验证按客户七步顺序执行：本步完成全部源码并统一代码 Review 后，第二步整理测试点，第三步编写并执行用例。先用受控本地桥和隔离数据核对正确性；真实模型 Gate 的身份、预算和付费授权另行取得，不在本方案中代填。测试结果、真实画像/调用证据和既有上下文对照不能用源码检查代替。

## N 方案自审和待确认记录

本方案已把原卡片只做本地状态、原反馈独立工具和融合回传三者区分；明确默认关闭、新绑定生效、文字限制、完整预览、关闭后待核对和双通道互斥。真实模型是否读取不由 connection_written 冒充。当前只形成待确认提案，没有新增模型工具、Schema实现或当前合同变更。

confirmation_status 为 pending，revision 为 1。需要用户阅读完整说明后明确回复“确认，按文字和卡片反馈 r1 实施”。方案语义改变需增 revision。此前技能包、私聊、帮手和浏览器电脑的确认不替代本次确认。

## O 基线和来源

{
  "commit": "2b88131f1fe1db78dc48c2655c2e00e762919d0e",
  "branch": "develop/V1.0.0",
  "files": {
    "crates/rovai-core/src/context_contract.rs": "63d76e9074fbfd55b078a250fbe5b25932fb71a7f5d232e99b52f6aa64b62150",
    "crates/rovai-core/src/context.rs": "508756018afb55bba7731b11f5901a4f9a042c6827a55d0b64d09703cf068035",
    "crates/rovai-core/src/mcp_projection.rs": "88ce43d76e6f2bc13b311dcc8e003d8b7f7407945b7b479c09d9e508ede3b7e0",
    "crates/rovai-core/src/builtin_tool_transport.rs": "f96b510959398316c3ae37c884415179ede44e682f0571666206ea39048be2d0",
    "apps/desktop/src/renderer/src/codeg-native-content/lib/json-render/catalog.ts": "530c6aa03216eb3f5f43112112573759994608696632d202691dc508f98f7546",
    "apps/desktop/src/renderer/src/codeg-native-content/components/message/json-render-block.tsx": "0dc587b39f137d2e297a8f90aa6061c3fd713b21e3059c919ffa7b01fd9f0365"
  },
  "donorRevision": "aff1e6c278453ed8d0b96a5f664edcdca621aa3f",
  "donorUiSkillSha256": "46bc09a16dc1f2d09f21004f0891a630ff5010f642b975d82f82be113330daff"
}


## 明确不变

本次仅整理既有 r1 确认记录与章节索引。正文中既有模型输入、字段、省略规则、权限、冻结、恢复及版本轴约定全部保留；此次不新增模型工具或改变输入语义。完整不变范围仍以本页已确认正文及原冻结附件为准。

## 二次确认

当前会话用户阅读对应客户版后明确回复：“ok”。原确认时间 2026-10-22T05:03:37.288974+00:00，Front Matter 只去除秒后的小数以兼容既有文档校验器，保留同一确认时刻到秒。revision 与 confirmed_revision 继续为 1，不重新代用户确认。投递时的 pending 快照保留，后续用户确认以本节及既有确认记录为准。

## 验证

既有正文的全部验证点继续有效。本轮整包编译、回归与跨平台结果由统一修复 PR 记录；未实际执行或缺少环境的项目继续标为待验证。编译通过、组件测试通过及最终真实用户验收分别记录，不将本节视为通过证明。
