---
document_type: implementation-plan
version: v1.73
authority: version-implementation-and-acceptance
status: in_progress
last_updated: 2026-10-18
---

# v1.73 实施计划

1. 已完成确认登记、固定附件与新基线核对；基础严格输入和设置存储已编码，分项验证单独留证。
2. 四类持久记录、父链/原请求、child kind/Run policy 和接管边界存储已编码并有隔离 Core 分项证据。冻结资格准入、角色绑定摘要、三类Profile与批准的父/私聊/子Bootstrap已编码；默认准入仍沿用原配置。格式35实际Dynamic Context/最终Blob/Manifest/回放已接线，7个定向Core owner通过，候选DDL扩入精确版本/角色约束；下一步合齐child材料合法owner、原生技能/MCP过滤及完整结构准入/受支持旧来源分类/升级，统一登记 Schema160/Migration210。现行运行库仍为 Schema159，不做部分安装。
3. 接 User Command/原 UUID 回执及六个模型操作、CLI 和四个原工具别名；Bootstrap/Dynamic Context 按已批准完整文本与版本分支，不提前广告未完成工具。
4. 接 Scheduler/Fleet、准备/输入接收、快速终态、审批、递归取消、原 Session 恢复、接管和后续完整草稿消费；每项扩所属 owner，不再建 broker 状态机。
5. 移植 Codeg 原 DelegationSettings、工具卡、分组、委派查看器；User-only 原生子会话读取、另一原生查看器、Grok 进度和后台观察也保留，不能互相替代。
6. 与 I04-T02/T03/T04 共同冻结阶段候选，集中构建/真实 Core 与 Runtime/必要平台验证、修复失败，再作阶段收口。当前不 push/Actions，不消耗模型预算。

六操作统一执行器 `delegation_dispatch` 已接实际 Core 内核及公开 IPC/CLI：
start/resume 的目录与文件工作在独立线程执行，提交前重新取得原全局调用门并核对完整 CLI 租约，
不能仅凭同一个 Run/Native Binding 继续提交；status 使用原通知快照等待，等待时不占数据库或调用门。
同 Run 换租约会唤醒全部旧等待者，断线只结束等待、不取消帮手。
start/cancel/resume 在新工作鉴权之前通过不暴露身份的证明查询原 Gateway 回执；
退役租约没有记录就拒绝，改变参数仍冲突，read/history/status 每次重新鉴权和查询，不缓存旧页。
原 Context owner 复用同库覆盖完整六操作、同一请求别名回放、旧回执零写入、继续原 Session、
清理前拒绝保留、两种观察后换租约拒绝、双等待者和断线无副作用。
Builtin39/36项、六命令/四原别名、封闭输出/安全错误/证据投影元数据与无限等待读取截止已接线；IPC3/Envelope2/AgentOutput10及实际159/209保持。
批准的输入/说明/TaskReport和帮助文本使用原全文；原30项黄金结果保持。条件输出规则实际校验，未知结果不能夹带私有字段。
同一 Context owner 借用原数据库验证真实 Windows 管道帧和断线，不启动 Host 或模型；CLI虚拟时钟验证无限读取和原有界超时。
此批不等于真实 Host、全部 Runtime/平台、正式场景或 I04 完成。
旧无资格 Run 仍不能调用新业务操作，Bootstrap/Charter冻结内容不因目录版本补权；完整160/210与旧来源资格仍按原计划推进。
实际159无委派表时先在原 agent_run 核对冻结资格，旧Run的六操作稳定拒绝，不借查新表报内部错误；同一 owner 在候选表安装前验证零写入。
统一执行器的起止已接原 ExecutionEvidenceService/canonical lifecycle，共用实际返回封套；
原回执早返回不记第二笔，每次实际执行尝试独立，避免同号新查询/提交前失败重试污染旧观察。
关联使用原读模型的精确操作/输出摘要载体判定，provider事件夹具与真实Core调用分开标明。
开始记录失败不派任务；结束记录故障保住原提交和回执，started行供后续Host重启核对，不宣称自动补齐。
下一项继续 User 查询、设置和父 Run 已终态时的 User resume；真实 Host/Runtime、恢复协调和原 UI 资格仍按完整清单推进。

依赖与客户事项以[执行计划](../../../integration/execution/i04-execution-plan.md)和[客户待办](../../../integration/execution/customer-todos.json)为准。
