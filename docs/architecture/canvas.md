---
document_type: architecture
authority: integration-canvas-boundaries
last_updated: 2026-10-16
---

# Codeg canvas integration

I03 ports multi-board navigation and all eight Codeg node kinds: folder, group, agent,
conversation, custom, note, file and terminal. The [full plan](../../integration/planning/plan-v4.md)
and C12/C13/C14 acceptance cases remain the completion boundary. A board API is foundation work,
not a substitute for interactive nodes, task execution or cross-platform qualification.

## Authority and migration

Use Rovai Core's SQLite, typed Domain Command Gateway and authenticated Host transport.
Board identity and metadata are owned by [Canvas Boards v1](../contracts/canvas-boards-v1.md).
Node types, graph writes and snapshots are owned by [Canvas Nodes v1](../contracts/canvas-nodes-v1.md).
The paged metadata directory is owned by [Canvas References v1](../contracts/canvas-references-v1.md).
Schema152/Migration202 appends one table to schema151 without changing previous model, skill,
member, Mission or Thread facts. Retain the closed source-contract and existing command-result
replay; do not copy Codeg's database or SeaORM/Tauri lifecycle into the Core.

Each mutation validates the User scope and exact board revision in the existing write transaction.
Commit a small invalidation event and the first command result together. Each read captures its
through-sequence and rows in one transaction. Reconnect, gap and stale responses cause a refetch;
they do not authorize clients to rebuild a state store from event payloads.

The client graph controller subscribes before its initial read and keeps one Host/board scope.
Revision/sequence hints reject older reads; conflict and unknown command results do not silently
rebase a gesture or generate a replacement command ID. Read failures after a committed result
retain that result. Renderer effect instances isolate Host changes and StrictMode remounts.
Controlled client tests cover this cache boundary; mounted UI and actual Core recovery remain separate qualification.

## Remaining node and interaction work

`canvas.references.page` reads bounded Thread/Mission metadata, actual active members and exact active private
conversations in one transaction per page. A fixed identity upper bound keeps a scan finite while activity changes;
the directory controller treats mid-scan invalidation as incomplete and schedules a trailing read. It does not freeze
one transaction across pages or infer live Runtime qualification. The renderer uses this directory for choices and
member regions rather than treating sidebar prefixes as complete. Folder/group worktree relations come from the
read-only [Workspace Worktrees v1](../contracts/workspace-worktrees-v1.md) Git topology operation. Main regions absorb
explicit linked checkouts; a child or project subdirectory stays exact. Missing topology remains visibly incomplete,
never guessed from path prefixes. [Navigation Project Groups v1](../contracts/navigation-project-groups-v1.md) now supplies
client-local schema7 group identity, membership and color to the sidebar and canvas. One BusinessApp-owned folder provider
merges sidebar, surface and manager demands behind the same bounded controller; failed reads keep known relations incomplete.
Management/dragging and real mounted qualification remain separate from these persistence and projection checks. Current SQL fixture and controlled
client results do not qualify the new Rust route or mounted UI before stage verification.

The next I03 slices port board-unit geometry, grid regions, collapse/expand, dragging,
box selection, alignment, resize, grouping/detach, previews and minimap. Keep the board's own
zoom independent of the application's text zoom. Persist shared node geometry separately from
client-local viewport, selection, expanded cards and unsent conversation drafts. Scope local
view keys by Core identity and board ID to avoid cross-backend state reuse.

`canvas-layout.ts` adapts the fixed donor's board-unit grids, drop hints, alignment guides and
shelf packing. Overlap follows actual paint order rather than treating Core UUIDs as numeric source IDs;
Thread and exact private-conversation references remain distinct. The client view-state module keeps
viewport/expansion keys under explicit Host/Core/board identity, with a minimap preference shared only
within that Host/Core. It does not adopt the donor's unscoped numeric-ID keys or persist Runtime sessions.
Unsent input now uses the existing Composer with independent Host/Core/board/pin/reference draft contexts;
the same card's inline/drawer presentations reuse its editor. Different cards do not merge unsent input.

References to AI members, Mission, Task, Thread and project files identify existing Core objects;
the canvas does not own copied business status or a second execution engine. Resolve visibility
and actions through the object's existing permissions and current snapshot. Missing or removed
objects must be visibly unavailable; never recreate one because its card survived. A board/node
operation may change layout without changing or deleting the referenced business object.

