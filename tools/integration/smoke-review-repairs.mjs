import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { mkdir, mkdtemp, realpath, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { DatabaseSync } from 'node:sqlite'
import { launchHost, within } from '../../scripts/lib/host-test-client.mjs'
import { coreDataDirectoryArguments } from '../../scripts/lib/runtime-camp-files-root.mjs'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..')
const binary = process.env.ROVAI_HOST_BIN
const ui = process.env.ROVAI_WEB_UI
const evidenceRoot = process.env.WORKBENCH_EVIDENCE_ROOT
assert(binary && ui && evidenceRoot && [binary, ui, evidenceRoot].every(path.isAbsolute),
  'ROVAI_HOST_BIN、ROVAI_WEB_UI、WORKBENCH_EVIDENCE_ROOT 必须是绝对路径')
await mkdir(evidenceRoot, { recursive: true })
const fixture = await realpath(await mkdtemp(path.join(evidenceRoot, 'review-repairs-')))
const formal = path.join(fixture, 'formal')
const dataDir = process.platform === 'win32'
  ? JSON.parse(execFileSync(binary, ['--prepare-windows-data-root', formal], { encoding: 'utf8', windowsHide: true })).core
  : formal
const skillRoot = path.join(dataDir, 'skills')
const mcp = path.join(dataDir, 'mcp.json')
await mkdir(skillRoot, { recursive: true })
const evidence = path.join(fixture, 'evidence')
await mkdir(evidence)
const repository = path.join(fixture, 'review repository')
await mkdir(repository)

function git(...args) {
  return execFileSync('git', ['-C', repository, ...args], { encoding: 'utf8', windowsHide: true })
}

function gitCommit(message) {
  git('add', '--all')
  git('-c', 'user.name=Rovai Acceptance', '-c', 'user.email=acceptance@example.invalid', 'commit', '-m', message)
}

function commitCount() {
  return Number(git('rev-list', '--count', 'HEAD').trim())
}

git('init')
git('branch', '-M', 'main')
git('config', 'core.autocrlf', 'false')
git('config', 'user.name', 'Rovai Acceptance')
git('config', 'user.email', 'acceptance@example.invalid')
const originalHistoryText = Array.from({ length: 1024 }, (_, index) => `历史完整内容第 ${index + 1} 行：用于核对文件版本不截断。\n`).join('')
await writeFile(path.join(repository, 'old history [record].txt'), originalHistoryText)
await writeFile(path.join(repository, 'README.md'), '# Isolated engineering regression fixture\n')
gitCommit('建立隔离回归仓库')
const historyBaseCommit = git('rev-parse', 'HEAD').trim()

const checks = []
let host
let stage = 'start_host'
let finalResult
const record = (id, detail = {}, status = 'passed') => {
  checks.push({ id, status, ...detail })
  console.log(JSON.stringify({ stage: id, status, cases: checks.length }))
}

async function response(operation, params = {}) {
  const result = await fetch(`${origin}/api/v1/request`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify({ operation, params }),
    signal: AbortSignal.timeout(15000)
  })
  assert.equal(result.status, 200, operation)
  return result.json()
}

