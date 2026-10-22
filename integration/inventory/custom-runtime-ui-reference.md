# 自定义智能体 r1 的页面源码依据与接线状态

客户已明确确认“自定义智能体接入 r1”。当前仍是第一步写生产源码，本文记录页面对应关系，不表示已通过软件 Review、构建、运行或视觉检查。

## 固定来源

| 来源 | 固定提交 | 参考位置 |
| --- | --- | --- |
| Codeg | `aff1e6c278453ed8d0b96a5f664edcdca621aa3f` | `src/components/settings/add-custom-agent-dialog.tsx`，`custom-agent-mcp-toggle.tsx`，`custom-agent-skills-toggle.tsx`，`src/lib/custom-agents.ts`，`src-tauri/src/acp/remote_registry.rs` |
| Rovai | `cc465dda562536b94f23787759e3206324baa29e` | `apps/desktop/src/renderer/src/MemberManagement.tsx`，`AppDialog.tsx`，`RebuiltSkillsSettings.tsx` |
| Codeg Android | `33dba91b1522330fe519c27c4833066e35c9495e` | `app/src/main/java/app/codeg/android/feature/settings/AgentsScreen.kt`、`AgentDetailScreen.kt` 与原生 `GlassRow`、Markdown 阅读组件 |
| Codeg iOS | `e2d75f918a64bc8a450216155cbc3ae4a820b0f1` | `CodegiOS/Features/Settings/Agents/AgentsSettingsView.swift`、`AgentDetailView.swift` 与原生设置、Markdown 阅读组件 |

## 页面及交互对应

| 融合页面 | 沿用的设计和行为 | 新增接线 |
| --- | --- | --- |
| Desktop/Web `CustomRuntimeSettings` | 原程序列表、添加表单字段分组、分发 JSON、MCP 与技能目录声明；Rovai 设置行及 `AppDialog` | Core 定义修订、完整配置预览、明确安装、安装来源、软移除、原编号只读核对 |
| Desktop/Web `CustomRuntimeRegistry` | 原版公开 ACP 注册表搜索和带入表单 | Core 有界只读缓存；按执行主机平台优先选择二进制；选择不直接安装 |
| Desktop/Web `MemberCustomRuntime` | 原队员模型设置的默认/指定切换和参数输入 | 明确程序定义引用；新任务使用保存值；原配置请求保留后只读核对 |
| Android `RovaiCustomRuntimeSettings` / iOS `RovaiCustomRuntimeSettings` | 原生程序列表、设置表单、平台确认弹窗 | 与桌面一致的管理、注册表、明确安装和恢复入口；环境新值只留当前表单内存 |
| Android/iOS `RovaiMemberCustomRuntime` | 原生队员设置的选择及模型表单 | 具体程序修订、默认或指定模型、参数 JSON、原命令核对 |
| Desktop/Web `NativeSkillLibrary` / Android/iOS `RovaiNativeSkills` | 原原生 Skills 列表、来源选择、附带文件导航、Markdown/原文阅读 | 自定义程序按队员的锁定安装读取声明目录，不接受前端任意目录 |

容器原则沿用原应用：页面仅由实际超出的列表/正文承担滚动；Android 使用单一 `LazyColumn`，iOS 使用原生 `List`/`Form` 或阅读区；桌面表单只由对话框正文滚动。没有通过隐藏所有滚动条或裁掉尾部内容处理贴合问题。

## 用户接口与模型输入边界

- 已确认的五个管理接口仍为 `runtime.custom.list/upsert/remove/install/inspect`。
- `runtime.custom.registry` 是单独的用户页面只读接口，使用固定公开来源；不加入模型工具目录。
- `nativeSkills.list` 对自定义程序增加用户页面参数 `agentId`。Core 从队员保存的定义及安装锁解析目录，不接受路径参数。内置运行类别保持原请求方式。
- 多个分发渠道全部保留；`distributionKind` 只决定本次安装渠道。专用技能目录优先于声明的共享目录；任务准备再按实际冻结的运行来源核对原生技能。
- 新环境明文不进入定义回显、冻结运行快照或客户端通用命令恢复日志。Core 使用不可变秘密引用；管理页恢复记录仅保存请求编号、操作和定义归属。
- 安装完成、ACP 声明、实际握手、真实工具调用和模型响应分别表示不同事实。当前源码不制造已运行资格或模型成功记录。

## 后续统一检查时必须补的证据

当前均未执行：固定供体同场景截图、两种主主题和窄窗口/软键盘、空/加载/错误/未决状态、关闭和返回、最后一项可达性、屏幕阅读名称、键盘操作、定义更新后新旧任务、断线重开、跨平台安装与目录、普通任务/独立帮手的实际绑定、已确认 CA01–CA30。

新代码还须与全部一期工作包一起做统一 Review、修复、编译、测试和真实 Actions；不能凭本表宣布工程验收或客户使用验收通过。
