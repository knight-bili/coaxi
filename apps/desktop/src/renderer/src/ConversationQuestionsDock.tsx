import { useEffect, useRef, useState } from 'react'
import {CircleHelp,ChevronDown} from 'lucide-react'
import type { ConversationQuestionOutcome, ConversationQuestionSet, ConversationRequestStatus, ConversationToolScope, StoredCommandResult } from '@contracts'
import { newCommandId } from '../../shared/command-id'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { readErrorMessage } from './error-message'
import { conversationOriginal, conversationToolRequest, type ConversationOriginal } from './conversation-tools-client'
import { AskQuestionCard } from './coaxi-native-content/components/chat/ask-question-card'
import type { QuestionAnswer } from './coaxi-native-content/lib/types'
import './coaxi-native-content/source-rich.css'

const stateText: Record<ConversationQuestionSet['state'], string> = { awaiting_answer: '等待你回答', answered: '答案已保存，等待原连接确认', delivered: '已写回原队员连接', cancelled: '原问题已停止，记录保留', outcome_unknown: '原连接结果待核对，不会自动重发' }
function Question({ value, name, onChanged }: { value: ConversationQuestionSet; name: string; onChanged(): void }): React.JSX.Element {
  const client = useThreadClient(), t = useUiText(), generation = useRef(0), locked = useRef(false)
  const [pending, setPending] = useState<ConversationOriginal | null>(null), [ready, setReady] = useState(false), [busy, setBusy] = useState(false), [error, setError] = useState(''), [status, setStatus] = useState<ConversationRequestStatus | null>(null)
  const key = `conversation-question:${client.presentationScope ?? ''}:${value.requestId}`
  useEffect(() => {
    const observed = ++generation.current; setReady(false); setPending(null); setStatus(null); setError(''); locked.current = false; setBusy(false)
    void conversationOriginal(client, key).then(original => { if (observed === generation.current) { setPending(original); setReady(true) } }).catch(reason => { if (observed === generation.current) setError(readErrorMessage(reason)) })
    return () => { generation.current++ }
  }, [client, key])
  const answer = async (_id: string, answer: QuestionAnswer): Promise<void> => {
    if (locked.current || !ready || pending || value.state !== 'awaiting_answer') throw new Error(t('请先核对原问题。'))
    locked.current = true; setBusy(true); setError(''); const observed = generation.current
    const original: ConversationOriginal = { commandId: newCommandId(), requestId: value.requestId, operation: answer.declined ? 'conversationTools.questions.decline' : 'conversationTools.questions.answer' }
    try {
      await conversationOriginal(client, key, original); if (observed !== generation.current) return; setPending(original)
      const result = answer.declined
        ? await conversationToolRequest(client, 'conversationTools.questions.decline', { commandId: original.commandId, requestId: value.requestId, expectedRevision: value.revision })
        : await conversationToolRequest(client, 'conversationTools.questions.answer', { commandId: original.commandId, requestId: value.requestId, expectedRevision: value.revision, answers: answer.answers })
      if (observed !== generation.current) return
      await conversationOriginal(client, key, null); if (observed === generation.current) { setPending(null); setStatus(result); onChanged() }
    } catch (reason) { if (observed === generation.current) setError(readErrorMessage(reason)); throw reason }
    finally { if (observed === generation.current) { locked.current = false; setBusy(false) } }
  }
  const reconcile = async (): Promise<void> => {
    if (locked.current) return
    locked.current = true; setBusy(true); setError(''); const observed = generation.current, original = pending
    try {
      if (original) {
        const receipt = await client.request<{ state: string; result?: StoredCommandResult }>('commands.reconcile', { operation: original.operation, params: { commandId: original.commandId } })
        if (observed !== generation.current) return
        if (receipt.state === 'recorded' && receipt.result?.commandId === original.commandId && receipt.result.commandType === original.operation) {
          await conversationOriginal(client, key, null); if (observed !== generation.current) return; setPending(null)
          if (receipt.result.status === 'rejected') setError(receipt.result.code)
        } else setError(t('原回答尚无确定回执，保留原编号；没有重复发送。'))
      }
      const result = await conversationToolRequest(client, 'conversationTools.questions.reconcile', { requestId: value.requestId })
      if (observed === generation.current) { setStatus(result); onChanged() }
    } catch (reason) { if (observed === generation.current) setError(readErrorMessage(reason)) }
    finally { if (observed === generation.current) { locked.current = false; setBusy(false) } }
  }
  const cancel = async (): Promise<void> => {
    if (locked.current) return
    locked.current = true; setBusy(true); setError(''); const observed = generation.current
    const original: ConversationOriginal = { operation: 'conversationTools.requests.resolve', commandId: newCommandId(), requestId: value.requestId }
    try {
      await conversationOriginal(client, key, original); if (observed !== generation.current) return; setPending(original)
      await conversationToolRequest(client, 'conversationTools.requests.resolve', { commandId: original.commandId, requestId: value.requestId, action: 'cancel_remaining' })
      if (observed !== generation.current) return; await conversationOriginal(client, key, null); if (observed === generation.current) { setPending(null); onChanged() }
    } catch (reason) { if (observed === generation.current) setError(readErrorMessage(reason)) }
    finally { if (observed === generation.current) { locked.current = false; setBusy(false) } }
  }
  const outcome = status?.outcome && 'answers' in status.outcome ? status.outcome as ConversationQuestionOutcome : null
  const seed = Object.fromEntries(value.questions.map((question, index) => {
    const selected = outcome?.answers[index]?.selected ?? [], labels = new Set(question.spec.options.map(option => option.label))
    return [question.id, { chosen: selected.filter(label => labels.has(label)), otherText: selected.filter(label => !labels.has(label)).join('\n') }]
  }))
  return <section className="feedback-history-item">
    <div><strong>{name}</strong><span> · {t(stateText[value.state])}</span></div>
    <div className="coaxi-native-rich"><AskQuestionCard key={`${value.requestId}:${value.revision}:${status?.state ?? ''}`} question={{ question_id: value.requestId, created_at: value.createdAt, questions: value.questions.map(question => ({ id: question.id, question: question.spec.question, header: question.spec.header, multi_select: question.spec.multiSelect, options: question.spec.options })) }} onAnswer={answer} readOnly={!ready || !!pending || busy || value.state !== 'awaiting_answer'} initialSelections={seed} title={t(value.state === 'awaiting_answer' ? '队员需要你的选择' : '原问题记录')} subtitle={t(stateText[value.state])} /></div>
    {outcome?.declined && <p>{t('你已明确选择不回答。')}</p>}{error && <p role="alert" className="feedback-error">{error}</p>}
    <div className="feedback-dock-actions"><button type="button" className="quiet-button" disabled={busy} onClick={() => void reconcile()}>{t(pending ? '核对原回答' : '查看原回执')}</button>{['awaiting_answer', 'outcome_unknown'].includes(value.state) && <button type="button" className="quiet-button" disabled={busy} onClick={() => void cancel()}>{t('取消这组问题的剩余投递')}</button>}</div>
  </section>
}
/** 问题与普通消息输入分别保存；卡片直接复用固定Coaxi原组件。 */
export function ConversationQuestionsDock({ scope, members }: { scope: ConversationToolScope; members: Array<{ agentId: string; displayName: string }> }): React.JSX.Element | null {
  const client = useThreadClient(), t = useUiText(), generation = useRef(0), sequence = useRef(0)
  const [page, setPage] = useState<{ items: ConversationQuestionSet[]; next: string | null }>({ items: [], next: null }), [error, setError] = useState('')
  const scopeKey = JSON.stringify(scope)
  const read = async (more = false): Promise<void> => {
    const observed = generation.current, request = ++sequence.current
    try { const result = await conversationToolRequest(client, 'conversationTools.questions.list', { scope, after: more ? page.next : null, limit: 50 }); if (observed === generation.current && request === sequence.current) { setPage(old => more ? { ...result, items: [...old.items, ...result.items.filter(item => !old.items.some(previous => previous.requestId === item.requestId))] } : result); setError('') } }
    catch (reason) { if (observed === generation.current && request === sequence.current) setError(readErrorMessage(reason)) }
  }
  useEffect(() => { generation.current++; setPage({ items: [], next: null }); setError(''); void read(); return () => { generation.current++ } }, [client, scopeKey])
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | null = null
    const refresh = (): void => { if (!timer && !document.hidden) timer = setTimeout(() => { timer = null; void read() }, 250) }
    const off = client.onEvent?.(event => { if (event.method === 'conversationTools.changed' || event.method === 'agent_run.terminal') refresh() }), invalidated = client.onInvalidated?.(refresh)
    window.addEventListener('focus', refresh)
    return () => { off?.(); invalidated?.(); window.removeEventListener('focus', refresh); if (timer) clearTimeout(timer) }
  }, [client, scopeKey])
  const active = page.items.filter(value => value.state === 'awaiting_answer' || value.state === 'answered' || value.state === 'outcome_unknown'), history = page.items.filter(value => !active.includes(value))
  const [open,setOpen] = useState(false)
  const activeIds = active.map(value=>value.requestId).join(',')
  useEffect(()=>{setOpen(false)},[scopeKey])
  useEffect(()=>{if(activeIds)setOpen(true)},[scopeKey,activeIds])
  if (!page.items.length && !error) return null
  const card = (value: ConversationQuestionSet): React.JSX.Element => <Question key={value.requestId} value={value} name={members.find(member => member.agentId === value.agentId)?.displayName ?? value.agentId} onChanged={() => void read()} />
  return <details className="feedback-dock conversation-questions-dock" open={open} onToggle={event=>setOpen(event.currentTarget.open)}><summary><CircleHelp size={14}/><span>{t('队员提问')}</span>{error?<small className="feedback-tool-issue">{t('读取失败')}</small>:!!active.length&&<small>{active.length}</small>}<ChevronDown size={13} className="feedback-tool-chevron"/></summary><div className="feedback-dock-body" aria-label={t('队员提问')}>{active.map(card)}{!!history.length && <details><summary>{t('已结束的问题记录')}</summary>{history.map(card)}</details>}{error && <div role="alert" className="feedback-error"><p>{t('暂时无法读取队员提问，请重试。')}</p><details><summary>{t('查看原因')}</summary><p>{error}</p></details><button type="button" className="quiet-button" onClick={() => void read()}>{t('重新读取')}</button></div>}{page.next && <button type="button" className="quiet-button" onClick={() => void read(true)}>{t('更多问题记录')}</button>}</div></details>
}
