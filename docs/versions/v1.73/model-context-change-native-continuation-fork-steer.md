---
document_type: model-context-change
version: v1.73
change_id: native-continuation-fork-steer
revision: 1
confirmed_revision: 1
confirmation_status: confirmed
confirmed_by: user
confirmed_at: 2026-10-22T09:53:51+00:00
authority: proposed-model-input-change-statement
implementation_status: proposal_only
last_updated: 2026-10-22
---

# 从旧会话继续工作和分叉讨论

原生继续分叉插话 r1 客户确认版 待确认

这份方案决定三件使用体验：怎样接着旧会话做事，怎样从某条回答另开一条路，以及怎样在队员工作中把完整补充送进去。它属于第一期完整融合范围，不另拆成以后再做的功能。确认后我继续写源码；软件使用验收仍在完整一期开发和工程验证结束后由你进行。

当前只是在确认方案，没有运行这些新功能，没有开始软件统一 Review 或测试。你此前确认的技能包、私聊材料、独立帮手、浏览器电脑、文字和卡片反馈、自定义智能体方案继续分别有效。

## 场景一 接着昨天那段会话往下做

你在原生会话库里找到昨天与 Codex 或 Claude 的讨论，点“继续”。软件先给你看这份记录的来源、使用的智能体、原工程、历史范围和将创建的新会话。你确认后，软件创建一个独立工作会话，只放入你选定的那位队员；原来已经打开的公聊、私聊和画布卡片不会被顶替。

“继续”会让原智能体程序恢复那段原生历史。原工具会按自己的历史与压缩规则恢复，软件不能据此宣称模型完整读过每条记录。你能看到来源和恢复状态，不能只看到一个没有依据的“成功”。

如果原文件已变化、找不到来源，或者当前程序不能恢复这个会话，软件会说明原因并保留原预览，不会偷偷把只读记录拼成一段普通消息冒充继续。需要同一工具的 ACP 接口时，会明确显示并让你选择已经添加的对应程序，不自动换程序或替你安装。

继续会话之后，原智能体程序可能继续写自己的原生日志；导入时保存的原始快照保持不变。旧记录里的工具权限不会自动变成新会话权限，其他队员的材料也不会自动加入。

<!--PAGE-->
# 从某条回答另开一条路

## 场景二 保留旧方案再试一个新方向

队员已经讨论了方案甲，你想从前面某条回答改成方案乙。你在那条回答上点“从这里分叉”，软件会同时显示分叉位置和原会话。只有能可靠定位到这条回答时，按钮才按“从这里”执行。

有些原工具只支持从会话末尾另开分支。此时软件会写明“只能从末尾分叉”，需要你另选这个动作；不会把从末尾另开包装成从所选回答另开。遇到重复的相同回答，软件还会核对它是第几次出现，避免分到另一条相同文字上。

分叉只是新建会话分支，不会替你重置 Git、回退工程文件、切换分支或改写原会话。你可以继续保留方案甲，再在新会话中研究方案乙。工程文件是否回退，仍由已有 Git 工作区中的明确操作决定。

继续和分叉都会保留原请求编号。断线时先读取原结果；在不知道外部程序是否已经执行的情况下，不再创建第二个原生会话。

<!--PAGE-->
# 队员工作中收到你的完整补充

## 场景三 补一句话并带上参考图

队员正工作时，你补充“按钮改成蓝色，以这张图为准”。如果这次连接实际支持安全插话，软件会先让你核对文字、图片和对应队员，再把这份完整输入送到这一轮。原来的输入草稿和其他卡片材料不会被顺手夹带进去。

如果任务恰好在发送前结束，原工具明确回答“没有接收”，整份输入会保留下来。你可以选择作为下一轮普通消息发送；不会只留下文字、丢掉图片。如果已经接收，则不会再排一遍。如果断线后无法确认，显示“待核对”，不重复发送。

原版 Codeg 也按实际接口和版本决定是否能直接插话。当前源码依据里的 ACP 原生推送只对白名单中的 Claude ACP 接口开放；它不等于所有名叫 Claude 的程序都支持，更不等于 Codex 已有同一能力。内置原生接口保持原执行方式，已确认的“队员主动检查补充”仍可使用。具备对应 ACP 程序时可明确选择该接口，不自动换通道。

插话开关默认关闭，开启后只作用于之后开始的任务。已经运行的任务不会突然新增输入通道。停止、换队员、接管、换工程或重新连接后，都要重新核对原任务归属。

## 这次需要你确认什么

是否同意以上三种处理方式，并按“原生继续分叉插话 r1”实施。确认只授权这些输入与恢复方式的开发，不包含付费模型调用、合入 main 或发布。后面的研发附录记录完整字段、兼容策略和待执行验证，你不需要代替我做技术验收。

<!--PAGE-->
# 研发说明

## 固定源码依据与当前差异

Codeg 固定提交 aff1e6c278453ed8d0b96a5f664edcdca621aa3f。fork.rs 的 ForkPoint、fingerprint_agent_message、fingerprint_occurrence 和 resolve_fork_point 定义分叉点；manager.rs 的 fork_session 在单连接 prompt 锁下核对正在执行状态，并保留原生新会话结果。当前融合历史库 NativeHistoryWorkbench.tsx 和 application/history_import.rs 仅导入及冻结查看，明确不赋予执行身份。

