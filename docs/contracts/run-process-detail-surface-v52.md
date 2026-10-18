---
document_type: protocol-contract
contract: run-process-detail-surface-v52
authority: execution-evidence-block-and-group-pagination
status: accepted
version: 52
source_version: v1.73
last_updated: 2026-10-19
---

# Run Process Detail Surface v52

继承 [v51](run-process-detail-surface-v51.md) 的执行行/位置规则及
[v43](run-process-detail-surface-v43.md) 的主线与组独立窗口；新增显式委派投影。
不改变模型输入、数据库结构、业务回执或 Runtime 执行权威。

## 协商与旧调用

不带 projection 的 page/changes 仍为 schema 2；`projection: "blocks"` 仍为 schema 3，保留旧分组和限制。
该 page 新增可选能力 `delegationBlocksVersion: 4`。客户端先按旧模式探测，仅看到声明才可使用
`projection: "delegationBlocks"`；未声明时继续旧模式，不向旧 Core 发送未知选择器并将错误当空记录。

新选择器同时适用于主线和 groupSequence 子窗口，均返回 schema 4。旧 blocks 与 groupSequence 仍互斥。
afterEntrySequence 仅允许出现在新选择器的 changes 请求。原 User 身份、精确 Thread/Run 归属与严格参数规则不变。

## 独立卡片与来源

同一只读事务中，可信 Core delegation.start/resume 从普通工具组拆为 item。须同时具备 Core canonical
authority/credibility、payload 的 Core 声明及一致 canonicalTool/toolName。Runtime 冒用同名不能取得资格；
尚无结果的可信操作也独立。每块增加 `projectionVersion: 4`。主 evidence 仍在 sequence..lastSequence 内，
组键 tools:首逻辑序号、根键 item:逻辑序号不变，不把外层 Shell 生命周期并入卡片主区间。

卡片可另带 supportingEvidence，最多一条，可位于主区间外。仅在同 epoch、纯 CLI 调用、时间位置与精确结果
证明同时成立且双方唯一时提供。多份 Shell 竞争同一 Core 或一份 Shell 对应多份 Core 均保留原始 Shell，不猜来源。
support 不增加步骤数，组分页不重复它；沿原 User Evidence 投影/正文覆盖读取，不授予新的任务正文权限。

## 成对增量位置

同一持久更新可能改变卡片及原工具组。主线和组 changes 按 `(changeSequence, 主逻辑sequence)` 排序，
请求位置为 `(afterChangeSequence, afterEntrySequence)`。缺 entry 表示该 change 已全部读完；有 entry 还须读取
同 change 内更大序号。主线/组内各最多 96 项，均不为同水位突破 limit。

响应回显 requestedAfterEntrySequence（携带时）；hasMore 时带 nextAfterEntrySequence，必须与
nextAfterChangeSequence 一起保存。读完后下一 change 为 Run 水位、无下一 entry；空 evidence 不表示读完。
entry 要求正 change、正序号且不超出 Run；组内还须处于 from/to 区间。非法位置拒绝。

读取时对同 epoch 的潜在 Shell/Core 时间候选传播最大持久 change，驱动更正记录、原载体和卡片一起刷新。
按“载体依赖候选 Core、卡片唯一性依赖候选载体证明、载体显示依赖唯一性”三次定向传播计算，
不迭代到整个连通历史，不跨 epoch 或仅按 operationId 连接；依赖不作为来源证明，不回写执行或业务记录。
v4 Evidence.changeSequence 是展示依赖水位，旧 v2/v3 不变。薄索引仍扫描历史，包围候选成本另计，并非恒定时间。

## 展开组移除

v4 组 page 隐去已转作 support 的载体，其余原记录可读。组 changes 的 suppressedOperations 为
`{executionEpoch, operationId, sequence}`，精确移除此前加载而现转作 support 的记录；每个描述占一项分页预算。
证明撤销时载体以更高展示水位重新返回。已读到组首的客户端从 groupSequence 刷新，以接回此前隐藏的更早项；
否则只更新已读区间。主线空组仍用空摘要移除。原缓存上限、视口保护、取消和授权失效清空规则保持；
主线定位不能使用较早的 support 序号替代卡片主序号。

## 实施与验证边界

本批仅实现 Core 读取及类型，客户端协商、成对游标、组内移除和真实界面接线继续开发。
原分页 slow owner 的同一数据库验证旧3、跨组载体、开始态/冒名、双向翻页、歧义/撤销/竞争、limit1、
组内移除/重新可见、越域/非法游标及零写，并复用10,000操作夹具测量有界响应与扫描时间。
这些检查不证明真实 provider、完整 Core.handle/Native/Web/Renderer 或阶段交付，实际结果另存执行证据。