客户已确认[私聊材料隔离 r1](../versions/v1.72/model-context-change-canvas-private-materials.md)。
[Scope 合同](../contracts/single-chat-composer-scope-v1.md)区分可信编辑端身份和卡片材料归属，
保留默认草稿、单一 FIFO、旧命令摘要与已冻结输入。Scope codec、真实 pin 校验和 Schema155/Migration205 存储已实现，
默认 SQL/quote/资源读取限定空 scope。Core 已实现材料七动作、精确读取、有界恢复及按 scope 消费后进入原单一 FIFO；
完整保留项与回执同事务，引用复用原捕获和模型投影。get/mutate/recovery 已接同一 Application、两端准入与共享客户端能力核对，
显式请求才回显 scopeVersion；快照草稿和编辑会话按同一归属过滤，原回执只读恢复不派发命令。
Core 已扩单 Conversation 编辑租约的 client+scope 接管和完整移回：空目标返回活动输入，占用目标另建完整 retained，
原 FIFO、材料与回执保持原子性。上传准备、资源授权、Renderer targetBody/placement/完整材料选择和本机意图仍待接线；
当前旧移回入口缺新版参数会明确拒绝，UI 尚未启用，真实 Host/Runtime 与完整隔离功能仍待验。

会话读取由 client 所有的 `ThreadProjectionStore` 按 Thread 隔离，现有页面的只读打开、刷新、缓存和历史分页已接入。
同 Thread 的同时打开合并运输；刷新发生在读取过程中时追加一次最新读取，不把较早的在途请求当刷新完成。
各 Thread 水位与历史窗口独立，未知/失败保留最后有效快照；分页被新水位越过后丢弃，防止旧内容覆盖已更新读取。
五个空闲快照沿用原缓存预算，已挂载/订阅或正在读取的 Thread 不受该空闲上限限制。
显式 `threads.enter` 继续由原导航入口发出，只把其有效结果交回同一读取 owner；画布恢复没有 enter、Runtime 启动或通知确认副作用。
服务重启清快照、推进代次并保留挂载/订阅关系；旧 client、删除资源和旧代次结果不能覆盖新读取。
实际多卡 UI、全部卡片事件分发、各 Thread 文件预览与离开保护聚合仍需接线及阶段资格验证，不能以当前单聚焦入口代替。

File nodes use the existing file-preview handle/root grant and actual path validation. A stored
path is not file authorization. Terminal nodes keep launch intent and geometry, never a durable
PTY ID or credential. Rovai currently has no interactive canvas PTY operation; its
`RuntimeTerminalAdmission` admits AgentRun terminal-state settlement, not PTY startup. Port
Codeg's actual PTY lifecycle and add explicit Core ownership, launch authorization and planned
shutdown integration before claiming terminal support. Reopening after restart requires a new
ephemeral session. Board removal must settle its owned terminals and remove node bindings,
while keeping independent Mission/Thread histories and user files. Those ownership and lifecycle
joins need implementation and actual process tests before terminal nodes are qualified.

## Current evidence boundary

The multi-board Core lifecycle and schema154/204 eight-kind node command/snapshot boundary
are implemented. Local isolated Host/HTTP verifies the actual 153 upgrade, graph gestures and restart;
the [node evidence](../../integration/execution/i03-canvas-nodes-result.json) records checks and remaining work. UI board list/switching,
all node rendering, business navigation, actual PTY, execution round-trip and
platform acceptance remain pending. No canvas content is sent to a model by these slices; any
later model-visible context change must follow the existing revision confirmation governance.

## Node mapping and retained behavior

| Codeg source | Destination authority / integration requirement |
|---|---|
| folder region | Actual directory plus the current navigation group/query; client directory preferences stay JSON and do not become a second project registry in SQLite |
| group region | Preserve folder-group membership and worktree inclusion; source numeric folder/group IDs cannot be copied into Core object IDs |
| agent region | Preserve source Runtime-type filtering and additionally bind the intended Core member identity; Runtime kind and member ID are different facts |
| conversation | Reference the real Thread and optional private conversation with its actual permissions; open through the current conversation surface |
| custom region | Ordered, deduplicated curated Thread references; grouping/detach changes board placement, not Thread business membership |
| note | Core-owned plain/structured note content and geometry; rendering must use the existing safe Markdown rules, with no implicit model input |
| file | Absolute source path plus current owner/root-grant resolution; path text cannot bypass file-preview authorization |
| terminal | Port `portable-pty` manager behavior: process generation, output sequence, bounded whole-chunk scrollback, resize and real shutdown; PTY process identity remains ephemeral |