let origin, token
try {
  host = launchHost(binary, [...coreDataDirectoryArguments(dataDir), '--skill-library-root', skillRoot, '--mcp-config-path', mcp], { cwd: root, windowsHide: true })
  await within(host.ready)
  const started = await host.request('host.web.start', { listen: '127.0.0.1:0', uiDirectory: ui })
  origin = started.origin
  assert(origin.startsWith('http://127.0.0.1:'))
  const login = await fetch(`${origin}/api/v1/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ protocolVersion: 4, administratorToken: started.administratorToken }),
    signal: AbortSignal.timeout(15000)
  })
  assert.equal(login.status, 200)
  token = (await login.json()).token

  const call = async (operation, params = {}) => {
    const body = await response(operation, params)
    assert.equal(body.error, null, JSON.stringify({ operation, error: body.error }))
    return body.result
  }
  const expectCoreError = async (operation, params, code) => {
    const body = await response(operation, params)
    assert(body.error, `${operation} should be refused with ${code}`)
    assert.match(JSON.stringify(body.error), new RegExp(code.replaceAll('.', '\\.')))
    return body.error
  }

  stage = 'create_real_core_thread'
  const workspace = await call('workspaces.inspect', { path: repository })
  const preflight = await call('threads.creationPreflight')
  assert(preflight.admissible && preflight.initialLeadAgentId)
  assert(preflight.presentMembers.every(member => !member.runtimeConfigured), '回归线程不得需要或启动 Runtime')
  const created = await call('threads.create', {
    commandId: randomUUID(),
    name: '工程修复真实 Host 回归',
    workspace: { projectPath: workspace.projectPath },
    memberAgentIds: preflight.presentMembers.map(member => member.agentId),
    defaultLeadAgentId: preflight.initialLeadAgentId,
    collaborationMode: 'peer',
    activationState: 'active'
  })
  assert.equal(created.status, 'applied')
  const threadId = created.payload.threadId
  const scope = { threadId }
  record('isolated_real_host_core_thread', { threadId, workspaceRoot: workspace.projectPath, runtimeInvoked: false })

  stage = 'issue_62_git_uuid_receipt_replay'
  await writeFile(path.join(repository, 'request replay.txt'), '初次命令提交内容。\n')
  git('add', '--', 'request replay.txt')
  const originalRequest = { scope, action: 'commit', message: '通过工作区命令提交一次', commandId: randomUUID() }
  const preview = await call('workbench.git.preview', originalRequest)
  const frozenRequest = { ...originalRequest, expectedRevision: preview.revision, confirmed: true }
  const firstResult = await call('workbench.git.execute', frozenRequest)
  assert.equal(firstResult.success, true, firstResult.error)
  const firstHead = git('rev-parse', 'HEAD').trim()
  const firstCount = commitCount()
  await writeFile(path.join(repository, 'state changed after receipt.txt'), '在回放前改变 Git 状态。\n')
  gitCommit('回执产生后由隔离夹具改变 Git 状态')
  const changedHead = git('rev-parse', 'HEAD').trim()
  assert.notEqual(changedHead, firstHead)
  const changedCount = commitCount()
  const replay = await call('workbench.git.execute', frozenRequest)
  assert.deepEqual(replay, firstResult, '同一 commandId 必须返回第一次执行回执')
  assert.equal(git('rev-parse', 'HEAD').trim(), changedHead, '重放不能新建提交或改变当前 HEAD')
  assert.equal(commitCount(), changedCount, '重放不能产生第二个提交')
  assert.equal(changedCount, firstCount + 1, '仅夹具的状态变化应新增一个提交')
  record('issue_62_git_command_replays_original_receipt_after_repository_changes', {
    commandId: originalRequest.commandId,
    originalHead: firstHead,
    changedHead,
    commitCountBeforeReplay: changedCount,
    replayCreatedCommit: false
  })

  finalResult = {
    status: 'passed',
    scope: 'real_host_core_isolated_git_fixture_no_model_execution',
    evidenceDirectory: evidence,
    dataDirectory: dataDir,
    skillLibraryRoot: skillRoot,
    mcpConfigPath: mcp,
    checks
  }
} catch (error) {
  const errorText = error instanceof Error ? String(error) : JSON.stringify(error)
  await writeFile(path.join(evidence, 'failure.json'), JSON.stringify({ stage, error: errorText, checks }, null, 2))
  console.log(JSON.stringify({ status: 'failed', stage, evidence, error: errorText.slice(0, 1200) }))
  process.exitCode = 1
} finally {
  if (host) await host.close()
  if (finalResult) {
    const database = new DatabaseSync(path.join(dataDir, 'rovai.sqlite'), { readOnly: true })
    try {
      assert.equal(database.prepare('SELECT count(*) AS n FROM agent_run').get().n, 0, '禁止启动真实 Runtime')
      assert.equal(Object.values(database.prepare('PRAGMA integrity_check').get())[0], 'ok')
    } finally { database.close() }
    record('isolated_core_database_integrity_and_zero_agent_runs')
    await writeFile(path.join(evidence, 'result.json'), JSON.stringify(finalResult, null, 2))
    console.log(JSON.stringify({ status: finalResult.status, cases: checks.length, evidence }))
  }
}
