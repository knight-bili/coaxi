import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'
import { RUNTIME_RENDERER_CORE_METHODS } from './runtime-core-methods'

const mainSource = readFileSync(new URL('./index.ts', import.meta.url), 'utf8')

describe('Runtime Renderer Core method allowlist', () => {
  it('exposes exactly the approved Runtime Renderer Core methods', () => {
    expect(RUNTIME_RENDERER_CORE_METHODS).toEqual([
      'configuration.migration.archives',
      'configuration.migration.targets',
      'configuration.migration.preview',
      'configuration.migration.confirm',
      'configuration.migration.receipt',
      'configuration.migration.undoPreview',
      'configuration.migration.undoConfirm',
      'configuration.providers.list',
      'configuration.providers.upsert',
      'configuration.providers.remove',
      'configuration.memberConnection.get',
      'configuration.memberConnection.set',
      'history.execution.preview',
      'history.execution.confirm',
      'history.execution.receipt',
      'history.execution.records',
      'runtime.steer.evidence',
      'runtime.steer.normal.preview',
      'runtime.steer.normal.confirm',
      'runtime.steer.normal.receipt',
      'runtime.steer.policy.get',
      'runtime.steer.policy.set',
      'runtime.steer.targets',
      'runtime.steer.materials.prepare',
      'runtime.steer.preview',
      'runtime.steer.confirm',
      'runtime.steer.receipt',
      'runtime.steer.previews',
      'runtime.custom.registry',
      'runtime.custom.list',
      'runtime.custom.upsert',
      'runtime.custom.remove',
      'runtime.custom.install',
      'runtime.custom.inspect',
      'runtime.discovery.rescan',
      'runtime.product.ensure',
      'runtime.product.check',
      'runtime.startup.get',
      'member.runtimeEnvironment.get',
      'runtime.environmentLegacy.list',
      'runtime.environmentLegacy.get',
      'runtime.environmentLegacy.acknowledge',
      'runtime.startup.inspect',
      'runtime.startup.check',
      'runtime.startup.save',
      'runtime.networkRecovery.wake',
      'runtime.modelCatalog.open',
      'runtime.pendingExecution.cancel'
    ])
    expect(RUNTIME_RENDERER_CORE_METHODS).not.toContain('core.shutdown')
    expect(RUNTIME_RENDERER_CORE_METHODS).not.toContain('runtime.arbitrary.invoke')

    const allowlistStart = mainSource.indexOf('const allowedMethods = new Set<CoreMethod>([')
    const allowlistEnd = mainSource.indexOf('\n])', allowlistStart)
    expect(allowlistStart).toBeGreaterThan(-1)
    expect(allowlistEnd).toBeGreaterThan(allowlistStart)
    const allowlist = mainSource.slice(allowlistStart, allowlistEnd)
    expect(allowlist).not.toContain("'core.shutdown'")
    expect(allowlist).not.toContain("'runtime.arbitrary.invoke'")
  })

  it('admits the existing conversation questions, grants and policy operations without private RPCs', () => {
    const start = mainSource.indexOf('const allowedMethods = new Set<CoreMethod>([')
    const end = mainSource.indexOf('\n])', start)
    const methods = mainSource.slice(start, end)
    for (const suffix of ['policy.get', 'policy.set', 'questions.list', 'questions.answer',
      'questions.decline', 'questions.reconcile', 'grants.preview', 'grants.confirm',
      'grants.revoke', 'grants.list', 'requests.get', 'requests.resolve',
      'automationOwner.get', 'automationOwner.set']) {
      expect(methods).toContain(`'conversationTools.${suffix}'`)
    }
    expect(methods).not.toContain("'core.shutdown'")
    expect(methods).not.toContain("'conversationTools.*'")
  })

  it('wakes the same Core recovery check when the operating system resumes', () => {
    expect(mainSource).toContain("powerMonitor.on('resume', wakeNetworkRecoveryAfterSystemResume)")
    expect(mainSource).toContain("core.request('runtime.networkRecovery.wake')")
    expect(mainSource).toContain(
      "powerMonitor.removeListener('resume', wakeNetworkRecoveryAfterSystemResume)"
    )
  })

  it('allows the Single Chat pending-input and scoped-material operations implemented by Core', () => {
    const allowlistStart = mainSource.indexOf('const allowedMethods = new Set<CoreMethod>([')
    const allowlistEnd = mainSource.indexOf('\n])', allowlistStart)
    expect(allowlistStart).toBeGreaterThan(-1)
    expect(allowlistEnd).toBeGreaterThan(allowlistStart)
    for (const method of ['singleChat.pendingInputs.edit', 'singleChat.composerDraft.mutate', 'singleChat.composerDraft.recovery',
      'delegations.resume', 'delegations.list', 'delegations.get', 'delegations.source', 'delegations.history', 'delegations.read', 'delegations.settings.get', 'delegations.settings.update',
      'runtime.interactions.list', 'runtime.interactions.get', 'runtime.interactions.resolve',
      'runtime.childReads.start', 'runtime.childReads.poll', 'runtime.childReads.page', 'runtime.childReads.close',
      'runtime.agentReads.start', 'runtime.agentReads.poll', 'runtime.agentReads.page', 'runtime.agentReads.close',
      'delegations.composer.get', 'delegations.conversation.get', 'delegations.composer.mutate', 'delegations.composer.recovery',
      'delegations.message.send', 'delegations.pendingInputs.get', 'delegations.pendingInputs.edit', 'delegations.takeover', 'delegations.cancel', 'delegations.conversation.end']) {
      expect(mainSource.slice(allowlistStart, allowlistEnd)).toContain(`'${method}'`)
    }
  })

  it('admits the implemented skill packs, canvas and ephemeral terminal operations at Native IPC', () => {
    const start = mainSource.indexOf('const allowedMethods = new Set<CoreMethod>([')
    const end = mainSource.indexOf('\n])', start)
    expect(start).toBeGreaterThan(-1)
    expect(end).toBeGreaterThan(start)
    const methods = mainSource.slice(start, end)
    for (const method of [
      'skillPacks.list', 'skillPacks.read', 'skillPacks.installations.list', 'skillPacks.install', 'skillPacks.uninstall', 'skillPacks.members.list', 'skillPacks.members.set',
      'canvas.boards.list', 'canvas.boards.get', 'canvas.boards.create', 'canvas.boards.update', 'canvas.boards.delete',
      'canvas.nodes.snapshot', 'canvas.nodes.apply', 'canvas.references.page',
      'canvas.terminals.start', 'canvas.terminals.snapshot', 'canvas.terminals.write', 'canvas.terminals.resize', 'canvas.terminals.stop'
    ]) expect(methods).toContain(`'${method}'`)
    expect(methods).not.toContain("'core.shutdown'")
  })
})
