import { expect, it } from 'vitest'
import type { CanvasNode, CanvasNodeData, CanvasThreadReference } from '@contracts'
import type { CanvasReferenceState } from './canvas-reference-controller'
import { canvasReferenceOptions, canvasSavedReference, resolveCanvasRegion } from './canvas-reference-model'

const thread = (threadId: string, projectPath = '/project'): CanvasThreadReference => ({ threadId, projectPath, projectBindingKind: 'directory',
  title: threadId, updatedAt: '2026-10-15T00:00:00Z', version: 1, missionId: null,
  members: [{ agentId: 'a', displayName: 'A', runtimeKind: 'codex-cli' }, { agentId: 'b', displayName: 'B', runtimeKind: 'gemini-cli' }],
  privateConversations: [{ conversationId: `${threadId}-private-a`, agentId: 'a', updatedAt: '2026-10-15T00:00:01Z' },
    { conversationId: `${threadId}-private-b`, agentId: 'b', updatedAt: '2026-10-15T00:00:02Z' }] })
const catalog = (...items: CanvasThreadReference[]): CanvasReferenceState => ({ items, complete: true, loading: false, error: null, pagesRead: 1 })
const node = (patch: Partial<CanvasNodeData>): CanvasNode => ({ id: 'node', boardId: 'board', createdAt: 'time', updatedAt: 'time', data: {
  kind: 'agent', agentId: null, runtimeKind: null, conversation: null, conversations: [], folderPath: null, folderGroupId: null, path: null,
  title: null, content: null, color: null, collapsed: false, gridColumns: 0, gridRows: 0, x: 0, y: 0, width: 400, height: 300, ...patch } })

it('keeps public, private and Mission-backed references distinct and only declares a missing identity unavailable after a full read', () => {
  const state = catalog(thread('same'), { ...thread('mission'), missionId: 'm' }), before = structuredClone(state)
  const options = canvasReferenceOptions(state.items)
  expect(options).toHaveLength(6)
  expect(options.filter(item => item.reference.threadId === 'same').map(item => item.reference.conversationId)).toEqual(['same-private-b', 'same-private-a', null])
  expect(options.some(item => item.label === '使命：mission')).toBe(true)
  const missing = { threadId: 'same', conversationId: 'ended-private' }
  expect(canvasSavedReference(missing, state)).toMatchObject({ reference: missing, available: false })
  expect(canvasSavedReference(missing, { ...state, complete: false }).available).toBeUndefined()
  expect(state).toEqual(before)
})

it('matches the same active teammate for identity and runtime, including only that teammate’s private conversations', () => {
  const state = catalog(thread('same'))
  expect(resolveCanvasRegion(node({ agentId: 'a', runtimeKind: 'gemini-cli' }), state, []).references).toEqual([])
  const references = resolveCanvasRegion(node({ agentId: 'a', runtimeKind: 'codex-cli' }), state, []).references
  expect(references.map(item => item.reference.conversationId)).toEqual(['same-private-a', null])
  expect(resolveCanvasRegion(node({ runtimeKind: 'gemini-cli' }), state, []).references.map(item => item.reference.conversationId)).toEqual(['same-private-b', null])
})

it('uses explicit parent/worktree relations for folders and groups and does not mistake missing relations for a complete empty region', () => {
  const state = catalog(thread('root'), thread('child', '/linked'), thread('other', '/elsewhere'))
  const folders = new Map<string, { paths: string[]; complete: boolean; parentPath?: string }>([['/project', { paths: ['/project', '/linked'], complete: true }], ['/linked', { paths: ['/linked'], complete: true }]])
  const folder = node({ kind: 'folder', folderPath: '/project' })
  expect(new Set(resolveCanvasRegion(folder, state, [], folders).references.map(item => item.reference.threadId))).toEqual(new Set(['root', 'child']))
  expect(new Set(resolveCanvasRegion(node({ kind: 'folder', folderPath: '/linked' }), state, [], folders).references.map(item => item.reference.threadId))).toEqual(new Set(['child']))
  expect(resolveCanvasRegion(folder, state, []).complete).toBe(false)
  const group = node({ kind: 'group', folderGroupId: 'group' })
  expect(resolveCanvasRegion(group, state, []).complete).toBe(false)
  expect(resolveCanvasRegion(group, state, [{ id: 'group', label: 'group', projectPaths: ['/project', '/linked'] }], folders).references).toHaveLength(6)
  folders.set('/linked', { paths: ['/linked'], complete: true, parentPath: '/project' })
  expect(resolveCanvasRegion(group, state, [{ id: 'group', label: 'child-only', projectPaths: ['/linked'] }], folders).references).toEqual([])
  expect(resolveCanvasRegion(node({ kind: 'folder', folderPath: '/linked' }), state, [], folders).references).toHaveLength(3)
})

it('retains unavailable curated references without silently deleting them or replacing a private reference with a public Thread', () => {
  const state = catalog(thread('same'))
  const selected = [{ threadId: 'same', conversationId: null }, { threadId: 'same', conversationId: 'same-private-a' }, { threadId: 'missing', conversationId: 'private' }]
  const region = node({ kind: 'custom', conversations: selected }), before = structuredClone(region)
  const resolved = resolveCanvasRegion(region, state, [])
  expect(resolved.references.map(item => item.reference)).toEqual([selected[1], selected[0], selected[2]])
  expect(resolved.references[2].available).toBe(false); expect(region).toEqual(before)
})
