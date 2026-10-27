// Adapted from Coaxi aff1e6c278453ed8d0b96a5f664edcdca621aa3f:
// src/components/chat/{ask-question-card,plan-approval-card}.tsx and
// src/lib/ask-question.ts (splitRecommended). Apache-2.0: integration/licenses/component-a/LICENSE.
import { useEffect, useId, useState, useSyncExternalStore } from 'react'
import * as Tabs from '@radix-ui/react-tabs'
import { Check, ChevronDown, ChevronRight, ChevronUp, ListTodo, Loader2, MessageCircleQuestionMark } from 'lucide-react'
import type { RuntimeInteractionAnswer, RuntimeInteractionView } from '@contracts'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { SafeMarkdown } from './SafeMarkdown'
import { runtimeInteractionController } from './runtime-interaction-controller'
import './runtime-interactions.css'

type Question = Extract<RuntimeInteractionView['body'], { kind: 'question' }>['questions'][number]
type PickState = { chosen: string[]; otherActive: boolean; otherText: string }
type Picks = Record<string, PickState>
const emptyPick = (): PickState => ({ chosen: [], otherActive: false, otherText: '' })
function splitRecommended(label: string): { text: string; recommended: boolean } {
  const text = label.match(/^(.*?)\s*\(recommended\)\s*$/i)?.[1].trim()
  return text ? { text, recommended: true } : { text: label, recommended: false }
}
const answered = (state?: PickState): boolean => Boolean(state && (state.chosen.length || (state.otherActive && state.otherText.trim())))
function recordedPicks(view: RuntimeInteractionView): Picks {
  if (view.body.kind !== 'question' || view.answer?.kind !== 'question') return {}
  const answers = view.answer.answers
  return Object.fromEntries(view.body.questions.map(question => {
    const labels = answers.find(answer => answer.questionId === question.id)?.labels ?? []
    const chosen = labels.filter(label => question.options.some(option => option.label === label))
    const otherText = labels.filter(label => !question.options.some(option => option.label === label)).join('\n')
    return [question.id, { chosen, otherText, otherActive: Boolean(otherText) }]
  }))
}
type CardProps = { view: RuntimeInteractionView; busy: boolean; onAnswer(answer: RuntimeInteractionAnswer): void }
export function NativeQuestionCard(props: CardProps): React.JSX.Element | null {
  return <QuestionCardContent key={props.view.id} {...props} />
}
function QuestionCardContent({ view, busy, onAnswer }: CardProps): React.JSX.Element | null {
  const t = useUiText()
  const prefix = useId()
  const questions = view.body.kind === 'question' ? view.body.questions : []
  const [draft, setDraft] = useState<Picks>({})
  const [activeId, setActiveId] = useState(questions[0]?.id ?? '')
  const [collapsed, setCollapsed] = useState(false)
  const readOnly = view.status !== 'pending'
  const state = readOnly ? recordedPicks(view) : draft
  const locked = busy || readOnly
  const count = questions.filter(question => answered(state[question.id])).length
  const multi = questions.length > 1
  const activeIndex = questions.findIndex(question => question.id === activeId)
  const next = questions[activeIndex + 1]
  const bodyHeight = `min(${Math.max(1, ...questions.map(q => q.options.length + 1)) * 4 + 1}rem, 50svh)`
  const change = (question: Question, edit: (value: PickState) => PickState): void => {
    setDraft(previous => ({ ...previous, [question.id]: edit(previous[question.id] ?? emptyPick()) }))
  }
  const select = (question: Question, label: string): void => {
    change(question, value => ({ ...value, chosen: question.multiSelect
      ? value.chosen.includes(label) ? value.chosen.filter(item => item !== label) : [...value.chosen, label]
      : [label], otherActive: question.multiSelect && value.otherActive }))
    if (!question.multiSelect) {
      const following = questions[questions.findIndex(item => item.id === question.id) + 1]
      if (following) setActiveId(following.id)
    }
  }
  const other = (question: Question): void => change(question, value => ({ ...value, otherActive: !value.otherActive,
    chosen: question.multiSelect ? value.chosen : [] }))
  const renderQuestion = (question: Question): React.JSX.Element => {
    const value = state[question.id] ?? emptyPick()
    const type = question.multiSelect ? 'checkbox' : 'radio'
    const textInput = <input className="native-answer-text" aria-label={question.options.length ? t('其他回答') : question.question}
      value={value.otherText} disabled={locked} autoFocus={!readOnly && question.options.length > 0}
      placeholder={t('填写你的回答…')} onChange={event => change(question, current => ({ ...current, otherActive: true, otherText: event.target.value }))} />
    return <div className="native-question-content">
      <div className="native-question-heading"><span className="native-badge">{t(question.multiSelect ? '多选' : '单选')}</span><p>{question.question}</p></div>
      {question.options.length === 0 ? textInput : <>
        <div className="native-options" role="group" aria-label={question.question}>
          {question.options.map((option, index) => {
            const selected = value.chosen.includes(option.label)
            const display = splitRecommended(option.label)
            return <label key={index} className={`native-option${selected ? ' is-selected' : ''}${locked ? ' is-locked' : ''}`}>
              <input type={type} name={`${prefix}:${question.id}`} value={String(index)} checked={selected} disabled={locked}
                onClick={event => { if (!question.multiSelect && selected) { event.preventDefault(); change(question, current => ({ ...current, chosen: [] })) } }}
                onChange={() => select(question, option.label)} />
              <span className="native-option-body"><span>{display.text}{display.recommended && <span className="native-badge recommended">{t('推荐')}</span>}</span>
                {option.description && <small>{option.description}</small>}</span>
            </label>
          })}
          <label className={`native-option${value.otherActive ? ' is-selected' : ''}${locked ? ' is-locked' : ''}`}>
            <input type={type} name={`${prefix}:${question.id}`} value="__other__" checked={value.otherActive} disabled={locked}
              onClick={event => { if (!question.multiSelect && value.otherActive) { event.preventDefault(); other(question) } }} onChange={() => other(question)} />
            <span>{t('其他')}</span>
          </label>
        </div>
        {value.otherActive && textInput}
      </>}
    </div>
  }
  if (questions.length === 0) return null
  return <section className={`native-question-card${collapsed ? ' is-collapsed' : ''}`} role="group" aria-label={t('队员提问')}>
    {multi && !collapsed && <div role="progressbar" aria-label={t('回答进度')} aria-valuenow={count} aria-valuemin={0} aria-valuemax={questions.length}
      className="native-question-progress"><span style={{ width: `${100 * count / questions.length}%` }} /></div>}
    <div className="native-question-inner">
      <header className="native-card-heading"><span className="native-question-icon"><MessageCircleQuestionMark size={16} aria-hidden="true" /></span>
        <div><strong>{t('队员提问')}</strong>{!collapsed && <small>{t(readOnly ? view.answer?.kind === 'question' ? view.answer.declined ? '已跳过此提问' : '已记录的回答' : '此提问已关闭' : '请回答以下问题，队员将据此继续。')}</small>}</div>
        {multi && <span className="native-question-count">{count}/{questions.length}</span>}
        <button type="button" className="native-icon-button" aria-expanded={!collapsed} aria-label={t(collapsed ? '展开提问' : '收起提问')}
          onClick={() => setCollapsed(value => !value)}>{collapsed ? <ChevronUp size={14} /> : <ChevronDown size={14} />}</button>
      </header>
      {!collapsed && <>
        {multi ? <Tabs.Root value={activeId} onValueChange={setActiveId} className="native-question-tabs">
          <Tabs.List className="native-tabs-list" aria-label={t('问题列表')}>{questions.map((question, index) => <Tabs.Trigger type="button" key={question.id} value={question.id}
            disabled={busy} className="native-tab" data-answered={answered(state[question.id])}>
            {answered(state[question.id]) ? <Check size={14} aria-hidden="true" /> : <span className="native-tab-number">{index + 1}</span>}<span>{question.header}</span>
          </Tabs.Trigger>)}</Tabs.List>
          {questions.map(question => <Tabs.Content key={question.id} value={question.id} className="native-question-body" style={{ height: bodyHeight }}>{renderQuestion(question)}</Tabs.Content>)}
        </Tabs.Root> : <div className="native-question-body">{questions.map(question => <div key={question.id}>{renderQuestion(question)}</div>)}</div>}
        {!readOnly && <footer className="native-card-actions"><button type="button" className="native-button ghost" disabled={locked} onClick={() => onAnswer({ kind: 'question', answers: [], declined: true })}>{t('跳过')}</button>
          <div className="native-actions-end">{multi && next && <button type="button" className="native-button outline" disabled={busy} onClick={() => setActiveId(next.id)}>{t('下一题')}<ChevronRight size={14} /></button>}
            <button type="button" className="native-button" disabled={locked || count !== questions.length} onClick={() => onAnswer({ kind: 'question', declined: false,
              answers: questions.map(question => { const value = state[question.id] ?? emptyPick(); return { questionId: question.id, labels: [...value.chosen, ...(value.otherActive && value.otherText.trim() ? [value.otherText.trim()] : [])] } }) })}>
              {busy && <Loader2 size={14} className="native-spinner" aria-hidden="true" />}{t('提交回答')}{multi && ` (${count})`}</button>
          </div></footer>}
      </>}
    </div>
  </section>
}

