---
document_type: implementation-plan
version: v1.73
authority: version-implementation-and-acceptance
status: in_progress
last_updated: 2026-10-17
---

# v1.73 实施计划

1. 已完成确认登记、固定附件与新基线核对；基础严格输入和设置存储已编码，分项验证单独留证。
2. 四类持久记录、父链/原请求、child kind/Run policy 和接管边界存储已编码并有隔离 Core 分项证据。冻结资格准入、角色绑定摘要、三类Profile与批准的父/私聊/子Bootstrap已编码；默认准入仍沿用原配置。格式35实际Dynamic Context/最终Blob/Manifest/回放已接线，7个定向Core owner通过，候选DDL扩入精确版本/角色约束；下一步合齐child材料合法owner、原生技能/MCP过滤及完整结构准入/受支持旧来源分类/升级，统一登记 Schema160/Migration210。现行运行库仍为 Schema159，不做部分安装。
3. 接 User Command/原 UUID 回执及六个模型操作、CLI 和四个原工具别名；Bootstrap/Dynamic Context 按已批准完整文本与版本分支，不提前广告未完成工具。
4. 接 Scheduler/Fleet、准备/输入接收、快速终态、审批、递归取消、原 Session 恢复、接管和后续完整草稿消费；每项扩所属 owner，不再建 broker 状态机。
5. 移植 Codeg 原 DelegationSettings、工具卡、分组、委派查看器；User-only 原生子会话读取、另一原生查看器、Grok 进度和后台观察也保留，不能互相替代。
6. 与 I04-T02/T03/T04 共同冻结阶段候选，集中构建/真实 Core 与 Runtime/必要平台验证、修复失败，再作阶段收口。当前不 push/Actions，不消耗模型预算。

六操作统一执行器 `delegation_dispatch` 已接实际 Core 内核，尚未挂到公开 IPC/CLI：
start/resume 的目录与文件工作在独立线程执行，提交前重新取得原全局调用门并核对完整 CLI 租约，
不能仅凭同一个 Run/Native Binding 继续提交；status 使用原通知快照等待，等待时不占数据库或调用门。
同 Run 换租约会唤醒全部旧等待者，断线只结束等待、不取消帮手。
start/cancel/resume 在新工作鉴权之前通过不暴露身份的证明查询原 Gateway 回执；
退役租约没有记录就拒绝，改变参数仍冲突，read/history/status 每次重新鉴权和查询，不缓存旧页。
原 Context owner 复用同库覆盖完整六操作、同一请求别名回放、旧回执零写入、继续原 Session、
清理前拒绝保留、两种观察后换租约拒绝、双等待者和断线无副作用。
这批保持 Builtin38/30项与实际159/209，不提前宣称39目录、实际 socket、Host/模型或正式场景通过。
下一项接实际 IPC 及完整39目录/CLI/四别名、闭合输出/错误/证据映射和无限等待读超时，
同时保留旧无资格 Run 的 Bootstrap/Charter 字节与权限；之后继续 User 查询、设置和父 Run 已终态时的 User resume。

依赖与客户事项以[执行计划](../../../integration/execution/i04-execution-plan.md)和[客户待办](../../../integration/execution/customer-todos.json)为准。
