---
document_type: model-context-change
version: v1.73
change_id: custom-acp-runtime
revision: 1
confirmed_revision: 1
confirmation_status: confirmed
confirmed_by: user
confirmed_at: 2026-10-22T07:21:57+00:00
authority: proposed-model-input-change-statement
implementation_status: proposal_only
last_updated: 2026-10-22
---

# 怎样添加你自己的智能体程序

Codeg 与 Rovai 第一期融合 客户确认版 r1 待确认

你以后可能想使用一个没有列在内置名单里的智能体程序。这份方案决定怎样把它加入融合工作台，并交给指定队员使用。原有二十一种内置程序继续保留。

## 场景一 你找到一个新的智能体

你在原版 Codeg 的注册表中找到一个程序，或手里已有它的安装说明。在设置里选择添加，我们沿用原版的注册表选择和手动填写方式，先列出名称、版本、支持的平台、启动参数以及它声明能用的技能和工具。

保存这张配置单不会替你启动任务。你选择安装时，系统才下载对应平台的包，核对实际包和版本，再登记本机的启动程序。下载失败或平台不适合，会停在该项并保留原因，其他内置程序继续可用。

你随后把这个程序选给甲队员。甲的身份、职责、记忆和工作关系仍属于 Rovai Core；程序只是甲开展工作的一个选择。乙队员继续使用原来的程序。

这份确认只批准下面列明的接入方式。它不包含使用你的付费模型额度、合入 main 或发布产品。

<!--PAGE-->

## 场景二 一个新程序怎样使用技能和工具

有些程序能读共享技能目录，有些需要你指定自己的技能目录；有些能接工具连接，有些不能。配置单会把这些声明写清楚，默认值沿用原版 Codeg，声明和实测状态分别显示。

你勾选能接工具，表示允许我们在该程序支持时传入已经选给队员的工具。它不代表检测已经通过。若程序拒绝这份连接，界面保留具体结果，不会换个程序、减少任务要求后声称兼容。

技能仍按你此前确认的技能包规则选择和冻结。浏览器、电脑、反馈和独立帮手继续分别遵守各自已经确认的规则。添加一个程序，不会自动共享你的桌面，也不会把其他私聊的材料交给它。

界面会区分已登记、已安装、已经握手、实际模型任务和真实工具运行。你可以按现有运行门禁选择试用，试用结果不能冒充全部兼容测试通过。工程测试仍由我负责，不会提前转给你验收。

<!--PAGE-->

## 场景三 做到一半你修改了程序设置

甲正在处理一个任务，你把这个程序的版本或参数改了。已经开始的任务保留原配置；后面的新任务才使用新的配置单。系统不会把一个进行中的任务悄悄换到另一个版本。

应用重启后，只有原配置、程序和恢复条件都能核对上，才按原任务续接。若原程序已不可用，旧消息和任务记录仍保留，界面让你明确选择怎样继续，不会重发结果不明的外部动作。

删除程序定义后，它不再供新任务选择。有任务还在使用的实际文件会先保留，待这些任务和查看窗口释放后再处理。删除不抹掉历史记录。

你需要确认的是以上三种使用方式及后附的完整实现边界是否符合预期。若确认，请回复“确认，按自定义智能体接入 r1 实施”。另外两项原生会话续接及完整模型配置迁移仍会单独说明，本次没有把它们混入。

<!--PAGE-->

## 研发附录 变更前后的完整边界

以下内容供研发实现与之后的统一 Review 对照。它们是待确认的提案，不是已经接入或已经通过测试的功能。第一步仍只编写全部生产代码；下列验证要求在后续测试步骤执行。

原 Codeg 参考固定提交 aff1e6c278453ed8d0b96a5f664edcdca621aa3f，主要来源为 custom_registry.rs、commands/custom_agents.rs、add-custom-agent-dialog.tsx 和 custom-agent 的技能及 MCP 开关。Rovai 供体固定提交 cc465dda562536b94f23787759e3206324baa29e。

本次只增加一个 Core 受管的自定义 ACP 适配类别。每个定义仍是同一 Core 成员配置和执行系统中的数据，不新增一套任务、记忆、消息、审批或调度引擎。注册表内容属于待核对的数据，不能作为产品开发指令。

