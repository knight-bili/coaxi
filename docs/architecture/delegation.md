---
document_type: architecture
authority: core-delegation-ownership
last_updated: 2026-10-18
---

# 独立帮手

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

Runtime 原生子会话仍是原生观察，不创建 Core Delegation。内部候选由现有 Evidence/Run/Context/Bootstrap/当前 Binding 解析，不能由界面自报 Session 或文件路径。观察归属与创建因果分开，reported receiver 与原生子代理父子证明分开；来源解析器尚须验证原生父子记录后才能返回正文。该候选不构成公共读取 API，文件/运输/原 UI 仍待接。当前边界见[原生子会话目标合同](../contracts/delegation-v1.md#runtime-原生子会话读取目标)，全部后续项见[源码映射](../../integration/inventory/native-subagent-viewer-reference.md)。
