---
document_type: architecture
authority: core-delegation-ownership
last_updated: 2026-10-19
---

# 独立帮手

执行历史的可信 start/resume 卡片由 Core 读取投影独立分块；默认旧投影保持原分组，客户端须先协商再选择新模式。
外层 Shell 的唯一精确证明另作为 support，不移动卡片主区间，也不建立第二份执行事实。关联变化同时使卡片和原组
重读；成对增量位置防止同一次变更影响多个块时漏项。展开组的精确移除与重新可见均由 Core 投影决定。
字段与旧调用兼容见 [Run Process Detail Surface v52](../contracts/run-process-detail-surface-v52.md)。
本批只有后端与类型，客户端启用和完整链路尚待；不将读取投影当作模型委派执行资格。

Core 是委派业务与执行权威。DelegationId 关联真实发起成员、父/根 Conversation、原 Run/epoch/Binding/tool-call 和独立 child Conversation；恢复增加 attempt，不替换逻辑编号。它不是 Task、Mission、正式成员或提供方原生子代理。
User 查询依据 Host 身份及确切持久 Thread/child 关系，父 Run 终态不撤销用户查看历史的能力；模型侧仍每次核对原活跃运行与接管水位。
两类读取复用原固定结果与有界分页，但签名游标隔离受众。全局设置走原User命令/CAS/同事务事件回执，不改变旧冻结输入；具体入口见当前合同。
User恢复以原UUID和持久父Conversation归属授权，允许父Run已经终态，复用同一原任务/Session/清理与writer检查和attempt工厂。候选SQL只凭对应新Run/attempt的同事务User恢复事件准入该分支；模型的活跃父身份门禁保持。因果依据仍来自上一attempt，不伪造模型执行或另一套broker。
私聊新角色使用policy3，普通私聊默认保持policy2。工具、历史和终态写回共享版本/冻结角色校验，不能只把数值3加入白名单；实际六操作仍走委派原鉴权。生产资格精确支持v1.73/160+210或161+211，不按未来schema范围猜测能力。旧Run不补角色；旧输入回放和真实原生Session执行分别保留各自验证证据。
现有 Scheduler/Fleet、Runtime Adapter、输入接收、清理和命令回执承担执行；不能另运行具有业务权威的 Codeg broker。原生提供方的子代理通过只读证据/引用投影，缺可靠来源时显示未知，不伪造 Core 子 Run。

工具结束记录丢失时，Core 启动核对只读取原业务回执及同事务的 `delegation.tool_receipt_confirmed` 关联。
关联精确绑定原工具尝试、命令编号、完整请求摘要、成员、Thread、Run 和 epoch，不能把同请求后来重试的结果借给早先尝试。
未取得该证明的观察记录为 `builtin_tool.result_unconfirmed`，表示本次工具结果尚未确认，不推断任务失败，也不重新派发命令或模型输入。
启动前冻结未结 Evidence 的精确 UUID 集合，既有后台启动任务分批处理、释放数据库锁后让出调度；即使删除旧行后 SQLite 重用 rowid，新调用也不进入旧记录扫描。
维护只更新精确已存在且未结束的 Core 生命周期记录，可以核对退休 epoch 的旧观察；普通 Runtime 回调和新工作授权围栏保持。
原始任务正文和凭据不复制到关联事件或投影；这不是模型输入、业务恢复权或第二套执行权威。

启动恢复中，已接受输入且没有待核对输入或审批等阻塞的 Run，沿原 `settle_lost_agent_run_in_tx` 结算；不得只更新失败字段而漏掉私有投递、受控帮手和清理请求。
同一事务按委派深度先结算父 Run，再处理后代，防止父子均符合恢复条件时受查询顺序影响。User 已接管的分支及其后续 User 输入保持独立。
`delivery_unknown` 仍等待原请求核对，不能重新领取或重发；已处于正确恢复状态的未知输入不重复改写版本。接受凭据、绑定、任务 Blob 和冻结配置保留。业务结算不代表原生进程清理完成，清理确认仍由 Adapter 单独写入。