Codeg connection.rs 的 build_steer_params 使用 _session/steering 与 _meta.steering.idleBehavior=promptRequired；回执分 injected、promptRequired、startedNewTurn。manager.rs 的 submit_feedback_native 在读取附件后再次核对轮次，并在调用者断开后继续记录真实结果。use-session-feedback.ts 与 queued-steer.ts 保留完整输入后处理无活动轮次。registry.rs 的 steering_prompt_required_min_version 仅对 ClaudeCode 返回 0.65.0；自定义程序还须实际报告精确的 claude-agent-acp 身份，且广告能力与真实版本均满足。以上是固定源码的行为依据，尚不是本融合产品的实测结论。

Rovai 固定提交 cc465dda562536b94f23787759e3206324baa29e。Core 继续拥有 Thread、成员、工作目录、审批、Run、Binding、当前输入和回执。继续/分叉不把原应用的 ConnectionManager 移植成第二个权威。原生历史中的会话编号、工程路径或工具 JSON 不构成授权，源定位必须由 Core 从已冻结导入和已选择的本机来源得到。

## 变更前后及不会改变的内容

变更前：导入记录只能查看、整理和导出；不存在从导入快照启动的 NativeSessionOrigin。文字和卡片反馈按已确认 r1 走队员主动检查通道，不主动向工作中的原生会话推送整份输入。

变更后：只有本次明确确认的继续/分叉动作，才在新工作会话的新绑定中冻结 NativeSessionOriginV1。只有用户启用且新任务冻结的原生插话意图，才带 FrozenNativeSteerV1；实际通道还要在本连接读取广告、版本和适配器资格。没有这两项的新旧内置任务省略字段，不能写 null 或空对象改变旧字节或摘要。

Bootstrap SESSION_CHARTER、MEMBER_IDENTITY、Memory 入口全文，Dynamic Context section 名称、顺序、预算、截断、Current Input、历史引用、工具目录、ModelSelection 和权限 shape 均不改写。原生恢复会让原工具重新采用它自己的历史，这是本次明确说明的新输入来源。Core 的当前输入仍通过原流程发送，不额外把整个导入 JSON、注册表说明或旧工具参数拼入提示词。

普通 Runtime 的配置摘要算法不改写；只有 nativeOrigin/nativeSteer 实际存在时，新绑定与 Host 配置摘要才绑定其规范 JSON。既有历史、Manifest、Skill 选择、Custom ACP 定义与秘密版本不重算，不对旧数据库执行全量输入重写。

## 原生继续和分叉的执行流程

Core 先核对用户、导入记录的 Thread 归属及修订、原文件句柄和字节修订、Provider 原生会话编号、工程指纹、成员版本、所选 Runtime 配置和真实接口。旧导入没有可复核来源句柄时，要求通过已有本机选择入口重新选择来源并核对，不能把导入文件名当路径。Web 与移动端只使用 Host 已登记的来源，不能传入任意服务器文件路径。

预览冻结新 Thread、Conversation、请求、来源和目标成员，10 分钟有效。确认后事务创建只有该成员的新 Thread 及私聊执行入口，登记外部启动意图；不覆盖任一已有公聊、私聊或画布材料。卡片入口只引用新会话的真实对象，不复制其他卡片材料，也不伪造跨 Thread 的授权。

Host 在数据库锁外执行对应 Runtime 的 continue/load/resume/fork。分叉到回答时沿原版规则只接受助手轮次；Codex 没有可用原生 item id 时使用助手文本 UTF-8 sha256 和从一开始的重复出现序号；Claude ACP 与 DeepSeek ACP 同时保留可用原生 id 与指纹。去掉尾部 :segment:n；无文本且无原生 id 的轮次不能声明精确分叉。精确分叉需要支持的实际接口，尾部回退必须重新选择。

原生 continue/fork 能力按具体协议确认，不从 UI 名称或成功安装推断。内置原生接口不因这份方案被改成 ACP；同一 Provider 需要 ACP 才能使用某项能力时，提供已添加且归属可证明的程序选项。不能匹配到同一 Provider 的自定义程序不获得接管原生记录的资格。

Core 记录实际原生会话 id、真实启动回执和绑定，再准许首次用户消息。消息不跟随启动动作隐式发送。同一 source/provider/nativeSessionId 在 Core 内持有执行租约；冲突只读提示。Core 无法约束另一个独立原应用的进程，检测到外部历史变化时保留来源快照并要求重新核对，不声称拥有跨应用绝对独占锁。

分叉不会调用 reset、checkout、rebase、stash 或工作树替换。外部原生程序实际创建新会话后，若 Core 绑定落盘失败，记录 unknown 与已知外部编号，禁止自动再分叉。只能从实际回执或原工具可证明的查询结果恢复绑定；无法证明时保留记录，不发首次消息。

## 插话的输入与回执规则

用户调用 targets 取得当前 Run、epoch、Binding、代次、兼容摘要和原生活动轮次。activeTurnId 是 Host 当前 prompt 的不可复用轮次令牌，不是从旧日志猜出的轮次。现有 Thread、Conversation、Run 和 Binding 编号沿用 Core 的有界不透明标识，不强行改变旧编号为 UUID。预览只冻结用户明确选择的文字和材料，不读取其他输入框、卡片、私聊、队员、文件夹或隐藏附件。按材料快照、当前归属和内容摘要复核；接口参数只含 Core 已登记 sourceId，不接受本机路径或外部 URL。

