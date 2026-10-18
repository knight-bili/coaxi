# Runtime 原生子代理查看器源码映射

固定 Codeg `aff1e6c278453ed8d0b96a5f664edcdca621aa3f`（Apache-2.0），固定 Rovai `cc465dda562536b94f23787759e3206324baa29e`（MIT）。本项对应无连字符的 `subagent-session-dialog.tsx`；Core 委派的 `sub-agent-session-dialog.tsx` 已有独立实现和[记录](delegation-viewer-ui-reference.md)，不互相替代。

| 原来源 | 必须保留的行为 | 融合位置与当前状态 |
|---|---|---|
| Codeg `agent-tool-call.tsx::parseChildSessionId` | `grokSubagentSession.childSessionId`、历史 `agent_stats.child_session_id` 指向 Grok；Codex 子代理明确编号指向 Codex。不把任意工具参数或子代理短编号当 Session | 当前 Core 已有 Codex/Grok 闭合观察。本批增加原证据到可信读取目标的内部解析；其他 Runtime 的内联 Agent 内容仍归原 Runtime 并集工作，不能因此省略 |
| Codeg `collab-agent-card.tsx`、`session-viewer-host.tsx` | 紧凑子代理入口、每层一个查看器、虚拟行卸载不丢已打开层 | 后续沿已移植的 Base UI Host 复用同层/嵌套规则；原生读取尚未接产品入口 |
| Codeg `subagent-session-dialog.tsx::load` | 只读原 Session；2秒刷新仅在原父卡仍活动时运行；读中跳过重叠请求，序号围栏拒绝迟到覆盖；刷新失败保留正文和阅读位置 | 后续客户端状态与原只读卡片，不接管、不审批、不创建 Conversation 或模型轮次 |
| Codeg `inProgressToolCallsByTurn` | 原父卡结束后，不因保存历史中的旧 `in_progress` 永久转圈 | 可信目标已分开观察状态和 `live`：终态父运行保留真实观察值，停止实时动画资格，不伪造子任务成功 |
| Codeg `getConversation` → `commands/conversations.rs::get_conversation` → 原 Grok/Codex parser | 读取已有原生文件并呈现完整顺序、工具和结果；不 attach/start | 文件定位、内容解析、有界完整翻页、Native/Web API 及 UI 仍待接；不能把本批身份解析当作全文读取可用 |
| Rovai `ExecutionEvidenceService::read_full_payload`、Context/Bootstrap/Conversation binding | Core 唯一归属和持久证据，User-only；界面不能提交任意 Native ID、路径或 Runtime override | `runtime_subagent_session.rs::NativeChildSessionService::resolve` 从原证据与同一 Run/epoch、Thread/Conversation、Context/Bootstrap 和原绑定派生不可反序列化目标 |

本批读取请求对象仅有 `threadId/agentRunId/evidenceId/receiverIndex`。不接收 `nativeSessionId/path/adapterKind/nativeBindingId`。返回的内部目标不实现 Serialize/Deserialize，不直接成为公共 API。Grok 必须有独立的 `nativeSessionId`；仅 `subagent_id` 的观察仍可显示状态，但不能打开会话。观察归属 Run 不被伪称为子代理创建 Run，尤其 Grok 明确保留 `originatingRunKnown=false`。

现有可验证绑定改变或原 epoch 不匹配时明确拒绝，不把旧证据改绑到新 Session。原绑定仍在时，父运行终态不删除其只读来源。`ObservedNativeSessionTarget` 只是 Core 关联的候选：Codex collab 的 receiver 不等于已经证明为子代理，目标保留 relationship，后续必须验证原生父子记录后才可返回任何会话正文。不能凭“当前配置目录中恰好存在同名文件”提升权限。历史绑定的来源保留/恢复和文件变化后的游标均继续属于本项完成条件。

有限实施队列：

1. **内部目标准入**：本批编码并扩原两个 Rust owner；最终结果在[工程记录](../execution/i04-native-child-authority-result.json)。不新增数据库或 Rust 测试 owner。
2. **原生来源与内容**：复用固定 Grok/Codex parser 的实际格式、Session/父链和工具顺序；Core 从可信 Runtime/绑定选择原目录，处理文件改写、消失、长正文及完整有界分页，补历史绑定读取。
3. **两端运输与原 UI**：User-only Native/Web 接同一 Core 读取；原抽屉、卡片、2秒刷新/迟到围栏/嵌套/隐藏和只读呈现，不能复用可审批委派入口。
4. **同场景验证**：隔离真实原生文件 → Core → 产品页面，与原 Codeg 组件做宽窄/日夜/空/活动/终态/失败恢复对照；真实 Runtime 格式和平台资格归阶段集中候选。

