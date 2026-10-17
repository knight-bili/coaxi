---
document_type: version-overview
version: v1.73
lifecycle: current
authority: version-scope-and-status
design_status: confirmed
implementation_status: in_progress
model_context_change: true
last_updated: 2026-10-18
---

# v1.73：Core 独立帮手

上一版本 [v1.72](../v1.72/README.md) 的历史记录及未完成环境验证保留。
本版本落实客户直接确认的 [I04 独立帮手 r1](model-context-change-native-delegation.md)。
一期完整范围仍为 81 簇、48 包、324 正式场景；本版本不是一期最终 MVP 完成声明。

## 研发状态

Core 新增六操作的严格输入类型及 User-only 设置存储原语，默认关闭、深度 1..8、缓存 512 MiB/0 不限、精确版本更新。
设置 SQL 目前只在隔离存储夹具中安装，尚未登记到启动迁移，也没有 Host/CLI/Renderer 入口。
持久委派/attempt/control、派生父链、原请求回放、任务正文核对、私有 Run 路由与接管序号原语已实现，候选身份 DDL 已在实际 Core 创建的隔离数据库分项验证；仍未登记启动迁移，不能当完整执行或接管功能交付。
新运行准入已接入冻结资格与三类角色，旧运行不会因全局设置后改而补权；未登记完整160/210时保持原配置。帮手Profile11、公开父会话Profile12、私聊父会话Profile13、格式35合法元组及批准的Charter全文已有实现。帮手使用独立身份、空记忆入口和空平台技能索引，既有绑定回放检查相同边界；这不代表所有原生技能/MCP投影已经过滤完成。
当前运行数据仍是 v1.72/Schema159/Migration209；目标 v1.73/Schema160/Migration210 将在完整关系、kind、Run policy 和格式约束集齐后原子接入。
文档 current 表示本轮研发目标，不表示数据合同已经升级。旧 Run、回执、Bootstrap 和模型输入字节没有因这批基础类型改变。
子 Run 调度、完整工具输出、接收/取消/恢复/接管、材料、原 UI 和阶段验证均未完成；不得把这些基础类型称为功能交付。
格式35的角色分支、最终Dynamic Context Blob/Manifest和已有Manifest回放已接入，并有隔离Core分项证据；此前的临时拒绝守卫已由实际格式分支替换。child不读取公屏/队员任务/父Mission，恢复入口也使用child身份；完整User后续材料归属、原生技能/MCP过滤及完整160/210启动准入仍须合齐，不能提前开通生产执行。

## 跨版本文档影响

| 范围 | 结论 | 证据或理由 |
| --- | --- | --- |
| Version lifecycle | 已更新 | 本概览、[实施计划](implementation-plan.md)、[版本索引](../README.md)与旧版 lifecycle 同步；旧实现事实不改写。 |
| Decisions | 已更新 | [V1.73-D01](decisions.md#v1-73-d01) 说明为何复用 Core 而不移植第二套 broker 业务权威，[导航](../../decisions/CURRENT.md)同步。 |
| Contracts | 已更新 | [Delegation v1](../../contracts/delegation-v1.md) 区分已实现基础输入/设置与待接入执行合同；[Builtin Transport v39](../../contracts/builtin-tool-transport-v39.md) 增六项委派、精确租约回放、封闭输出与无限等待，完整Host生命周期资格单独留证。 |
| Architecture | 已更新 | [委派架构](../../architecture/delegation.md)规定单一权威和原生子代理区别；领域词汇在根 CONTEXT.md 同步。 |
| UI | 确认无需更新 | 本批没有 Renderer 或跨版本 UX 改动；原 Codeg 两类查看器及设置页仍按已确认 r1 待移植。 |
| Runtime Activity | 确认无需更新 | 本批无新活动映射；前两批 User-only 原生观察证据保持。 |
| Runtime compatibility | 确认无需更新 | 没有启动实际 Runtime 或新增适用平台资格。 |
| Documentation routing | 已更新 | 版本、合同及架构索引补委派入口；具体批准记录独立保存。 |
| Root README | 确认无需更新 | 产品定位、公开支持范围和已交付能力未改变。 |

## 验证与收口

User 查询与设置的后端及 Native/Web 接线见[查询设置分项记录](../../../integration/execution/i04-delegation-user-queries-result.json)：12项定向Rust、45项两端运输和10项文档测试通过，父Run停止仍可读、游标受众隔离、设置事件/回执失败整笔回滚。
后续User resume见[用户恢复分项记录](../../../integration/execution/i04-delegation-user-resume-result.json)：13项定向Rust、47项两端运输和10项文档测试通过，父实际正常结束/取消、原Session边界、User同事务证明、并发/接管/闭父及故障回滚均有组件证据；原生接收/终态仍是夹具。原页面、完整启动迁移与真实Host/Runtime仍待，不授予正式场景或阶段收口资格。

检查执行结果在[本轮执行记录](../../../integration/execution/i04-delegation-foundation-result.json)逐项记录。
后续身份约束、候选 DDL 和接管存储分项见[关系执行记录](../../../integration/execution/i04-delegation-identity-result.json)。
冻结资格、Bootstrap与默认路径回归见[冻结输入分项记录](../../../integration/execution/i04-delegation-policy-bootstrap-result.json)。
格式35、最终Blob/Manifest、组件回放及默认路径回归见[输入格式分项记录](../../../integration/execution/i04-delegation-format35-result.json)；测试工厂、实际Core调用和待完整准入的资格分别列明。
本批存储夹具不代表完整 Core 启动迁移或命令回执集成。D01–D32、真实 Gate 与 I04 原 84 正式场景仍按实际运行逐项取证。
专用身份/硬预算、平台条件留册；不新增客户阶段验收，I12 后才交用户使用软件。