每份文字最多4096个 Unicode 字符，材料最多10项，单项最多20MiB，同一原生活动轮次的图片合计32MiB；在写出前事务预留额度，未知结果也占用额度。管理请求只传材料引用和摘要，最多2MiB；原生图片读取和编码在 Host 的受限过程执行，单次原生写出上限64MiB。超过上限先拒绝并完整保留输入，不能在已接收后丢掉图片来冒充完整记录。

确认复用预览编号和摘要并使用新命令 UUID。预览和回执都按 thread/conversation/agent/run/epoch/binding/generation/activeTurnId 校验；读取材料后、写出前再核对。当前任务停止、接管、换工程或新轮次开始时，原预览不得投给新任务。

直接 ACP 推送严格沿原版广告、精确程序身份和实际版本门槛；内置原生接口没有经证明的等价通道时，不照抄 ACP 方法名调用。现阶段原生推送按已添加且合格的 Claude ACP 程序接线；其他原接口继续已有主动检查反馈与普通排队方式。原生 Codex turn/steer、Pi steering 等独立通道不能仅凭名称开放，若新增等价实现，必须保持本 r1 的完整输入、结果与归属语义并补对应资格证据。

injected 表示原工具明确接受；promptRequired 表示明确未消耗输入，保留完整草稿供用户选择普通下一轮；startedNewTurn 表示已消耗但启动了独立轮次，不重复排队，并停用该连接的主动推送。缺字段、未知枚举、超时、断线、回执写盘失败均保留 unknown，不按同一输入自动重发，也不自动切换到已确认的主动检查反馈工具。

connection_written 仅表示实际写入原生连接；native_acknowledged 表示收到对应原生回执。两者均不表示模型已阅读或已完成工作。NativeInputDeliveryV1 单独记录预览、完整材料摘要、原生原始回执摘要和执行归属；既有 Runtime Input Delivery Evidence 仍只证明原来的投递，监控查看器显示额外记录，不修改旧 Manifest 来伪装它原本包含这份输入。

## 持久化与恢复

新增 Core 自有来源句柄、执行预览、原生启动意图、来源执行租约、原生插话策略、插话预览和输入投递账本。所有预览保留完整材料的受管 Blob 引用及固定摘要；大图片与原生凭据不进入命令回显、诊断普通文本或浏览器请求恢复日志。修改/移除 Runtime、关闭 Thread 或撤销本地来源都会结束新准入，已有记录不删除。

继续和分叉登记的原始导入快照始终只读；真实原生继续可能写入原工具自己的日志，这是新的执行结果，不能回写导入快照。恢复加密备份后先核对当前主机的实际来源、程序、安装与工程；没有证明则显示环境待补，不用备份中的绝对路径绕过本机选择和授予。

新版本轴为 NativeSourceReferenceV1、NativeSessionOriginV1、NativeForkPointV1、FrozenNativeSteerV1、NativeInputDeliveryV1。Bootstrap5、formatter5、charter22、direct29、public34、delegation35、Builtin39、IPC3、Envelope2、Output10、MCPProjection2 不变。SQLite 采用追加表/列迁移；不清库，不改旧消息、旧输入、秘密版本或已冻结的自定义安装锁。

## API 完整输入与字段语义

下附候选 JSON 中的全部输入 shape。object 都拒绝未知字段；列出的 required 字段全部必须提供；nullable 仅允许明确列出的 null，不允许省略。唯一省略字段是旧 FrozenAgentRuntimeConfig 中本次新增的两个 Option；None 时完全不序列化。输出对象同样闭合，实际响应只能符合下面的完整定义，不能以“其他元数据”扩展模型输入。

history.execution.preview 仅生成来源/接口/目标预览；confirm 固定原 UUID 执行一次；receipt 只读原结果。runtime.steer.policy.get/set 使用独立默认关闭策略，set 要求原 revision 和命令 UUID。targets 只返回属于指定作用域的活动目标；preview 冻结完整输入；confirm 只引用原预览；receipt/previews 只读原编号或分页记录。

NativeSourceReference 中 importId/importRevision 指本 Core 已冻结导入，不是任意原生 id；nativeSessionId 必须由 Host 从来源解析并复核；sourceHandleId 是本机登记句柄，不是文件访问权。NativeForkPoint 的 messageOccurrence 是相同助手文本第几次出现，从1开始；point 在 continue/fork_tail 时必须为 null，在 fork_point 时必须完整提供。capabilityStatus 为 unavailable 或 requires_acp_program 时，不准执行 confirm。

NativeSteerInput.text 保留原文和空白；空白文本且没有材料时拒绝。materials 是用户明确选择并已冻结的 sourceId 与摘要；displayName 只用于显示，不用来读取文件。target 所有字段必须与实际运行同时匹配。expiresAt 按 Core 时间比较；客户端时间不延长10分钟有效期。

NativeInputDeliveryV1 是新增的独立投递账本，完整保留确认时的 input 引用、输入摘要和目标归属；nativePromptDigest 只在实际完成原生序列化后填写，对实际写出的原生 prompt 块规范 JSON 的 UTF-8 字节取摘要。nativeRequestBytes 记录该次原生请求的实际字节数；原生原始回执摘要按收到的完整回执字节计算。没有这些事实时对应字段为 null，不从预览推算已写入。createdAt/updatedAt 使用 Core UTC 时间；只读监控按原命令查此账本，不改旧投递证据。

