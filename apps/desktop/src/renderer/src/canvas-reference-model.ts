// Codeg aff1e6c278453ed8d0b96a5f664edcdca621aa3f region membership, adapted to Core Thread/private identities.
// Apache-2.0: integration/licenses/codeg/LICENSE. Business relationships remain Core-owned.
import type { CanvasConversationRef, CanvasNode, CanvasThreadReference } from '@contracts'
import type { CanvasFolderGroupOption, CanvasReferenceOption } from './CanvasNodeEditor'
import type { CanvasReferenceState } from './canvas-reference-controller'
import { canvasReferenceKey } from './canvas-layout'
import { uiAttribute } from './interface-language'

export interface CanvasFolderResolution { paths: readonly string[]; complete: boolean; issue?: string }
export interface CanvasRegionResolution { references: CanvasReferenceOption[]; complete: boolean; issue?: string }
const recency = (value: string) => Number.isFinite(Date.parse(value)) ? Date.parse(value) : 0
function optionsForThread(thread: CanvasThreadReference, memberIds?: ReadonlySet<string>): Array<CanvasReferenceOption & { updatedAt: string }> {
  const title = thread.missionId ? uiAttribute('使命：{0}', thread.title) : thread.title
  return [{ reference: { threadId: thread.threadId, conversationId: null }, label: title, available: true, updatedAt: thread.updatedAt },
    ...thread.privateConversations.filter(conversation => !memberIds || memberIds.has(conversation.agentId)).map(conversation => ({
      reference: { threadId: thread.threadId, conversationId: conversation.conversationId }, available: true, updatedAt: conversation.updatedAt,
      label: uiAttribute('{0} · {1} 的单聊', title, thread.members.find(member => member.agentId === conversation.agentId)?.displayName ?? uiAttribute('队员'))
    }))]
}
const byRecency = (a: CanvasReferenceOption & { updatedAt: string }, b: CanvasReferenceOption & { updatedAt: string }) => {
  const left = canvasReferenceKey(a.reference), right = canvasReferenceKey(b.reference)
  return recency(b.updatedAt) - recency(a.updatedAt) || (left < right ? -1 : left > right ? 1 : 0)
}
export function canvasReferenceOptions(items: readonly CanvasThreadReference[]): CanvasReferenceOption[] {
  return items.flatMap(thread => optionsForThread(thread)).toSorted(byRecency)
}
export function canvasSavedReference(reference: CanvasConversationRef, catalog: CanvasReferenceState): CanvasReferenceOption {
  const thread = catalog.items.find(item => item.threadId === reference.threadId)
  const option = thread && optionsForThread(thread).find(item => canvasReferenceKey(item.reference) === canvasReferenceKey(reference))
  if (option) return option
  return { reference, label: thread && reference.conversationId ? uiAttribute('{0} · 单聊不可用', thread.title)
    : uiAttribute(catalog.complete ? '对话已结束或不可用' : '尚未核对的对话'), available: catalog.complete ? false : undefined }
}

/** Every member/runtime condition applies to the same actual active membership, never the default Lead alone. */
export function resolveCanvasRegion(node: CanvasNode, catalog: CanvasReferenceState, groups: readonly CanvasFolderGroupOption[],
  folders?: ReadonlyMap<string, CanvasFolderResolution>): CanvasRegionResolution {
  const data = node.data
  let complete = catalog.complete, issue = catalog.error ? uiAttribute('对话清单读取未完成') : undefined
  if (data.kind === 'custom') {
    const options = canvasReferenceOptions(catalog.items)
    const known = new Map(options.map(option => [canvasReferenceKey(option.reference), option]))
    const ranks = new Map(options.map((option, index) => [canvasReferenceKey(option.reference), index]))
    return { references: data.conversations.map(reference => known.get(canvasReferenceKey(reference)) ?? canvasSavedReference(reference, catalog))
      .toSorted((a, b) => (ranks.get(canvasReferenceKey(a.reference)) ?? Number.MAX_SAFE_INTEGER) - (ranks.get(canvasReferenceKey(b.reference)) ?? Number.MAX_SAFE_INTEGER)), complete, issue }
  }
  const selected: Array<CanvasReferenceOption & { updatedAt: string }> = []
  if (data.kind === 'agent') {
    for (const thread of catalog.items) {
      const members = thread.members.filter(member => (!data.agentId || member.agentId === data.agentId) && (!data.runtimeKind || member.runtimeKind === data.runtimeKind))
      if (members.length) selected.push(...optionsForThread(thread, new Set(members.map(member => member.agentId))))
    }
  } else if (data.kind === 'folder' || data.kind === 'group') {
    const group = groups.find(group => group.id === data.folderGroupId)
    const roots = data.kind === 'folder' && data.folderPath ? [data.folderPath] : group?.projectPaths
    if (!roots) return { references: [], complete: false, issue: uiAttribute('项目分组在当前工作区不可用') }
    const paths = new Set<string>()
    for (const root of roots) {
      const related = folders?.get(root)
      for (const path of related?.paths ?? [root]) paths.add(path)
      if (!related?.complete) { complete = false; issue = related?.issue || uiAttribute('工作树关系尚未核对') }
    }
    for (const thread of catalog.items) if (thread.projectBindingKind === 'directory' && paths.has(thread.projectPath)) selected.push(...optionsForThread(thread))
  }
  return { references: selected.toSorted(byRecency), complete, issue }
}