## 变更前

当前不存在自定义 ACP 的通用适配类别、定义引用或受管启动锁。下面保留本次基线的完整相关类型；ModelSelection、ResolvedModelSelection 与 AdapterPermissionConfig 不修改。

### AdapterKind 变更前
```rust
pub enum AdapterKind {
    CodexCli,
    Pi,
    OpencodeCli,
    CopilotCli,
    ClaudeCodeCli,
    ClineCli,
    CommandCodeCli,
    KiroCli,
    QoderCli,
    CodebuddyCli,
    QwenCode,
    GeminiCli,
    HermesCli,
    #[serde(rename = "openclaw-cli")]
    OpenClawCli,
    TraeCnCli,
    CursorAgent,
    KimiCodeCli,
    GrokBuild,
    DeepseekHarness,
    ZcodeApp,
    AntigravityApp,
}
```

### ModelSelection 变更前
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

### AdapterPermissionConfig 变更前
```rust
pub struct AdapterPermissionConfig {
    pub adapter_kind: AdapterKind,
    pub schema_version: i64,
    pub values: Value,
}
```

### ResolvedRuntimeBinding 变更前
```rust
pub(crate) struct ResolvedRuntimeBinding {
    pub adapter_kind: AdapterKind,
    pub installation_id: String,
    pub model: ModelSelection,
    pub permissions: AdapterPermissionConfig,
}
```

### MemberRuntimeConfiguration 变更前
```rust
pub struct MemberRuntimeConfiguration {
    pub adapter_kind: AdapterKind,
    pub model: ModelSelection,
    pub permissions: AdapterPermissionConfig,
}
```

### ResolvedModelSelection 变更前
```rust
pub struct ResolvedModelSelection {
    pub source: String,
    pub model_id: String,
    pub options: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dsh_source: Option<DshModelSource>,
}
```