export function NativePlanCard(props: CardProps): React.JSX.Element {
  return <PlanCardContent key={props.view.id} {...props} />
}
function PlanCardContent({ view, busy, onAnswer }: CardProps): React.JSX.Element {
  const t = useUiText()
  const [changesOpen, setChangesOpen] = useState(false)
  const [feedback, setFeedback] = useState('')
  const locked = busy || view.status !== 'pending'
  const decide = (decision: 'approve' | 'request_changes' | 'abandon', text: string | null = null): void => onAnswer({ kind: 'plan_approval', decision, feedback: text })
  const plan = view.body.kind === 'plan_approval' ? view.body.planMarkdown : ''
  return <section className="native-plan-card" role="group" aria-label={t('计划确认')}>
    <header className="native-card-heading"><ListTodo size={16} aria-hidden="true" /><strong>{t('计划确认')}</strong></header>
    {plan ? <div className="native-plan-body" role="region" tabIndex={0} aria-label={t('完整计划，可滚动')}><SafeMarkdown>{plan}</SafeMarkdown></div>
      : <p className="native-plan-empty">{t('队员没有提供计划正文。')}</p>}
    {view.status !== 'pending' ? <div className="native-plan-record">
      {view.answer?.kind === 'plan_approval' && <><strong>{t(({ approve: '已选择批准计划', request_changes: '已提出修改意见', abandon: '已选择放弃计划' } as const)[view.answer.decision])}</strong>
        {view.answer.feedback && <p>{view.answer.feedback}</p>}</>}
    </div> : changesOpen ? <div className="native-plan-changes"><textarea value={feedback} onChange={event => setFeedback(event.target.value)} rows={3} autoFocus
      disabled={locked} aria-label={t('修改意见')} placeholder={t('告诉队员需要修改什么…')} />
      <div className="native-card-actions native-actions-end"><button type="button" className="native-button ghost" disabled={locked} onClick={() => setChangesOpen(false)}>{t('取消')}</button>
        <button type="button" className="native-button" disabled={locked || !feedback.trim()} onClick={() => decide('request_changes', feedback.trim())}>
          {busy && <Loader2 size={14} className="native-spinner" aria-hidden="true" />}{t('发送修改意见')}</button></div>
    </div> : <div className="native-card-actions">
      <button type="button" className="native-button" disabled={locked} onClick={() => decide('approve')}>{busy && <Loader2 size={14} className="native-spinner" aria-hidden="true" />}{t('批准计划')}</button>
      <button type="button" className="native-button outline" disabled={locked} onClick={() => setChangesOpen(true)}>{t('要求修改')}</button>
      <button type="button" className="native-button danger" disabled={locked} onClick={() => decide('abandon')}>{t('放弃计划')}</button>
    </div>}
  </section>
}

