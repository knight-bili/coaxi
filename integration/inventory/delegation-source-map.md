# I04：原生父子会话与队员交接的源码映射

2026-10-17。本文件是融合研发调查记录，不是已生效的模型输入、数据库或 API 合同。I04-T01 原生父子/异步委派仍需实现与工程验证，不能因本调查或地图修复记为通过。

固定来源：Codeg `aff1e6c278453ed8d0b96a5f664edcdca621aa3f`；Rovai 供体 `cc465dda562536b94f23787759e3206324baa29e`。当前融合 Core 的事实以本仓当前代码及现行合同为准，不能用供体旧版本或历史字段推断现行行为。文件 SHA 与实际检查日志见 `integration/execution/i04-collaboration-source-result.json`。

## 先分清三种记录

| 记录 | 原应用事实 | 融合处理 |
| --- | --- | --- |
| Rovai 公开消息交接 | 一条公开消息对每个目标创建 Delivery；调度成功后冻结一个接受多条输入的 batch AgentRun。来源见 `docs/architecture/public-a2a-message-delivery.md`，以及现行 Message Delivery、ContextManifest 合同。 | 沿用 Core 的消息、Delivery、AgentRunInput 和原会话路由；不重新建立 root/depth 业务树，也不把消息 anchor 当权限或完成关系。 |
| Codeg 独立子会话 | broker task、父 tool-use、父 conversation、子 conversation、子 connection 是分别定位的身份；子会话有自己的原生 Session。 | 保留独立子会话事实；画布计数和打开入口必须来自真实父子关联，不能用私聊数量、Task 数或 batch Run 数替代。 |
| Rovai 业务 Task | `collaboration.rs:515` 的 TaskRecord 有负责人、业务状态、创建/关闭来源；`create_task:1994` 验证当前负责人权限、当前成员和容量。 | broker task ID 不直接塞进业务 Task；子执行结束也不能自动宣告业务 Task、Mission 或自动化已完成。关联及结算需要单独核定。 |

## Codeg 已核查的执行边界

| 来源 | 核查事实 | 需要移植/适配的部分 |
| --- | --- | --- |
| `src/contexts/delegation-context.tsx:46`、`:125`、`:228` | 父工具与子连接/会话单独关联；权限响应保留子连接身份；完成后短暂保留实时状态再分离。 | 沿用原卡片与子会话入口的展示层级；最终权限交给 Core 校验，Renderer 不授予权限。 |
| `src-tauri/src/acp/delegation/types.rs:54`、`:202`、`:231` | 新建委派和恢复有不同请求；状态含 running/completed/failed/canceled/unknown；阻塞区分权限、问题、计划，并带稳定 request ID。 | 状态不能折叠成成功/失败；unknown 不推导为失败或重新发送。阻塞提示保留原请求身份。 |
| `src-tauri/src/commands/delegation.rs` | 开关默认关闭；深度和按 Runtime 的模式/配置有持久设置；已完成结果有容量限制。 | 核对完整设置页、默认值、支持平台和容量策略；不只移植卡片。来源的限制值不自动成为新 Core 合同。 |
| `src-tauri/src/acp/delegation/listener.rs:81`、`:684`、`:738`、`:778`、`:1188` | token 绑定启动时的父连接；实际父 conversation 由服务端查询；状态/取消按父身份查询；恢复必须有当前父 conversation，不能信任调用者自报。 | 使用 Core 已有认证/执行租约和持久归属，重启后按稳定关系核对；不复制第二个常驻 broker 权威。来源注释中把部分输入校验交给模型，融合服务端必须独立验证。 |
| `src-tauri/src/acp/delegation/broker.rs:2298` | 启动先登记进行中的准备，防止取消窗口遗漏；发送前占用 task/tool/child 身份并保存关系；处理快速完成、发送失败和先取消后完成的竞争。 | 在 Core 事务/执行边界冻结稳定关系，保留原请求幂等结果；不能靠 Renderer 本地 Map 作为重启依据。已读启动主干至 2659，其他分支仍在实现前逐点复核。 |
| 同文件 `:3196`、`:3221`、`:3247` | 父连接退出与父轮次取消分开；轮次取消先同步封住旧 tool-call 与准备，再异步回收子进程；保留已消费 tool ID，避免误绑定下一轮。 | 适配 Core 的 exact Run/epoch 和停止许可；旧轮次、迟到回执不能取消或覆盖下一轮。保留完整取消链，不只停止页面动画。 |
| 同文件 `:3412` | 状态支持即时/有界/无限等待；先订阅 Notify 再取快照，避免丢唤醒；新阻塞 request 只需唤醒一次；不跨子查询长持全局锁。 | 事件与分页/等待合同由 Core 提供，回执和阻塞都保留身份；明确断连/超时不等于子执行失败。 |
| 同文件 `:3933`；`spawner.rs:85` | 恢复按原 DB 子记录校验父归属，必须有可恢复原生 Session；completed 拒绝恢复。复用连接与新建连接分开标记，失败时不误拆别人的活连接。 | 原 task/子会话身份保持，检查原生 Binding 代次、实际活跃执行与工作区；不能以“恢复”为名再建一次任务。 |
| `src-tauri/src/db/migration/m20260522_000001_delegation_columns.rs` | 在来源自身 conversation 上保存父 tool ID 与 delegation call ID 并索引。 | 不直接搬 Codeg 数据库或迁移编号；先检查融合现行 conversation kind、唯一约束和 Core 恢复策略。 |
| `src/components/canvas/nodes/conversation-card-node.tsx:137` | child_count 使用紧凑的原版图标/数字入口。 | 按原卡片复用，数据来自 Core 真正独立子会话关系；需实际运行同场景视觉检查后才授予 UI 完成资格。 |