## 变更前 FrozenAgentRuntimeConfig

```rust
pub struct FrozenAgentRuntimeConfig {
    // Historical environment references are ignored, including malformed ones.
    // The in-memory launch map never participates in Run or Fleet identity.
    #[serde(skip)]
    pub environment: Option<crate::member_environment::LaunchEnvironment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_api: Option<crate::runtime_custom_api::CustomApiSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camp_fast: Option<crate::camp_fast::FrozenThreadMemberFast>,
    pub adapter_kind: AdapterKind,
    pub installation_id: String,
    pub installation_generation: i64,
    pub search_environment_generation: i64,
    pub executable_path: String,
    pub auth_scope: String,
    pub reported_version: Option<String>,
    pub executable_fingerprint: String,
    pub capabilities: Vec<String>,
    pub protocol_version: String,
    pub model: ResolvedModelSelection,
    pub permissions: AdapterPermissionConfig,
    pub native_session_compatibility_key: Option<String>,
    pub binding_compatibility_digest: String,
    pub host_config_digest: String,
    pub config_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_runtime: Option<FrozenCustomRuntimeV1>,
}
```

## 变更前 ModelSelection

```rust
pub enum ModelSelection {
    RuntimeDefault,
    Explicit {
        #[serde(rename = "modelId")]
        model_id: String,
        #[serde(default)]
        options: Value,
        #[serde(default, rename = "dshSource", skip_serializing_if = "Option::is_none")]
        dsh_source: Option<DshModelSource>,
    },
}
```

## 变更前 ResolvedModelSelection

```rust
pub struct ResolvedModelSelection {
    pub source: String,
    pub model_id: String,
    pub options: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dsh_source: Option<DshModelSource>,
}
```

## 变更前 AdapterPermissionConfig

```rust
pub struct AdapterPermissionConfig {
    pub adapter_kind: AdapterKind,
    pub schema_version: i64,
    pub values: Value,
}
```

## 变更后 FrozenAgentRuntimeConfig

```rust
pub struct FrozenAgentRuntimeConfig {
    // Historical environment references are ignored, including malformed ones.
    // The in-memory launch map never participates in Run or Fleet identity.
    #[serde(skip)]
    pub environment: Option<crate::member_environment::LaunchEnvironment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_api: Option<crate::runtime_custom_api::CustomApiSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camp_fast: Option<crate::camp_fast::FrozenThreadMemberFast>,
    pub adapter_kind: AdapterKind,
    pub installation_id: String,
    pub installation_generation: i64,
    pub search_environment_generation: i64,
    pub executable_path: String,
    pub auth_scope: String,
    pub reported_version: Option<String>,
    pub executable_fingerprint: String,
    pub capabilities: Vec<String>,
    pub protocol_version: String,
    pub model: ResolvedModelSelection,
    pub permissions: AdapterPermissionConfig,
    pub native_session_compatibility_key: Option<String>,
    pub binding_compatibility_digest: String,
    pub host_config_digest: String,
    pub config_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_runtime: Option<FrozenCustomRuntimeV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_origin: Option<NativeSessionOriginV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_steer: Option<FrozenNativeSteerV1>,
}
```

## 变更后 ModelSelection

```rust
pub enum ModelSelection {
    RuntimeDefault,
    Explicit {
        #[serde(rename = "modelId")]
        model_id: String,
        #[serde(default)]
        options: Value,
        #[serde(default, rename = "dshSource", skip_serializing_if = "Option::is_none")]
        dsh_source: Option<DshModelSource>,
    },
}
```

## 变更后 ResolvedModelSelection

```rust
pub struct ResolvedModelSelection {
    pub source: String,
    pub model_id: String,
    pub options: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dsh_source: Option<DshModelSource>,
}
```

## 变更后 AdapterPermissionConfig

```rust
pub struct AdapterPermissionConfig {
    pub adapter_kind: AdapterKind,
    pub schema_version: i64,
    pub values: Value,
}
```

## 变更后 NativeSessionOriginV1

```rust
pub struct NativeSessionOriginV1 {
    pub schema_version: u32,
    pub origin_id: String,
    pub command_id: String,
    pub source: NativeSourceReferenceV1,
    pub mode: NativeContinuationMode,
    pub point: Option<NativeForkPointV1>,
    pub workspace_digest: String,
}
```

## 变更后 FrozenNativeSteerV1

```rust
pub struct FrozenNativeSteerV1 {
    pub schema_version: u32,
    pub policy_revision: i64,
    pub enabled: bool,
}
```

## 接口 history.execution.preview

```text
 object additionalProperties=false required=schemaVersion,sourceThreadId,importId,expectedImportRevision,agentId,expectedAgentVersion,mode,point,title
.schemaVersion {"const":1,"type":"integer"}
.sourceThreadId {"type":"string","minLength":1,"maxLength":128}
.importId {"type":"string","minLength":64,"maxLength":64,"pattern":"^[0-9a-f]{64}$"}
.expectedImportRevision {"type":"string","minLength":64,"maxLength":64,"pattern":"^[0-9a-f]{64}$"}
.agentId {"type":"string","minLength":1,"maxLength":128}
.expectedAgentVersion {"type":"integer","minimum":1,"maximum":9007199254740991}
.mode {"type":"string","enum":["continue","fork_tail","fork_point"]}
.point oneOf 非空shape 或 null
.point schema=NativeForkPointV1 完整字段见同名shape
.title {"type":"string","minLength":0,"maxLength":1024}
```