文件来源接续（2026-10-18）：`runtime_subagent_session/source.rs` 已编码内部文件层。Codex 只选标准文件名中的精确 thread/最新 rollout，核对原头部 `id` 和两种原 parent 字段；`history_base` 优先于 `forked_from_id`，按精确 rollout 索引保留每份原文件、cut 和 seed，缺源/循环/重复来源明确报错。Grok 在原父 `updates.jsonl` 的显式生命周期或 `subagents/<id>/meta.json` 核对 `child_session_id`，跨 cwd group 的子文件可定位；保留子 `chat_history.jsonl` 供原问答答案恢复。不回退短编号或仅凭相邻目录读取。

文件层以只读句柄和64KiB块摘要保留完整字节，长UTF-8/工具输出不转为截断预览；追加后原快照页保持，同长度改写按块摘要拒绝。路径逐层拒绝链接/reparse，打开后核对根内路径与文件身份。所有原文文档及元数据仅在 Core 内部，不可序列化，也没有注册API；`binding_home` 是待由生产启动捕获/历史绑定恢复提供的内部来源，不从UI或当前配置推断。原文件归并为消息/工具/答案的内容投影、历史 cut/seed 的实际拼接、源目录持久捕获、签名客户端游标、API/原UI和平台资格仍待接；分块来源检查不能称为全文查看器已可用。

完整 I04 四包/21簇/84正式场景保持，本批正式场景新增通过0。原许可通知见[来源许可](../licenses/README.md)。

启动来源接续（2026-10-18）：`origin.rs` 的 CapturedNativeSource 来自 Codex/Acp Host 最终 ManagedProcessLaunchSpec，保留在原 Host 上；Application 的既有绑定路径从精确 Runtime 取得它。Core 绑定、来源事件与原回执同事务，未保存完整环境/凭据或改变模型输入。已记录来源支持后来绑定/epoch变化后的历史只读；旧未记录来源只保留原当前绑定候选，正文明确不可用，不用当前配置补猜。原 source 文件层通过 open_recorded 消费持久目录。当前仍必须完成消息/工具/问答、Codex cut/seed 的归并、签名游标、可取消I/O、两端API和原UI；主程序编译检查与同库/文件夹具分别留证，不代替真实 Runtime/GUI。

内容归并接续（2026-10-18）：私有 `content` 层已移植 Grok 原模型、材料投影与本层解析/工具/问答/压缩/用量规则，Codex 历史 cut/seed 输入已实际拼接；超原预览上限的正文/JSON保持完整。后代统计只从显式 child_session_id 和 Core 再次核验的父链读取，卡片预览保持原500字符规则，不向客户端授予任意 Session 读取。逐条 JSON 与稳定 fallback 时间、坏行/部分尾帧说明、解析取消、摘要秘密不渲染及重复投影有原 filesystem owner 证据。固定源码、原范围和适配记录在[移植清单](native-grok-content-port.json)，实际结果见[分项证据](../execution/i04-native-content-result.json)。

本层未完清单：Codex 原完整消息/代码模式/目标/用量解析；Grok 完整会话统计/上下文窗口目录及原完整 parser 差分；完整有界分页/签名快照及后代引用；首次快照发现/哈希的后台取消；两端公共读取与原 UI/同场景。旧无记录来源导入和真实 Host/Runtime/平台也继续。上述“待接”旧段落保留当批事实，以本段为最新细分状态，不能把本层或历史来源检查当成完整查看器资格。

Codex 主体接续（2026-10-18）：原消息 parser 与代码模式/MCP/延迟等待、去重/promotion、计划/目标、思考/图片、collab/花费与占用已移植；固定[来源范围和适配](native-codex-content-port.json)可查。输入来自现有 Core 核验文件和历史拼接，后代/工程 patch 源行使用可信回调，不导入原 suffix 文件猜选、任意 cwd 读取或执行代码。显示摘要只有 title/model/branch/时间/数量，投影内容变化进入快照指纹。22组原版断言与实际文件分项的资格由[接续证据](../execution/i04-native-codex-content-result.json)拥有。