export function RuntimeInteractionDock({ conversationId, sourceLabel, visible = true }: { conversationId: string; sourceLabel?: string; visible?: boolean }): React.JSX.Element | null {
  const client = useThreadClient()
  const [owner, setOwner] = useState({ client, generation: 0 })
  if (owner.client !== client) setOwner({ client, generation: owner.generation + 1 })
  const controller = runtimeInteractionController(client, conversationId)
  // Key the complete presentation by authenticated client + Conversation below;
  // a hidden source keeps no listener, and never reads another source's body.
  return <InteractionDockContent key={JSON.stringify([owner.generation, conversationId])} controller={controller} sourceLabel={sourceLabel} visible={visible} />
}
function InteractionDockContent({ controller, sourceLabel, visible }: { controller: ReturnType<typeof runtimeInteractionController>; sourceLabel?: string; visible: boolean }): React.JSX.Element | null {
  const t = useUiText()
  const state = useSyncExternalStore(controller.subscribe, controller.getSnapshot, controller.getSnapshot)
  const [selected, setSelected] = useState<string | null>(null)
  const [historyOpen, setHistoryOpen] = useState(false)
  const [lastActiveId, setLastActiveId] = useState<string | null>(null)
  useEffect(() => visible ? controller.retain() : undefined, [controller, visible])
  const active = state.summaries.filter(item => !['sent', 'closed'].includes(item.status))
  const pending = active.filter(item => item.status === 'pending')
  const activeId = pending.at(-1)?.id ?? active.at(-1)?.id
  useEffect(() => { if (activeId) setLastActiveId(activeId) }, [activeId])
  const retainedId = state.summaries.some(item => item.id === lastActiveId) ? lastActiveId : undefined
  const id = selected && state.summaries.some(item => item.id === selected) ? selected : activeId ?? retainedId ?? (historyOpen ? state.summaries[0]?.id : undefined) ?? Object.keys(state.intents)[0]
  useEffect(() => { if (visible && id) void controller.open(id) }, [controller, visible, id])
  if (!state.summaries.length && !Object.keys(state.intents).length && !state.error) return null
  const view = id ? state.views[id] : undefined
  const intent = id ? state.intents[id] : undefined
  const busyFor = (candidate: RuntimeInteractionView): boolean => Boolean(!visible || candidate.id !== id || state.intents[candidate.id] || state.error
    || state.summaries.some(summary => summary.id === candidate.id && summary.version > candidate.version))
  return <aside className="native-interaction-dock" aria-label={t('队员交互')} hidden={!visible}>
    {!id && !historyOpen && <button type="button" className="native-button ghost" onClick={() => setHistoryOpen(true)}>{t('查看提问与计划记录（{0}）', state.summaries.length)}</button>}
    <div className="native-interaction-toolbar">{sourceLabel && <span title={sourceLabel}>{sourceLabel}</span>}
      <button type="button" className="native-button ghost" onClick={() => void controller.refresh()}>{t('刷新记录')}</button>
      {id && state.summaries.length > 0 && <select aria-label={t('选择提问或计划记录')} value={id} onChange={event => setSelected(event.target.value)}>
        {state.summaries.map((item, index) => <option key={item.id} value={item.id}>{t(item.kind === 'question' ? '队员提问' : '计划确认')} {state.summaries.length - index} · {t(statusCopy[item.status])}</option>)}
      </select>}
    </div>
    {selected && pending.some(item => item.id !== selected) && <button type="button" className="native-button outline" onClick={() => setSelected(null)}>{t('查看待回答问题（{0}）', pending.length)}</button>}
    {state.error && <div className="native-interaction-notice" role="alert">{t('读取失败，已保留当前回答。')}<button type="button" className="native-button outline" onClick={() => void controller.refresh()}>{t('重新读取')}</button></div>}
    {!view && id && !state.error && <p role="status">{t(intent ? '原请求当前不可读取，仍可核对原提交。' : '正在读取提问或计划…')}</p>}
    {Object.values(state.views).map(candidate => <div key={candidate.id} className="native-interaction-view" hidden={candidate.id !== id}>
      {candidate.body.kind === 'question' ? <NativeQuestionCard view={candidate} busy={busyFor(candidate)} onAnswer={answer => controller.answer(candidate.id, answer)} />
        : <NativePlanCard view={candidate} busy={busyFor(candidate)} onAnswer={answer => controller.answer(candidate.id, answer)} />}
    </div>)}
    {view && view.status !== 'pending' && <p className="native-interaction-status" role="status">{t(statusCopy[view.status])}{view.status === 'resolved' && t('；等待投递，尚未确认队员收到。')}
      {view.status === 'closed' && t('；此请求已失效，不能再提交。')}{view.status === 'delivery_unknown' && t('；不会自动重发。')}</p>}
    {id && state.decisionError === id && <p className="native-interaction-notice" role="alert">{t('提交未被接受，请核对最新请求后再操作。')}</p>}
    {Object.entries(state.intents).map(([pendingId, pendingIntent], index) => <div key={pendingId} className="native-interaction-notice" role="group" aria-label={t('原提交 {0}', index + 1)}>
      <span role="status">{t('原提交 {0}', index + 1)} · {t(pendingIntent.unknown ? '提交结果待核对，原回答已保留。' : '正在提交回答…')}</span>
      <button type="button" className="native-button outline" onClick={() => void controller.recover(pendingId)}>{t('核对原提交')}</button>
      {pendingIntent.unknown && <button type="button" className="native-button outline" onClick={() => void controller.recover(pendingId, true)}>{t('重试原提交')}</button>}
    </div>)}
  </aside>
}
const statusCopy = { pending: '待回答', resolved: '答案已保存', dispatching: '正在投递答案', sent: '答案已发送', closed: '请求已关闭', delivery_unknown: '答案投递结果待核对' } as const
