# 原生问题与计划确认的源码参考

固定Codeg提交为 `aff1e6c278453ed8d0b96a5f664edcdca621aa3f`，Rovai为 `cc465dda562536b94f23787759e3206324baa29e`；均使用既有本地供体，不升级来源或修改供体。许可见[来源保留](../licenses/README.md)。

| 来源 | 真实职责 | 融合位置或待办 |
| --- | --- | --- |
| Codeg `src-tauri/src/acp/question.rs` | 多题、单/多选、Other及Grok扩展回复；单选string、多选array | `crates/rovai-core/src/runtime_interaction.rs` 首批转换；Codex/Claude/Pi/MCP等格式继续保留，不以这两种ACP代替 |
| Codeg `src-tauri/src/acp/cursor_ext.rs` | Cursor问题原编号/选项编号，Other通过skipped.reason完整传递；计划accepted/rejected/cancelled | 同一Core转换层，按原ID而非标签猜选项；混合Other不能只发已映射部分 |
| Codeg `src-tauri/src/acp/plan_approval.rs` | Grok approved/keep_planning/abandoned；修改反馈另需User后续输入 | 转换显式返回独立followup_feedback，当前不自动发送或构造额外提示 |
| Codeg `src/components/chat/ask-question-card.tsx`、`plan-approval-card.tsx` | 多题分页/多选/Other、提交期间锁定、失败保留输入；计划批准/修改/放弃 | 后续原组件移植依据，本批未改Renderer，不声称完成视觉或交互验证 |
| Rovai `action.rs` 的 `ResolveActionApprovalCommand` | 已冻结选项的权限批准；只有optionId，没有多题自由回答 | 保留权限权威；不伪装成permission grant或绕过原选项摘要校验 |
| Rovai `application.rs` 的Cursor阻塞请求分支 | 问题直接skipped、计划直接rejected | 这是尚未完成的执行入口；转换层不等于已处理原生请求 |

规范化按固定来源的四题、每题二至四选项、12字符标题、4096字符问题/选项/Other、262144字符计划、16384字符反馈边界。原版typed UI允许其他来源的开放问题；对应原生格式接入时必须保留该能力，本批Cursor/Grok原生格式保持其二选项下限。

Core问题位置编号在同一完整请求内确定性生成，不是鉴权标识；Cursor原生问题/选项ID逐字节保留。Grok用问题文本作回复key，因此拒绝重复问题以免覆盖；重复选项/原生ID、超限或单选多答等明确拒绝。供体部分路径会静默截断/去重，本批保留合法输入的原编码，并用显式错误保护完整User文字；界面接入后错误须可见、保留输入。Cursor Other合并后的reason超限同样拒绝，不丢掉后半回答。

后续执行队列保持完整范围：

1. Core持久请求：原Run/epoch、Conversation/Thread、Native Session/Binding、原RPC编号、冻结正文摘要及Core代次；新结构使用下一独立迁移，不修改已提交210。
2. User-only答复/决定：只收Core请求编号、版本及回答，从持久Core记录读取原问题/协议/原生ID；不接受UI重交原请求以改绑或替换问题。原UUID完整请求、CAS、原命令回执恢复、跨会话/模型代答拒绝，不能把自由回答写成权限批准。
3. Runtime交付：先记发送标记，再经原路由发送；未知结果核对原请求、不重发；停止、失效或重启关闭旧请求，旧epoch不能批准新请求。界面断线只重读Core待处理请求，不能调用interrupted_response替User跳过；该回复只用于原Runtime/Run停止或路由失效。
4. Grok修改意见以明确User输入登记到Core原发送队列并可见；不由Renderer隐式追加隐藏Native prompt。
5. 接已确认r1的question/plan_approval阻塞投影、原状态等待广播、原卡片和两类查看器；模型仅读已有r1安全摘要，完整问题/答复走User与确切child读取权限，不追加到Bootstrap或父历史，接管水位继续生效。字段存在不代表请求可用。
6. 补其他来源格式及真实Runtime/平台矩阵，与I04其他工作包一同候选验证；本批不增加84正式场景通过数。