## Core 现状及本轮实际改动

Core 的公开执行已经是 Delivery-first，历史 `a2aParentAgentRunId`、root/depth 字段不代表新 batch 的父子权威。`runtime.rs:43` 的根输出筛选排除明确的子输出、replay 和 snapshot；本轮不放宽此筛选，不让子内容混入父 Run。

现行 conversation 仍区分 camp_member/single_chat（`db.rs:5531` 的结构检查与相关迁移）；历史初始 CREATE TABLE 片段不等于当前完整 Schema。独立子会话的具体Core关系、约束、API、输入和恢复设计现已形成待确认r1；仍未新增生产表、字段、业务写API或模型工具，不能把提案当实现。

本轮修复 `camp-world-map-model.ts` 只读投影：从目标 batch 的已接受 inputMessageIds 找到当前已加载公开消息，再核对其 Agent 作者、sourceAgentRunId 与明确接收者；只显示当前仍在执行、可见成员之间的一次会合。多输入选取一个可显示来源，不改写持久父子关系、模型输入、交接路由或成功状态。旧 a2a 仅沿原关系兼容，batch 不回退猜测旧父字段。缺来源、撤销、私聊、旧 Run、已结束或恢复阻塞均不强行连线。

单元及静态页面检查不替代真实 Core/Runtime 执行。原生独立子会话与 84 个 I04 正式场景仍待相应实现和验证。

## 接下来的有限队列

### 两种子会话查看器须分开移植

继续核查确认两个相近文件不是同一个能力。`src/components/message/sub-agent-session-dialog.tsx:4` 是broker委派的完整只读流式查看及child权限/问题/计划入口，调用LiveTranscriptView；`:68`使用非模态可嵌套Drawer，`:81`的h-full/min-h-0容器与`:85`的16px对齐保持父页可操作。`delegated-sub-thread.tsx:9`原卡片只显示状态与导航，完整子结果不内嵌；`:59`查看器宿主跨虚拟行生命周期保留。

`src/components/message/subagent-session-dialog.tsx:4`则读取Grok等Runtime自行产生的原生子Session，没有Core/broker子Conversation或child连接，不可当成前一个查看器。`:55`仅运行中每2秒读取，`:123`后台刷新不清空内容，`:127`序号围栏拒绝迟到读取，`:65`保留解析器真实状态，父执行结束不能靠旧in_progress一直转圈。其只读证据回接留在I04，不把原生工具输出归为父Run根输出，也不伪造为Core已调度子AgentRun。

具体Core委派r1现位于`integration/proposals/native-delegation-r1.md`，仍pending；两种查看器的原布局和运行职责都留在移植清单。与模型新语义无关的原生子Session只读观察可独立推进；来源路径/Session ID只用于定位，不授予读取任意本机原生会话的权限。

1. **具体设计完成，待对应r1确认**：独立子会话的Core记录、身份、权限、API、版本与恢复已写入`integration/proposals/native-delegation-r1.md`及完整机器附件；不能把Codeg broker task当业务Task，也不能把该设计记作已实现。
2. 完整模型输入前后与客户故事版Word/PDF交客户审核r1。只暂缓该新语义的实现、Schema和当前合同登记；原生子Session只读回接及其余I04独立任务继续，不沿用技能包/私聊材料r1确认。
3. 与方案并行处理现有协作展示、Runtime 兼容、停止恢复、记忆与自动化的独立缺口；严格对照原两应用 UI。
4. 实现批准后的子执行链路与原版画布入口；权限/问题/计划、取消所有竞争窗口、原身份恢复、快速终态、未知/跨父查询分别验证。
5. 阶段代码集齐再冻结 I04 候选，做必要构建、实际集成与平台矩阵；不逐改动 push 或触发 Actions。

客户不承担上述工程验证。本文件没有新增客户技术验收门槛，也没有把待实现项移出完整一期。
