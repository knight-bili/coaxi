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
