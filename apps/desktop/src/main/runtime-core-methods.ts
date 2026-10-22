import type { CoreMethod } from '@contracts'

export const RUNTIME_RENDERER_CORE_METHODS = [
  'history.execution.preview', 'history.execution.confirm', 'history.execution.receipt',
  'history.execution.records', 'runtime.steer.evidence',
  'runtime.steer.normal.preview', 'runtime.steer.normal.confirm', 'runtime.steer.normal.receipt',
  'runtime.steer.policy.get', 'runtime.steer.policy.set', 'runtime.steer.targets',
  'runtime.steer.materials.prepare', 'runtime.steer.preview', 'runtime.steer.confirm', 'runtime.steer.receipt', 'runtime.steer.previews',
  'runtime.custom.registry',
  'runtime.custom.list', 'runtime.custom.upsert', 'runtime.custom.remove', 'runtime.custom.install', 'runtime.custom.inspect',
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
] as const satisfies readonly CoreMethod[]