## 接口 history.execution.confirm

```text
 object additionalProperties=false required=schemaVersion,commandId,previewId,previewDigest
.schemaVersion {"const":1,"type":"integer"}
.commandId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.previewId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.previewDigest {"type":"string","minLength":71,"maxLength":71,"pattern":"^sha256:[0-9a-f]{64}$"}
```

## 接口 history.execution.receipt

```text
 object additionalProperties=false required=schemaVersion,commandId
.schemaVersion {"const":1,"type":"integer"}
.commandId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
```

## 接口 runtime.steer.policy.get

```text
 object additionalProperties=false required=
```

## 接口 runtime.steer.policy.set

```text
 object additionalProperties=false required=schemaVersion,commandId,expectedRevision,members
.schemaVersion {"const":1,"type":"integer"}
.commandId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.expectedRevision {"type":"integer","minimum":0,"maximum":9007199254740991}
.members array maxItems=4096
.members[] object additionalProperties=false required=agentId,enabled
.members[].agentId {"type":"string","minLength":1,"maxLength":128}
.members[].enabled {"type":"boolean"}
```

## 接口 runtime.steer.targets

```text
 object additionalProperties=false required=schemaVersion,scope
.schemaVersion {"const":1,"type":"integer"}
.scope schema=NativeSteerScopeV1 完整字段见同名shape
```

## 接口 runtime.steer.preview

```text
 object additionalProperties=false required=schemaVersion,target,input
.schemaVersion {"const":1,"type":"integer"}
.target schema=NativeSteerTargetV1 完整字段见同名shape
.input schema=NativeSteerInputV1 完整字段见同名shape
```

## 接口 runtime.steer.confirm

```text
 object additionalProperties=false required=schemaVersion,commandId,previewId,previewDigest
.schemaVersion {"const":1,"type":"integer"}
.commandId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.previewId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.previewDigest {"type":"string","minLength":71,"maxLength":71,"pattern":"^sha256:[0-9a-f]{64}$"}
```

## 接口 runtime.steer.receipt

```text
 object additionalProperties=false required=schemaVersion,commandId
.schemaVersion {"const":1,"type":"integer"}
.commandId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
```

## 接口 runtime.steer.previews

```text
 object additionalProperties=false required=schemaVersion,scope,after,limit
.schemaVersion {"const":1,"type":"integer"}
.scope schema=NativeSteerScopeV1 完整字段见同名shape
.after oneOf 非空shape 或 null
.after {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.limit {"type":"integer","minimum":1,"maximum":50}
```

## 完整输出 NativeSourceReferenceV1

```text
 object additionalProperties=false required=schemaVersion,importId,importRevision,provider,nativeSessionId,sourceHandleId,sourceRevision
.schemaVersion {"const":1,"type":"integer"}
.importId {"type":"string","minLength":64,"maxLength":64,"pattern":"^[0-9a-f]{64}$"}
.importRevision {"type":"string","minLength":64,"maxLength":64,"pattern":"^[0-9a-f]{64}$"}
.provider {"type":"string","enum":["codex","claude","qoder","grok","opencode","gemini","openclaw","cline","hermes","codebuddy","kimi_code","pi","cursor","deepseek","antigravity"]}
.nativeSessionId {"type":"string","minLength":1,"maxLength":4096}
.sourceHandleId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.sourceRevision {"type":"string","minLength":71,"maxLength":71,"pattern":"^sha256:[0-9a-f]{64}$"}
```

## 完整输出 NativeForkPointV1

```text
 object additionalProperties=false required=selectedTurnId,messageId,messageFingerprint,messageOccurrence
.selectedTurnId {"type":"string","minLength":1,"maxLength":4096}
.messageId {"type":"string","minLength":1,"maxLength":4096}
.messageFingerprint oneOf 非空shape 或 null
.messageFingerprint {"type":"string","minLength":71,"maxLength":71,"pattern":"^sha256:[0-9a-f]{64}$"}
.messageOccurrence oneOf 非空shape 或 null
.messageOccurrence {"type":"integer","minimum":1,"maximum":4294967295}
```

## 完整输出 NativeSessionOriginV1

```text
 object additionalProperties=false required=schemaVersion,originId,commandId,source,mode,point,workspaceDigest
.schemaVersion {"const":1,"type":"integer"}
.originId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.commandId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.source schema=NativeSourceReferenceV1 完整字段见同名shape
.mode {"type":"string","enum":["continue","fork_tail","fork_point"]}
.point oneOf 非空shape 或 null
.point schema=NativeForkPointV1 完整字段见同名shape
.workspaceDigest {"type":"string","minLength":71,"maxLength":71,"pattern":"^sha256:[0-9a-f]{64}$"}
```

## 完整输出 FrozenNativeSteerV1

```text
 object additionalProperties=false required=schemaVersion,policyRevision,enabled
.schemaVersion {"const":1,"type":"integer"}
.policyRevision {"type":"integer","minimum":1,"maximum":9007199254740991}
.enabled {"const":true,"type":"boolean"}
```

