import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import type { RuntimeInteractionView, ResolveRuntimeInteractionParams, StoredCommandResult, CoreEvent } from '@contracts'
import { ThreadClientProvider, type ThreadClient } from '../../../apps/desktop/src/renderer/src/camp-client'
import { RuntimeInteractionDock } from '../../../apps/desktop/src/renderer/src/RuntimeInteractionDock'
import '../../../apps/desktop/src/renderer/src/styles.css'

const question = (id = 'question-1'): RuntimeInteractionView => ({ id, conversationId: 'private-a', kind: 'question', status: 'pending', version: 1, answer: null, closeReason: null,
  body: { kind: 'question', questions: [
    { id: 'scope', header: '项目范围', question: '这次先处理哪一部分？', multiSelect: false, options: [
      { label: '先修复当前问题 (Recommended)', description: '保持现有功能，先处理已经确认的问题。' }, { label: '__other__', description: '这是队员提供的真实选项，仍须保留原文。' }
    ] },
    { id: 'checks', header: '验证方式', question: '完成后需要做哪些检查？', multiSelect: true, options: [
      { label: '行为测试', description: '检查实际输入和输出。' }, { label: '界面对照', description: '比较原应用与融合后的界面。' }
    ] }
  ] } })
const plan = (empty = false): RuntimeInteractionView => ({ id: 'plan-1', conversationId: 'private-a', kind: 'plan_approval', status: 'pending', version: 1, answer: null, closeReason: null,
  body: { kind: 'plan_approval', planMarkdown: empty ? '' : '# 本次实施计划\n\n先确认文件范围，再调整代码。\n\n' + Array.from({ length: 24 }, (_, n) => `${n + 1}. 保留原功能，完成第 ${n + 1} 项行为检查。`).join('\n') } })