统计与原parser对照接续（2026-10-18）：Grok完整统计、原cache→config→id窗口回落及显示摘要已接核验源。两份固定原parser实际31组受控文件对照中30组完整投影相同、1组保留原版截断的工具正文、0项未解释差异，见[本批记录](../execution/i04-native-parser-parity-result.json)和[可复现配方](../verification/native-parser-reference/README.md)。原主体字节、22源文件、102包锁图及受控fixture边界已核对，不能称完整原App或全部用户语料等价。

可信工程回调接续（2026-10-18）：`NativeChildSessionService::read_content` 已沿原Current User准入与持久来源接完整内容。最终Host启动cwd作为可选私有字段保留，不读当前Camp或当前Run来回填旧目录，原生cwd只可选根内位置；旧v1缺字段保留原补丁，互相矛盾的已记录目录拒绝选择。新增 `content/project.rs` 复用原文件快照及原Codex几何算法，完整UTF8、单次读取缓存、改写核对/取消和Windows备用流拒绝由两个原owner分层核验。实际[本批证据](../execution/i04-native-project-patch-result.json)不授予完整Host复用、公共API、UI或平台资格。

旧格式分类接续（2026-10-18）：固定Claude parser的`toolUseResult.agentId`对应父文件同名目录下的`subagents/agent-ID.jsonl`，Qoder复用部分读取规则但根与producer独立。Core新增单独`ClaudeAgentLink`私有文件层，以精确Agent/Task调用与结构化确认同时核对，不把短agentId、消息parentUuid或子文件内sessionId当作独立Session授权；完整正文保持。见[固定来源与改动](native-legacy-agent-source.json)及[实际分项结果](../execution/i04-legacy-agent-source-result.json)。没有给现有C/G准入添加Claude分支，不能以文件层测试声称Claude Runtime或完整parser可用。

当前剩余：私有完整签名快照、后台注册/销毁/租约及每页原Core重授权已接，Native/Web四方法名单和Web闭合投影已编码；接下来实际Core/两端调用与固定原只读UI/同场景；Claude旧格式完整parser/实际来源捕获与证据关联、Qoder独立根及旧无记录来源导入同步留在I04格式与Runtime并集收口。真实Host/Runtime/平台及其余工作包保持。前述旧检查点不据历史文字重做已验证分项，也不将组件通过当完整查看器或I04完成。

完整快照接续（2026-10-18）：新私有snapshot.rs沿既有Core用户准入与固定原parser，完整消息/工具JSON/统计进入不可变JSON投影，按UTF8边界分页；进程密钥签名快照、游标和已核实后代入口，引用不携带原生路径，状态/live随当前Core目标。Grok父关联记录和meta复用完整验证解码，不再受旧1MiB预览截断。首次发现/hash/序列化均有取消检查，取消不控制Runtime。[分项记录](../execution/i04-native-snapshot-result.json)仅授予本机最低编译/原文件owner资格；后台注册、重授权、运输与原UI仍未接，不重复宣称31组原parser完整App资格。

后台阅读接续（2026-10-18）：Registry沿原Core引用与固定C/G parser，用有界独立std线程读取，不将SQLite跨线程或占用Tokio blocking池；关闭不提前释放worker名额，迟到成功不发布，缓存/空闲和关闭墓碑有限。四个User方法每次poll/page重做Core resolve，后代保留原root归属，Web按闭合字段投影；缺原生日志时间沿Core持久创建时间。见[本批记录](../execution/i04-native-reader-result.json)。原source owner真实Codex文件/受控线程、原Web projection和客户端owner拥有各自分项，不代表实际Host/网络/原UI或全部Grok生命周期资格。

查看器客户端接续（2026-10-18）：四个 User 方法已接完整分页客户端和原生入口，原生与 Core 委派共用单层/嵌套 Host；隐藏/关闭回收阅读、签名后代父租约、完整元数据列表及刷新失败保留位置已有定向及实际 Renderer 检查。原 Codeg Drawer/header/角色容器有实际日夜宽窄外壳对照，正文适配器为明确标记夹具，完整原富内容尚未完成。来源、32px点击区修正、实际资格与有限剩余见[本批UI映射](native-session-viewer-ui-reference.md)和[工程证据](../execution/i04-native-viewer-ui-result.json)。以上旧待接段落仅保留当批事实，不重做已完成来源/parser/快照/后台/API；下一项集中补富内容与真实Core/两端候选。