## 完整输出 NativeContinuationPreviewV1

```text
 object additionalProperties=false required=schemaVersion,previewId,previewDigest,expiresAt,source,mode,point,sourceThreadId,newThreadId,newConversationId,agentId,expectedAgentVersion,runtimeConfigDigest,workspaceDigest,title,historyMessageCount,capabilityStatus,reason
.schemaVersion {"const":1,"type":"integer"}
.previewId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.previewDigest {"type":"string","minLength":71,"maxLength":71,"pattern":"^sha256:[0-9a-f]{64}$"}
.expiresAt {"type":"string","minLength":1,"maxLength":64}
.source schema=NativeSourceReferenceV1 完整字段见同名shape
.mode {"type":"string","enum":["continue","fork_tail","fork_point"]}
.point oneOf 非空shape 或 null
.point schema=NativeForkPointV1 完整字段见同名shape
.sourceThreadId {"type":"string","minLength":1,"maxLength":128}
.newThreadId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.newConversationId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.agentId {"type":"string","minLength":1,"maxLength":128}
.expectedAgentVersion {"type":"integer","minimum":1,"maximum":9007199254740991}
.runtimeConfigDigest {"type":"string","minLength":64,"maxLength":64,"pattern":"^[0-9a-f]{64}$"}
.workspaceDigest {"type":"string","minLength":71,"maxLength":71,"pattern":"^sha256:[0-9a-f]{64}$"}
.title {"type":"string","minLength":1,"maxLength":1024}
.historyMessageCount {"type":"integer","minimum":0,"maximum":9007199254740991}
.capabilityStatus {"type":"string","enum":["available","requires_acp_program","unavailable"]}
.reason oneOf 非空shape 或 null
.reason {"type":"string","minLength":1,"maxLength":1024}
```

## 完整输出 NativeContinuationConfirmV1

```text
 object additionalProperties=false required=schemaVersion,commandId,previewId,previewDigest
.schemaVersion {"const":1,"type":"integer"}
.commandId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.previewId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.previewDigest {"type":"string","minLength":71,"maxLength":71,"pattern":"^sha256:[0-9a-f]{64}$"}
```

## 完整输出 NativeContinuationReceiptV1

```text
 object additionalProperties=false required=schemaVersion,commandId,state,originId,threadId,conversationId,reportedSessionId,error
.schemaVersion {"const":1,"type":"integer"}
.commandId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.state {"type":"string","enum":["prepared","starting","binding_recorded","failed","unknown"]}
.originId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.threadId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.conversationId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.reportedSessionId oneOf 非空shape 或 null
.reportedSessionId {"type":"string","minLength":1,"maxLength":4096}
.error oneOf 非空shape 或 null
.error {"type":"string","minLength":1,"maxLength":2048}
```

## 完整输出 NativeSteerScopeV1

```text
 object additionalProperties=false required=threadId,conversationId,scopeKind
.threadId {"type":"string","minLength":1,"maxLength":128}
.conversationId oneOf 非空shape 或 null
.conversationId {"type":"string","minLength":1,"maxLength":128}
.scopeKind {"type":"string","enum":["public","single_chat","delegation_child"]}
```

## 完整输出 NativeSteerTargetV1

```text
 object additionalProperties=false required=scope,agentId,agentRunId,executionEpoch,nativeBindingId,nativeBindingGeneration,bindingCompatibilityDigest,activeTurnId
.scope schema=NativeSteerScopeV1 完整字段见同名shape
.agentId {"type":"string","minLength":1,"maxLength":128}
.agentRunId {"type":"string","minLength":1,"maxLength":128}
.executionEpoch {"type":"integer","minimum":1,"maximum":9007199254740991}
.nativeBindingId {"type":"string","minLength":1,"maxLength":128}
.nativeBindingGeneration {"type":"integer","minimum":1,"maximum":9007199254740991}
.bindingCompatibilityDigest {"type":"string","minLength":64,"maxLength":64,"pattern":"^[0-9a-f]{64}$"}
.activeTurnId {"type":"string","minLength":1,"maxLength":4096}
```

## 完整输出 NativeSteerMaterialV1

```text
 object additionalProperties=false required=sourceId,snapshotDigest,displayName,kind,byteSize
.sourceId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.snapshotDigest {"type":"string","minLength":71,"maxLength":71,"pattern":"^sha256:[0-9a-f]{64}$"}
.displayName {"type":"string","minLength":1,"maxLength":512}
.kind {"type":"string","enum":["image","file","text_reference"]}
.byteSize {"type":"integer","minimum":0,"maximum":20971520}
```

## 完整输出 NativeSteerInputV1

```text
 object additionalProperties=false required=text,materials
.text {"type":"string","minLength":0,"maxLength":4096}
.materials array maxItems=10
.materials[] schema=NativeSteerMaterialV1 完整字段见同名shape
```

## 完整输出 NativeSteerPreviewV1

```text
 object additionalProperties=false required=schemaVersion,previewId,previewDigest,expiresAt,target,input,inputDigest,capabilityDigest
.schemaVersion {"const":1,"type":"integer"}
.previewId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.previewDigest {"type":"string","minLength":71,"maxLength":71,"pattern":"^sha256:[0-9a-f]{64}$"}
.expiresAt {"type":"string","minLength":1,"maxLength":64}
.target schema=NativeSteerTargetV1 完整字段见同名shape
.input schema=NativeSteerInputV1 完整字段见同名shape
.inputDigest {"type":"string","minLength":71,"maxLength":71,"pattern":"^sha256:[0-9a-f]{64}$"}
.capabilityDigest {"type":"string","minLength":71,"maxLength":71,"pattern":"^sha256:[0-9a-f]{64}$"}
```