Source graph operations include create/update, batch move, collect/group, detach, single/batch
delete and deleted-conversation pruning. Preserve their atomic gesture boundaries, exact board
identity and stale-response protection. The donor's LWW event replay is adapted to Core's
revision/CAS and invalidation snapshot rules; do not run both protocols as parallel authorities.

## Mounted conversation instances

The Renderer now keeps exact card-context instances in `CanvasConversationsController` and maps
stable keyed children through the existing `ViewportPortal`. Each `CanvasThreadWorkspace` uses
the original `ThreadWorkspace` and an independent composer context. A presentation change does
not recreate the editor; the donor's single drawer sits alongside multiple inline instances.
The shared `threadWorkspaceActions` factory contains the original eight handler declarations,
and the original command and send-format helpers remain unchanged. It is not another execution layer.

One `CanvasThreadReader` per unique Thread bridges Core events, Host invalidation and the existing
bounded fallback poll into `ThreadProjectionStore`. Its subscription precedes the reconciliation
read. It never enters a Thread, creates a Conversation or starts a Runtime.
Closing prepares every selected editor before removing any instance; a failed preparation unlocks
all prepared instances. Graph mutations only prepare/unlock affected editors and retain orphaned
instances so that a removed pin does not discard input.

All card providers share one window file resource pool and retention union through the same API
object. Contained decks use local coordinates and scoped DOM IDs; focus selects the native binding
and the file-find surface. Shared Thread preview tabs do not imply shared composer drafts.
Browser checks mount three production card/editor components with in-memory Core and files;
they do not mount the full ReactFlow/BusinessApp or establish real Core/model/platform acceptance.
Notification visibility/ack unions are now wired through the same Attention Controller, with clipping,
occlusion checks and independent per-Thread requests. Mock Core browser checks cover offscreen,
covered and closed cards, source reappearance and a frozen uncertain retry. Real Core notification
acceptance, private attachment/quote isolation and recovery of closed orphan inputs remain required I03 work.

Expanded card contexts now persist in Host/Owner + Core + Board scoped local presentation records.
Only exact validated live graph references restore. Restoring reads existing Threads (fresh per Thread),
checks the original private Conversation and never enters a Thread or restores execution liveness.
Transient leave/suspend retains expansion intent; explicit collapse removes only that context.
Drawer placement remains session-only and reconstruction uses inline cards. Corrupt/future records
and unmatched references are retained; quota failures retain in-memory intent and expose retry.
The restore batch reports progress and can stop without accepting its late reads.

CanvasSurface waits for a valid graph and a completed leave transaction before one restore pass
per mount/retry. Navigation prepares input while keeping the original instances mounted and locked;
only the observed committed destination releases them. Failure, refusal and superseded no-ops
unlock the same editors and preserve undo history. Concurrent navigation attempts share preparation
and release it only after all attempts settle. Metadata dialogs keep their editors; create/delete
commands release affected instances only after an applied receipt, including reconciliation of an
unknown result. An empty selection chooses the next board directly without starting an editor leave.
Renderer quit preparation rolls back if another renderer guard fails; failures after successful native
quit preflight, real Core shutdown and platform qualification remain pending. Node geometry depends on stable context/mode keys,
not changing content elements or visibility reports; project group projections are memoized to avoid
the ReactFlow update loop found by the real mounted canvas fixture.
The fixture mounts CanvasWorkspace/CanvasSurface/ReactFlow/editors with mock Core, observes actual
page reload/localStorage recovery and confirms readonly calls. Native process/crash/Host recovery,
concurrent graph changes, private attachments and scale/platform qualification remain pending.

Mission-backed public cards now reuse MissionIntro and the existing window Mission action owner.
The current Thread projection and Mission list record must agree on both IDs before exposing the
Mission entry. Delivery/activity and cumulative Diff use the existing MissionActivityDocument and
Core missions.delivery/changes/fileDiff paths inside each card's contained preview. Missing or
rebound Missions show unavailable content in retained activity tabs, never a substituted Mission.
Activity source links focus the original card's public message; Task/Mission focus lookup is scoped
to its workspace shell. A private card sends public-source navigation through the existing guarded
full public workspace and focuses only after successful navigation; that private route still needs
full integration qualification. Diff disclosure/tree/modal IDs are per instance, and the viewport portal sits
above selected graph nodes so selection cannot intercept real conversation controls.
These are current Thread-to-Mission links; durable independent Mission/Task/Diff pins, file-node
authorization joins and actual Core/Runtime/end-to-end qualification remain required I03 work.