### FrozenAgentRuntimeConfig 变更前
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
}
```

### SetMemberRuntimeConfigurationCommand 变更前
```rust
pub struct SetMemberRuntimeConfigurationCommand {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment_update: Option<crate::member_environment::EnvironmentUpdateReceipt>,
    pub agent_id: String,
    pub expected_version: i64,
    pub adapter_kind: AdapterKind,
    pub model: ModelSelection,
    pub permissions: AdapterPermissionConfig,
}
```

## 变更后

AdapterKind 增加 serde 名称 custom-acp，对应的 ALL 列表从二十一项扩展为二十二项。三个配置类型只增加可省略的 customRuntimeRef；冻结运行类型只增加可省略的 customRuntime。内置程序的新字段均不出现，旧 JSON 继续按原方式读取。

customRuntimeRef 只准在 adapterKind 为 custom-acp 时存在，并且必须与 Core 定义的修订号及摘要一致；自定义程序缺引用或内置程序夹带引用均拒绝。引用不能由模型、技能或前端标签拼造。模型选择仍使用原 ModelSelection，权限仍使用原 AdapterPermissionConfig，不开放任意权限透传。

### AdapterKind 变更后
```rust
pub enum AdapterKind {
    CodexCli,
    Pi,
    OpencodeCli,
    CopilotCli,
    ClaudeCodeCli,
    ClineCli,
    CommandCodeCli,
    KiroCli,
    QoderCli,
    CodebuddyCli,
    QwenCode,
    GeminiCli,
    HermesCli,
    #[serde(rename = "openclaw-cli")]
    OpenClawCli,
    TraeCnCli,
    CursorAgent,
    KimiCodeCli,
    GrokBuild,
    DeepseekHarness,
    ZcodeApp,
    AntigravityApp,
    CustomAcp,
}
```

### ResolvedRuntimeBinding 变更后
```rust
pub(crate) struct ResolvedRuntimeBinding {
    pub adapter_kind: AdapterKind,
    pub installation_id: String,
    pub model: ModelSelection,
    pub permissions: AdapterPermissionConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_runtime_ref: Option<CustomRuntimeRefV1>,
}
```

### MemberRuntimeConfiguration 变更后
```rust
pub struct MemberRuntimeConfiguration {
    pub adapter_kind: AdapterKind,
    pub model: ModelSelection,
    pub permissions: AdapterPermissionConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_runtime_ref: Option<CustomRuntimeRefV1>,
}
```

### SetMemberRuntimeConfigurationCommand 变更后
```rust
pub struct SetMemberRuntimeConfigurationCommand {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment_update: Option<crate::member_environment::EnvironmentUpdateReceipt>,
    pub agent_id: String,
    pub expected_version: i64,
    pub adapter_kind: AdapterKind,
    pub model: ModelSelection,
    pub permissions: AdapterPermissionConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_runtime_ref: Option<CustomRuntimeRefV1>,
}
```

### FrozenAgentRuntimeConfig 变更后
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

### CustomRuntimeRefV1 变更后
```rust
pub struct CustomRuntimeRefV1 {
    pub id: String,
    pub definition_revision: i64,
    pub definition_digest: String,
}
```

### FrozenCustomRuntimeV1 变更后
```rust
pub struct FrozenCustomRuntimeV1 {
    pub schema_version: u32,
    pub reference: CustomRuntimeRefV1,
    pub registry_id: String,
    pub source: String,
    pub distribution_kind: String,
    pub resolved_definition: CustomRuntimeDefinitionV1,
    pub launch_lock_digest: String,
    pub declared_mcp: bool,
    pub declared_skill_roots: Vec<String>,
    pub observed_capabilities_digest: Option<String>,
}
```

## 新定义的完整字段及省略规则

下面的结构是规范化输入和读取结果。输入中全部顶层字段均显式提供，nullable 字段可为 null；不接收未列字段。原 Codeg 定义导入时，先按本文缺省规则补齐并显示差异，再以新命令保存。读取结果另外包含 Core 生成的 revision、definitionDigest、createdAt 和 updatedAt。

所有字符串按 Unicode 标量计上限，JSON UTF8 总量另限二兆字节；全库最多四千零九十六个定义。环境名和值属于启动配置，不直接写入模型上下文、公开输出或诊断。摘要参与绑定，不替代秘密存储。

env 输入值可以是本次明确提交的字符串，或者同一定义、同一分发渠道和同一键的既有秘密引用 secretId 与 revision。Core 把新值移入现有私有秘密存储，读取结果、冻结定义及启动锁只保存引用。旧秘密引用内容不可原位改写；轮换产生新引用和修订，活动任务仍保留旧引用。删除键表示后续定义不再使用该变量，空字符串表示明确设置为空。跨定义、跨键或已失效引用一律拒绝。普通读取不提供取回明文的接口。

以下共享类型共同构成完整结构。类型名只是复用定义，所有定义均在本节展开；object closed 表示未列键拒绝，所有列出的字段均必填，nullable 用 null 表达。读取型环境字段只含秘密引用。

### SecretRef
```text
SecretRef = object closed required {
  secretId: {"type":"string","format":"uuid"}
  revision: {"type":"integer","minimum":1,"maximum":9007199254740991}
}
```

### Arguments
```text
Arguments = array 0..128 of {"type":"string","minLength":0,"maxLength":32768}
```

### EnvironmentInput
```text
EnvironmentInput = map max 256 keys {"pattern":"^[A-Za-z_][A-Za-z0-9_]*$"} values {"type":"string","minLength":0,"maxLength":32768} | SecretRef
```

### EnvironmentRead
```text
EnvironmentRead = map max 256 keys {"pattern":"^[A-Za-z_][A-Za-z0-9_]*$"} values SecretRef
```

### NpxInput
```text
NpxInput = object closed required {
  package: {"type":"string","minLength":1,"maxLength":512}
  args: Arguments
  env: EnvironmentInput
  cmd: {"type":"string","minLength":1,"maxLength":512} | {"type":"null"}
  nodeRequired: {"type":"string","minLength":1,"maxLength":512} | {"type":"null"}
}
```

### NpxRead
```text
NpxRead = object closed required {
  package: {"type":"string","minLength":1,"maxLength":512}
  args: Arguments
  env: EnvironmentRead
  cmd: {"type":"string","minLength":1,"maxLength":512} | {"type":"null"}
  nodeRequired: {"type":"string","minLength":1,"maxLength":512} | {"type":"null"}
}
```

### UvxInput
```text
UvxInput = object closed required {
  package: {"type":"string","minLength":1,"maxLength":512}
  args: Arguments
  env: EnvironmentInput
  cmd: {"type":"string","minLength":1,"maxLength":512} | {"type":"null"}
  uvRequired: {"type":"string","minLength":1,"maxLength":512} | {"type":"null"}
  python: {"type":"string","minLength":1,"maxLength":512} | {"type":"null"}
}
```

### UvxRead
```text
UvxRead = object closed required {
  package: {"type":"string","minLength":1,"maxLength":512}
  args: Arguments
  env: EnvironmentRead
  cmd: {"type":"string","minLength":1,"maxLength":512} | {"type":"null"}
  uvRequired: {"type":"string","minLength":1,"maxLength":512} | {"type":"null"}
  python: {"type":"string","minLength":1,"maxLength":512} | {"type":"null"}
}
```

### BinaryInput
```text
BinaryInput = object closed required {
  archive: {"type":"string","format":"uri","maxLength":8192}
  cmd: {"type":"string","minLength":1,"maxLength":32768}
  args: Arguments
  env: EnvironmentInput
  sha256: {"type":"string","pattern":"^[a-fA-F0-9]{64}$"} | {"type":"null"}
}
```

### BinaryRead
```text
BinaryRead = object closed required {
  archive: {"type":"string","format":"uri","maxLength":8192}
  cmd: {"type":"string","minLength":1,"maxLength":32768}
  args: Arguments
  env: EnvironmentRead
  sha256: {"type":"string","pattern":"^[a-fA-F0-9]{64}$"} | {"type":"null"}
}
```

### DistributionInput
```text
DistributionInput = object closed required {
  npx: NpxInput | {"type":"null"}
  uvx: UvxInput | {"type":"null"}
  binary: map max 12 keys {"enum":["darwin-aarch64","darwin-x86_64","linux-aarch64","linux-x86_64","windows-aarch64","windows-x86_64","aarch64-apple-darwin","x86_64-apple-darwin","aarch64-unknown-linux-gnu","x86_64-unknown-linux-gnu","aarch64-pc-windows-msvc","x86_64-pc-windows-msvc"]} values BinaryInput
}
```

### DistributionRead
```text
DistributionRead = object closed required {
  npx: NpxRead | {"type":"null"}
  uvx: UvxRead | {"type":"null"}
  binary: map max 12 keys {"enum":["darwin-aarch64","darwin-x86_64","linux-aarch64","linux-x86_64","windows-aarch64","windows-x86_64","aarch64-apple-darwin","x86_64-apple-darwin","aarch64-unknown-linux-gnu","x86_64-unknown-linux-gnu","aarch64-pc-windows-msvc","x86_64-pc-windows-msvc"]} values BinaryRead
}
```

### DefinitionInput
```text
DefinitionInput = object closed required {
  schemaVersion: const 1
  id: {"type":"string","format":"uuid"}
  registryId: {"type":"string","pattern":"^[a-z0-9][a-z0-9._-]{0,63}$"}
  name: {"type":"string","minLength":1,"maxLength":80}
  description: {"type":"string","minLength":0,"maxLength":4000}
  version: {"type":"string","minLength":1,"maxLength":128}
  distributionKind: enum ["npx","uvx","binary"]
  spec: DistributionInput
  iconUrl: {"type":"string","minLength":1,"maxLength":262144} | {"type":"null"}
  skillsSharedStore: {"type":"boolean"}
  skillsDir: {"type":"string","minLength":1,"maxLength":32768} | {"type":"null"}
  source: enum ["registry","manual"]
  sourceRef: {"type":"string","minLength":1,"maxLength":8192} | {"type":"null"}
  versionProbe: {"type":"string","minLength":1,"maxLength":32768} | {"type":"null"}
  supportsMcp: {"type":"boolean"}
  enabled: {"type":"boolean"}
}
```

### DefinitionView
```text
DefinitionView = object closed required {
  schemaVersion: const 1
  id: {"type":"string","format":"uuid"}
  registryId: {"type":"string","pattern":"^[a-z0-9][a-z0-9._-]{0,63}$"}
  name: {"type":"string","minLength":1,"maxLength":80}
  description: {"type":"string","minLength":0,"maxLength":4000}
  version: {"type":"string","minLength":1,"maxLength":128}
  distributionKind: enum ["npx","uvx","binary"]
  spec: DistributionRead
  iconUrl: {"type":"string","minLength":1,"maxLength":262144} | {"type":"null"}
  skillsSharedStore: {"type":"boolean"}
  skillsDir: {"type":"string","minLength":1,"maxLength":32768} | {"type":"null"}
  source: enum ["registry","manual"]
  sourceRef: {"type":"string","minLength":1,"maxLength":8192} | {"type":"null"}
  versionProbe: {"type":"string","minLength":1,"maxLength":32768} | {"type":"null"}
  supportsMcp: {"type":"boolean"}
  enabled: {"type":"boolean"}
  revision: {"type":"integer","minimum":1,"maximum":9007199254740991}
  definitionDigest: {"type":"string","pattern":"^sha256:[a-f0-9]{64}$"}
  createdAt: {"type":"string","format":"date-time"}
  updatedAt: {"type":"string","format":"date-time"}
}
```

### CustomRuntimeRef
```text
CustomRuntimeRef = object closed required {
  id: {"type":"string","format":"uuid"}
  definitionRevision: {"type":"integer","minimum":1,"maximum":9007199254740991}
  definitionDigest: {"type":"string","pattern":"^sha256:[a-f0-9]{64}$"}
}
```

### LaunchLock
```text
LaunchLock = object closed required {
  schemaVersion: const 1
  definition: CustomRuntimeRef
  platform: enum ["windows-x64","windows-arm64","macos-x64","macos-arm64","linux-x64","linux-arm64"]
  executable: {"type":"string","minLength":1,"maxLength":32768}
  arguments: Arguments
  environmentDigest: {"type":"string","pattern":"^sha256:[a-f0-9]{64}$"}
  artifactDigest: {"type":"string","pattern":"^sha256:[a-f0-9]{64}$"}
  resolvedPackage: {"type":"string","minLength":1,"maxLength":512} | {"type":"null"}
  resolvedVersion: {"type":"string","minLength":1,"maxLength":512}
  skillRoots: array 0..2 of {"type":"string","minLength":1,"maxLength":32768}
  supportsMcpDeclared: {"type":"boolean"}
  protocolCapabilitiesDigest: {"type":"string","pattern":"^sha256:[a-f0-9]{64}$"} | {"type":"null"}
}
```

## 字段语义和原版兼容

registryId 是原版稳定标识，不能与内置类别混用。source 为 registry 或 manual；原版缺少 source 按 registry。description 缺省空字符串，skillsSharedStore 缺省 false，skillsDir、iconUrl、versionProbe 缺省 null，supportsMcp 缺省 true，args 缺省空数组，env 缺省空对象。原版已有字段全部保留，不把任一分发渠道丢掉；distributionKind 决定当前实际使用哪个渠道。

spec 中 npx、uvx 可为 null，binary 为平台表；所选渠道必须有完整条目。平台别名只映射到上面列明的六种 Host 平台。无当前平台条目时拒绝安装，不能猜用其他平台。注册表图标先按原版下载并验证媒体类型，离线显示；图标不能成为程序入口。

npx 和 uvx 在实际启动前必须解析为精确包版本和本地受管入口，并把锁文件及产物摘要写入 launchLockDigest。用户明确安装才允许联网；版本变化使用新修订。binary 的 archive 只准 HTTPS 下载，先验证可用摘要，拒绝归档路径穿越、软链接、硬链接和解压超额；未提供上游摘要时显示该事实并固定本次实际产物摘要，不声称上游已签名。

cmd 是包内启动入口，不能逃出安装目录；参数按数组原样传给既有受管进程入口，不拼成额外 shell 命令。versionProbe 沿原版版本探测用途保留，解析为可执行入口与参数后调用，不能在版本探测阶段执行任意 shell 字符串。环境变量仍经过既有安装、权限和敏感值处理。

skillsDir 在保存时展开本机家目录并要求绝对路径。原版声明的共享库和专用库顺序保留，专用库优先。模型仍只接收本队员选择且按已确认技能包规则冻结的材料。路径本身不证明拥有跨项目、跨队员或本机控制权限。其他平台的目录选择和运行资格分别留存。

supportsMcp 是声明，协议 initialize 和 session 能力返回才是观察值。新适配器传入的 MCP 仍由 Core 投影选定清单，未选中项不转发。兼容性不明时保留候选和具体失败，不能凭空报告支持。自定义程序的实际操作能力沿当前平台和权限门禁核对；工作目录限制不能被描述为操作系统沙箱。

## 操作目录与事务

下面是新增 User 操作的完整输入。它们不增加模型可见工具名；新写操作使用原 Domain Command Gateway 的一次事务及精确请求重放。只有已经认证的 User 可以管理定义。模型和远端卡片不能借这个接口注册程序。

### runtime.custom.list
```text
input object closed required {
  after: {"type":"string","format":"uuid"} | {"type":"null"}
  limit: {"type":"integer","minimum":1,"maximum":100}
}
result object closed required {
  items: array 0..100 of DefinitionView
  next: {"type":"string","format":"uuid"} | {"type":"null"}
}
```

### runtime.custom.upsert
```text
input object closed required {
  commandId: {"type":"string","format":"uuid"}
  expectedRevision: {"type":"integer","minimum":0,"maximum":9007199254740991}
  definition: DefinitionInput
}
result existing StoredCommandResult; resultEntity custom_runtime_definition; payload {id, revision, definitionDigest}; rejected also persisted
```

### runtime.custom.remove
```text
input object closed required {
  commandId: {"type":"string","format":"uuid"}
  id: {"type":"string","format":"uuid"}
  expectedRevision: {"type":"integer","minimum":0,"maximum":9007199254740991}
}
result existing StoredCommandResult; payload {id, revision, removed:true}; active leases keep immutable referenced artifacts
```

### runtime.custom.install
```text
input object closed required {
  commandId: {"type":"string","format":"uuid"}
  reference: CustomRuntimeRef
  platform: enum ["windows-x64","windows-arm64","macos-x64","macos-arm64","linux-x64","linux-arm64"]
}
result existing managed-installation receipt; definition changes require a new command; artifacts are pinned before launch
```

### runtime.custom.inspect
```text
input object closed required {
  reference: CustomRuntimeRef
}
result existing runtime installation/probe snapshot, with full definition reference and launch lock digest; no paid model invoked
```

expectedRevision 为零表示创建，更新、移除和安装都必须指定已读修订。定义以注册标识和本地 UUID 唯一映射，不能覆盖另一份定义。下载或进程不在 SQLite 事务内执行；先保存受管安装意图，再由现有安装工作流推进。相同 UUID 的原请求返回首次结果，改变载荷或归属报冲突。中断后的外部状态不明时只核对原操作，不盲目重做。

移除只结束新选择并登记退休修订；活动 Run、Binding 和查看器租约引用的定义及产物保持不可变。清理需要没有活动引用，并采用现有受管安装文件规则；不递归删除外部技能目录、客户项目、源会话或用户凭据。

## 模型输入和版本轴

变更发生在新自定义绑定选择的运行类别、冻结运行来源和对应证据；运行类别显示为 custom-acp，私有冻结快照保留 customRuntime 完整来源，配置摘要绑定 definitionRevision、definitionDigest 和 launchLockDigest。环境明文、凭据和安装定义不作为一段提示词加入模型。

Bootstrap 的 SESSION_CHARTER、MEMBER_IDENTITY、Memory 入口全文不替换；Dynamic Context 的 section 名称、顺序、Current Input、历史选择、预算、截断和引用规则均沿用原值。ModelSelection、ResolvedModelSelection 和权限值类型原文已完整保留。新自定义适配器使用既有 ACP 运输，不另外包一层输入标记，不修改用户原文，不拼入任意注册表描述。

新增版本轴为 CustomRuntimeDefinitionV1、CustomRuntimeRefV1、FrozenCustomRuntimeV1 和 CustomAcpLaunchLockV1。Bootstrap v5、formatter 5、charter 22、direct formatter 与 manifest 29、public formatter 与 manifest 34、delegation context 35、Builtin 39、IPC 3、Envelope 2、Output 10 和 MCP Projection 2 均不改变。内置 Runtime 的配置和输入须通过字节级对照；没有新增字段时完全省略，不能序列化 null 或空自定义对象改变旧摘要。

新自定义类别在 User 配置 API 和既有运行类别字符串字段中出现；模型可见内置工具名、Schema 和完整结果 formatter 不调整。若实现发现必须修改它们，或必须改变上述输入 section 或版本轴，先修订本文并再次确认，不把当前确认扩大成未说明的输入改动。

## 数据迁移 失效和恢复

新增定义、修订、启动锁和受管安装关系的存储，保留旧二十一种 Runtime 记录和旧冻结字段；旧行读取新字段缺省为 None，不回算旧摘要、不批量改绑旧 NativeSession。数据库 schema 的实际迁移号在实施时按当时唯一顺序分配，并记录在本文实施结论，不能伪报已经迁移。

定义修订、程序产物、技能声明或 MCP 声明变化都使之后的新绑定按新配置核对。已经运行的责任使用原冻结快照；旧修订和产物有租约时保留。重启只有在相同快照和现有恢复门禁允许时续接；无法证明时保留历史，明确让 User 重建绑定。执行 epoch、审批、停止、预算和私聊隔离继续由 Core 控制。

恢复、定义更新、删除或导入都不自动发送消息、重放支付或改写外部项目。旧会话引用与原始磁盘记录不迁移到另一归属。跨平台目录无法沿用时显示具体待配置项，不把 Windows 的实际检查结果当作 Mac 或 Linux 资格。

## 研发顺序和待执行验证

取得本 r1 明确确认后，依次编写定义与修订存储、User 操作、受管安装映射、通用 ACP 适配及冻结快照、原版设置页、Web 和原生客户端适配。每个小功能中文 commit，全部第一期生产源码写齐才统一 Review；当前步骤不执行以下软件验证。

下列每项均需要之后编写或复用可执行用例并留存原始结果。表述为通过判据，全部状态均为计划，不能当作已通过。真实模型 Gate 沿仓库通用双轨流程冻结新旧输入、案例、通过标准、身份和预算；未提供身份及预算前不发起计费模型请求。

CA01 触发 内置21种配置未选择自定义。通过判据 模型输入字节及已冻结旧回执保持原样。状态 待执行。

CA02 触发 新定义保存后未启动。通过判据 没有新会话、Run、模型请求或下载。状态 待执行。

CA03 触发 从注册表导入缺省字段。通过判据 默认值固定为原版语义并显示归一化差异。状态 待执行。

CA04 触发 手动填写npx定义。通过判据 参数逐项保留，不经shell拼接。状态 待执行。

CA05 触发 uvx定义含解释器版本。通过判据 固定包、入口和解释器选择可复现。状态 待执行。

CA06 触发 二进制平台不匹配。通过判据 具体平台原因可见，不猜用别的平台。状态 待执行。

CA07 触发 压缩包校验不符。通过判据 不安装不启动，保留失败回执。状态 待执行。

CA08 触发 压缩包路径穿越或链接。通过判据 拒绝越界成员，不写用户原目录。状态 待执行。

CA09 触发 未知注册表键或分发类型。通过判据 预览或保存明确拒绝，不默默丢字段。状态 待执行。

CA10 触发 Windows命令入口与带空格参数。通过判据 复用既有受管入口和转义，不调用额外shell。状态 待执行。

CA11 触发 环境变量含密钥。通过判据 不进入模型文本、公开日志或客户端回显。状态 待执行。

CA12 触发 指定本机技能目录。通过判据 显示真实目标、冻结选择，不自动继承其他队员材料。状态 待执行。

CA13 触发 共享技能声明关闭。通过判据 不凭目录存在替队员开通技能。状态 待执行。

CA14 触发 MCP声明关闭。通过判据 新绑定不转发MCP，界面给出原因。状态 待执行。

CA15 触发 MCP声明开启但协议拒绝。通过判据 记录真实失败，不把勾选变成实测通过。状态 待执行。

CA16 触发 新会话成功的真实工具调用。通过判据 分别留存协议握手、模型调用、工具结果证据。状态 待执行。

CA17 触发 运行中更新定义。通过判据 旧Run继续使用原冻结定义；新任务才使用新引用。状态 待执行。

CA18 触发 停机后恢复旧任务。通过判据 原定义和锁可用才续接，否则保留历史并要求重建。状态 待执行。

CA19 触发 删除仍有活动租约的定义。通过判据 停止新选择，旧产物待租约释放才清理。状态 待执行。

CA20 触发 恢复与另一个定义版本冲突。通过判据 不把旧NativeSession接到新配置上。状态 待执行。

CA21 触发 同UUID重试相同写入。通过判据 返回首次回执，不重复写配置或下载。状态 待执行。

CA22 触发 同UUID换定义或归属。通过判据 明确冲突，不覆盖首次结果。状态 待执行。

CA23 触发 公开会话和两个私聊。通过判据 材料、绑定和反馈工具仍分别隔离。状态 待执行。

CA24 触发 独立帮手使用自定义程序。通过判据 沿已确认帮手边界，不读取父方其他私聊。状态 待执行。

CA25 触发 浏览器电脑未授权。通过判据 不因自定义定义获得控制权限。状态 待执行。

CA26 触发 前后台或Host断线。通过判据 原命令及状态保留，不自动重发外部动作。状态 待执行。

CA27 触发 Windows macOS Linux六目标。通过判据 编译与真实运行资格分列；未执行不报通过。状态 待执行。

CA28 触发 新旧真实模型对照Gate。通过判据 案例、预算、原始输入摘要和判据冻结；未提供身份额度不发请求。状态 待执行。

CA29 触发 引用另一程序或字段的环境秘密。通过判据 拒绝跨定义或字段引用，不凭知道UUID获得秘密。状态 待执行。

CA30 触发 读取定义和冻结运行来源。通过判据 只返回秘密引用与版本，不返回环境明文；轮换保留旧运行引用。状态 待执行。

## 当前记录和确认范围

基线提交 c5d17165a8d27065789d693ea557d928cd57e67e，分支 develop/V1.0.0。本文 revision 为 1，confirmed_revision、confirmed_by 和 confirmed_at 均为空；当前未确认。本提案自审只审查方案、字段对照及文档排版，不是提前开始软件统一 Review。

当前批准边界仅适用于这份自定义智能体接入 r1。原生会话继续、分叉、原生插话和完整模型配置迁移不包含在内。软件用户层验收仍在 I12 之后；方案审核不会变成客户逐项技术测试。main 合入、发布和付费模型调用均需原有授权条件满足后再执行。


## 明确不变

本次仅整理既有 r1 确认记录与章节索引。正文中既有模型输入、字段、省略规则、权限、冻结、恢复及版本轴约定全部保留；此次不新增模型工具或改变输入语义。完整不变范围仍以本页已确认正文及原冻结附件为准。

## 二次确认

当前会话用户阅读对应客户版后明确回复：“确认，按自定义智能体接入 r1 实施”。原确认时间 2026-10-22T07:21:57.433315+00:00，Front Matter 只去除秒后的小数以兼容既有文档校验器，保留同一确认时刻到秒。revision 与 confirmed_revision 继续为 1，不重新代用户确认。投递时的 pending 快照保留，后续用户确认以本节及既有确认记录为准。

## 验证

既有正文的全部验证点继续有效。本轮整包编译、回归与跨平台结果由统一修复 PR 记录；未实际执行或缺少环境的项目继续标为待验证。编译通过、组件测试通过及最终真实用户验收分别记录，不将本节视为通过证明。
