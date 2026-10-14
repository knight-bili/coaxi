---
document_type: protocol-contract
contract: canvas-nodes-v1
authority: core-canvas-node-graph-and-atomic-gestures
status: accepted
version: 1
last_updated: 2026-10-14
---

# Canvas Nodes v1

继承 [Canvas Boards v1](canvas-boards-v1.md)，在同一个 Core 数据库保存节点图。
范围来自固定 Codeg `aff1e6c278453ed8d0b96a5f664edcdca621aa3f` 的 canvas entity/service、
commands、store 与 viewport storage，来源摘要见[源码清单](../../integration/execution/i03-node-source-audit-result.json)。
合同定义工程边界，不代表交互画布、PTY 或整个 I03 已验收。

## 读取与命令

| Operation | 参数 | 结果 |
|---|---|---|
| `canvas.nodes.snapshot` | `{boardId}` | `{board, nodes, throughGlobalSequence}` |
| `canvas.nodes.apply` | `{commandId, command:{boardId, expectedRevision, mutation}}` | 首次持久化命令结果 |

Snapshot 在同一读事务捕获 Board、节点和序列；不存在或已删除的画板返回 null Board 和空节点。
节点包含 Core 创建的 UUID `id`、不可变 `boardId`、`data`、`createdAt` 与 `updatedAt`。
读取不创建、修复或清理节点。所有参数与嵌套对象拒绝未知字段。

写入只接受 User，不接受 Camp scope、execution epoch 或额外 expected-entity 列表。
所有手势使用当前画板同一个 revision；元数据和节点写入共用该版本，不能分别维护冲突真源。
冲突返回 `canvas.nodes.conflict` 和当前 revision；缺失画板返回 `canvas.nodes.board_not_found`。
按 [Domain Command Result v2](domain-command-result-v2.md) 保存首次结果，回放不再修改图或发布失效事件。

`mutation.type` 为以下闭集，字段均用 camelCase：

| type | 字段 | 原子行为 |
|---|---|---|
| create | node | 创建一个新节点，ID 由 Core 分配 |
| update | nodeId, patch | 更新展示、几何或 custom 成员；kind、画板和绑定身份不变 |
| move | moves:[{nodeId,x,y}] | 整批移动；不操作其他画板或已删除 ID |
| group | targetNodeId / newRegion 二选一，conversations，consumeNodeIds | 合并到 custom 区域并同时移除已被接纳的会话 pin；其他种类不被吸收删除 |
| detach | regionId, conversation, x, y | custom 中移除该精确成员并生成 pin；其他 region 保持其动态绑定 |
| delete | nodeIds | 仅删除本画板节点，不删除引用的业务对象或用户文件 |
| prune | 无 | 清除本画板失效会话 pin、custom 中的失效引用 |

改变图时，节点差异、Board revision/updatedAt、一个 `canvas.board.invalidated` 以及首次命令结果同事务提交。
事件仍仅为 `{boardId,revision}`；客户端重新读取权威快照，不移植 Codeg 的 LWW 全状态事件为第二套真源。
结果仅返回 `{boardId,revision,nodeIds,deletedNodeIds}`，不在每次拖动收据重复保存整份便签正文。
无变化时返回 `canvas.nodes.unchanged`、原 revision 与空 ID 列表，不产生画布失效；普通 command.result 事实保持既有规则。
校验失败不保存部分手势；SQL、事件或结果保存失败则一起回滚。

## 八种 data

共同字段为 title、color、collapsed、gridColumns、gridRows、x、y、width、height。
绑定字段为 folderPath、folderGroupId、runtimeKind、agentId、conversation、conversations、content、path。
`conversation` 是 `{threadId,conversationId:null|string}`；custom 保存此引用的有序去重数组，
因此公开 Thread 与它内部的指定 Conversation 不会因同一个 threadId 被错误合并。

| kind | 内容与引用 |
|---|---|
| folder | folderPath；引用实际工程目录，不复制 Codeg 数字 folder ID 或建第二个项目注册表 |
| group | folderGroupId；引用既有导航偏好身份，实时分组解析仍由导航能力提供 |
| agent | runtimeKind 筛选和/或 agentId；Runtime 种类与队员身份分开，创建时检查指定队员仍存在 |
| conversation | 一个实际存在且未进入删除的 Core Thread/Conversation 引用 |
| custom | 最多 200 个精确会话引用，保持首次出现顺序；新增引用检查实际存在 |
| note | content 原文，包括换行和 Markdown；展示仍须使用现有安全 Markdown 规则 |
| file | path，保存源绝对路径，存在性与可读性由实际预览授权链路判定 |
| terminal | path，保存工作目录意图；不保存 PTY ID、凭据或原生进程身份 |

Note 以外不接受正文，file/terminal 以外不接受 path。其他不相干绑定也拒绝。
标题和颜色修剪，空标题清除；正文不修剪。颜色沿用 Board 的固定主题词汇。
尺寸夹在 48..20000，坐标夹在 -1000000..1000000，非有限值拒绝。
region 的网格轴夹在 0..12，0 为自动，非 region 轴为0；detach pin 为源尺寸224×132。
路径限制4096个Unicode标量，拒绝NUL/相对路径；保留其他主机的绝对路径作为不可用引用，不能因此获得本机访问权。
文件和目录暂时不存在时，节点身份与几何仍保留。已有业务对象后来消失时允许显示不可用；新会话引用不接受不存在的对象。

## 存储与删除

Schema154/Migration204 从准确 schema153 原子增加 `canvas_node`、画板索引和迁移回执，再更新 marker。
几何、展示与绑定各自保存为独立字段，每项事实只有一个真源；只有 custom 的有序成员列表使用受限 JSON。
批量移动只读写布局，不读取或绑定便签正文；其他手势只加载涉及的节点，不加载整板。
命令收据不复制业务状态或完整节点正文。
旧 Board、安装、队员、Run、Manifest、Blob 和命令结果不改写；历史格式与模型输入合同不变。
当前准入只检查必要结构，不放宽历史来源匹配，不比较历史 SQL 全文来认定新库。

Board 删除在同一事务删除它的节点并保留 Board tombstone，不恢复旧 ID。
本切片没有交互 PTY，不能据此声称已结算终端；引入真实终端前必须接入所属进程的关闭/删除生命周期。
跨画板业务删除 funnel 的自动清理、导航组实时解析、Mission/Task 入口、Renderer 布局和客户端视口继续属于 I03 后续工作，
本合同的 prune 不替代这些生命周期接线，也不缩减[完整一期计划](../../integration/planning/plan-v4.md)。
画布正文不会自动进入模型输入；以后若改变该边界，继续遵循既有模型输入变更治理。

## 验证

纯输入 owner 拥有八种类型、几何/网格、路径与成员上限；扩展 SQLite owner 拥有204回执回滚、
CAS/跨画板、原子手势与结果回滚、精确引用、重开、删除与旧结果回放。
真实 Host/HTTP 另证明153升级、封闭入口与实际持久化 seam；这些都不能替代交互画布、真实模型或全平台资格。