## Private input handoff in the Renderer

SingleChatPanel binds PrivateInputController to the admitted client, Thread, original Conversation and
exact canvas context. Reads validate scope metadata without registering. Meaningful local edits and
explicit commands persist the body-location index before registration or dispatch. The Core remains
the authority for revisions, retained materials, one FIFO and the edit lease; no parallel execution queue exists.

Full returns and swaps lock the same context's body store while preserving other contexts. A durable
intent freezes the original UUID and complete parameters, then records the outcome before atomically
applying body/receipt markers. Reload performs receipt lookup; only an explicit unknown retry resends.
Late unapplied bodies cannot be paired with newer Core materials. Accepted sends only clear their matching
original body. Native default bodies use the same admitted local recovery storage as canvas bodies.

The mounted three-panel fixture uses a simulated Core and isolated browser storage. Complete retained
selection, mirrored bodies, other-card isolation, reply loss, reload, same-request retry, save/cancel and
takeover are checked there. Actual Host/IPC, multiwindow/crash recovery, scale and final model input
remain unqualified. Scoped upload/source-digest proof, scoped/retained file authorization, explicit legacy
material adoption and orphan recovery UI remain required; card files render names only until authorization lands.

Private attachment locators now distinguish scoped composers, scoped pending edits and retained inputs.
Core resolves the source from its exact Conversation/client/scope row; pending edits also require the current
Pending ID and lease token. A retained ID grants no independent access. These owners cannot fall back to
legacy managed-attachment lookup. Persistent scope ownership survives pin deletion for orphan recovery.
Main's two file entry points preserve the complete descriptor and reject scope data smuggled into an old
default owner. Renderer and Main file identities include the exact scope/retained/lease before caching.
Scoped upload, content-change proof and actual Host resource qualification remain pending; file UI stays gated.

## 私聊源观察（2026-10-16）

