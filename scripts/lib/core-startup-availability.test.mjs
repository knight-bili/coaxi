import assert from 'node:assert/strict'
import { randomUUID } from 'node:crypto'
import { execFileSync, spawn } from 'node:child_process'
import { lstat, mkdtemp, mkdir, readFile, realpath, rename, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { createInterface } from 'node:readline'
import { DatabaseSync } from 'node:sqlite'
import test from 'node:test'
import { composerDocumentForAddress } from './create-configured-camp.mjs'
import {
  coreDataDirectoryArguments,
  removeEphemeralRuntimeCampFilesRoot
} from './runtime-camp-files-root.mjs'

const repository = resolve(import.meta.dirname, '../..')
const binary = process.env.COAXI_CORE_BIN
  ?? join(repository, 'target', 'debug', process.platform === 'win32' ? 'coaxi-core.exe' : 'coaxi-core')

// Core startup used to install a process-global wrapper. A ManagedProcess unit
// test without run_core would miss that policy and pass on the broken build.
test('Core-managed macOS probes can start their own native sandbox', { skip: process.platform !== 'darwin' }, async () => {
  const fixture = await realpath(await mkdtemp(join(tmpdir(), 'coaxi-native-sandbox-')))
  const dataDir = join(fixture, 'data')
  const resultPath = join(fixture, 'sandbox-exit.txt')
  const fakeCodex = join(fixture, 'codex')
  const quotedResultPath = `'${resultPath.replaceAll("'", "'\\''")}'`
  await writeFile(fakeCodex, `#!/bin/sh
/usr/bin/sandbox-exec -p '(version 1)(allow default)' /usr/bin/true
probe_exit=$?
printf '%s\\n' "$probe_exit" > ${quotedResultPath}
# Keep discovery on this fixture, then reject protocol startup without a model.
if [ "$1" = '--version' ]; then
  printf 'codex-cli 0.153.4\\n'
  exit 0
fi
exit 1
`, { mode: 0o700 })
  const core = startCore(dataDir, join(dataDir, 'managed-skill-library'), join(dataDir, 'mcp.json'), [], {
    COAXI_CODEX_BIN: fakeCodex
  })
  try {
    await core.ready
    await core.settled()
    await core.request('runtime.product.ensure', { runtimeKind: 'codex-cli' })
    let sandboxExit = null
    const deadline = Date.now() + 10000
    while (Date.now() < deadline) {
      sandboxExit = await readFile(resultPath, 'utf8').catch((error) => {
        if (error.code === 'ENOENT') return null
        throw error
      })
      if (sandboxExit?.trim()) break
      await new Promise((resolve) => setTimeout(resolve, 20))
    }
    assert.equal(sandboxExit?.trim(), '0', 'Coaxi must not prevent a Runtime from creating its own sandbox')
  } finally {
    await core.close()
    await removeEphemeralRuntimeCampFilesRoot(dataDir, { temporaryDirectory: fixture })
    await rm(fixture, { recursive: true, force: true })
  }
})

// This owns the real run_core -> ready -> RPC seam: an initializer-only unit test
// cannot prove that a healthy authority remains reachable after a filesystem error.
test('optional startup failures preserve authority RPC and can recover without restarting Core', async (t) => {
  for (const scenario of [
    { id: 'mcp', path: 'mcp.json', kind: 'directory', method: 'mcp.config.get' },
    { id: 'maintenance', path: 'managed-skill-library/.staging', kind: 'file', readySubsystem: 'skills' },
    { id: 'runtime.claude-code-cli', path: 'runtime-private', kind: 'file' },
    { id: 'maintenance', path: 'runtime/mcp', kind: 'file' }
  ]) {
    await t.test(scenario.id, async () => {
      const fixture = await realpath(await mkdtemp(join(tmpdir(), 'coaxi-startup-availability-')))
      const dataDir = process.platform === 'win32'
        ? JSON.parse(execFileSync(binary, ['--prepare-windows-data-root', join(fixture, 'formal')], { encoding: 'utf8' })).core
        : join(fixture, 'data')
      const skillRoot = join(dataDir, 'managed-skill-library')
      const mcpPath = join(dataDir, 'mcp.json')
      if (process.platform === 'win32') {
        // Only native Core creates private managed directories on Windows.
        const seed = startCore(dataDir, skillRoot, mcpPath)
        try { await seed.ready; await seed.settled() } finally { await seed.close() }
      } else {
        await mkdir(skillRoot, { recursive: true, mode: 0o700 })
      }
      const faultPath = join(dataDir, scenario.path)
      if (await lstat(faultPath).catch(() => null)) await rename(faultPath, `${faultPath}.before-fault`)
      await mkdir(dirname(faultPath), { recursive: true, mode: 0o700 })
      if (scenario.kind === 'directory') await mkdir(faultPath)
      else await writeFile(faultPath, 'preserve this non-authority fixture')
      const core = startCore(dataDir, skillRoot, mcpPath)
      try {
        const frame = await core.ready
        assert.ok(core.startupFrames.some(entry => entry.phase === 'preparing_runtime_storage'
          && entry.authorityState?.kind === (process.platform === 'win32' ? 'admitted' : 'confirmed_absent')),
        'Main receives the authority assessment before Runtime storage admission')
        assert.ok(frame.subsystems.some((entry) => entry.state === 'initializing'), 'authority ready precedes optional initialization')
        const statuses = await core.settled()
        assert.equal(statuses.find((entry) => entry.id === scenario.id)?.state, 'degraded')
        if (scenario.readySubsystem) assert.equal(statuses.find((entry) => entry.id === scenario.readySubsystem)?.state, 'ready')
        assert.ok((await core.request('members.list')).length > 0)
        assert.ok(await core.request('navigation.snapshot'))
        if (scenario.method) {
          await assert.rejects(core.request(scenario.method), (error) =>
            error.code === 'subsystem_unavailable'
            && error.kind === 'infrastructure_failure'
            && error.retryable === true
            && error.details.subsystem === scenario.id
          )
        }
        assert.equal(core.process.exitCode, null, 'a local initializer failure must not kill Core')
        await rename(faultPath, `${faultPath}.retained`)
        const recovered = await core.request('runtime.subsystems.retry')
        assert.equal(recovered.find((entry) => entry.id === scenario.id)?.state, 'ready')
        assert.ok((await core.request('members.list')).length > 0)
        assert.equal(core.process.exitCode, null, 'retry must repair the feature in the same Core')
      } finally {
        await core.close()
        await removeEphemeralRuntimeCampFilesRoot(dataDir, { temporaryDirectory: fixture })
        await rm(fixture, { recursive: true, force: true })
      }
    })
  }
})

// This owns the startup subsystem -> Runtime availability separation. Before
// the regression fix, a missing Pi executable made both layers report a
// failure even though executable presence belongs only to Runtime discovery.
test('a missing Pi executable stays in Runtime availability instead of subsystem health', {
  // macOS adds fixed package-manager locations after PATH, so a developer's
  // installed Pi cannot be hidden without mutating the host. Linux has no such
  // locations and Windows closes Pi discovery over the explicit override.
  skip: process.platform === 'darwin'
}, async () => {
  const fixture = await realpath(await mkdtemp(join(tmpdir(), 'coaxi-missing-pi-availability-')))
  const dataDir = process.platform === 'win32'
    ? JSON.parse(execFileSync(binary, ['--prepare-windows-data-root', join(fixture, 'formal')], { encoding: 'utf8' })).core
    : join(fixture, 'data')
  const emptySearchPath = join(fixture, 'empty-runtime-search')
  await mkdir(emptySearchPath)
  const missingPi = join(fixture, 'missing-runtime', process.platform === 'win32' ? 'pi.cmd' : 'pi')
  const core = startCore(
    dataDir,
    join(dataDir, 'managed-skill-library'),
    join(dataDir, 'mcp.json'),
    [],
    {
      PATH: emptySearchPath,
      COAXI_PI_BIN: missingPi,
      SHELL: join(fixture, 'missing-shell')
    }
  )
  try {
    await core.ready
    const statuses = await core.settled()
    assert.equal(statuses.find((entry) => entry.id === 'runtime.pi')?.state, 'ready')

    const health = await core.request('health.check')
    assert.equal(
      health.runtimeAvailability.find((entry) => entry.runtimeKind === 'pi')?.status,
      'missing'
    )
  } finally {
    await core.close()
    await removeEphemeralRuntimeCampFilesRoot(dataDir, { temporaryDirectory: fixture })
    await rm(fixture, { recursive: true, force: true })
  }
})

// This owns the current Core RPC -> publication transaction rollback seam.
// Retired queue rows remain intact; failed publication must never launch a model.
test('failed direct publication preserves legacy input without leaking new content', async () => {
  const fixture = await realpath(await mkdtemp(join(tmpdir(), 'coaxi-queue-events-')))
  const dataDir = process.platform === 'win32'
    ? JSON.parse(execFileSync(binary, ['--prepare-windows-data-root', join(fixture, 'formal')], { encoding: 'utf8' })).core
    : join(fixture, 'data')
  const skillRoot = join(dataDir, 'managed-skill-library')
  const mcpPath = join(dataDir, 'mcp.json')
  let core = startCore(dataDir, skillRoot, mcpPath)
  let database
  try {
    await core.ready
    await core.settled()
    const agent = (await core.request('members.list')).find(profile => profile.presence === 'present')
    const created = await core.request('camps.create', {
      commandId: randomUUID(), name: 'Private queue notification fixture', workspace: null,
      memberAgentIds: [agent.agentId], defaultLeadAgentId: agent.agentId, collaborationMode: 'peer'
    })
    assert.equal(created.status, 'applied')
    const threadId = created.payload.threadId
    await core.close()
    database = new DatabaseSync(join(dataDir, 'coaxi.sqlite'))
    const now = new Date().toISOString()
    database.prepare(`INSERT INTO pending_camp_input(
      id, camp_id, enqueue_sequence, state, structured_content_json, execution_json, user_id,
      last_attempt_error_code, created_at, updated_at
    ) VALUES ('held-head', ?, 1, 'needs_repair', ?, 'null', 'local_user', 'fixture.hold', ?, ?)`)
      .run(threadId, JSON.stringify([{ kind: 'text', text: 'Private held head' }]), now, now)
    // A publication transaction must fail before it can create a Run or execute a
    // model. The scheduler still has to announce the committed needs_repair state.
    database.exec(`CREATE TRIGGER refuse_fixture_publication BEFORE INSERT ON camp_message
      BEGIN SELECT RAISE(ABORT, 'fixture publication refusal'); END;`)
    database.close()
    database = null
    core = startCore(dataDir, skillRoot, mcpPath)
    await core.ready
    await assert.rejects(core.request('camp.messages.send', {
      commandId: randomUUID(), threadId, content: composerDocumentForAddress({ mode: 'default' }, 'Private queued body'),
      execution: { taskId: null, purpose: 'private queue fixture', completionRole: 'required' }
    }), error => error.code === 'CORE_REQUEST_FAILED' && /fixture publication refusal/u.test(error.message))
    await core.close()
    database = new DatabaseSync(join(dataDir, 'coaxi.sqlite'), { readOnly: true })
    assert.equal(database.prepare('SELECT COUNT(*) AS count FROM agent_run').get().count, 0)
    assert.equal(database.prepare('SELECT COUNT(*) AS count FROM camp_message').get().count, 0)
    assert.equal(database.prepare("SELECT COUNT(*) AS count FROM event_log WHERE payload_json LIKE '%Private queued body%'").get().count, 0)
    const held = database.prepare('SELECT state, structured_content_json FROM pending_camp_input WHERE id = ?').get('held-head')
    assert.equal(held.state, 'needs_repair')
    assert.equal(JSON.parse(held.structured_content_json)[0].text, 'Private held head')
  } finally {
    database?.close()
    await core.close()
    await removeEphemeralRuntimeCampFilesRoot(dataDir, { temporaryDirectory: fixture })
    await rm(fixture, { recursive: true, force: true })
  }
})

// Current direct sends must remain usable after restarting with legacy failed turns.
test('idle camps accept direct input after legacy failed-turn recovery without replaying retired queues', async () => {
  const fixture = await realpath(await mkdtemp(join(tmpdir(), 'coaxi-idle-queue-recovery-')))
  const dataDir = process.platform === 'win32'
    ? JSON.parse(execFileSync(binary, ['--prepare-windows-data-root', join(fixture, 'formal')], { encoding: 'utf8' })).core
    : join(fixture, 'data')
  const skillRoot = join(dataDir, 'managed-skill-library')
  const mcpPath = join(dataDir, 'mcp.json')
  let core = startCore(dataDir, skillRoot, mcpPath)
  let database
  const camps = []
  const sendText = async (threadId, body) => {
    // No new Run or model is allowed in this transport/scheduler fixture.
    return core.request('camp.messages.send', {
      commandId: randomUUID(), threadId, content: composerDocumentForAddress({ mode: 'default' }, body), execution: null
    })
  }
  try {
    await core.ready
    await core.settled()
    const agentId = (await core.request('members.list')).find(profile => profile.presence === 'present').agentId
    for (const backlog of [false, true]) {
      const created = await core.request('camps.create', {
        commandId: randomUUID(), name: 'Idle queue recovery fixture', workspace: null,
        memberAgentIds: [agentId], defaultLeadAgentId: agentId, collaborationMode: 'peer'
      })
      assert.equal(created.status, 'applied')
      const threadId = created.payload.threadId
      const first = await sendText(threadId, 'A')
      assert.equal(first.commandResult.code, 'camp_message.sent')
      camps.push({ threadId, backlog, messageId: first.commandResult.payload.threadMessageId, turnId: randomUUID() })
    }
    await core.close()
    database = new DatabaseSync(join(dataDir, 'coaxi.sqlite'))
    const now = new Date().toISOString()
    for (const { threadId, backlog, messageId, turnId } of camps) {
      database.prepare(`INSERT OR IGNORE INTO conversation(id, camp_id, agent_id, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?)`).run(randomUUID(), threadId, agentId, now, now)
      const conversationId = database.prepare('SELECT id FROM conversation WHERE camp_id = ? AND agent_id = ?').get(threadId, agentId).id
      database.prepare(`INSERT INTO camp_turn(id, camp_id, trigger_type, trigger_id, status, created_at, updated_at)
        VALUES (?, ?, 'camp_message', ?, 'waiting', ?, ?)`).run(turnId, threadId, messageId, now, now)
      // Minimal terminal history from the old retry-wait rule, never dispatchable.
      database.prepare(`INSERT INTO agent_run(
        id, camp_turn_id, conversation_id, trigger_camp_message_id, initial_camp_context_through_sequence,
        initial_conversation_context_through_sequence, responsibility_key, start_reason,
        purpose, completion_role, effective_config_json, status,
        idempotency_key, manual_retry_allowed, last_error_code, created_at, ended_at, updated_at
      ) VALUES (?, ?, ?, ?, 1, 0, 'initial', 'initial', 'fixture', 'required', '{}',
        'failed', ?, 1, 'runtime_launch_failed', ?, ?, ?)`)
        .run(randomUUID(), turnId, conversationId, messageId, randomUUID(), now, now, now)
      database.prepare('UPDATE camp_message SET camp_turn_id = ? WHERE id = ?').run(turnId, messageId)
      for (const [index, body] of ['B', 'C'].entries()) {
        database.prepare(`INSERT INTO pending_camp_input(
          id, camp_id, enqueue_sequence, state, structured_content_json, execution_json, user_id, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, 'null', 'local_user', ?, ?)`)
          .run(randomUUID(), threadId, index + 1, backlog ? 'queued' : 'cancelled', JSON.stringify([{ kind: 'text', text: body }]), now, now)
      }
    }
    database.close()
    database = null
    core = startCore(dataDir, skillRoot, mcpPath)
    await core.ready
    await core.settled()
    for (const { threadId } of camps) {
      const direct = await sendText(threadId, 'Fresh input after restart')
      assert.equal(direct.commandResult.code, 'camp_message.sent')
    }
    await core.close()
    database = new DatabaseSync(join(dataDir, 'coaxi.sqlite'), { readOnly: true })
    for (const { threadId, backlog, turnId } of camps) {
      const turn = database.prepare('SELECT status, ended_at FROM camp_turn WHERE id = ?').get(turnId)
      assert.equal(turn.status, 'failed')
      assert.ok(turn.ended_at)
      const bodies = database.prepare('SELECT body FROM camp_message WHERE camp_id = ? ORDER BY sequence').all(threadId).map(row => row.body)
      assert.deepEqual(bodies, ['A', 'Fresh input after restart'])
      const legacy = database.prepare('SELECT state FROM pending_camp_input WHERE camp_id = ? ORDER BY enqueue_sequence').all(threadId)
      assert.deepEqual(legacy.map(row => row.state), backlog ? ['queued', 'queued'] : ['cancelled', 'cancelled'])
    }
    assert.equal(database.prepare('SELECT COUNT(*) AS count FROM agent_run').get().count, 2,
      'Startup must neither retry old Runs nor invent new Runs for this non-executing fixture')
    assert.equal(database.prepare("SELECT COUNT(*) AS count FROM agent_run WHERE status = 'failed' AND manual_retry_allowed = 1 AND last_error_code = 'runtime_launch_failed'").get().count, 2)
  } finally {
    database?.close()
    await core.close()
    await removeEphemeralRuntimeCampFilesRoot(dataDir, { temporaryDirectory: fixture })
    await rm(fixture, { recursive: true, force: true })
  }
})

test('authority recovery errors produce a retryable refusal rather than a crash or false ready', async () => {
  const fixture = await realpath(await mkdtemp(join(tmpdir(), 'coaxi-authority-recovery-')))
  const dataDir = process.platform === 'win32'
    ? JSON.parse(execFileSync(binary, ['--prepare-windows-data-root', join(fixture, 'formal')], { encoding: 'utf8' })).core
    : join(fixture, 'data')
  const skillRoot = join(dataDir, 'managed-skill-library')
  const mcpPath = join(dataDir, 'mcp.json')
  const seed = startCore(dataDir, skillRoot, mcpPath)
  let writer
  let refused
  let recovered
  try {
    await seed.ready
    await seed.settled()
    const before = await seed.request('members.list')
    await seed.close()
    writer = new DatabaseSync(join(dataDir, 'coaxi.sqlite'))
    // Valid pending authority, with a fixture-only failure at its settlement
    // boundary. A generic SQLite writer lock fails earlier during DB open/seed
    // and cannot exercise run_core's post-database_ready recovery refusal.
    writer.exec(`
      INSERT INTO planned_shutdown_cycle(core_generation, protocol_version, requested_at)
      VALUES ('acceptance-refused-settlement', 3, '2026-08-30T00:00:00Z');
      CREATE TRIGGER acceptance_refuse_settlement BEFORE UPDATE ON planned_shutdown_cycle
      WHEN OLD.core_generation = 'acceptance-refused-settlement'
      BEGIN SELECT RAISE(ABORT, 'injected controlled shutdown recovery failure'); END;
    `)
    writer.close()
    writer = null
    refused = startCore(dataDir, skillRoot, mcpPath)
    await assert.rejects(refused.ready, (error) =>
      error.code === 'authority_recovery_failed' && error.phase === 'recovering_authority'
      && error.startupStatus === 'failed' && error.retryable === true
    )
    assert.ok(refused.startupFrames.some(entry => entry.phase === 'preparing_runtime_storage'
      && entry.authorityState?.kind === 'admitted'), 'a later refusal cannot erase knowledge of an existing authority')
    await refused.close()
    assert.equal(refused.process.exitCode, 0, 'deterministic refusal must not be an unexpected Core crash')
    writer = new DatabaseSync(join(dataDir, 'coaxi.sqlite'))
    assert.equal(writer.prepare("SELECT settled_at FROM planned_shutdown_cycle WHERE core_generation = 'acceptance-refused-settlement'").get().settled_at, null)
    writer.exec('DROP TRIGGER acceptance_refuse_settlement')
    writer.close()
    writer = null
    recovered = startCore(dataDir, skillRoot, mcpPath)
    await recovered.ready
    assert.deepEqual(await recovered.request('members.list'), before)
  } finally {
    writer?.close()
    await seed.close()
    if (refused) await refused.close()
    if (recovered) await recovered.close()
    await removeEphemeralRuntimeCampFilesRoot(dataDir, { temporaryDirectory: fixture })
    await rm(fixture, { recursive: true, force: true })
  }
})

test('a startup retry cannot initialize a disappeared authority', async () => {
  const fixture = await realpath(await mkdtemp(join(tmpdir(), 'coaxi-existing-authority-fence-')))
  const dataDir = process.platform === 'win32'
    ? JSON.parse(execFileSync(binary, ['--prepare-windows-data-root', join(fixture, 'formal')], { encoding: 'utf8' })).core
    : join(fixture, 'data')
  const core = startCore(dataDir, join(dataDir, 'managed-skill-library'), join(dataDir, 'mcp.json'), ['--require-existing-authority'])
  try {
    await assert.rejects(core.ready, error => error.code === 'authority_required_existing_missing' && error.retryable === false)
    await core.close()
    assert.equal(await lstat(join(dataDir, 'coaxi.sqlite')).catch(() => null), null)
    assert.equal(await lstat(join(dataDir, 'lumen.sqlite')).catch(() => null), null)
    assert.equal(core.process.exitCode, 0)
  } finally {
    await core.close()
    await removeEphemeralRuntimeCampFilesRoot(dataDir, { temporaryDirectory: fixture })
    await rm(fixture, { recursive: true, force: true })
  }
})

function startCore(dataDir, skillRoot, mcpPath, extraArgs = [], environment = {}) {
  const spawnOptions = { cwd: repository, stdio: ['pipe', 'pipe', 'pipe'] }
  if (Object.keys(environment).length > 0) {
    spawnOptions.env = { ...process.env, ...environment }
  }
  const child = spawn(binary, [
    ...coreDataDirectoryArguments(dataDir),
    '--skill-library-root', skillRoot,
    '--mcp-config-path', mcpPath,
    ...extraArgs
  ], spawnOptions)
  const pending = new Map()
  const startupFrames = []
  let nextId = 0
  let stderr = ''
  const notifications = []
  let resolveReady
  let rejectReady
  const ready = new Promise((resolve, reject) => { resolveReady = resolve; rejectReady = reject })
  const startupTimeout = setTimeout(() => rejectReady(new Error(`Core ready timeout: ${stderr}`)), 15000)
  const closed = new Promise((resolve) => child.once('close', resolve))
  child.stdin.on('error', () => {})
  child.stderr.on('data', (chunk) => { stderr = (stderr + chunk).slice(-8000) })
  child.on('error', rejectReady)
  child.once('exit', (code) => {
    clearTimeout(startupTimeout)
    const error = new Error(`Core exited before usable ready/RPC (code=${code}): ${stderr}`)
    rejectReady(error)
    for (const request of pending.values()) request.reject(error)
    pending.clear()
  })
  const lines = createInterface({ input: child.stdout })
  lines.on('line', (line) => {
    const message = JSON.parse(line)
    if (message.kind === 'core_startup') startupFrames.push(message)
    if (message.method) notifications.push(message)
    if (message.kind === 'core_startup' && message.status === 'ready') {
      clearTimeout(startupTimeout)
      resolveReady(message)
    }
    if (message.kind === 'core_startup' && ['failed', 'blocked'].includes(message.status)) {
      clearTimeout(startupTimeout)
      rejectReady(Object.assign(new Error(message.error?.message ?? 'Startup refused'), message.error, {
        phase: message.phase, startupStatus: message.status
      }))
    }
    if (message.id && pending.has(message.id)) {
      const request = pending.get(message.id)
      pending.delete(message.id)
      if (message.error) request.reject(message.error)
      else request.resolve(message.result)
    }
  })
  async function request(method, params = {}) {
    await ready
    const id = String(++nextId)
    let timeout
    try {
      return await Promise.race([
        new Promise((resolve, reject) => {
          pending.set(id, { resolve, reject })
          child.stdin.write(`${JSON.stringify({ id, method, params })}\n`)
        }),
        new Promise((_, reject) => { timeout = setTimeout(() => reject(new Error(`${method} timed out`)), 10000) })
      ])
    } finally {
      clearTimeout(timeout)
      pending.delete(id)
    }
  }
  return {
    process: child,
    startupFrames,
    ready,
    request,
    notifications,
    async settled() {
      const deadline = Date.now() + 10000
      while (Date.now() < deadline) {
        const states = await request('runtime.subsystems.get')
        if (states.every((entry) => entry.state !== 'initializing')) return states
        await new Promise((resolve) => setTimeout(resolve, 20))
      }
      throw new Error('Optional initialization did not settle')
    },
    async close() {
      child.stdin.end()
      const timeout = setTimeout(() => child.kill('SIGKILL'), 3000)
      await closed
      clearTimeout(timeout)
      clearTimeout(startupTimeout)
      lines.close()
    }
  }
}

// RPC -> immutable search snapshot -> child environment -> restart is the owner
// seam. Input-name cases stay in runtime_startup's unit matrix.
test('startup settings keep drafts private and persist only the selected Runtime environment', { skip: process.platform === 'win32', timeout: 90_000 }, async () => {
  const fixture = await realpath(await mkdtemp(join(tmpdir(), 'coaxi-runtime-startup-settings-')))
  const dataDir = join(fixture, 'data')
  const defaultProgram = join(fixture, 'default-codex')
  const selectedProgram = join(fixture, 'custom codex')
  const marker = join(fixture, 'environment.txt')
  const shellQuote = text => `'${text.replaceAll("'", "'\\''")}'`
  await writeFile(defaultProgram, '#!/bin/sh\nprintf "codex-cli 0.153.4\\n"\n', { mode: 0o700 })
  await writeFile(selectedProgram, `#!/bin/sh
printf '%s' "\${STARTUP_TEST_VALUE-unset}" > ${shellQuote(marker)}
if [ "$1" = '--version' ]; then printf 'codex-cli 0.153.4\\n'; exit 0; fi
# An explicit check sees the draft but cannot authenticate or become Ready.
exit 1
`, { mode: 0o700 })
  const environment = { COAXI_CODEX_BIN: defaultProgram }
  let core = startCore(dataDir, join(dataDir, 'managed-skill-library'), join(dataDir, 'mcp.json'), [], environment)
  try {
    await core.ready
    await core.settled()
    const runtimeKind = 'codex-cli'
    const initial = await core.request('runtime.startup.get', { runtimeKind })
    assert.equal(initial.revision, 0)
    const configuration = { programPath: selectedProgram, environment: [{ name: 'STARTUP_TEST_VALUE', value: 'draft-private-value' }] }
    const inspected = await core.request('runtime.startup.inspect', { runtimeKind, configuration })
    assert.equal(inspected.status, 'recognized')
    assert.equal(await readFile(marker, 'utf8'), 'draft-private-value')
    assert.equal((await core.request('runtime.startup.get', { runtimeKind })).revision, 0)
    const checked = await core.request('runtime.startup.check', { runtimeKind, configuration })
    assert.equal(checked.status, 'authentication_required')
    assert.equal((await core.request('runtime.startup.get', { runtimeKind })).revision, 0)
    assert.ok(!JSON.stringify(core.notifications).includes('draft-private-value'))
    configuration.environment[0].value = 'saved-private-value'
    const saved = await core.request('runtime.startup.save', { runtimeKind, expectedRevision: 0, configuration })
    assert.equal(saved.revision, 1)
    assert.equal(await readFile(marker, 'utf8'), 'saved-private-value')
    assert.equal((await core.request('runtime.startup.get', { runtimeKind: 'pi' })).configuration.environment.length, 0)
    assert.equal((await core.request('runtime.startup.save', { runtimeKind, expectedRevision: 0, configuration })).revision, 1, 'retrying the same write is idempotent')
    await assert.rejects(core.request('runtime.startup.save', { runtimeKind, expectedRevision: 0, configuration: { ...configuration, environment: [] } }))
    await core.close()
    core = startCore(dataDir, join(dataDir, 'managed-skill-library'), join(dataDir, 'mcp.json'), [], environment)
    await core.ready
    await core.settled()
    assert.deepEqual((await core.request('runtime.startup.get', { runtimeKind })).configuration, configuration)
    assert.equal(await readFile(marker, 'utf8'), 'saved-private-value')
    await rename(selectedProgram, `${selectedProgram}.moved`)
    const health = await core.request('runtime.discovery.rescan', {})
    assert.equal(health.runtimeAvailability.find(item => item.runtimeKind === runtimeKind).discovery.discoveryStatus, 'missing', 'an invalid explicit path does not fall back to the old/default program')
    await core.request('runtime.startup.save', { runtimeKind, expectedRevision: 1, configuration: { programPath: null, environment: [] } })
    const restored = await core.request('health.check')
    assert.equal(restored.runtimeAvailability.find(item => item.runtimeKind === runtimeKind).discovery.executablePath, defaultProgram)
    assert.ok(!JSON.stringify([...core.notifications, restored]).includes('saved-private-value'))
    assert.equal(process.env.STARTUP_TEST_VALUE, undefined)
  } finally {
    await core.close()
    await removeEphemeralRuntimeCampFilesRoot(dataDir, { temporaryDirectory: fixture })
    await rm(fixture, { recursive: true, force: true })
  }
})