let current = question()
let requests: Array<{ method: string; params: unknown }> = []
let intent: ResolveRuntimeInteractionParams | undefined
let receipt: StoredCommandResult | undefined
let complete: ((value: StoredCommandResult) => void) | undefined
let fail: ((reason: Error) => void) | undefined
const events = new Set<(event: CoreEvent) => void>()
const retained = new Map<string, RuntimeInteractionView>()
let queued: RuntimeInteractionView[] = []
let deleted = false
const all = <T extends HTMLElement = HTMLElement>(selector: string): T[] => [...document.querySelectorAll<T>(selector)].filter(node => !node.closest('[hidden]'))
const query = <T extends HTMLElement = HTMLElement>(selector: string): T | undefined => all<T>(selector)[0]
let setScope: (scope: string) => void = () => {}
let setHost: (host: number) => void = () => {}
let visible: (visible: boolean) => void = () => {}
const emit = (): void => events.forEach(listener => listener({ method: 'runtime.interaction.changed', params: { conversationId: current.conversationId, interactionId: current.id, version: current.version } }))
function makeClient(): ThreadClient {
  return { platform: 'win32', onEvent: listener => { events.add(listener); return () => { events.delete(listener) } }, request: async (method, params) => {
    requests.push({ method, params: structuredClone(params) })
    if (method === 'runtime.interactions.list') return { conversationId: (params as { conversationId: string }).conversationId, items: deleted ? [] : [current, ...queued].map(view => ({ ...view, createdAt: '' })), nextCursor: null }
    if (method === 'runtime.interactions.get') { const id = (params as { interactionId: string }).interactionId; return deleted ? null : structuredClone(id === current.id ? current : retained.get(id) ?? null) }
    if (method === 'commands.reconcile') return receipt ? { state: 'recorded', result: receipt } : { state: 'unknown' }
    if (method === 'runtime.interactions.resolve') {
      intent = structuredClone(params as ResolveRuntimeInteractionParams)
      return await new Promise<StoredCommandResult>((resolve, reject) => { complete = resolve; fail = reject })
    }
    throw new Error('Unexpected operation ' + method)
  } } as ThreadClient
}
const clients = [makeClient(), makeClient()]
function Fixture(): React.JSX.Element {
  const [scope, changeScope] = useState('private-a')
  const [host, changeHost] = useState(0)
  const [show, changeVisible] = useState(true)
  setScope = changeScope; setHost = changeHost; visible = changeVisible
  return <ThreadClientProvider client={clients[host]}><main id="fixture-layout" style={{ width: 'min(760px, calc(100vw - 24px))', margin: '20px auto', minWidth: 0 }}>
    <RuntimeInteractionDock conversationId={scope} sourceLabel="开发队员" visible={show} />
    <textarea id="composer" aria-label="普通输入" placeholder="继续输入…" style={{ width: '100%', minHeight: 60 }} />
  </main></ThreadClientProvider>
}
createRoot(document.getElementById('root')!).render(<Fixture />)
Object.assign(window, { interactionTest: {
  settle: () => Promise.race([new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(resolve, 20)))), new Promise(resolve => setTimeout(resolve, 150))]),
  reset: (kind = 'question', single = false) => {
    queued = []
    deleted = false
    current = kind === 'question' ? question() : plan(kind === 'empty-plan')
    if (single && current.body.kind === 'question') current.body.questions = current.body.questions.slice(0, 1)
    intent = undefined; receipt = undefined; complete = undefined; fail = undefined; requests = []
    clients.push(makeClient()); setHost(clients.length - 1)
  },
  replace: () => { retained.set(current.id, structuredClone(current)); current = question('question-2'); emit() },
  queue: () => { queued.push(structuredClone(current)); retained.set(current.id, structuredClone(current)); current = question('question-queued'); emit() },
  deleteRequest: () => { deleted = true; emit() },
  nextAfterDelete: () => { deleted = false; current = question('next-question'); emit() },
  host: () => setHost(1),
  hide: () => visible(false), show: () => visible(true),
  fail: () => fail?.(new Error('transport lost')),
  record: (status: 'resolved' | 'sent' | 'delivery_unknown' | 'closed' = 'resolved') => {
    current = { ...current, status, version: current.version + 1, answer: intent?.command.answer ?? current.answer }
    if (intent) receipt = { commandId: intent.commandId, commandType: 'runtime.interactions.resolve', status: 'applied', code: 'interaction.resolved', payload: {}, resultEntity: null, recordedAt: '' }
    emit()
  },
  complete: () => { if (receipt) complete?.(receipt) },
  theme: (theme: string) => { document.documentElement.dataset.theme = theme },
  query, all,
  snapshot: () => ({ requests, intent, text: (() => { const node = document.getElementById('fixture-layout')!.cloneNode(true) as HTMLElement; node.querySelectorAll('[hidden]').forEach(child => child.remove()); return node.textContent })(), listeners: events.size,
    checked: all<HTMLInputElement>('.native-option input:checked').map(input => input.value),
    input: query<HTMLInputElement>('.native-answer-text')?.value,
    feedback: query<HTMLTextAreaElement>('.native-plan-changes textarea')?.value,
    body: query('.native-plan-body') ? { height: query('.native-plan-body')!.clientHeight, scrollHeight: query('.native-plan-body')!.scrollHeight } : null,
    pageOverflowX: document.documentElement.scrollWidth > window.innerWidth,
    questionBody: query('.native-question-body[data-state=active], .native-question-body:not([data-state])') ? {
      height: query('.native-question-body[data-state=active], .native-question-body:not([data-state])')!.clientHeight,
      scrollHeight: query('.native-question-body[data-state=active], .native-question-body:not([data-state])')!.scrollHeight
    } : null,
    bounds: query('.native-question-card, .native-plan-card')?.getBoundingClientRect().toJSON(),
    actions: all<HTMLButtonElement>('.native-card-actions button').map(button => ({ text: button.textContent, disabled: button.disabled, bounds: button.getBoundingClientRect().toJSON() }))
  })
} })
