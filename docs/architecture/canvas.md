---
document_type: architecture
authority: integration-canvas-boundaries
last_updated: 2026-10-15
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
完整保留项与回执同事务，引用复用原捕获和模型投影。新 get/mutate/recovery 的 Application/Host/客户端仍待接线，
上传准备、编辑接管/完整移回及本机意图恢复继续，不能据此暴露 scopeVersion 能力或宣称完整隔离功能完成。

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
