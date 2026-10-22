---
document_type: model-context-change
version: v1.73
change_id: portable-model-configuration
revision: 1
confirmed_revision: null
confirmation_status: pending
authority: proposed-model-input-change-statement
implementation_status: proposal_only
last_updated: 2026-10-22
---

# 怎样把原来的模型和设置搬进融合工作台

完整模型配置迁移 r1 客户确认版 待确认

我建议把原应用的配置完整保留，再让你决定哪些项目和队员采用它。配置搬进来以后，软件先展示结果，不启动任务、不替你安装程序，也不切换正在工作的模型。你需要审核的是下面三段使用方式；工程验证仍由我完成，完整一期交付后再由你实际使用验收。

## 场景一 把旧电脑里的设置带过来

你在旧 Codeg 中已经配置好模型地址、模型列表、智能体、常用指令和任务模板。进入融合工作台的“配置迁移”，选择原应用导出的配置，软件先列出能搬入的内容。服务商地址和模型名称可以直接核对，密钥只显示“已包含”或“需要重新填写”。

导出时默认不带密钥。你可以明确选择“在加密文件中包含模型密钥”，再设置口令。以后换机器导入时，软件把密钥放进新主机的加密存储，界面和运行记录仍只保存对应编号。原 Codeg 已导出的明文配置也可以读取，但融合工作台重新导出时使用加密文件。

模型服务商、智能体设置、自定义程序、快捷语、任务模板和可迁移偏好都进入本期范围。需要另一台机器的程序、目录或权限时，软件会保留原配置，并明确列出需要补的环境。你可以继续使用已经可用的队员，再处理这些项目。

<!--PAGE-->
# 同名配置和正在工作的队员怎样处理

## 场景二 两边都叫公司模型但内容不同

新工作台里已有“公司模型”，导入文件里也有同名配置。软件把两份并排显示，你可以保留现有、另存一份，或明确替换选中的那份。同名有多条时，每条分别选择；软件不会凭名称猜出你想替换哪一条。

假设你只想让“前端队员”以后使用导入的模型，就明确勾选这位队员，核对它的新模型和设置。其他队员沿用原设置。前端队员此刻正在运行的任务也继续使用开始时确定的模型和密钥版本；本次设置用于之后新开始的任务。

如果旧任务重启时缺少原密钥或原程序，软件会保留任务历史并提示环境缺失，不拿另一把密钥或另一个模型接着跑。导入快捷语只增加指令库；导入任务模板只增加模板库，点击确认本身不创建使命、发送消息或启动定时任务。

原生程序配置也有自己的范围。导入不会改写你在工作台外使用的 Codex、Claude、Gemini 配置。明确选择了导入连接的新任务，使用工作台管理的独立连接环境。已有原生登录方式仍可以继续选择。

<!--PAGE-->
# 中断后继续核对和跨机器同步

## 场景三 确认后断线了

你核对预览并点确认，连接随后中断。重新打开时，软件先读取这次原请求的结果。如果已经应用，显示应用了哪些配置和队员；如果还不能确认，保留原请求，不再导入第二份。预览期间本机设置发生变化，会要求重新核对，不覆盖新改动。

WebDAV 继续可以同步配置。默认同步范围保持界面偏好；要同步完整配置以及密钥，需要分别明确开启。另一台机器收到文件后先进入待核对区，不会因为同名队员而自动套用连接，也不会自动安装程序。你确认映射后，才影响那台机器的新任务。

如果决定撤销这次导入，软件先显示哪些项目在导入后又被修改了。没有后续修改的项目可以按原记录恢复；存在后续修改的项目需要另选处理，避免撤销覆盖你刚做的新设置。已经发生的任务和消息保留。

## 本次确认的范围

是否同意以上方式，并按“完整模型配置迁移 r1”开发。确认包含完整配置迁移、服务商管理、明确应用到队员、配置同步和恢复的源码实现；不包含付费模型调用、合入 main 或发布。后附完整研发约定和待执行验证，当前没有把这些功能算作已完成。

<!--PAGE-->
# 研发说明和固定源码依据

Codeg 固定提交 aff1e6c278453ed8d0b96a5f664edcdca621aa3f，Rovai 固定提交 cc465dda562536b94f23787759e3206324baa29e。融合工程基于当前 develop/V1.0.0 源码。本方案只保存候选字段和设计，不在确认前改生产实现、数据库或当前合同。

Codeg config_sync/domains.rs 的 CONFIG_DOMAINS 顺序为 modelProviders、agentSettings、customAgents、quickMessages、taskTemplates、preferences。ModelProviderDto 有 name、apiUrl、apiKey、agentTypesJson、agentType、model；ProviderRefDto 用 agentType 和 name 引用服务商，原数据库允许重复名称。融合导入以“源文件摘要加域名加行序号”的 entryId 定位每条记录，不沿用原版更新最低数据库编号的重复项选择。

Codeg src/lib/types.ts 的 MODEL_PROVIDER_AGENT_TYPES 只列 claude_code、codex、gemini。其他内置程序依旧按其原有专用设置入口工作，不在本方案中把任意 OPENAI 环境变量冒充它们已支持的连接。旧文件携带的其他 agentType 或未来未知域加密保留、可导出并显示原因；它们不获得可执行身份，具体接入仍依照实际原接口。这是来源兼容行为，不把一期既定专用设置功能移出范围。

原 UI 参考 src/components/settings/model-provider-settings.tsx、add-model-provider-dialog.tsx、edit-model-provider-dialog.tsx、codex-model-list-editor.tsx，以及原配置同步页面。新界面复用原列表、编辑分组和模型列表交互，使用融合工程已有 AppDialog、队员编辑和备份设置容器。移动端沿用各自原生 Form 或 List 设计；Web 使用同一 Host 管理的数据。真实界面同场景对照在后续统一验证步骤进行。

