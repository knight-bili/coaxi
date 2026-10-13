# I02-T02 Runtime 移植边界

当前源码正在加入Gemini Core类型与共享设置页，并以Migration197从schema146扩展到147；Core目录变为19种，
Hermes/OpenClaw继续待移植。下方18种是本批前的固定来源盘点，不应误读为新源码现状。
Gemini平台资格仍为未验证，Core编译与迁移回滚由新Core integration检查；真实模型/停止/恢复/工具未通过。
规范见[Gemini Runtime v1](../../docs/contracts/gemini-runtime-v1.md)。

2026-10-14。盘点结果见 [i02-runtime-inventory-result.json](i02-runtime-inventory-result.json)。这是源码差异和研发顺序，不代表 Runtime 已通过准入或执行。

一期保留规划中的21种 Runtime。当前 Rovai Core 的 `AdapterKind::ALL` 声明18种；Codeg另有 Gemini、Hermes、OpenClaw。不能把三种来源入口删掉，也不能启动 Codeg 的 Tauri 执行层形成第二个业务权威。移植发生在 Core 的 Runtime 适配、Host 合同和共享成员设置页面内。

| 来源 | 当前源码差异 | 下一步实现及验收 |
| --- | --- | --- |
| Core 已声明18种 | 发现、产品检查、模型目录、成员配置和执行合同已有骨架；源码声明不等于本机准入 | 保留原 AdapterKind 与成员配置；使用隔离安装/环境测试发现、模型、权限、真实流式、停止、恢复、工具及错误路径 |
| Gemini | Codeg ACP registry 已有发行与 MCP 能力；Core enum 中没有该产品 | 对齐固定 Codeg 的安装/发行及 ACP 行为，补 Core 产品发现、模型/权限/环境 DTO、会话事件和取消恢复，再接共享成员选择器 |
| Hermes | Codeg 使用固定社区 npm bridge，对官方源码版本和独立 Python 环境有约束 | 保留来源和固定版本证明，独立安装根及依赖，不等同于任意 `hermes` 同名命令；补 Core ACP 路径、工具、模型与会话生命周期 |
| OpenClaw | Codeg 固定2026.9.8；本机2026.6.10。来源 Node 门槛24.16/26.1，本机工作区24.9；来源不支持非空 ACP `mcpServers` | 独立 Node/产品版本，先明确工具能力边界及 Core 内兼容方式；不升级全局产品、不发送来源拒绝的 MCP 条目，不把空握手写成工具已可用 |

自审后的实施顺序：

1. 先为三种新增产品核对固定源码的发行、协议、模型、权限、认证及恢复字段，列明真实支持与不支持的能力。当前产品命令别名仅用于发现候选，须由 Core 核验产品与可执行完整性。
2. 在 Core 扩展适配与序列化合同，梳理涉及的数据库约束/迁移；沿现有 `runtime.installations.*`、`runtime.product.*` 和成员配置操作，不给 Renderer 独立进程控制权。
3. 移植 Codeg ACP 解析及事件语义，继续由 Core 管理 Run、Usage、批准、取消、恢复与内置工具权限。不能用两套 Session/Run 状态相互抄写。
4. 共享成员设置明确产品、模型、环境、权限和实际能力；新增来源的安装、更新、来源说明、错误及重开一致性均纳入一期。
5. 先在无凭据、无推理的协议夹具检查配置/事件/错误，再使用独立开发身份跑真实模型、流式、停止、恢复与 Skill/MCP。没有账号或外部资源的项目登记客户待办；可修的协议/构建问题由研发继续处理。

当前本机仅发现 Codex、Claude Code、OpenClaw 三个入口。Codex 登录状态已只读核实有效，未复制凭据；其余认证未检查。18个入口没有出现在当前 PATH，不能推断未安装。全部21种 Core 准入及真实模型验证仍为 `not_run`，Mac/Linux 与客户实机仍须补齐。

等待投递撤回成功路径依赖可控的领取前窗口，保留在 [研发待办](development-todos.json)。阅读记录没有投递；正式发布的快速领取试验产生了一条失败 Run，没有真实模型运行证明。不能以这两种数据替代撤回成功、真实流式或恢复验收。

## Gemini固定版本的已验证输入

固定Codeg使用@google/gemini-cli@0.60.0，官方tag对应733edcb597ce690ac2e2fe3b3b3690b60a4c8f27。已在独立安装和GEMINI_CLI_HOME验证真实--acp --skip-trust进程：initialize返回协议1、gemini-cli/0.60.0及四种认证方法；刻意空认证的session/new返回-32000，没有成功Session。强制独立文件凭据存储、未继承认证环境，进程正常退出。见[i02-gemini-native-result.json](i02-gemini-native-result.json)。

上游session/new/load成功响应包含models.availableModels/currentModelId和modes.availableModes/currentModeId；plan仅在原生启用时出现。Core需沿已有ACP模型映射及会话生命周期接入，产品启动使用已验证的两个参数，不能复用Qwen的--mcp-config启动参数而假定Gemini支持。初始化中的loadSession/图片音频/MCP是宣告，真实模型、流式、停止、恢复和工具均未通过；Core仍未增加gemini-cli，移植任务保持实施中。
