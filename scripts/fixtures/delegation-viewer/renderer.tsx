import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import * as Dialog from '@radix-ui/react-dialog'
import type { ActionApprovalView, AgentRunExecutionEvidenceView, CoreEvent, DelegationConversationView, RuntimeInteractionView, SingleChatRunView, StoredCommandResult } from '@contracts'
import { ThreadClientProvider, type ThreadClient } from '../../../apps/desktop/src/renderer/src/camp-client'
import { DelegationChildren, DelegationViewerHost } from '../../../apps/desktop/src/renderer/src/DelegationViewer'
import { childChatFixture, chatMembers } from './child-chat'
import '../../../apps/desktop/src/renderer/src/styles.css'

const at = '2026-10-18T00:00:00Z'
const run = (n: number): SingleChatRunView => ({ id: `run-${n}`, executionEpoch: 1, status: 'succeeded', createdAt: at, startedAt: at, endedAt: at,
  threadTurnId: `turn-${n}`, triggerConversationMessageId: `input-${n}`, version: 1, cancelRequestedAt: null, lastErrorCode: null,
  finalConversationMessageId: `reply-${n}`, executionEvidenceCount: 1, executionEvidenceChangeSequence: 1 })
const evidence = (n: number): AgentRunExecutionEvidenceView => ({ id: `evidence-${n}`, agentRunId: `run-${n}`, executionEpoch: 1, sequence: 1,
  eventType: 'activity.completed', kind: 'command', phase: 'completed', payload: { item: { id: `command-${n}`, type: 'commandExecution', command: `printf 'CHILD_${n}_ONLY'`, status: 'completed', output: `RESULT_${n}_ONLY` } },
  contentBlobId: null, contentByteCount: 15, isTruncated: false, occurredAt: at })
const approval: ActionApprovalView = { id: 'approval-1', actionId: 'action-1', actionKind: 'shell_command', actionSummary: 'Read child workspace', reason: null,
  canonicalInput: { command: 'ls child-only' }, agentRunId: 'run-1', agentId: 'owner-a', adapterKind: 'codex-cli', nativeMethod: 'session/request_permission',
  requestDigest: 'request-1', permissionSemantics: 'runtime_managed_v2', status: 'pending', requestedForUserId: 'local_user',
  resolvedByType: null, resolvedById: null, resolutionCode: null, version: 3, requestedAt: at, resolvedAt: null,
  options: [{ optionId: 'allow-child-1', kind: 'allow_once', label: 'Allow exact child once', consequence: '', nativeResponseDigest: 'response-1' }] }
const makeDetail = (n: number): DelegationConversationView => ({ delegationId: `delegation-${n}`, threadId: 'thread-a', ownerAgentId: 'owner-a',
  rootConversationId: 'parent-a', parentConversationId: n === 3 ? 'child-1' : 'parent-a', parentDelegationId: n === 3 ? 'delegation-1' : null,
  conversationId: `child-${n}`, depth: n === 3 ? 2 : 1, currentAttemptNo: 1, controlVersion: 1, takenOver: false, agentRunId: `run-${n}`, executionEpoch: 1,
  status: 'succeeded', agentType: n === 2 ? 'claude_code' : 'codex', createdAt: at, startedAt: at, endedAt: at, cleanupPending: false,
  ...(n === 1 ? { blockedOn: { kind: 'permission' as const, request_id: 'approval-1', title: 'Read child workspace' } } : {}),
  activity: { runs: [run(n)], executionEvidence: [evidence(n)], approvals: n === 1 ? [approval] : [] } })
let details = [makeDetail(1), makeDetail(2), makeDetail(3)]
details[0] = { ...details[0], status: 'waiting', endedAt: null, activity: { ...details[0].activity,
  runs: [{ ...details[0].activity.runs[0], status: 'waiting', endedAt: null, finalConversationMessageId: null }] } }