## 变更前

当前 backup/light_config.rs 只导出 appearance、general、windowClose、messageLibrary 四类有限字段，formatVersion 为1。应用配置需要原本机摘要匹配，不能导入模型服务商、密钥、队员连接或原完整模型目录。

当前 FrozenAgentRuntimeConfig 中 custom_api 只表示从原生配置只读取得的 Claude 和 Codex 连接证据。它使用原生目录、凭据版本和文件修订，不是通用可导入的服务商库。environment 字段 serde skip，属于当前 Host 启动时加载的成员环境，不参加历史 Run 身份。不能直接把导入环境覆盖到这里，同时宣称旧任务仍使用旧版本。

下列代码块是当前结构原文，作为实施前对照。省略属性仅由原代码决定，不用新字段去重算旧执行。

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
    pub native_origin: Option<crate::native_lifecycle::NativeSessionOriginV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_steer: Option<crate::native_lifecycle::FrozenNativeSteerV1>,
}
```

```rust
pub enum CustomApiConfiguration {
    #[serde(rename = "claude-code-cli")]
    ClaudeCode {
        mode: Option<ConnectionMode>,
        base_url: String,
        models: ClaudeApiModels,
    },
    #[serde(rename = "codex-cli")]
    Codex {
        mode: Option<ConnectionMode>,
        base_url: String,
        models: Vec<CustomApiModel>,
        default_model: String,
        default_row_id: Option<String>,
    },
}
```

```rust
pub struct CustomApiSnapshot {
    pub configuration: CustomApiConfiguration,
    /// Legacy catalog identity participates in compatibility, never model admission.
    #[serde(default)]
    pub configured_model_ids: Option<Vec<String>>,
    pub context: native::NativeContext,
    pub native_revision: String,
    pub credential_version: String,
    pub credential_source: native::CredentialSource,
    pub provider_id: String,
    /// Compatibility for already-frozen records; never used to select a connection.
    #[serde(default, skip_serializing)]
    pub explicit_mode: bool,
}
```

## 变更后

在 FrozenAgentRuntimeConfig 增加唯一可省略字段 portable_connection，JSON 名称 portableConnection，类型为 Option<FrozenPortableConnectionV1>，使用 serde default 和 skip_serializing_if Option::is_none。队员配置增加可选 PortableMemberBindingV1，指向具体服务商版本。没有明确应用导入连接的新旧任务均省略 portableConnection；不写 null、空对象，不改变旧配置或摘要。

明确采用导入连接的新 Run，在原配置冻结事务中选择一个不可变服务商版本、密钥版本、环境版本、完整模型目录和独立原生作用域。FrozenPortableConnectionV1 的完整 shape 见下文；环境值只在加密存储，冻结记录含有名字和不可变秘密引用。Host 必须解析这些确切版本，找不到时阻止恢复，不能读取服务商最新版本或队员最新环境补位。

现有 ModelSelection 的 runtime_default 与 explicit 及 modelId/options shape 保持。显式选择仍受实际 Runtime 目录与参数验证约束；导入模型说明不构成能力广告。Codex 的 base 模型和 overrides 沿原版目录投影算法校验，保留 customs、excludedOfficials、default 和合法完整覆盖字段，未知覆盖字段保存在原配置中并显示未应用原因。Claude 保留 main、reasoning、haiku、sonnet、opus、fable、customOption、customOptionName、customOptionDescription；Gemini 保留原模型字符串。空值、继承和清除在预览中分别显示，不把空值猜成另一个默认模型。

对显式采用 portableConnection 的新 Host，hostConfigDigest 与 bindingCompatibilityDigest 在既有基础摘要外绑定完整 FrozenPortableConnectionV1 规范 JSON，再计算 configDigest。nativeOrigin、nativeSteer、customRuntime 已确认意图继续由同一冻结步骤按固定顺序组合。序列化与摘要顺序必须在实际实现和测试中固定；未采用便不加包裹，旧摘要不重算。

Bootstrap、Dynamic Context 的章节、顺序、文本、预算、截断、Current Input、历史引用、Skill 选择、权限、MCP 工具和交互反馈合同保持。新连接改变明确选定任务所用的地址、凭据和模型目录，不额外把导入 JSON、密钥、配置说明拼成提示词。版本轴继续为 Bootstrap5、Formatter5、Charter22、Direct29、Public34、Delegation35、Builtin39、IPC3、Envelope2、Output10、MCPProjection2；新增 FrozenPortableConnection 独立 version1。需要改变这些轴或语义时必须提交新 revision。

## 读取和归属

Native 从本机文件选择器导入；Web 和移动端通过现有管理员认证的 Host 有界上传通道登记文件。Core 选择受管路径，客户端只持有 archiveId。解密口令只随本次操作通过私有通道传入，不进普通草稿、日志、诊断包、事件或命令回放正文。普通读取只返回掩码预览和秘密引用。

支持融合格式1兼容导入、融合完整格式2、Codeg schemaVersion1 明文配置及原 AES256GCM 包装。原 manifest 的大小和 SHA256 必须与原字节一致；原加密格式按固定 Codeg crypto.rs 解密，不猜口令、不降级。归档最大64MiB、解密后64MiB、域最多32个、单域8192条、总条目8192、单字段字符串1MiB、JSON深度32、单次预览管理请求2MiB、队员映射最多128位，超出保留原文件并明确拒绝，不静默截断可执行配置。

每个 archiveId 绑定 Host、已认证 DraftClient、文件摘要和受管文件；预览10分钟有效。密钥、环境字符串和原未知字段在加密归档区保留，界面只取经过分类和掩码的字段。apiUrl 仅接受明确的 http 或 https 绝对地址，不允许 userinfo 或 fragment；查询值视为敏感内容并掩码，不产生隐式网络请求。

## 六类配置的具体映射

modelProviders：原行独立定位，导入为 PortableProviderV1。默认创建库条目，不自动给队员选用。apiKey 只有本次显式包含时存入新秘密版本；否则标记需要填写。修改和移除产生新版本或停用头记录，已有冻结版本保留。

agentSettings：保留 agentType、registryId、enabled、sortOrder、envJson 和 provider 引用。用户在预览中逐队员映射。envJson 必须为字符串到字符串的对象，映射到专属不可变环境引用；不写全局 shell，不自动替换普通成员环境。enabled 与排序只影响目标设置目录，不能自动加入 Thread、改权限或启动工作。重复 provider 引用需明确选择准确源行，否则该映射冲突且不能确认。

customAgents：逐字段保留原 CustomAgentDto，按已确认自定义智能体 r1 转为待安装定义，分配新 UUID，秘密转换到该定义自己的引用。skillsDir 原版未导出，不从其他配置猜入；目录、包摘要、平台、可执行入口继续走已有安装预览和明确安装命令。导入不执行包管理器、下载、versionProbe 或任意命令。

quickMessages：content 映射融合库 text，title 保留，sortOrder 保留原顺序；无原ID时创建预览冻结的新 UUID。同名冲突按所选目标编号处理，不按名称覆盖。

taskTemplates：name、title、config 原文进入模板库，config 只解析为原模板设置。绝对路径、本机工程或程序引用须显式重映射；没有映射时保留模板但不允许启动。导入不自动创建任务、使命、计划或执行。

preferences：system_language_settings、appearance_mode、appearance_zoom_level、system_close_behavior_settings 逐值转为既有语言、主题、缩放和关闭行为枚举；不能转换的保留原值并显示原因。logging.level、completed_cache_max_mb 和渠道消息偏好按同名已有设置和限值映射。delegation.enabled、depth_limit、agent_defaults，feedback.enabled、question.enabled、session_info.enabled、chat_authoring.automations_enabled、chat_authoring.work_tasks_enabled 只有明确勾选其行为设置时，才映射已确认的帮手、反馈、交互和工作创作策略；未勾选不改变。原模型可见开关须经过各自冻结规则，不能直接注入新工具。代理、终端、GitHub账号、服务token、Webhook、宠物本机路径、画布与打开标签均不是 portable preference，不混入普通偏好同步。

## 两次核对和事务边界

先登记只读受管归档，再生成预览。预览包含准确源摘要、全部选择、目标成员版本、安装代次、服务商头版本、对应设置与模板库修订，展示创建、替换、跳过、冲突、缺环境。默认全部未选择，单一条目有冲突则 canApply=false，用户可以取消选中该条再生成新预览。

确认只提交原 previewId、previewDigest、commandId。Core 在同一数据库事务重查归属、时效和全部目标修订，创建服务商与秘密版本、库条目和成员引用并记录原回执；任何一项失配则全批拒绝。事务中不操作外部程序或全局原生文件。界面偏好文件由受管 outbox 在提交后依既有所有者写入，回执分别列出已提交与尚待写入；写入前核对旧摘要，有后续变化时暂停该文件应用，不把部分写入说成全部成功。对于该 outbox，本方案 MigrationReceiptV1 的 applied 表示数据库提交，文件状态必须在原回执补充独立列示，不能仅靠 applied 宣称全域生效。

相同 commandId 与相同输入读取原结果，换输入拒绝。脱离客户端的 Core 任务持有生命周期；未知回执只能核对，不能自动再导入、换UUID或补发应用。用户明确选择新任务连接时调用现有运行准入，导入确认不连带运行。configuration.secrets.prepare 只能经私有凭据通道，返回 SecretRefV1 与 expiresAt，不经普通请求日志或恢复存储；同operationId用主机私有HMAC核对同一输入。未附加到确认事务的暂存引用10分钟失效，不能跨client、用途或主机使用。

## 导出和同步

完整格式2默认包含可迁移配置但不含秘密；另行勾选后才收集确切秘密版本放进已加密文件。加密沿融合现有 backup crypto 容器，口令至少8个 Unicode 字符。原生登录Cookie、OAuth会话、系统钥匙串、服务管理员令牌、外部渠道凭据和本机安装锁不随配置库导出，完整工作区备份仍按自己的合同处理。

WebDAV 原界面偏好同步保持默认范围。新增完整配置及秘密开关分别默认关闭；上传冻结单次快照和完整加密文件摘要。下载先登记候选，另一台机器自行选择目标；不把源机器 memberId 当成本机同名队员，不自动应用模型行为设置。ETag冲突保留双方文件，远端清理继续既有显式预览和回执，不顺带删除本机历史。

## 数据库和跨机器恢复

计划在当前原生r1之后追加 Migration216 与 projection schema166，严格识别原215/165，不改写旧迁移。拟新增 portable_provider_revision、portable_provider_head、portable_secret_revision、portable_member_connection、configuration_import_archive、configuration_import_preview、configuration_import_receipt、configuration_import_before、configuration_file_outbox 九类存储。主键分别为 provider(id,revision)、provider(id)、secret(id,revision)、member(agentId)、archive(id)、preview(id)、receipt(commandId)、before(commandId,domain,targetId)、outbox(commandId,domain,targetId)。

不可变版本包含规范内容摘要；秘密为现有主机加密机制加密的内容，不把明文写到 value_json。预览和回执绑定 clientId、archiveId、摘要、状态和时间；before 保存原版本引用及目标应用后修订供撤销。撤销也是显式新命令，当前目标修订必须等于原导入写后的修订，冲突目标拒绝，不重写已执行任务或消息。实际追加的数据库索引、检查约束、迁移识别和 fixtures 在统一验证阶段核对。

完整备份包含这些历史和秘密。跨主机恢复需要原加密主机存储可解密及对应程序可用；缺少时显示 unavailable，不重加密伪造原版本。新主机重新配置只创建新版本供新任务使用。目录和绝对路径保留来源事实，不能自动改写旧 frozen Runtime 的安装锁、作用域或摘要。

# 完整候选字段和操作

以下是候选合同，不是当前生效合同。所有 object 默认为 additionalProperties=false；未列可选字段必须出现，可空用 anyOf 明确表示。JSON 元数据通用内容只在配置库保存，进入 Runtime 投影前再核对原版具体字段、枚举和边界。definition 与 modelSelection 复用各自完整既有形状，管理层仍需按这些形状验证，不能因为此处通用 JSON 描述而放宽生产入口。

## SecretRefV1

```json
{"type":"object","additionalProperties":false,"properties":{"id":{"type":"string","format":"uuid"},"revision":{"type":"integer","minimum":1,"maximum":9007199254740991}},"required":["id","revision"]}
```

## PortableConnectionRefV1

```json
{"type":"object","additionalProperties":false,"properties":{"id":{"type":"string","format":"uuid"},"revision":{"type":"integer","minimum":1,"maximum":9007199254740991},"digest":{"type":"string","pattern":"^sha256:[a-f0-9]{64}$"}},"required":["id","revision","digest"]}
```

## PortableProviderV1

```json
{"type":"object","additionalProperties":false,"properties":{"schemaVersion":{"const":1},"id":{"type":"string","format":"uuid"},"revision":{"type":"integer","minimum":1,"maximum":9007199254740991},"digest":{"type":"string","pattern":"^sha256:[a-f0-9]{64}$"},"name":{"type":"string","maxLength":256},"agentType":{"type":"string","enum":["claude_code","codex","gemini"]},"apiUrl":{"type":"string","maxLength":8192},"credential":{"anyOf":[{"$ref":"#/schemas/SecretRefV1"},{"type":"null"}]},"rawAgentTypesJson":{"type":"string","maxLength":1048576},"rawModel":{"anyOf":[{"type":"string","maxLength":1048576},{"type":"null"}]},"model":{"oneOf":[{"type":"object","additionalProperties":false,"properties":{"kind":{"const":"claude"},"main":{"type":"string","maxLength":1024},"reasoning":{"type":"string","maxLength":1024},"haiku":{"type":"string","maxLength":1024},"sonnet":{"type":"string","maxLength":1024},"opus":{"type":"string","maxLength":1024},"fable":{"type":"string","maxLength":1024},"customOption":{"type":"string","maxLength":1024},"customOptionName":{"type":"string","maxLength":1024},"customOptionDescription":{"type":"string","maxLength":4096}},"required":["kind","main","reasoning","haiku","sonnet","opus","fable","customOption","customOptionName","customOptionDescription"]},{"type":"object","additionalProperties":false,"properties":{"kind":{"const":"codex"},"customs":{"type":"array","items":{"type":"object","additionalProperties":false,"properties":{"slug":{"type":"string","maxLength":1024},"displayName":{"type":"string","maxLength":1024},"contextWindow":{"anyOf":[{"type":"integer","minimum":1,"maximum":2147483647},{"type":"null"}]},"base":{"type":"string","maxLength":1024},"overrides":{"type":"object","maxProperties":4096,"description":"JSON总字节不超过1MiB，嵌套不超过32层；原版目录字段合法性仍逐字段核对"}},"required":["slug","displayName","contextWindow","base","overrides"]},"maxItems":512},"excludedOfficials":{"type":"array","items":{"type":"string","maxLength":1024},"maxItems":2048},"default":{"anyOf":[{"type":"string","maxLength":1024},{"type":"null"}]}},"required":["kind","customs","excludedOfficials","default"]},{"type":"object","additionalProperties":false,"properties":{"kind":{"const":"gemini"},"model":{"anyOf":[{"type":"string","maxLength":1024},{"type":"null"}]}},"required":["kind","model"]}]},"enabled":{"type":"boolean"},"createdAt":{"type":"string","maxLength":64},"updatedAt":{"type":"string","maxLength":64}},"required":["schemaVersion","id","revision","digest","name","agentType","apiUrl","credential","rawAgentTypesJson","rawModel","model","enabled","createdAt","updatedAt"]}
```

## FrozenPortableConnectionV1

```json
{"type":"object","additionalProperties":false,"properties":{"schemaVersion":{"const":1},"reference":{"$ref":"#/schemas/PortableConnectionRefV1"},"projectionVersion":{"const":1},"agentType":{"type":"string","enum":["claude_code","codex","gemini"]},"apiUrl":{"type":"string","maxLength":8192},"credential":{"anyOf":[{"$ref":"#/schemas/SecretRefV1"},{"type":"null"}]},"model":{"oneOf":[{"type":"object","additionalProperties":false,"properties":{"kind":{"const":"claude"},"main":{"type":"string","maxLength":1024},"reasoning":{"type":"string","maxLength":1024},"haiku":{"type":"string","maxLength":1024},"sonnet":{"type":"string","maxLength":1024},"opus":{"type":"string","maxLength":1024},"fable":{"type":"string","maxLength":1024},"customOption":{"type":"string","maxLength":1024},"customOptionName":{"type":"string","maxLength":1024},"customOptionDescription":{"type":"string","maxLength":4096}},"required":["kind","main","reasoning","haiku","sonnet","opus","fable","customOption","customOptionName","customOptionDescription"]},{"type":"object","additionalProperties":false,"properties":{"kind":{"const":"codex"},"customs":{"type":"array","items":{"type":"object","additionalProperties":false,"properties":{"slug":{"type":"string","maxLength":1024},"displayName":{"type":"string","maxLength":1024},"contextWindow":{"anyOf":[{"type":"integer","minimum":1,"maximum":2147483647},{"type":"null"}]},"base":{"type":"string","maxLength":1024},"overrides":{"type":"object","maxProperties":4096,"description":"JSON总字节不超过1MiB，嵌套不超过32层；原版目录字段合法性仍逐字段核对"}},"required":["slug","displayName","contextWindow","base","overrides"]},"maxItems":512},"excludedOfficials":{"type":"array","items":{"type":"string","maxLength":1024},"maxItems":2048},"default":{"anyOf":[{"type":"string","maxLength":1024},{"type":"null"}]}},"required":["kind","customs","excludedOfficials","default"]},{"type":"object","additionalProperties":false,"properties":{"kind":{"const":"gemini"},"model":{"anyOf":[{"type":"string","maxLength":1024},{"type":"null"}]}},"required":["kind","model"]}]},"environment":{"type":"array","items":{"type":"object","additionalProperties":false,"properties":{"name":{"type":"string","pattern":"^[A-Za-z_][A-Za-z0-9_]{0,255}$"},"secret":{"$ref":"#/schemas/SecretRefV1"}},"required":["name","secret"]},"maxItems":256},"environmentDigest":{"type":"string","pattern":"^sha256:[a-f0-9]{64}$"},"catalogDigest":{"type":"string","pattern":"^sha256:[a-f0-9]{64}$"},"projectionDigest":{"type":"string","pattern":"^sha256:[a-f0-9]{64}$"},"installationId":{"type":"string","maxLength":256},"installationGeneration":{"type":"integer","minimum":1,"maximum":9007199254740991},"hostPlatform":{"type":"string","enum":["windows-x64","macos-arm64","macos-x64","linux-x64"]},"nativeScopeId":{"type":"string","format":"uuid"}},"required":["schemaVersion","reference","projectionVersion","agentType","apiUrl","credential","model","environment","environmentDigest","catalogDigest","projectionDigest","installationId","installationGeneration","hostPlatform","nativeScopeId"]}
```

## PortableMemberBindingV1

```json
{"type":"object","additionalProperties":false,"properties":{"schemaVersion":{"const":1},"agentId":{"type":"string","maxLength":256},"revision":{"type":"integer","minimum":1,"maximum":9007199254740991},"connection":{"anyOf":[{"$ref":"#/schemas/PortableConnectionRefV1"},{"type":"null"}]}},"required":["schemaVersion","agentId","revision","connection"]}
```

## ConfigurationArchiveV2

```json
{"type":"object","additionalProperties":false,"properties":{"formatVersion":{"const":2},"product":{"const":"codeg-rovai-config"},"archiveId":{"type":"string","format":"uuid"},"appVersion":{"type":"string","maxLength":128},"createdAt":{"type":"string","maxLength":64},"sourcePlatform":{"type":"string","maxLength":64},"domains":{"type":"object","maxProperties":32,"description":"固定已知域加保留未知域。只应用显式选择并可映射的条目；未知域只加密保留"},"secretsIncluded":{"type":"boolean"},"secrets":{"type":"array","items":{"type":"object","additionalProperties":false,"properties":{"archiveSecretId":{"type":"string","format":"uuid"},"value":{"type":"string","maxLength":1048576}},"required":["archiveSecretId","value"]},"maxItems":2048}},"required":["formatVersion","product","archiveId","appVersion","createdAt","sourcePlatform","domains","secretsIncluded","secrets"]}
```

## MigrationSelectionV1

```json
{"type":"object","additionalProperties":false,"properties":{"entryId":{"type":"string","pattern":"^sha256:[a-f0-9]{64}$"},"domain":{"type":"string","enum":["modelProviders","agentSettings","customAgents","quickMessages","taskTemplates","preferences"]},"decision":{"type":"object","additionalProperties":false,"properties":{"entryId":{"type":"string","pattern":"^sha256:[a-f0-9]{64}$"},"action":{"type":"string","enum":["skip","create","replace"]},"targetId":{"anyOf":[{"type":"string","maxLength":256},{"type":"null"}]},"expectedRevision":{"anyOf":[{"type":"integer","minimum":0,"maximum":9007199254740991},{"type":"null"}]}},"required":["entryId","action","targetId","expectedRevision"]}},"required":["entryId","domain","decision"]}
```

## MemberApplyV1

```json
{"type":"object","additionalProperties":false,"properties":{"agentId":{"type":"string","maxLength":256},"expectedVersion":{"type":"integer","minimum":1,"maximum":9007199254740991},"providerEntryId":{"type":"string","pattern":"^sha256:[a-f0-9]{64}$"},"environmentEntryId":{"anyOf":[{"type":"string","pattern":"^sha256:[a-f0-9]{64}$"},{"type":"null"}]},"modelSelection":{"$ref":"#/schemas/ModelSelection"},"installationId":{"type":"string","maxLength":256},"expectedInstallationGeneration":{"type":"integer","minimum":1,"maximum":9007199254740991}},"required":["agentId","expectedVersion","providerEntryId","environmentEntryId","modelSelection","installationId","expectedInstallationGeneration"]}
```

## MigrationPreviewV1

```json
{"type":"object","additionalProperties":false,"properties":{"schemaVersion":{"const":1},"previewId":{"type":"string","format":"uuid"},"previewDigest":{"type":"string","pattern":"^sha256:[a-f0-9]{64}$"},"expiresAt":{"type":"string","maxLength":64},"archiveId":{"type":"string","format":"uuid"},"sourceDigest":{"type":"string","pattern":"^sha256:[a-f0-9]{64}$"},"localDigest":{"type":"string","pattern":"^sha256:[a-f0-9]{64}$"},"selections":{"type":"array","items":{"$ref":"#/schemas/MigrationSelectionV1"},"maxItems":8192},"members":{"type":"array","items":{"$ref":"#/schemas/MemberApplyV1"},"maxItems":128},"rows":{"type":"array","items":{"type":"object","additionalProperties":false,"properties":{"entryId":{"type":"string","pattern":"^sha256:[a-f0-9]{64}$"},"domain":{"type":"string","maxLength":64},"name":{"type":"string","maxLength":512},"action":{"type":"string","enum":["skip","create","replace"]},"status":{"type":"string","enum":["ready","conflict","needs_environment","unsupported"]},"before":{"type":"object","maxProperties":4096,"description":"JSON总字节不超过1MiB，嵌套不超过32层；原版目录字段合法性仍逐字段核对"},"after":{"type":"object","maxProperties":4096,"description":"JSON总字节不超过1MiB，嵌套不超过32层；原版目录字段合法性仍逐字段核对"},"reason":{"anyOf":[{"type":"string","maxLength":4096},{"type":"null"}]}},"required":["entryId","domain","name","action","status","before","after","reason"]},"maxItems":8192},"canApply":{"type":"boolean"}},"required":["schemaVersion","previewId","previewDigest","expiresAt","archiveId","sourceDigest","localDigest","selections","members","rows","canApply"]}
```

## MigrationReceiptV1

```json
{"type":"object","additionalProperties":false,"properties":{"schemaVersion":{"const":1},"commandId":{"type":"string","format":"uuid"},"previewId":{"type":"string","format":"uuid"},"previewDigest":{"type":"string","pattern":"^sha256:[a-f0-9]{64}$"},"state":{"type":"string","enum":["prepared","applied","rejected","unknown"]},"counts":{"type":"object","additionalProperties":{"type":"integer","minimum":0,"maximum":9007199254740991},"maxProperties":32},"changedMembers":{"type":"array","items":{"type":"string","maxLength":256},"maxItems":128},"errorCode":{"anyOf":[{"type":"string","maxLength":256},{"type":"null"}]},"historyPreserved":{"const":true},"files":{"type":"array","items":{"type":"object","additionalProperties":false,"properties":{"domain":{"type":"string","maxLength":64},"targetId":{"type":"string","maxLength":512},"state":{"type":"string","enum":["pending","applied","conflict","failed"]},"errorCode":{"anyOf":[{"type":"string","maxLength":256},{"type":"null"}]}},"required":["domain","targetId","state","errorCode"]},"maxItems":128}},"required":["schemaVersion","commandId","previewId","previewDigest","state","counts","changedMembers","errorCode","historyPreserved","files"]}
```

## ProviderDefinitionV1

```json
{"type":"object","additionalProperties":false,"properties":{"schemaVersion":{"const":1},"id":{"type":"string","format":"uuid"},"name":{"type":"string","maxLength":256},"agentType":{"type":"string","enum":["claude_code","codex","gemini"]},"apiUrl":{"type":"string","maxLength":8192},"credential":{"anyOf":[{"$ref":"#/schemas/SecretRefV1"},{"type":"null"}]},"rawAgentTypesJson":{"type":"string","maxLength":1048576},"rawModel":{"anyOf":[{"type":"string","maxLength":1048576},{"type":"null"}]},"model":{"oneOf":[{"type":"object","additionalProperties":false,"properties":{"kind":{"const":"claude"},"main":{"type":"string","maxLength":1024},"reasoning":{"type":"string","maxLength":1024},"haiku":{"type":"string","maxLength":1024},"sonnet":{"type":"string","maxLength":1024},"opus":{"type":"string","maxLength":1024},"fable":{"type":"string","maxLength":1024},"customOption":{"type":"string","maxLength":1024},"customOptionName":{"type":"string","maxLength":1024},"customOptionDescription":{"type":"string","maxLength":4096}},"required":["kind","main","reasoning","haiku","sonnet","opus","fable","customOption","customOptionName","customOptionDescription"]},{"type":"object","additionalProperties":false,"properties":{"kind":{"const":"codex"},"customs":{"type":"array","items":{"type":"object","additionalProperties":false,"properties":{"slug":{"type":"string","maxLength":1024},"displayName":{"type":"string","maxLength":1024},"contextWindow":{"anyOf":[{"type":"integer","minimum":1,"maximum":2147483647},{"type":"null"}]},"base":{"type":"string","maxLength":1024},"overrides":{"type":"object","maxProperties":4096,"description":"JSON总字节不超过1MiB，嵌套不超过32层；原版目录字段合法性仍逐字段核对"}},"required":["slug","displayName","contextWindow","base","overrides"]},"maxItems":512},"excludedOfficials":{"type":"array","items":{"type":"string","maxLength":1024},"maxItems":2048},"default":{"anyOf":[{"type":"string","maxLength":1024},{"type":"null"}]}},"required":["kind","customs","excludedOfficials","default"]},{"type":"object","additionalProperties":false,"properties":{"kind":{"const":"gemini"},"model":{"anyOf":[{"type":"string","maxLength":1024},{"type":"null"}]}},"required":["kind","model"]}]},"enabled":{"type":"boolean"}},"required":["schemaVersion","id","name","agentType","apiUrl","credential","rawAgentTypesJson","rawModel","model","enabled"]}
```

## ModelSelection

```json
{"oneOf":[{"type":"object","additionalProperties":false,"properties":{"mode":{"const":"runtime_default"}},"required":["mode"]},{"type":"object","additionalProperties":false,"properties":{"mode":{"const":"explicit"},"modelId":{"type":"string","maxLength":1024},"options":{"description":"沿既有serde_json Value和Runtime选项准入，省略默认null；本提案不收窄其类型"},"dshSource":{"type":"string","enum":["native","web"]}},"required":["mode","modelId"]}]}
```

## 用户操作接口

只有管理员 User 调用，不增加模型工具。列表返回脱敏 PortableProviderV1 列表和 next；预览与撤销预览返回 MigrationPreviewV1，撤销行的before/after相反且仍以当前目标修订为基准；确认和回查返回 MigrationReceiptV1。服务商 upsert 的 definition 完整见 ProviderDefinitionV1，返回新PortableProviderV1；remove返回同原命令回执和保留的头版本。新密钥只经configuration.secrets.prepare的私有提交变为引用，返回SecretRefV1和expiresAt；后续管理命令只持引用。memberConnection.get/set分别返回PortableMemberBindingV1和原命令回执，现有模型选择继续走已有队员Runtime命令和预览。所有读写的空、可选与显式null规则按下面shape执行。

### configuration.migration.preview

```json
{"type":"object","additionalProperties":false,"properties":{"schemaVersion":{"const":1},"archiveId":{"type":"string","format":"uuid"},"sourceDigest":{"type":"string","pattern":"^sha256:[a-f0-9]{64}$"},"selections":{"type":"array","items":{"$ref":"#/schemas/MigrationSelectionV1"},"maxItems":8192},"members":{"type":"array","items":{"$ref":"#/schemas/MemberApplyV1"},"maxItems":128}},"required":["schemaVersion","archiveId","sourceDigest","selections","members"]}
```

### configuration.migration.confirm

```json
{"type":"object","additionalProperties":false,"properties":{"schemaVersion":{"const":1},"commandId":{"type":"string","format":"uuid"},"previewId":{"type":"string","format":"uuid"},"previewDigest":{"type":"string","pattern":"^sha256:[a-f0-9]{64}$"}},"required":["schemaVersion","commandId","previewId","previewDigest"]}
```

### configuration.migration.receipt

```json
{"type":"object","additionalProperties":false,"properties":{"schemaVersion":{"const":1},"commandId":{"type":"string","format":"uuid"}},"required":["schemaVersion","commandId"]}
```

### configuration.providers.list

```json
{"type":"object","additionalProperties":false,"properties":{"after":{"anyOf":[{"type":"string","format":"uuid"},{"type":"null"}]},"limit":{"type":"integer","minimum":1,"maximum":100}},"required":["after","limit"]}
```

### configuration.providers.upsert

```json
{"type":"object","additionalProperties":false,"properties":{"schemaVersion":{"const":1},"commandId":{"type":"string","format":"uuid"},"expectedRevision":{"type":"integer","minimum":0,"maximum":9007199254740991},"definition":{"$ref":"#/schemas/ProviderDefinitionV1"}},"required":["schemaVersion","commandId","expectedRevision","definition"]}
```

### configuration.providers.remove

```json
{"type":"object","additionalProperties":false,"properties":{"schemaVersion":{"const":1},"commandId":{"type":"string","format":"uuid"},"id":{"type":"string","format":"uuid"},"expectedRevision":{"type":"integer","minimum":1,"maximum":9007199254740991}},"required":["schemaVersion","commandId","id","expectedRevision"]}
```

### configuration.memberConnection.get

```json
{"type":"object","additionalProperties":false,"properties":{"agentId":{"type":"string","maxLength":256}},"required":["agentId"]}
```

### configuration.memberConnection.set

```json
{"type":"object","additionalProperties":false,"properties":{"schemaVersion":{"const":1},"commandId":{"type":"string","format":"uuid"},"agentId":{"type":"string","maxLength":256},"expectedVersion":{"type":"integer","minimum":1,"maximum":9007199254740991},"connection":{"anyOf":[{"$ref":"#/schemas/PortableConnectionRefV1"},{"type":"null"}]}},"required":["schemaVersion","commandId","agentId","expectedVersion","connection"]}
```

### configuration.secrets.prepare

```json
{"type":"object","additionalProperties":false,"properties":{"schemaVersion":{"const":1},"operationId":{"type":"string","format":"uuid"},"purpose":{"type":"string","enum":["provider_key","member_environment"]},"value":{"type":"string","maxLength":1048576}},"required":["schemaVersion","operationId","purpose","value"]}
```

### configuration.migration.undoPreview

```json
{"type":"object","additionalProperties":false,"properties":{"schemaVersion":{"const":1},"originalCommandId":{"type":"string","format":"uuid"}},"required":["schemaVersion","originalCommandId"]}
```

### configuration.migration.undoConfirm

```json
{"type":"object","additionalProperties":false,"properties":{"schemaVersion":{"const":1},"commandId":{"type":"string","format":"uuid"},"previewId":{"type":"string","format":"uuid"},"previewDigest":{"type":"string","pattern":"^sha256:[a-f0-9]{64}$"}},"required":["schemaVersion","commandId","previewId","previewDigest"]}
```

# 原始输入字段对照

以下为固定Codeg DTO原文，原字段缺省和序列化名称见其 serde 属性。融合保留源原文，同时对可应用条目执行本方案更严格的闭集归属与修订校验。未知域或字段不拼进模型输入。

## ModelProviderDto

```rust
pub struct ModelProviderDto {
    pub name: String,
    pub api_url: String,
    pub api_key: String,
    #[serde(default)]
    pub agent_types_json: String,
    pub agent_type: String,
    #[serde(default)]
    pub model: Option<String>,
}
```

## ProviderRefDto

```rust
pub struct ProviderRefDto {
    pub agent_type: String,
    pub name: String,
}
```

## AgentSettingDto

```rust
pub struct AgentSettingDto {
    pub agent_type: String,
    #[serde(default)]
    pub registry_id: String,
    pub enabled: bool,
    #[serde(default)]
    pub sort_order: i32,
    #[serde(default)]
    pub env_json: Option<String>,
    /// Absent when the setting uses no provider, or when the referenced
    /// provider row was gone at collection time.
    #[serde(default)]
    pub provider: Option<ProviderRefDto>,
}
```

## CustomAgentDto

```rust
pub struct CustomAgentDto {
    pub registry_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub distribution_kind: String,
    #[serde(default)]
    pub spec_json: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub skills_shared_store: bool,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub version_probe: Option<String>,
    #[serde(default)]
    pub supports_mcp: bool,
}
```

## QuickMessageDto

```rust
pub struct QuickMessageDto {
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub sort_order: i32,
}
```

## TaskTemplateDto

```rust
pub struct TaskTemplateDto {
    pub name: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub config: String,
}
```

# 待执行验证

这些用例尚未执行。待全部一期生产源码写齐并统一Review修复后，按用户规定在第二步整理测试点、第三步写并执行测试，之后整改、回归和真实Actions。模型实测仍需用户另供身份和预算，本文不授权付费调用。

CF01 旧格式1只含四偏好域。通过标准：沿原白名单读取，模型和成员不变。

CF02 原Codeg六域齐全。通过标准：逐域完整预览，未选择均不应用。

CF03 同名同类型重复服务商。通过标准：按源行和目标编号定位，不选最低编号。

CF04 源providerRef匹配多条。通过标准：成员映射显示冲突，阻止未解决批次。

CF05 选择新建而非替换。通过标准：产生预览冻结的新编号，现有条目不变。

CF06 替换时目标版本已变化。通过标准：原确认拒绝，全批不写。

CF07 导出默认秘密关闭。通过标准：文件不携带明文密钥或可解密秘密值。

CF08 明确包含秘密并加密。通过标准：正确口令可恢复精确内容，界面掩码。

CF09 错误口令或篡改密文。通过标准：拒绝且不写任何配置。

CF10 原Codeg明文及加密文件。通过标准：核对manifest与原SHA，不降级解密。

CF11 超过大小或嵌套上限。通过标准：明确拒绝，无部分可执行导入。

CF12 未知域和其他agentType。通过标准：加密保留可导出，列出未应用原因。

CF13 Claude九类模型字段。通过标准：完整保留，无静默丢fable或自定义选项。

CF14 Codex裸slug和旧models。通过标准：按原版规则生成候选且保留原文。

CF15 Codex完整customs和excludedOfficials。通过标准：保留全部合法覆盖，未知字段显示未应用。

CF16 Gemini模型和连接。通过标准：仅所选成员的新任务采用，实际协议验证。

CF17 只有env没有provider。通过标准：预览准确区别清除与继承，不猜另一服务商。

CF18 同一源映射两位成员。通过标准：各自独立修订和环境引用，不能串密钥。

CF19 未选择的成员。通过标准：配置字节和新任务选项保持。

CF20 正在运行的任务后改provider。通过标准：旧Run摘要、连接、秘密版本保持。

CF21 旧任务重启缺旧秘密。通过标准：明确阻止，不能使用新密钥补位。

CF22 新任务所用本机原生配置。通过标准：外部全局文件保持，使用受管独立作用域。

CF23 导入customAgents。通过标准：只注册待安装定义，无下载或命令执行。

CF24 自定义环境秘密归属。通过标准：不能复用另一程序或平台的引用。

CF25 快捷语同名和排序。通过标准：准确新增或替换且保留顺序，不发送。

CF26 模板原文和本机绝对路径。通过标准：保留并提示重映射，不自动运行。

CF27 工具行为偏好默认未选。通过标准：已运行和以后任务的开关均不改变。

CF28 明确选择工具行为偏好。通过标准：沿已确认策略冻结，只影响新任务。

CF29 禁止偏好夹带账号和服务token。通过标准：拒绝应用且不出现在普通导出。

CF30 确认后断线。通过标准：同UUID只读原回执，没有第二次写入。

CF31 同UUID换preview或client。通过标准：拒绝，原请求归属保留。

CF32 预览超过10分钟。通过标准：拒绝并保留选择以重新预览。

CF33 任一数据库写入失败。通过标准：整个配置事务回滚。

CF34 界面文件在事务后变化。通过标准：outbox暂停并列出原因，不覆盖新文件。

CF35 WebDAV默认模式。通过标准：只同步原偏好，不携带模型秘密。

CF36 另一机器下载完整配置。通过标准：先待核对，不自动映射同名队员。

CF37 WebDAV ETag变化。通过标准：保留两端且不覆盖或误删。

CF38 撤销时没有后续修改。通过标准：按before引用恢复，历史消息不变。

CF39 撤销时有后续修改。通过标准：冲突项拒绝，不覆盖后续工作。

CF40 完整备份跨平台缺程序。通过标准：保留全部版本并显示环境待补。

CF41 省略portableConnection的旧Run。通过标准：序列化和所有摘要保持原值。

CF42 新portableConnection身份。通过标准：真实版本/秘密/目录投影绑定摘要，变化后不能混用旧Host。

CF43 Desktop Web Android iOS界面。通过标准：原版同场景、空错加载恢复和小窗对照。

CF44 真实运行资格。通过标准：三类程序真实连接/模型/停止/恢复分别记录，源码不能冒充通过。

# 版本与实施顺序

确认具体r1后，先写Core不可变配置库、秘密引用和迁移，再写原格式兼容解析、完整预览与原请求事务；随后连接所选成员的新任务和受管Host，再接服务商页面、配置迁移、双原生入口、WebDAV完整范围和撤销。每个小功能独立中文提交，本七步大阶段结束才push，不以每个提交触发编译或Actions。

本次不改变48包81簇324场景的一期终点。尚未有运行证据的接口与程序继续保留在统一工程验证清单，不让客户提前做技术验收，也不把“保留未知字段”当成某个原生程序已经可执行。

独立确认依据为 docs/development/model-context-change-governance.md 第49至56行，要求看过完整变更说明后明确同意具体revision。收到“确认，按完整模型配置迁移 r1 实施”后才改生产实现；此前技能、私聊、帮手、浏览器、反馈、自定义智能体和原生继续的确认继续分别有效。