## 完整输出 NativeSteerReceiptV1

```text
 object additionalProperties=false required=schemaVersion,commandId,previewId,target,inputDigest,status,nativeOutcome,nativeReceiptDigest,error
.schemaVersion {"const":1,"type":"integer"}
.commandId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.previewId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.target schema=NativeSteerTargetV1 完整字段见同名shape
.inputDigest {"type":"string","minLength":71,"maxLength":71,"pattern":"^sha256:[0-9a-f]{64}$"}
.status {"type":"string","enum":["prepared","connection_written","native_acknowledged","not_consumed","requires_user_review","unknown"]}
.nativeOutcome oneOf 非空shape 或 null
.nativeOutcome {"type":"string","enum":["injected","promptRequired","startedNewTurn"]}
.nativeReceiptDigest oneOf 非空shape 或 null
.nativeReceiptDigest {"type":"string","minLength":71,"maxLength":71,"pattern":"^sha256:[0-9a-f]{64}$"}
.error oneOf 非空shape 或 null
.error {"type":"string","minLength":1,"maxLength":2048}
```

## 完整输出 NativeInputDeliveryV1

```text
 object additionalProperties=false required=schemaVersion,deliveryId,commandId,previewId,previewDigest,target,input,inputDigest,nativePromptDigest,nativeRequestBytes,status,nativeOutcome,nativeReceiptDigest,createdAt,updatedAt,error
.schemaVersion {"const":1,"type":"integer"}
.deliveryId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.commandId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.previewId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.previewDigest {"type":"string","minLength":71,"maxLength":71,"pattern":"^sha256:[0-9a-f]{64}$"}
.target schema=NativeSteerTargetV1 完整字段见同名shape
.input schema=NativeSteerInputV1 完整字段见同名shape
.inputDigest {"type":"string","minLength":71,"maxLength":71,"pattern":"^sha256:[0-9a-f]{64}$"}
.nativePromptDigest oneOf 非空shape 或 null
.nativePromptDigest {"type":"string","minLength":71,"maxLength":71,"pattern":"^sha256:[0-9a-f]{64}$"}
.nativeRequestBytes oneOf 非空shape 或 null
.nativeRequestBytes {"type":"integer","minimum":0,"maximum":67108864}
.status {"type":"string","enum":["prepared","connection_written","native_acknowledged","not_consumed","requires_user_review","unknown"]}
.nativeOutcome oneOf 非空shape 或 null
.nativeOutcome {"type":"string","enum":["injected","promptRequired","startedNewTurn"]}
.nativeReceiptDigest oneOf 非空shape 或 null
.nativeReceiptDigest {"type":"string","minLength":71,"maxLength":71,"pattern":"^sha256:[0-9a-f]{64}$"}
.createdAt {"type":"string","minLength":1,"maxLength":64}
.updatedAt {"type":"string","minLength":1,"maxLength":64}
.error oneOf 非空shape 或 null
.error {"type":"string","minLength":1,"maxLength":2048}
```

## 完整输出 NativeSteerPolicyV1

```text
 object additionalProperties=false required=schemaVersion,revision,members
.schemaVersion {"const":1,"type":"integer"}
.revision {"type":"integer","minimum":0,"maximum":9007199254740991}
.members array maxItems=4096
.members[] object additionalProperties=false required=agentId,enabled
.members[].agentId {"type":"string","minLength":1,"maxLength":128}
.members[].enabled {"type":"boolean"}
```

## 完整输出 NativeSteerPolicySetV1

```text
 object additionalProperties=false required=schemaVersion,commandId,expectedRevision,members
.schemaVersion {"const":1,"type":"integer"}
.commandId {"type":"string","minLength":36,"maxLength":36,"pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"}
.expectedRevision {"type":"integer","minimum":0,"maximum":9007199254740991}
.members array maxItems=4096
.members[] object additionalProperties=false required=agentId,enabled
.members[].agentId {"type":"string","minLength":1,"maxLength":128}
.members[].enabled {"type":"boolean"}
```

# 待执行验证

以下36项均为计划，尚未写入或运行测试；在全部一期源码写齐和统一Review修复后执行。需要真实模型身份或额度的项目单独记录，不以模拟结果代替。

NC01 触发 旧内置任务无新字段。通过判据 同一固定输入、配置、Bootstrap、Context、Manifest与摘要逐字节不变。状态 待执行。

NC02 触发 关闭原生插话。通过判据 旧任务与新任务均不因关闭状态生成空nativeSteer对象。状态 待执行。

NC03 触发 继续真实来源。通过判据 核对导入修订、当前源文件、Provider原生id和实际恢复回执。状态 待执行。

NC04 触发 来源路径含恶意字符串。通过判据 不把导入日志中的路径当授权或执行参数。状态 待执行。

NC05 触发 旧导入缺少来源句柄。通过判据 要求重新选择来源并核对，不拿快照冒充可运行。状态 待执行。