let requests: Array<{ method: string; params: unknown; host: number }> = []
let permissionAnswers: unknown[] = []
let question: RuntimeInteractionView = { id: 'question-child-1', conversationId: 'child-1', kind: 'question', status: 'pending', version: 1, answer: null, closeReason: null,
  body: { kind: 'question', questions: [{ id: 'scope', header: '范围', question: '只处理这个帮手的目录吗？', multiSelect: false, options: [{ label: '只处理 child-1 (Recommended)', description: '保持其他会话材料不变。' }] }] } }
const events = [new Set<(event: CoreEvent) => void>(), new Set<(event: CoreEvent) => void>()]
const emit = (n = 1): void => events.forEach(set => set.forEach(listener => listener({ method: 'delegation.changed', params: { threadId: 'thread-a', delegationId: `delegation-${n}`, conversationId: `child-${n}` } })))
let fail = false, hold = false, delayed: (() => void) | null = null
let showRows: (show: boolean) => void = () => {}, hostChange: (host: number) => void = () => {}, changeParent: (parent: string) => void = () => {}, showSurface: (show: boolean) => void = () => {}
let leaveGuard: (() => { complete(didLeave: boolean): void }) | null = null
const chat = childChatFixture(() => details[0], detail => { details[0] = detail }, () => emit())
const clients = [0, 1].map(host => ({ platform: 'win32', onEvent: (listener: (event: CoreEvent) => void) => { events[host].add(listener); return () => { events[host].delete(listener) } },
  editingRecovery: chat.editingRecovery, singleChatAttachments: chat.attachments,
  attachments: { kind: 'download', download: async () => ({ opened: false }), preview: async () => null },
  request: async (method: string, params: unknown) => {
    requests.push({ method, params: structuredClone(params), host })
    const reply = await chat.request(method, params)
    if (reply.handled) return reply.value
    if (method === 'delegations.list') {
      const parent = (params as { parentConversationId: string }).parentConversationId
      if (parent === 'unavailable-parent') throw Error('The new parent is unavailable')
      return { parentConversationId: parent, items: details.filter(detail => detail.parentConversationId === parent).map(({ activity: _, ...detail }) => detail) }
    }
    if (method === 'delegations.get') {
      if (fail) throw new Error('offline')
      const detail = details.find(detail => detail.delegationId === (params as { delegationId: string }).delegationId)!
      if (hold) { hold = false; await new Promise<void>(resolve => { delayed = resolve }) }
      return structuredClone(detail)
    }
    if (method === 'delegations.history') {
      const n = Number((params as { task_id: string }).task_id.split('-').at(-1))
      return { task_id: `delegation-${n}`, conversation_id: `child-${n}`, messages: [
        { id: `input-${n}`, sequence: 1, author_type: 'delegation_parent', body: `任务 ${n}：核对自己的材料。HOST_${host}`, created_at: at },
        ...(details[n - 1].status === 'succeeded' ? [{ id: `reply-${n}`, sequence: 2, author_type: 'assistant', body: `帮手 ${n} 已保存完整结果。\n\n${'保留完整的会话正文。'.repeat(n === 2 ? 180 : 1)}`, attachments: n === 1 ? [{ id: 'final-file-1', name: 'child-1-result.txt', kind: 'file' }] : [], created_at: at }] : [])
      ] }
    }
    if (method === 'runtime.interactions.list') {
      const conversationId = (params as { conversationId: string }).conversationId
      return { conversationId, items: conversationId === 'child-1' ? [{ ...question, createdAt: at }] : [], nextCursor: null }
    }
    if (method === 'runtime.interactions.get') return structuredClone(question)
    if (method === 'runtime.interactions.resolve') {
      const { commandId, command } = params as { commandId: string; command: { answer: RuntimeInteractionView['answer'] } }
      question = { ...question, answer: command.answer, status: 'resolved', version: 2 }; emit()
      return { commandId, commandType: method, status: 'applied', code: 'interaction.resolved', payload: {}, resultEntity: null, recordedAt: at } satisfies StoredCommandResult
    }
    if (method === 'agentRunExecution.page') return { blocks: [], runtimePhase: 'executing', runtimeThinkingTitle: null }
    throw new Error('Unexpected request ' + method)
  }
} as ThreadClient))
function Fixture(): React.JSX.Element {
  const [rows, setRows] = useState(true), [host, setHost] = useState(0), [parent, setParent] = useState('parent-a'), [visible, setVisible] = useState(true)
  showRows = setRows; hostChange = setHost; changeParent = setParent; showSurface = setVisible
  return <ThreadClientProvider client={clients[host]}><DelegationViewerHost threadId="thread-a" members={chatMembers} visible={visible} onLeaveGuardChange={guard => { leaveGuard = guard }} profileById={new Map()} busy={false} onResolveApproval={(approval, optionId) => {
    permissionAnswers.push({ id: approval.id, runId: approval.agentRunId, version: approval.version, optionId })
    details[0] = { ...details[0], blockedOn: { kind: 'question', request_id: 'question-child-1' }, activity: { ...details[0].activity, approvals: [] } }; emit()
  }}>
    <main style={{ padding: 16, maxWidth: 390 }}><h2>父会话</h2><input id="parent-input" aria-label="父会话输入" placeholder="父会话仍可操作" style={{ width: 240 }} />
      <Dialog.Root><Dialog.Trigger id="outside">父会话操作</Dialog.Trigger><Dialog.Portal><Dialog.Overlay style={{ position: 'fixed', inset: 0, zIndex: 100 }} /><Dialog.Content aria-describedby={undefined} style={{ position: 'fixed', top: 60, left: 30, zIndex: 101 }}><Dialog.Title>父页面的模态窗口</Dialog.Title></Dialog.Content></Dialog.Portal></Dialog.Root>
      {rows && <DelegationChildren parentConversationId={parent} />}
    </main>
  </DelegationViewerHost></ThreadClientProvider>
}
createRoot(document.getElementById('root')!).render(<Fixture />)
Object.assign(window, { viewerTest: {
  settle: () => new Promise(resolve => setTimeout(resolve, 550)),
  rows: showRows, setRows: (value: boolean) => showRows(value), host: (value: number) => hostChange(value), parent: (value: string) => changeParent(value),
  fail: (value: boolean) => { fail = value }, hold: () => { hold = true }, release: () => delayed?.(), refresh: () => emit(),
  chat, visible: (value: boolean) => showSurface(value),
  prepareLeave: () => { try { const preparation = leaveGuard?.(); preparation?.complete(false); return true } catch { return false } },
  finish: () => {
    question = { ...question, status: 'sent', version: question.version + 1 }
    details[0] = { ...details[0], status: 'succeeded', endedAt: at, blockedOn: undefined, activity: { ...details[0].activity, runs: [run(1)] } }; emit()
    events.forEach(set => set.forEach(listener => listener({ method: 'runtime.interaction.changed', params: { threadId: 'thread-a', conversationId: 'child-1', interactionId: question.id, version: question.version } })))
  },
  theme: (value: string) => { document.documentElement.dataset.theme = value },
  snapshot: () => ({ requests, permissionAnswers, dialogs: document.querySelectorAll('.delegation-drawer[data-open]').length,
    dialogTexts: [...document.querySelectorAll('.delegation-drawer')].map(node => node.textContent), active: document.activeElement?.getAttribute('aria-label') ?? document.activeElement?.className,
    overflowX: document.documentElement.scrollWidth > window.innerWidth,
    bounds: [...document.querySelectorAll('.delegation-drawer')].map(node => node.getBoundingClientRect().toJSON()),
    body: [...document.querySelectorAll<HTMLElement>('.delegation-drawer-body')].map(node => ({ height: node.clientHeight, scrollHeight: node.scrollHeight })),
    listeners: events.map(set => set.size)
  })
} })
