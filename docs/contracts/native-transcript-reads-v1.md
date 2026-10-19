---
document_type: protocol-contract
contract: native-transcript-reads-v1
authority: core-native-transcript-read-jobs-and-signed-windows
status: accepted
version: 1
last_updated: 2026-10-19
---

# Native Transcript Reads v1

Core 的原生正文读取是 Current User 专用的只读工作，不授予模型或 Runtime 执行、接管、恢复、输入或文件访问权。提供方原生 Agent、原生 Session 和 Core Delegation 保留各自身份；不得将短 Agent 编号填成 Session。Core 委派控制仍由 [Delegation v1](delegation-v1.md)拥有。

## 两类入口

| 操作族 | start 的 reference | 来源 |
| --- | --- | --- |
| `runtime.childReads` | `{threadId,agentRunId,evidenceId,receiverIndex}` | Core 已确认的 Codex/Grok 原生 Session 接收记录 |
| `runtime.agentReads` | `{threadId,agentRunId,evidenceId}` | Core 观察到的 Claude/Qoder 原生工具记录与当轮捕获来源 |

两族均提供 `.start`、`.poll`、`.page`、`.close`。start 接 `{readId,reference}`；childReads 另外可接既有 `{descendant:{parentReadId,descendantReference}}`。agentReads 不接受 descendant 或 receiverIndex。请求严格拒绝额外字段，不接受 actor、路径、home、Runtime、Agent 编号或 Session 覆盖。

Agent 来源必须属于原 Thread/Run/epoch/Conversation，并通过 ContextManifest/Bootstrap 与 System 捕获的历史 Host 来源核定。原 `toolCallId` 和 `rawInputDigest` 必须与该捕获父文件中原 Agent/Task 调用及其 canonical 输入摘要吻合，再由 Claude 结构化 ack 或 Qoder 独立 meta 找到代理文件。缺证明或存在冲突时不可读取，不查当前环境目录猜补。观察 Run 不自动等于原生 Agent 的创建 Run。

## 工作生命周期

readId 是调用端生成的规范 UUID，读取族和请求不可改绑。同 ID 同请求返回原工作；不同请求、跨族借用和已关闭 ID 重用拒绝。Core 代次在 start/poll 的 `{readId,coreGeneration,state,snapshotReference,code}` 中返回，state 为 reading/ready/failed。

两族复用同一 Registry：最多16个活动读者、4个文件 worker，300秒闲置清理及既有1024关闭墓碑软限。worker 自己持有并发许可；移除入口不会让未结束的文件操作绕过上限。关闭、过期或 Core shutdown 发出协作取消，取消只停止读取，不停止提供方代理。close 仅接 `{readId}`，只返回 `{closed:true}`，可在原来源删除后幂等清理。

Core 在 SQLite 范围内准备权限；文件查找、校验、解析、工程补丁行回调及完整 JSON 序列化在文件 worker 上执行，不将数据库带入 worker。错误只投影允许的稳定代码，不发送原始文件错误、目录或启动数据。poll、重复 start 返回已完成工作和每次 page 发布前，必须用刚从 Core 准备的权限复核原目标。已退休轮次可以读原合法历史，但 live 由新的 Core 状态计算，不从旧快照继承。

## 签名窗口

page 接 `{readId,snapshotReference,cursor?,maxBytes?}`。maxBytes 默认32768，合法范围4–32768。返回 `{snapshotReference,projectionDigest,sourceDigest,offset,totalBytes,fragment,nextCursor,complete,reportedStatus,live}`。offset/totalBytes/窗口上限均按 UTF-8 字节计，fragment 保持字符边界；正文不按预览长度裁剪。

完整投影包含 schema_version=1、turns、context_tokens、session_stats、summary、notes、child_references。Agent 自身统计另外放可选 agent_stats，保留原工具时间线和原任务元数据；普通 Session 投影不新增空字段。没有可靠子状态时 reportedStatus 为 unknown，不由外层工具结束推断代理完成。

快照与续页令牌使用进程期 HMAC，绑定读取身份、精确 Core 范围和正文摘要；Agent 另外使用独立的身份摘要域，冻结已选历史来源、原工具/输入和代理身份。令牌不包含明文目录或原生地址。错类型、错读者、错快照、错范围、篡改和非法字节位置拒绝。客户端按位置拼接完整 UTF-8 后核对 projectionDigest 再解析；Core 重启后旧工作与令牌不能恢复执行，须创建新只读工作。

已完成快照保持当次核验的完整正文；刷新用新 readId 重新核验文件，不拼接两个文件版本。当前已确认的 Session 后代使用原签名 descendant 引用。Agent 不发行 Session 后代令牌；嵌套 Agent 与资源文件能力须沿独立的来源链另行核实，不借当前 Camp 目录或原生文本扩大权限。

## 两端运输与工程资格

Native IPC 与认证 Web 均只准入上述四操作，Web 回复按字段闭集投影。读取请求不进入持久命令/材料恢复，不在重登录后自动重发。清理请求没有提供方副作用。

本合同约束已编码的 Core 读取机制，不宣称原 UI、实际 Core.handle/IPC/HTTP、实际 Claude/Qoder Host/CLI 身份映射、跨平台、嵌套 Agent 或文件动作完成。对应工程证据与有限剩余项见[一期执行队列](../../integration/execution/i04-execution-plan.md)；SQL producer 元数据夹具不等于真实提供方资格。
