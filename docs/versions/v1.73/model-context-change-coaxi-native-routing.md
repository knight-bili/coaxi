---
document_type: model-context-change
version: v1.73
revision: 1
confirmation_status: confirmed
confirmed_revision: 1
confirmed_by: JiaLe
confirmed_at: 2026-10-26T23:00:00+08:00
---

# 模型路由与自查变更说明 r1

- 基线：79acf59a4815781b26a376c261b2dd17071a3c25；日期：2026-10-26。

## 二次确认

- 确认依据：用户“UI方案驳回…其他的通过”，随后“全部开始修复和优化执行，安装完通知我”。时间为本次执行登记时间，原确认以会话记录为准。
- 本说明 r1 已批准实施；真实模型资格依实际 Gate 结果记录。

## 变更前

1. rovai-interaction 工具目录固定 31 项：13 项浏览器、18 项电脑；工具描述反复使用 codeg 品牌，末尾另附 Rovai 权限约束。
2. cli-operations Skill 以“Use rovai --help”开头，只覆盖消息、Tasks、Missions、历史与记忆，没有“Coaxi 内置浏览器／共享窗口”的原生工具选择规则。
3. interaction delivery evidence 的初始状态为 candidate、runtimeToolNames=[]、realToolCallEvidenceDigest=null；当前没有持续更新目录发现和真实调用成功的代码，不能把空记录当成工具不存在。
4. AI 没有按本轮身份读取自身 Coaxi 工具状态的工具。用户设置页的诊断不等于模型可以自查。

## 变更后

1. 原 31 项工具只替换描述中的 codeg/Codeg 为 Coaxi。名称、参数、授权规则、origin/generation/ref 约束均保留。完整拟议目录见《模型工具目录拟议版_r1.json》。
2. 在 cli-operations 的“Choose the result to preserve”前，原样插入下文；完整文件见《CLI能力路由拟议版_r1.md》：

```markdown
## Coaxi built-in resources

When the user names Coaxi's built-in browser, its right-side browser panel, or a page shared in Coaxi, use the tools of the rovai-interaction MCP server for that resource. Start with browser_list_tabs, then browser_snapshot for the chosen tab. Use the returned tab identity, generation and element ref for later actions. Do not substitute Chrome, another browser, a desktop screenshot, an external computer-use skill, or a web search for this named target.

When the user names a desktop window shared in Coaxi, use computer_list_windows and computer_snapshot from rovai-interaction for that window. A visible window or tab is not a grant. Reading needs a read grant; clicking, typing and other writes need a control grant. browser_eval needs its own enabled capability and user approval for the exact code.

For Coaxi tool availability, use coaxi_diagnose_capabilities when present. Distinguish an assigned capability, its frozen delivery to the current run, a successful native tool call, and a resource grant. Do not claim one from evidence of another. If the MCP tools are absent or discovery fails, describe the missing Coaxi connection and the smallest recovery action. Do not invent a rovai browser/computer CLI command, enable a capability, widen a grant, or switch to another resource on the user's behalf. After a write returns an unknown outcome, inspect current state before any retry.
```

3. 新增一个只读 coaxi_diagnose_capabilities 工具；精确 description/inputSchema 见《能力自查工具拟议定义_r1.json》。它使用既有每轮 MCP 身份，不增加全机权限、不调用电脑截图、不打开页面、不修改任何设置。支持 stdio MCP 的新运行可发现此工具，即使浏览器／电脑开关关闭，也能解释关闭原因。
4. 工具返回 shape 固定如下，所有字段必填；未知用 null，不用空列表伪装失败：

```json
{
  "schemaVersion": 1,
  "scope": "current_agent_run",
  "connection": "ready|unavailable|unknown",
  "capabilities": [{
    "name": "desktop|browser|browser_eval|work_task|automation|session_info|ask_user|feedback|delegation|member_dispatch|member_info",
    "assigned": true,
    "frozenForCurrentRun": true,
    "discovered": null,
    "successfulCallObserved": false,
    "resourceAccess": "not_applicable|none|read|control|mixed|unknown",
    "reason": "ready|disabled_global|disabled_member|not_delivered|transport_unavailable|unsupported_runtime|grant_required|control_required|approval_required|unknown",
    "recovery": ["retry_discovery|start_new_run|open_member_tools|share_resource|allow_control|approve_exact_code|inspect_unknown_outcome|contact_user"]
  }]
}
```

