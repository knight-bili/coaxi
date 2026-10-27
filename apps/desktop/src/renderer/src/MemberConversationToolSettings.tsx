import { SettingsDisclosureHint } from './SettingsDisclosureHint'
import { useEffect, useRef, useState } from 'react'
import type { ConversationToolFlags, ConversationToolPolicy, ConversationToolsInputs, StoredCommandResult } from '@contracts'
import { newCommandId } from '../../shared/command-id'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { readErrorMessage } from './error-message'
import { conversationOriginal, conversationToolRequest, type ConversationOriginal } from './conversation-tools-client'

const empty: ConversationToolFlags = { question: false, sessionInfo: false, automations: false, workTasks: false }
const features: Array<{ key: keyof ConversationToolFlags; title: string; detail: string }> = [
  { key: 'question', title: '向你提问', detail: '队员遇到分歧时，等待你在原问题卡中选择或填写。' },
  { key: 'sessionInfo', title: '参考你选中的旧讨论', detail: '只读取你本次授权并放入消息的资料，不因同一工程自动读取私聊。' },
  { key: 'automations', title: '安排后续工作', detail: '可以保存定时任务或手动任务，沿原自动化设置管理。' },
  { key: 'workTasks', title: '创建使命待办', detail: '保存完整待办，等待你从使命板启动。' }
]
type Save = ConversationToolsInputs['conversationTools.policy.set']
/** 沿原Coaxi队员设置容器，四个总开关与队员分配分别展示。 */
export function MemberConversationToolSettings({ agentId, disabled = false }: { agentId: string; disabled?: boolean }): React.JSX.Element {
  const client = useThreadClient(), t = useUiText(), generation = useRef(0), locked = useRef(false)
  const [base, setBase] = useState<ConversationToolPolicy | null>(null), [flags, setFlags] = useState(empty), [global, setGlobal] = useState(empty), [feedback, setFeedback] = useState<boolean | null>(null)
  const [pending, setPending] = useState<ConversationOriginal | null>(null), [busy, setBusy] = useState(false), [error, setError] = useState('')
  const key = `conversation-tools-policy:${client.presentationScope ?? ''}:${agentId}`
  const accept = (value: ConversationToolPolicy): void => { setBase(value); setFlags(value.members.find(member => member.agentId === agentId)?.flags ?? empty); setGlobal(value.globalFlags); setFeedback(value.feedbackGate) }
  const load = async (): Promise<void> => {
    const observed = generation.current
    try {
      const original = await conversationOriginal(client, key)
      const value = await conversationToolRequest(client, 'conversationTools.policy.get', {})
      if (observed === generation.current) { accept(value); setPending(original) }
    } catch (reason) { if (observed === generation.current) setError(readErrorMessage(reason)) }
  }
  useEffect(() => { generation.current++; locked.current = false; setBusy(false); setBase(null); setPending(null); setError(''); void load(); return () => { generation.current++ } }, [client, agentId, key])
  const save = async (): Promise<void> => {
    if (locked.current || !base || pending) return
    locked.current = true; setBusy(true); setError(''); const observed = generation.current
    const input: Save = { commandId: newCommandId(), expectedRevision: base.revision, globalFlags: global, feedbackGate: feedback, members: [...base.members.filter(member => member.agentId !== agentId), { agentId, flags }] }
    const original: ConversationOriginal = { operation: 'conversationTools.policy.set', commandId: input.commandId }
    try {
      await conversationOriginal(client, key, original); if (observed !== generation.current) return; setPending(original)
      const value = await conversationToolRequest(client, 'conversationTools.policy.set', input)
      if (observed !== generation.current) return
      await conversationOriginal(client, key, null); if (observed === generation.current) { accept(value); setPending(null) }
    } catch (reason) { if (observed === generation.current) setError(readErrorMessage(reason)) }
    finally { if (observed === generation.current) { locked.current = false; setBusy(false) } }
  }
  const reconcile = async (): Promise<void> => {
    if (locked.current || !pending) return
    locked.current = true; setBusy(true); setError(''); const observed = generation.current, original = pending
    try {
      const value = await client.request<{ state: string; result?: StoredCommandResult }>('commands.reconcile', { operation: original.operation, params: { commandId: original.commandId } })
      if (observed !== generation.current) return
      if (value.state !== 'recorded' || value.result?.commandId !== original.commandId || value.result.commandType !== original.operation) throw new Error(t('原保存尚无确定回执，请保留编号，稍后继续核对。'))
      await conversationOriginal(client, key, null); if (observed !== generation.current) return; setPending(null)
      if (value.result.status === 'rejected') { await load(); throw new Error(value.result.code) }
      accept(value.result.payload as unknown as ConversationToolPolicy)
    } catch (reason) { if (observed === generation.current) setError(readErrorMessage(reason)) }
    finally { if (observed === generation.current) { locked.current = false; setBusy(false) } }
  }
  const changed = !!base && (JSON.stringify(flags) !== JSON.stringify(base.members.find(member => member.agentId === agentId)?.flags ?? empty) || JSON.stringify(global) !== JSON.stringify(base.globalFlags) || feedback !== base.feedbackGate)
  const blocked = disabled || busy || !base || !!pending
  return <section className="member-editor-section interaction-member-settings" aria-label={t('提问与后续工作')}>
    <div className="member-editor-section-heading"><h2>{t('提问与后续工作')}</h2><span>{t('用于之后开始的新执行')}</span></div>
    <details><summary>{t('全局工具开关')}<SettingsDisclosureHint /></summary>{features.map(feature => <label key={feature.key} className="interaction-setting-row"><span><strong>{t(feature.title)}</strong><small>{t('允许分配给已选择的队员')}</small></span><input className="workspace-check" type="checkbox" role="switch" checked={global[feature.key]} disabled={blocked} onChange={event => setGlobal(value => ({ ...value, [feature.key]: event.target.checked }))} /></label>)}<label className="settings-form-field"><span>{t('文字与卡片反馈的总开关')}</span><select className="workspace-field" value={feedback === null ? 'inherit' : String(feedback)} disabled={blocked} onChange={event => setFeedback(event.target.value === 'inherit' ? null : event.target.value === 'true')}><option value="inherit">{t('沿用原反馈设置')}</option><option value="false">{t('新执行关闭反馈工具')}</option><option value="true">{t('允许原已开启反馈的队员')}</option></select></label></details>
    {features.map(feature => <label key={feature.key} className="interaction-setting-row"><span><strong>{t(feature.title)}</strong><small>{t(feature.detail)}{!global[feature.key] && ` ${t('总开关当前关闭。')}`}</small></span><input className="workspace-check" type="checkbox" role="switch" checked={flags[feature.key]} disabled={blocked} onChange={event => setFlags(value => ({ ...value, [feature.key]: event.target.checked }))} /></label>)}
    <p className="interaction-help">{t('总开关与此队员的分配都开启才生效，仍需队员程序支持。Coaxi 内置工具保留自己的授权；关闭这里的工具不会撤销原内置授权。已有执行保留原设置。')}</p>
    {error && <p role="alert" className="environment-error">{error}</p>}<div className="interaction-actions">{pending ? <button className="quiet-button" type="button" disabled={busy} onClick={() => void reconcile()}>{t('核对原保存请求')}</button> : <><button className="quiet-button" type="button" disabled={busy} onClick={() => void load()}>{t('重新读取')}</button><button className="quiet-button" type="button" disabled={blocked || !changed} onClick={() => void save()}>{t(busy ? '正在保存…' : '保存工具设置')}</button></>}</div>
  </section>
}