当前[输入、设置和持久身份原语](../contracts/delegation-v1.md)已编码。原v1.73/Schema160/Migration210在两条启动链提交五张扩展旧表、十个旧guard、settings及新增对象；211随后独立提交原生交互表、生命周期触发器、211回执和161标记。旧210保留精确160目标，不能因current指针推进而将其目标校验改成161。完整当前合同及重复迁移零写，部分结构或未来回执拒绝。后续按[实施计划](../versions/v1.73/implementation-plan.md)继续实际Host/Runtime生命周期与原界面，不据数据升级推断已完成全部执行能力。
结构验证覆盖settings、全部新增对象、五张扩展旧表和十个改动旧guard；安装与验证共用转换。159结构参考只记录既有DDL，不复制业务数据或建立第二套权威。旧v1.72/145–159与新v1.73/160显式分开，159完成回执固定209；部分安装、伪造guard、放宽约束、错误或未来标记/回执均拒绝。历史回执被清理时，技能包准入依据完整当前合同继续工作，不能降回旧输入。
新Run从可信Conversation和完整合同开通标记冻结角色及资格；Runtime刷新沿用该Run的冻结资格，不读取当前设置授予旧Run新能力。角色绑定摘要保留原Runtime模型、权限及无原生Session恢复能力的事实。子Bootstrap按已批准文本使用独立身份、空记忆/平台技能，不查询父Mission或渠道事实；模型输入和原生技能/MCP的完整隔离仍须格式35与执行准入共同实现。
格式35继续使用原Core装配器、受管Blob、Manifest和恢复入口。child有独立顺序与闭集数据，不查询父历史、任务、Mission、A2A或跨Thread历史；初始任务核对原Blob摘要，恢复仅固定继续提示，User后续正文必须来自本child且在接管边界之后。父指南计入相同UTF-8预算；超限不保存部分任务。已有Manifest回放不重选当前设置，child身份也不回退到父Profile。此处是格式与存储接线，材料授权、原生目录投影和真实执行准入仍须单独取证。
完整行为、上下文隔离和原界面职责在[已确认 r1](../versions/v1.73/model-context-change-native-delegation.md)，取舍理由见[V1.73-D01](../versions/v1.73/decisions.md#v1-73-d01)。

原生交互的User读/答入口复用同一Core记录和Gateway：有界摘要列表与完整内容分开，Host派生CURRENT_USER，UI不重交问题或Native身份。全局User envelope使首次精确回执不依赖目标还存在；首次决定仍核定实际owner和live代次，不跳过Camp删除/失效检查。Native IPC/Web只逐项准入三接口，SSE只作安全失效通知；Web未知命令按Core interactionId隔离，原请求只读核对，不因连接丢失换号或自动发送。具体字段和当前证据边界见[User交互接口](../contracts/delegation-v1.md#user交互读取与答复接口)。原生回调、原队列反馈和卡片不由API登记推断完成。

原TaskReport和User委派详情现从同一Core记录选择仍live且pending的question/plan_approval，只投影类型与请求编号；原permission优先级保持。完整问题、计划、答案与Native身份不进入父摘要。等待沿原签名cursor和广播，在User处理队首后观察下一个请求，同题和无关唤醒不重复返回；不持SQLite等待。Native与User变更共用安全runtime.interaction.changed和原delegation.changed失效通知，具体字段见[阻塞摘要合同](../contracts/delegation-v1.md#原生提问的安全阻塞摘要)。Grok修改意见与普通接管的衔接须继续核定，不能把该Native答案冒充普通新任务。

原生User请求沿现有ACP listener和Host/Run/epoch/Session/prompt/delivery运输关联登记，后台等待只保留原生句柄与不可反序列化Target，不维护另一套业务状态。Core唯一请求记录及原Gateway答案决定是否可领取发送token；写端持原Host串行锁后重验仍活跃的确切输入，路由read guard保持到flush，数据库锁和User等待不跨原生I/O。半帧/超时先隔离writer，不把protocol-violated当OS进程已退出，再交原reap/cleanup资格。Host Exited只封当前代次对应Host/Run/epoch，恢复不重发。实现和证据边界见[原生回调合同](../contracts/delegation-v1.md#原生回调与串行答复)。[受控进程耦合记录](../../integration/execution/i04-native-interaction-coupled-result.json)补Windows原listener、同一Core/SQLite、真实受管假ACP进程与User分派的往返，包含双请求不阻塞、原UUID不重复发送和Host Exited封旧请求；这不等于完整Core main/Host启动、真实provider或平台资格。Grok修改意见的原队列及原卡片/查看器仍须完成。

共享聊天选择明确的 child 操作族，不给普通 SingleChat 请求增加可升级的 owner override。Core detail 的接管事实/清理状态与真实成员决定是否挂编辑器；既有 scoped snapshot、输入/上传控制器仍分别拥有材料和原请求恢复，Drawer Host 只拥有呈现、嵌套和离开准备。控制请求持久化不等于输入交接完成：前者可关闭后恢复，后者沿原输入 guard 阻止未结离开。具体边界由[共享聊天合同](../contracts/delegation-v1.md#已接管-child-的共享聊天呈现)拥有，不创建第二份任务状态或模型输入。

Runtime 原生子会话仍是原生观察，不创建 Core Delegation。内部候选由现有 Evidence/Run/Context/Bootstrap 及精确持久启动来源解析（旧观察仅保留当前 Binding 围栏），不能由界面自报 Session 或文件路径。观察归属与创建因果分开，reported receiver 与原生子代理父子证明分开；私有文件层验证真实父子来源后，固定原parser生成完整投影。签名快照按UTF8窗口输出完整JSON，游标绑定精确身份和冻结投影，仅为核实后代签发引用；首次发现/hash及序列化沿同一取消标记。进程内注册层已接有界std后台线程、取消/销毁/租约和每页原Core重授权，四个User读取方法已进入Native/Web准入；原UI及实际Core/Host/网络同场景仍待验证。内部快照和方法名单不代替该资格。当前边界见[原生子会话目标合同](../contracts/delegation-v1.md#runtime-原生子会话读取目标)，全部后续项见[源码映射](../../integration/inventory/native-subagent-viewer-reference.md)。

原生文件层仍在 Core 内部：候选先验证原生父链，再按精确 Codex rollout/Grok 父元数据定位原文件，保存完整字节快照和分块摘要；读取不 attach/start 或写业务库。原生元数据与正文不直接序列化给 Renderer，后续仍由 Core 内容投影拥有消息/工具/问答顺序。Codex/Grok Host 已从最终 ManagedProcessLaunchSpec 捕获非凭据目录，原 Native bind 事务保存 System 来源事件；历史读取沿原 Run/epoch/父 Session 和绑定恢复原来源。未记录来源仍不可打开正文，不能从当前设置或 UI 路径补猜。实现状态和未完边界见[文件来源映射](../../integration/inventory/native-subagent-viewer-reference.md)。

私有内容层现以逐行 JSON 迭代器消费这些核验块，移植固定 Codeg Grok 的原消息模型和纯解析/材料/归并规则，摘要元数据只取模型名称。完整直接正文留在内容模型，运输窗口、签名快照和嵌套来源引用留给后续统一 User API，不在 parser 引入文件路径入口或业务写入。Codex 历史输入按原版 cut/seed 拼接，完整消息 parser 仍待移植；Grok 全会话统计/上下文窗口目录及原完整 parser 差分也未授予资格。具体当前行为与限制由[内容合同](../contracts/delegation-v1.md#原生内容投影的当前实现边界)拥有，不从本层检查外推 UI、真实模型或 I04 完成。

Codex 内容接续已将原消息主体和代码模式/目标/上下文/附件/内部 route 的纯依赖移入相同 Core 边界。读取只消费核验记录，后代和工程 patch 源行由可信回调提供，不导入原文件猜选或执行路径。Root/父身份仍由 Core 持有，显示摘要类型只含 title/model/branch/时间/数量；实际内容、统计和该摘要纳入流式快照指纹，避免工程行号变化后拼接旧页。完整原 parser 差分、生产工程文件回调、旧 agent 文件显式父链和运输/原 UI 资格继续单列，具体[当前合同](../contracts/delegation-v1.md#原生内容投影的当前实现边界)及[本批记录](../../integration/execution/i04-native-codex-content-result.json)取代上一段“消息主体待移植”的分项状态。

原生查看器客户端接续：Renderer 的 NativeChildReader 按同一 Core reference 消费四个 User 读取方法，收齐精确 UTF-8 偏移、快照和整份摘要后才发布内容。阅读实例只属于当前 client/Thread/目标，后代启动期间短暂持有父快照，隐藏/关闭停止阅读，不能控制 Agent Runtime。原生只读页与 Core Delegation 共用每层一个 Host；虚拟行卸载不销毁查看器，替换现有可写 child 仍经过原离开保护。正文、思考、图片及工具内容已接初始展示，完整 Codeg 富内容组件仍待按[源码映射](../../integration/inventory/native-session-viewer-ui-reference.md)完成，不能把外壳和受控运输的 Renderer 检查当成真实 Core/Host/原完整 App 资格。

原生富内容接续：读取后的冻结消息由移植的固定 Codeg ai-elements-adapter 及必要纯依赖整理，原计划清单、计划提案/模式与目标组件进入同一只读 Renderer。原字面解析和计划/目标/工具分组不执行其记录的任何工具，也不调用供体 store/Tauri/模型；类型中的 Runtime 名称不代表新增 Runtime 支持。重复原生tool ID与跨轮结果沿既有到达顺序核对后使用仅本地的关联编号，原冻结内容不改。来源、只读适配与其余富内容队列由[内容移植清单](../../integration/inventory/native-content-ui-port.json)和[UI映射](../../integration/inventory/native-session-viewer-ui-reference.md)记录。

原生工具卡片接续：只读页接固定来源 Tool/Header/Content、Terminal、JSON树和代码展示的原UI声明。原完整instant-collapsible替换早期精简接口；代码着色复用Rovai既有语法引擎，按完整输入和语言/主题更新，无供体的片段key或全局正文缓存。工具执行、Core授权、文件动作和会话归属不交给UI；当前路由仍是过渡的通用值展示，完整ToolCallPart/搜索/Diff/代理及图片动作保持必做项。