NC06 触发 源文件在预览后变化。通过判据 拒绝旧预览并保留原快照。状态 待执行。

NC07 触发 工程与来源不一致。通过判据 拒绝启动，不自动切工程或读取另一目录。状态 待执行。

NC08 触发 目标成员版本变化。通过判据 拒绝原确认，不覆盖新配置。状态 待执行。

NC09 触发 新工作会话隔离。通过判据 只创建目标成员的新Thread和私聊，不替换已有卡片。状态 待执行。

NC10 触发 同源会话并发继续。通过判据 Core租约冲突可读，不建立两个独占绑定。状态 待执行。

NC11 触发 外部应用修改历史。通过判据 发现变化时保留状态并要求重新核对。状态 待执行。

NC12 触发 分叉到重复助手文字。通过判据 使用助手指纹和从1开始的重复序号定位。状态 待执行。

NC13 触发 用户文字与助手文字相同。通过判据 不把用户轮次计入助手重复序号。状态 待执行。

NC14 触发 没有文本或有效原生id。通过判据 不把不可定位轮次按精确分叉执行。状态 待执行。

NC15 触发 仅支持尾部分叉。通过判据 要求明确选择尾部动作，不悄悄回退。状态 待执行。

NC16 触发 分叉与Git独立。通过判据 工程工作树和Git分支不因新会话自动改变。状态 待执行。

NC17 触发 已创建原生会话后断线。通过判据 读取原意图与外部已知编号，不重复分叉。状态 待执行。

NC18 触发 原生程序不支持恢复。通过判据 清晰显示原因，不把导入JSON转成普通提示词。状态 待执行。

NC19 触发 ACP广告或版本缺失。通过判据 原生插话不可用，既有主动检查反馈仍独立工作。状态 待执行。

NC20 触发 自定义程序冒充同名Provider。通过判据 只按实际程序来源和协议资格核对，不按UI名称授权。状态 待执行。

NC21 触发 文字带图片插话。通过判据 预览、实际原生块及回执摘要覆盖同一完整输入。状态 待执行。

NC22 触发 另一输入框存在材料。通过判据 不夹带其他卡片、私聊或队员材料。状态 待执行。

NC23 触发 材料sourceId或摘要更换。通过判据 写出前拒绝，不按显示名或路径重读。状态 待执行。

NC24 触发 同轮次32MiB并发预留。通过判据 事务保留总额度，未知状态不释放造成越界。状态 待执行。

NC25 触发 4096字符和空输入边界。通过判据 按Unicode字符计算，原文空白不改写，无材料空输入拒绝。状态 待执行。

NC26 触发 任务在读取图片时结束。通过判据 再次核对活动轮次，不送给新一轮。状态 待执行。

NC27 触发 injected真实回执。通过判据 记录native_acknowledged，不声称模型已读或已完成。状态 待执行。

NC28 触发 promptRequired真实回执。通过判据 完整材料保持，用户选择普通消息后才走原流程。状态 待执行。

NC29 触发 startedNewTurn真实回执。通过判据 已消耗的输入不重发，停用本连接主动推送。状态 待执行。

NC30 触发 缺少或未知原生回执。通过判据 保留unknown，不自动回退到其他通道。状态 待执行。

NC31 触发 同UUID重试及改变载荷。通过判据 原结果幂等；改变预览、目标或摘要拒绝。状态 待执行。

NC32 触发 停止接管撤销换成员。通过判据 原预览不能重定向到新Run、epoch或Binding。状态 待执行。

NC33 触发 预览10分钟与重连。通过判据 Core时间过期拒绝；原回执仍可只读核对。状态 待执行。

NC34 触发 跨平台及备份恢复。通过判据 编译与运行资格分开记录，不凭旧路径恢复权限。状态 待执行。

NC35 触发 DesktopWeb双原生页面。通过判据 来源/位置/完整输入/未知状态和原请求操作一致。状态 待执行。

NC36 触发 首次消息和付费边界。通过判据 确认来源本身不隐式发送模型任务，真实付费验证另获额度。状态 待执行。

# 确认与执行边界

revision 1；confirmation_status pending；confirmed_revision null。这份说明只属于独立原生继续分叉插话改动，不能借用任何其他r1的确认。确认后按七步执行规则继续生产源码，小功能中文commit，完整步骤结束才push。未确认时只调查和编辑方案，不改受保护实现、Schema、当前合同或执行clean break。


## 明确不变

本次仅整理既有 r1 确认记录与章节索引。正文中既有模型输入、字段、省略规则、权限、冻结、恢复及版本轴约定全部保留；此次不新增模型工具或改变输入语义。完整不变范围仍以本页已确认正文及原冻结附件为准。

## 二次确认

当前会话用户阅读对应客户版后明确回复：“确认，按原生继续分叉插话 r1 实施”。原确认时间 2026-10-22T09:53:51.621558+00:00，Front Matter 只去除秒后的小数以兼容既有文档校验器，保留同一确认时刻到秒。revision 与 confirmed_revision 继续为 1，不重新代用户确认。投递时的 pending 快照保留，后续用户确认以本节及既有确认记录为准。

## 验证

既有正文的全部验证点继续有效。本轮整包编译、回归与跨平台结果由统一修复 PR 记录；未实际执行或缺少环境的项目继续标为待验证。编译通过、组件测试通过及最终真实用户验收分别记录，不将本节视为通过证明。