Migration206/schema156 按 Conversation/source ref 独立保存内容及对象身份观察，不把摘要加入 SourceRef 或模型输入。
选择材料和显式迁入时记录，发送/排队准入前重检；变化阻断并保留整份输入，队首失效记为 needs_repair。
目录的安全递归观察复用既有句柄和层级/条目边界，公共/Agent live source 及旧冻结 Run 不改。
实现边界见 [私聊输入合同](../contracts/single-chat-composer-scope-v1.md#私聊源观察与接受前检查2026-10-16)。
Native新桥的目录请求和共享Native文件恢复界面已接线；Web恢复接口、原文件重选、真实资源/IPC与迁入/孤立恢复仍待。
共享控制器只呈现平台原日志/回执，不成为字节或模型材料权威；迟到运输错误不能覆盖已查回结果。
边界见[共享文件准备与Native恢复](../contracts/single-chat-composer-scope-v1.md#共享文件准备界面与-native-恢复2026-10-16)。
大材料扫描对事务锁及请求响应的影响须随I03规模检查处理，不能据局部检查宣称高可用完成。


## 独立 Mission、Task 与 Diff 身份

I03-C02 在原 conversation 卡片容器上增加显式 `objectReference`，用只含身份/选择提示的独立子表持久化，
并非另一份 Mission、Task 或 Git 状态。Core 初次写入核对原对象与公开 Thread；对象消失后仍保留卡片，
后续原对象读取再次核对身份与 Thread，不从 Thread 当前 Mission 重新绑定。
Mission Diff 只保存 Mission ID 与可选原文件名，基线、实际 Git、临时 viewId 与权限均属于既有 Mission 服务。
Schema157/Migration207 保留原八类节点表，新增引用表及不可变/归属守卫，与旧节点几何写入共用原命令事务和回执。
新快照能力控制首次对象写入；旧省略、新字段闭合、原编号未知恢复和失败回滚仍须阶段 Core/Host 实测。
后续客户端接线复用同一引用：新增选择器按原 Thread/对象读取，卡片点击后才挂载只读详情；
Mission/Task 的编辑在既有工作区，使用原对象定位与精确 Task 锚定，不另建业务写入或执行层。
Diff 在画布详情中复用原累计变更组件，先读原 Mission 工作区基准，再以当前列表/viewId 打开精确原路径；
多个挂载查看器最后关闭才释放同客户端/Mission 的快照。业务引用从普通对话展开/镜像/分组手势和旧恢复入口中排除。
选择器与详情的隔离模拟证据只证明客户端交互；源157/207、真实Host及实际工作区跳转仍须阶段候选集中运行，C02未收口。

## 文件节点与原文件预览权威

I03-C03 新增独立 `fileReference:{threadId}`：画布保存原公开工程会话与精确路径，具体文件能力继续属于既有Core/Main/Host文件预览链。
`bind_file` 只为旧文件节点明确追加一次来源，不按当前会话、相同路径或上次焦点推断。来源失效保留，不授予目录权限。
Schema158/Migration208 仅追加引用子表与来源/不可变守卫，读投影在原23列加4个业务引用列后追加1列，仍以单查询读取。
创建、旧节点绑定、布局版本、事件和首次结果继续在原Gateway事务中；旧canonical请求新增字段None时省略。
具体合同见[文件节点来源绑定](../contracts/canvas-nodes-v1.md#文件节点来源绑定2026-10-16)。
客户端新建须明确选择原工程 Thread，旧节点通过同一编辑器显式绑定；未知回执保留表单与原选择，核对成功后清除旧运输错误。
`CanvasFilePreview` 将精确绝对路径编码后交给原 `FilePreviewApi.restore`，不直接读取路径、不借用当前工程，也不自动启动系统应用。
固定文件展示与原工作区共用窗口 `FilePreviewResources` 和原 Thread session，但保存各卡自己的 tab ID、DOM scope、查找焦点和源码显示状态。
首次可见才读取；收起/删除撤销展示 owner，不关闭另一处仍使用的共享预览。重新展开再次核对来源，内容变化先保留旧正文并提示，显式刷新才换内容。
文件中的关联链接沿原句柄打开原 Thread 工作区并传递定位目标；根文件固定不变，迟到导航不能抢走新的页面意图。
HTML 展示含同句柄镜像计入原窗口上限；后台冷恢复只走预览，不触发系统打开。文件头删除固定本行目标，工具栏删除读取即时选中项，确认框列出路径。
隔离界面已挂载生产画布与原读者，模拟 Core/文件验证选择、未知恢复、变化、失效、收起/重开和删除；
当前158/208实际 Rust、Host/Native IPC、真实 HTML、完整应用导航和规模仍须阶段候选验证，代码就绪不等同工程通过。

## 临时终端移植基础（2026-10-16）

`canvas_terminal` 拥有临时会话来源与代次，来源只从存活画板的 terminal 节点读取；外部请求只包含原 board/node，
不接收 cwd、shell、进程 ID 或文件 RootGrant。目录是启动位置，不能宣称 OS 沙箱；可信 Owner、实际目录和删除竞争仍需启动入口校验。
状态将 Starting、Running、Stopping、Exited、Failed 分开：请求停止立即拒绝迟到启动提交和输入，只有实际回收结果才记录退出。
重启使用新代次，旧代次的输入、resize 与游标不能作用于新会话。原请求 ID 保留用于后续启动恢复，不能在丢回执后默发新启动。

输出按最多8192字节的原始块累积，保留最近128KiB且最多2048块；整块淘汰，回放同时返回起始/截止序号和缺口。
读取重叠按同一锁内游标消除，中文或转义序列跨块不先进行有损解码。缩放只在实际PTY操作成功后更新记录。

Codeg供体与portable-pty0.8.1的直接child kill不能单独证明进程树回收。PTY字节辅助进程因此必须通过现有
ManagedProcess启动，复用Windows原Job归属和Unix既有身份回收；遵循现有[受管进程决策V1.72-D26](../versions/v1.72/decisions.md)边界。
辅助进程只处理一次启动和有界stdin/stdout帧，不打开数据库、不管理画布引用、不启动AgentRun或模型；实际业务会话仍由Core管理。
父端未读时输出管道反压，输入队列最多8帧，单次输入16KiB；EOF、root退出和输出排空分别观察，不把它们冒充整树回收。

当前只完成闭合请求、来源查询、纯状态与辅助进程源码；公共方法/能力、CLI入口、父端受管进程、节点/画板删除、计划退出和终端读者尚未接线。
纯状态源单独编译通过不代表辅助进程或完整Core编译通过；完整PTY/Host/平台必须在阶段候选实际验证，C04保持开发中。
