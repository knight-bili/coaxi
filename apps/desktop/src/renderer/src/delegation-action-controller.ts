import type { StoredCommandResult } from '@contracts'
import type { DelegationActionMethod } from '../../shared/private-conversation-operations'
import { newCommandId } from '../../shared/command-id'
import type { ThreadClient } from './camp-client'
import { singleChatRecoveryStorage } from './single-chat-composer-store'

export interface DelegationActionOrigin { threadId: string; parentConversationId: string; delegationId: string; conversationId: string }
type Client = Pick<ThreadClient, 'request' | 'editingRecovery' | 'privateInputRecovery' | 'onInvalidated' | 'retryPendingCommand'>
interface Intent { version: 1; origin: DelegationActionOrigin; method: DelegationActionMethod; params: { commandId: string; command: Record<string, unknown> }; result?: StoredCommandResult }
interface State { pending: boolean; busy: boolean; unknown: boolean; error: string | null; result: StoredCommandResult | null }
const object = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value)
const invalid = (): never => { throw new Error('delegation.control_recovery_invalid') }
const identity = (origin: DelegationActionOrigin): string => JSON.stringify([origin.threadId, origin.parentConversationId, origin.delegationId, origin.conversationId])
function validateCommand(origin: DelegationActionOrigin, method: DelegationActionMethod, command: Record<string, unknown>): void {
  const keys = method === 'delegations.takeover' ? ['delegationId', 'expectedControlVersion']
    : method === 'delegations.cancel' ? ['delegationId', 'attemptNo']
      : method === 'delegations.resume' ? ['delegationId', 'expectedAttemptNo', 'reason'] : method === 'delegations.conversation.end' ? ['conversationId'] : invalid()
  if (Object.keys(command).some(key => !keys.includes(key))) invalid()
  if (method === 'delegations.conversation.end') { if (command.conversationId !== origin.conversationId) invalid(); return }
  if (command.delegationId !== origin.delegationId) invalid()
  const version = command[method === 'delegations.takeover' ? 'expectedControlVersion' : method === 'delegations.cancel' ? 'attemptNo' : 'expectedAttemptNo']
  if (!Number.isSafeInteger(version) || Number(version) < 1) invalid()
  if (method === 'delegations.resume' && command.reason !== undefined && (typeof command.reason !== 'string' || !command.reason.trim() || new TextEncoder().encode(command.reason).length > 2048)) invalid()
}
function receipt(intent: Intent, result: StoredCommandResult): void {
  if (!object(result) || result.commandId !== intent.params.commandId || result.commandType !== intent.method
    || !['accepted', 'applied', 'rejected'].includes(result.status) || !object(result.payload)) invalid()
}
function parse(value: unknown, origin: DelegationActionOrigin): Intent | null {
  if (value == null) return null
  if (!object(value) || value.version !== 1 || !object(value.origin) || identity(value.origin as unknown as DelegationActionOrigin) !== identity(origin)
    || !object(value.params) || typeof value.params.commandId !== 'string' || !object(value.params.command)) invalid()
  const intent = value as unknown as Intent
  validateCommand(origin, intent.method, intent.params.command)
  if (intent.result) receipt(intent, intent.result)
  return intent
}
/** Four existing Core User operations; no implicit takeover or new runtime input.
 * The persisted original request survives drawer unmount and unknown replies. */
export class DelegationActionController {
  private state: State = { pending: false, busy: false, unknown: false, error: null, result: null }
  private listeners = new Set<() => void>()
  private intent: Intent | null = null
  private flight: Promise<void> | null = null
  private storage: ReturnType<typeof singleChatRecoveryStorage>
  private key: string
  constructor(private client: Client, readonly origin: DelegationActionOrigin) {
    this.origin = { threadId: origin.threadId, parentConversationId: origin.parentConversationId, delegationId: origin.delegationId, conversationId: origin.conversationId }
    this.storage = singleChatRecoveryStorage(client)
    this.key = 'delegation-action:v1:' + identity(this.origin)
    try { this.intent = parse(this.storage.get(this.key), this.origin); this.state.pending = !!this.intent }
    catch { this.state.pending = true; this.state.error = 'delegation.control_recovery_invalid' }
  }
  getSnapshot = (): State => this.state
  subscribe = (listener: () => void): (() => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener) } }
  private publish(change: Partial<State>): void { this.state = { ...this.state, ...change }; this.listeners.forEach(listener => listener()) }
  private perform(operation: () => Promise<void>): Promise<void> {
    if (this.flight) return this.flight
    this.publish({ busy: true, error: null })
    this.flight = Promise.resolve().then(operation).catch(error => { this.publish({ error: error instanceof Error ? error.message : 'delegation.control_failed' }); throw error })
      .finally(() => { this.flight = null; this.publish({ busy: false }) })
    return this.flight
  }
  private accept(intent: Intent, result: StoredCommandResult): void {
    receipt(intent, result)
    const recorded = { ...intent, result }
    this.storage.set(this.key, recorded)
    this.intent = recorded
    this.storage.set(this.key, null)
    this.intent = null
    this.publish({ pending: false, unknown: false, result, error: result.status === 'rejected' ? result.code : null })
    if (result.status === 'rejected') throw new Error(result.code)
  }
  run(method: DelegationActionMethod, command: Record<string, unknown>): Promise<void> {
    if (this.state.pending || this.flight) return Promise.reject(new Error('delegation.control_pending'))
    validateCommand(this.origin, method, command)
    const intent: Intent = { version: 1, origin: this.origin, method, params: { commandId: newCommandId(), command: structuredClone(command) } }
    try { this.storage.set(this.key, intent) } catch { this.publish({ error: 'delegation.control_not_saved' }); return Promise.reject(new Error('delegation.control_not_saved')) }
    this.intent = intent
    this.publish({ pending: true, unknown: false, result: null })
    return this.perform(async () => { this.accept(intent, await this.client.request<StoredCommandResult>(method, structuredClone(intent.params))) })
  }
  reconcile = (): Promise<void> => this.perform(async () => {
    const intent = this.intent
    if (!intent) { if (this.state.pending) invalid(); return }
    if (intent.result) { this.accept(intent, intent.result); return }
    const reply = await this.client.request<{ state: string; result?: StoredCommandResult }>('commands.reconcile', { operation: intent.method, params: structuredClone(intent.params) })
    if (reply.state === 'recorded' && reply.result) this.accept(intent, reply.result)
    else this.publish({ unknown: reply.state === 'unknown', error: 'delegation.control_result_unknown' })
  })
  retry = async (): Promise<void> => {
    await this.reconcile()
    const intent = this.intent
    if (!intent || !this.state.unknown) return
    await this.perform(async () => {
      this.publish({ unknown: false })
      if (this.client.retryPendingCommand) { await this.client.retryPendingCommand(intent.method, structuredClone(intent.params)); return }
      this.accept(intent, await this.client.request<StoredCommandResult>(intent.method, structuredClone(intent.params)))
    })
    if (this.intent) await this.reconcile()
  }
}
const owners = new WeakMap<object, Map<string, DelegationActionController>>()
export function delegationActionController(client: Client, origin: DelegationActionOrigin): DelegationActionController {
  let contexts = owners.get(client)
  if (!contexts) { contexts = new Map(); owners.set(client, contexts) }
  const key = identity(origin)
  let controller = contexts.get(key)
  if (!controller) { controller = new DelegationActionController(client, origin); contexts.set(key, controller) }
  return controller
}