capabilities 必须正好含上述 11 项，每项一次。assigned/frozen/discovered/successfulCallObserved 均允许 null，分别代表对应阶段无法核实，不能互相推断。resourceAccess 只汇总本轮本队员已授权资源，不能返回其他对话、其他队员的授权。successfulCallObserved 仅在原生服务成功接收并完成调用后更新；目录发现成功不算业务调用成功。诊断调用自身不计入浏览器／电脑成功证据。

## 明确不变

- 不修改 SESSION_CHARTER、MEMBER_IDENTITY、记忆选择、历史截断、预算或 Dynamic Context section；新增信息由只读工具按需提供，不每轮重复灌入。
- CLI Skill 产生新 revision；交互工具目录产生新 digest/合同修订。保持 31 个旧工具参数和结果不变，新诊断项使目录在适用运行中最多为 32 项。
- 旧运行、旧 Binding、已冻结清单保持原版，不热替换、不重发历史、不执行 clean break。新运行采用新版本；用户切换后由 UI 明确提示下一轮生效。
- 不将 source_adapted/candidate 改写为 qualified。运行时是否真的会选择正确工具，必须留真实测试证据后才能声明。
- 配置或授权的任何扩大仍由用户执行；没有“自动修复权限”接口。

## 验证

- r1 已按本文件开头记录的用户确认迁入当前 docs/versions/v1.73/；实施与验证记录附后，不能将本地检查写成真实模型资格结果。
- 定向反例：浏览器关闭、仅分享但未配工具、原生 MCP 不支持、旧轮未投递、目录故障、只读页面请求点击、eval 未审批、窗口撤权、其他队员／私聊越权、写入结果未知；均不得改走 Chrome／computer Skill 或盲目重试。
- 真实对照：冻结 6 个独立夹具任务——内置浏览器读取、点击搜索、共享窗口读取、窗口安全点击、关闭工具自查、写入未知恢复。新旧版本各执行一次，记录实际工具名、原生资源身份、授权、结果与成本；失败不做无限付费重试。不得拿单元测试替代真实模型结果。
- UI 整理只合并入口，保留实时反馈的确认与回执、旧讨论的选择授权与撤回，不删除能力。

## 实际实现与证据边界（2026-10-27）

- 精确目录落在 [旧目录](../../../crates/rovai-core/src/interaction_tools/catalog-v1.json)、[新目录](../../../crates/rovai-core/src/interaction_tools/catalog.json)；[诊断定义](../../../crates/rovai-core/src/interaction_tools/diagnostic-tool.json)与 [CLI Skill](../../../skills/cli-operations/SKILL.md)保存批准的替换文本。旧工具名称、顺序及参数不变。
- 目录选择按 Run 私有保存；旧 manifest 沿 v1 目录／协议，新 Run 使用 v2 诊断协议。没有数据库迁移、权限扩大、历史重发或 Bootstrap／Dynamic Context 修改。
- 三项交互能力按本轮身份核实开关、连接、目录发现、成功业务结果及资源授权；任务、自动化、会话信息、提问核实全局／队员开关和冻结状态，调用发现与成功证据缺失时保留 null。反馈、父子委派、队员调度、队员查询尚无统一证据源，诊断如实返回 unknown/null，不由“已配置”推断“已成功”。
- 成功证据不接受业务 error、未知动作 effect、unknown/unsatisfied 核验；缺失或过期桌面连接立即拒绝普通工具，不入队等待超时。只读页面的 eval 先要求 control，再要求本次代码审批。
- 已执行 Core 全量库回归、诊断定向反例、桌面类型／单元与隔离 Renderer 回归。真实 provider 的新旧对照尚未执行，本批本地候选更新不授予 Runtime qualified 资格，也不计入一期正式场景完成数。共享 Skill 的通用 12 例及本说明 6 项专属对照仍属于正式 Gate 的待完成项。
