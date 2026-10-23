import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { mkdir, mkdtemp, realpath, symlink, unlink, writeFile } from 'node:fs/promises'
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

  stage = 'issue_74_literal_git_path'
  const literalFile = process.platform === 'win32' ? '-literal [group] entry.txt' : '-literal [group]: entry.txt'
  const literalContent = '按 Git 路径参数原样暂存和提交，不拆分空格、方括号或冒号。\n'
  await writeFile(path.join(repository, literalFile), literalContent)
  const stageRequest = { scope, action: 'stage', paths: [literalFile], commandId: randomUUID() }
  const stagePreview = await call('workbench.git.preview', stageRequest)
  const staged = await call('workbench.git.execute', {
    ...stageRequest, expectedRevision: stagePreview.revision, confirmed: true
  })
  assert.equal(staged.success, true, staged.error)
  const status = await call('workbench.git.read', { scope, action: 'status' })
  assert.equal(status.success, true, status.error)
  assert(status.output.includes(literalFile), '暂存状态应保留完整字面路径')
  const commitRequest = { scope, action: 'commit', message: '提交含特殊字符的字面路径', commandId: randomUUID() }
  const commitPreview = await call('workbench.git.preview', commitRequest)
  const committed = await call('workbench.git.execute', {
    ...commitRequest, expectedRevision: commitPreview.revision, confirmed: true
  })
  assert.equal(committed.success, true, committed.error)
  const historicalFile = await call('workbench.git.read', {
    scope, action: 'file_at', target: 'HEAD', paths: [literalFile]
  })
  assert.equal(historicalFile.success, true, historicalFile.error)
  assert.equal(historicalFile.output, literalContent)
  record('issue_74_git_preserves_literal_leading_dash_spaces_brackets_and_platform_colon', {
    path: literalFile,
    colon: process.platform === 'win32' ? 'skipped_win32_filename_not_supported' : 'tested',
    staged: true,
    committedContentMatched: true
  })

  stage = 'issue_63_managed_worktree_workspace_admission'
  git('branch', 'review-managed-worktree')
  const addWorktreeRequest = {
    scope, action: 'worktree_add', target: 'review-managed-worktree', commandId: randomUUID()
  }
  const worktreePreview = await call('workbench.git.preview', addWorktreeRequest)
  const worktreeCreated = await call('workbench.git.execute', {
    ...addWorktreeRequest, expectedRevision: worktreePreview.revision, confirmed: true
  })
  assert.equal(worktreeCreated.success, true, worktreeCreated.error)
  const worktreePath = worktreeCreated.workingDirectory
  await writeFile(path.join(evidence, 'managed-worktree-result.json'), JSON.stringify(worktreeCreated, null, 2))
  assert(path.isAbsolute(worktreePath), 'Core 创建的受管工作树应返回绝对路径')
  const worktreeInspection = await call('workspaces.inspect', { path: worktreePath })
  const worktreeSelection = await call('workspaces.validate', { path: worktreePath })
  assert.equal(worktreeSelection.projectPath, worktreeInspection.projectPath)
  const worktreeThread = await call('threads.create', {
    commandId: randomUUID(),
    name: '受管工作树工程会话',
    workspace: { projectPath: worktreeInspection.projectPath },
    memberAgentIds: preflight.presentMembers.map(member => member.agentId),
    defaultLeadAgentId: preflight.initialLeadAgentId,
    collaborationMode: 'peer',
    activationState: 'active'
  })
  assert.equal(worktreeThread.status, 'applied')
  const worktreeFiles = await call('workbench.files.list', {
    scope: { threadId: worktreeThread.payload.threadId }, path: ''
  })
  assert(worktreeFiles.entries.some(entry => entry.name === 'README.md'))

  const ordinaryDataChild = path.join(dataDir, 'ordinary-unregistered-workspace')
  await mkdir(ordinaryDataChild, { recursive: true })
  const ordinaryRefusal = await response('workspaces.inspect', { path: ordinaryDataChild })
  assert(ordinaryRefusal.error, 'Core dataDir 普通子目录仍须拒绝作为工程')
  const fakeOwnerKey = 'b'.repeat(64)
  const treesDirectory = path.join(dataDir, 'workspace-trees')
  const fakeTreePath = path.join(treesDirectory, fakeOwnerKey)
  await mkdir(fakeTreePath, { recursive: true })
  await writeFile(path.join(treesDirectory, `${fakeOwnerKey}.json`), JSON.stringify({
    root: workspace.projectPath,
    branch: 'review-forged-owner',
    path: fakeTreePath
  }))
  const fakeOwnerRefusal = await response('workspaces.inspect', { path: fakeTreePath })
  assert(fakeOwnerRefusal.error, '伪造 owner 记录且未登记的目录须拒绝')
  record('issue_63_registered_worktree_opens_as_workspace_and_unregistered_data_is_refused', {
    worktreePath,
    inspectAndValidateAccepted: true,
    newThreadCreated: true,
    workbenchFilesReadable: true,
    ordinaryDataChildRefused: true,
    forgedOwnerRefused: true
  })

  stage = 'issue_73_unready_mission_scope_refusal'
  const missionCreated = await call('missions.create', {
    commandId: randomUUID(),
    command: {
      title: '未启动 Git Mission 隔离回归',
      description: '验证旧客户端缺少 missionId 时不回落到源工程。',
      projectPath: workspace.projectPath,
      projectBindingKind: 'directory',
      memberAgentIds: preflight.presentMembers.map(member => member.agentId),
      defaultLeadAgentId: preflight.initialLeadAgentId,
      tags: []
    }
  })
  assert.equal(missionCreated.status, 'applied')
  const missionThreadId = missionCreated.payload.threadId
  const missionId = missionCreated.payload.missionId
  const mission = await call('missions.get', { missionId })
  assert.equal(mission.workspaceEverCreated, false)
  await expectCoreError('workbench.files.list', {
    scope: { threadId: missionThreadId }, path: ''
  }, 'workbench.mission_unavailable')
  await expectCoreError('workbench.files.list', {
    scope: { threadId: missionThreadId, missionId: 'wrong-mission-owner' }, path: ''
  }, 'workbench.mission_owner_changed')
  record('issue_73_unstarted_git_mission_never_falls_back_to_source_directory', {
    missionId,
    missionThreadId,
    workspaceEverCreated: mission.workspaceEverCreated,
    oldClientWithoutMissionIdRefused: true,
    wrongMissionOwnerRefused: true,
    readyWorktreePositivePath: 'not_covered_without_starting_a_real_Mission_Runtime'
  })

  stage = 'issue_71_organizer_tags_navigation_projection'
  const organizerScope = { threadId }
  const organizerBefore = await call('workbench.organizer.get', {
    scope: organizerScope, level: 'thread'
  })
  const expectedTags = ['review-tag-alpha', 'review-tag-bravo']
  const organizerSaved = await call('workbench.organizer.set', {
    scope: organizerScope,
    level: 'thread',
    expectedDigest: organizerBefore.digest,
    value: { ...organizerBefore.value, tags: expectedTags }
  })
  assert.deepEqual(organizerSaved.value.tags, expectedTags)
  assert.deepEqual((await call('workbench.organizer.get', {
    scope: organizerScope, level: 'thread'
  })).value.tags, expectedTags, 'Core organizer 应持久保存标签')
  const message = await call('thread.messages.send', {
    commandId: randomUUID(),
    threadId,
    content: { version: 2, segments: [{ kind: 'text', text: '为会话已读回执建立隔离序号。' }] },
    sourceAttachments: [],
    quotes: [],
    replyToThreadMessageId: null,
    execution: null
  })
  assert.equal(message.commandResult.status, 'applied')
  const navigationSnapshot = await call('navigation.snapshot')
  const snapshotThread = navigationSnapshot.projects
    .flatMap(group => group.recentThreads).find(item => item.id === threadId)
  assert(snapshotThread, 'navigation.snapshot 应包含测试会话')
  assert.deepEqual(snapshotThread.tags, expectedTags)
  const foundThread = await call('navigation.findThread', { threadId })
  assert.deepEqual(foundThread.tags, expectedTags)
  const listedThreads = await call('navigation.threads', { threadIds: [threadId] })
  const listedThread = listedThreads.threads.find(item => item.id === threadId)
  assert.deepEqual(listedThread?.tags, expectedTags)
  const throughGlobalSequence = navigationSnapshot.throughGlobalSequence
  assert(throughGlobalSequence > 0, '隔离消息应产生可确认的全局序号')
  const viewed = await call('navigation.campViewed', { threadId, throughGlobalSequence })
  assert.equal(viewed.threadId, threadId)
  assert.equal(viewed.changed, true)
  assert.deepEqual(viewed.navigation.threads.find(item => item.id === threadId)?.tags, expectedTags)
  const viewedReplay = await call('navigation.campViewed', { threadId, throughGlobalSequence })
  assert.equal(viewedReplay.changed, false)
  assert.equal(viewedReplay.lastSeenGlobalSequence, viewed.lastSeenGlobalSequence)
  assert.deepEqual(viewedReplay.navigation.threads.find(item => item.id === threadId)?.tags, expectedTags)
  record('issue_71_saved_thread_tags_survive_navigation_reads_and_view_receipt', {
    threadId,
    tags: expectedTags,
    projections: ['organizer.get', 'navigation.snapshot', 'navigation.findThread', 'navigation.threads', 'navigation.campViewed'],
    viewedSequence: throughGlobalSequence,
    replayChanged: viewedReplay.changed
  })

  stage = 'issue_77_external_directory_alias_read_and_search'
  const externalDirectory = path.join(fixture, 'external specs')
  await mkdir(externalDirectory)
  await writeFile(path.join(externalDirectory, 'guide.txt'), 'SAFE_EXTERNAL_ALIAS_MARKER\n')
  assert(!path.resolve(externalDirectory).startsWith(path.resolve(repository)))
  const symlinkProbe = path.join(fixture, 'directory-link-capability-probe')
  let directoryLinksAvailable = true
  try {
    await symlink(externalDirectory, symlinkProbe, 'dir')
  } catch (error) {
    if (process.platform === 'win32' && ['EPERM', 'EACCES'].includes(error?.code)) {
      directoryLinksAvailable = false
    } else {
      throw error
    }
  }
  if (directoryLinksAvailable) await unlink(symlinkProbe)
  if (!directoryLinksAvailable) {
    record('issue_77_external_alias_is_scoped_and_searchable_from_isolated_fixture', {
      status: 'skipped',
      reason: 'Windows 当前用户没有创建目录符号链接的权限',
      externalFixtureOnly: true
    }, 'skipped')
  } else {
    const organizerBeforeAlias = await call('workbench.organizer.get', {
      scope: organizerScope, level: 'thread'
    })
    const aliasSaved = await call('workbench.organizer.set', {
      scope: organizerScope,
      level: 'thread',
      expectedDigest: organizerBeforeAlias.digest,
      value: {
        ...organizerBeforeAlias.value,
        directoryAliases: [{ path: externalDirectory, name: 'external-specs', group: 'acceptance' }]
      }
    })
    assert.equal(aliasSaved.value.directoryAliases[0].name, 'external-specs')
    const linkedEntries = await call('workbench.files.list', { scope: organizerScope, path: '' })
    const linkedDirectory = linkedEntries.entries.find(entry => entry.name === 'external-specs')
    assert.equal(linkedDirectory?.managedAlias, true)
    assert.equal(linkedDirectory?.directory, true)
    const aliasedFile = await call('workbench.files.read', {
      scope: organizerScope, path: 'external-specs/guide.txt'
    })
    assert.equal(aliasedFile.text, 'SAFE_EXTERNAL_ALIAS_MARKER\n')
    const search = await call('workbench.files.tools', {
      scope: organizerScope,
      operation: 'search',
      path: 'external-specs',
      query: 'SAFE_EXTERNAL_ALIAS_MARKER',
      content: true,
      caseSensitive: true
    })
    assert(search.results.some(item => item.path === 'external-specs/guide.txt'))
    record('issue_77_external_alias_is_scoped_and_searchable_from_isolated_fixture', {
      aliasPath: externalDirectory,
      aliasReadMatched: true,
      searchMatched: true,
      externalFixtureOnly: true
    })
  }

  stage = 'issue_78_history_rename_delete_and_merge_parent_diff'
  const historicalRead = async (target, filePath) => call('workbench.git.read', {
    scope, action: 'file_at', target, paths: [filePath]
  })
  const baseFile = await historicalRead(historyBaseCommit, 'old history [record].txt')
  assert.equal(baseFile.success, true, baseFile.error)
  assert.equal(baseFile.output, originalHistoryText, '历史全文应完整返回，不能截断')
  git('mv', '--', 'old history [record].txt', 'renamed history [record].txt')
  gitCommit('重命名历史记录文件')
  const renamedCommit = git('rev-parse', 'HEAD').trim()
  const renamedFile = await historicalRead(renamedCommit, 'renamed history [record].txt')
  assert.equal(renamedFile.success, true, renamedFile.error)
  assert.equal(renamedFile.output, originalHistoryText)
  git('rm', '--', 'renamed history [record].txt')
  gitCommit('删除历史记录文件')
  const deletedCommit = git('rev-parse', 'HEAD').trim()
  const deletedFile = await historicalRead(deletedCommit, 'renamed history [record].txt')
  assert.equal(deletedFile.success, false, '删除版本不应返回已不存在文件的内容')

  await writeFile(path.join(repository, 'main-only.txt'), 'main baseline\n')
  gitCommit('建立合并第一父提交基线')
  git('switch', '-c', 'review-merge-feature')
  await writeFile(path.join(repository, 'feature-only.txt'), 'second parent change\n')
  gitCommit('建立合并第二父提交改动')
  git('switch', 'main')
  await writeFile(path.join(repository, 'first-parent-only.txt'), 'first parent baseline\n')
  gitCommit('建立合并第一父提交改动')
  const firstParent = git('rev-parse', 'HEAD').trim()
  git('merge', '--no-ff', '-m', '建立第二父提交差异', 'review-merge-feature')
  const mergeCommit = git('rev-parse', 'HEAD').trim()
  assert.equal(git('show', '-s', '--format=%P', mergeCommit).trim().split(/\s+/).length, 2)
  const mergeParentDiff = await call('workbench.git.read', {
    scope, action: 'diff_compare', target: `${firstParent}...${mergeCommit}`
  })
  assert.equal(mergeParentDiff.success, true, mergeParentDiff.error)
  assert(mergeParentDiff.output.includes('feature-only.txt'), '与第一父提交比较应显示第二父提交带来的文件')
  assert(!mergeParentDiff.output.includes('first-parent-only.txt'), '第一父提交已有的改动不应伪装成第二父差异')
  record('issue_78_file_history_is_complete_and_merge_diff_uses_second_parent_changes', {
    baseCommit: historyBaseCommit,
    renamedCommit,
    deletedCommit,
    mergeCommit,
    parentCount: 2,
    historicalTextBytes: Buffer.byteLength(originalHistoryText),
    secondParentChangeVisible: true
  })

  stage = 'issue_79_branch_comparison'
  git('switch', '-c', 'review-branch-compare')
  await writeFile(path.join(repository, 'branch-only.txt'), 'branch comparison target\n')
  gitCommit('建立分支比较目标改动')
  const comparisonBranch = git('rev-parse', 'HEAD').trim()
  git('switch', 'main')
  const branchDiff = await call('workbench.git.read', {
    scope,
    action: 'diff_compare',
    target: 'refs/heads/main...refs/heads/review-branch-compare'
  })
  assert.equal(branchDiff.success, true, branchDiff.error)
  assert(branchDiff.output.includes('branch-only.txt'), '分支比较应显示目标分支新增内容')
  record('issue_79_branch_comparison_executes_against_validated_target', {
    baseBranch: 'refs/heads/main',
    targetBranch: 'refs/heads/review-branch-compare',
    targetCommit: comparisonBranch,
    changedPathVisible: true
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
  const errorText = error instanceof Error ? (error.stack ?? String(error)) : JSON.stringify(error)
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
