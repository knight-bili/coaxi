import assert from 'node:assert/strict'
import test from 'node:test'
import { installCanvasPrivateIpc } from '../fixtures/canvas-private-ipc.mjs'

function fixture() {
  const handlers = new Map(), calls = []
  const ipc = { handle(channel, handler) { handlers.set(channel, handler) } }, original = ipc.handle
  const seam = installCanvasPrivateIpc(ipc)
  ipc.handle('rovai:request', async (_event, method, params) => {
    calls.push({ method, params: structuredClone(params) })
    return { kind: 'value', value: { status: 'applied', code: 'applied', payload: {} } }
  })
  ipc.handle('rovai:private-attachment-prepare', async (_event, request) => {
    calls.push({ request: structuredClone(request) })
    return { commandId: request.commandId, state: 'recorded' }
  })
  ipc.handle('unrelated', async () => 'unchanged')
  seam.finishRegistration(); assert.equal(ipc.handle, original)
  return { seam, calls, invoke: (channel, ...args) => handlers.get(channel)({}, ...args) }
}
const owner = { conversationId: 'private', scope: { kind: 'canvas', boardId: 'board', nodeId: 'pin' } }
const mutation = (id, type = 'swap_retained') => ({ commandId: id, command: {
  conversationId: owner.conversationId, composerScope: owner.scope, action: { type } } })

test('private IPC seam observes unrelated requests and loses only one already-committed matching reply', async () => {
  const f = fixture(); f.seam.loseNextSwap(owner)
  assert.equal(await f.invoke('unrelated'), 'unchanged')
  const foreign = mutation('foreign'); foreign.command.composerScope = { ...owner.scope, nodeId: 'other' }
  assert.equal((await f.invoke('rovai:request', 'singleChat.composerDraft.mutate', foreign)).kind, 'value')
  assert.equal((await f.invoke('rovai:request', 'singleChat.composerDraft.mutate', mutation('own'))).kind, 'failure')
  assert.equal(f.calls.length, 2, 'the dropped response follows actual handler completion')
  assert.equal((await f.invoke('rovai:request', 'singleChat.composerDraft.mutate', mutation('own'))).kind, 'value')
  assert.deepEqual(f.seam.snapshot().faults, [{ kind: 'lost_committed_swap_reply', commandId: 'own' }])
})

test('private IPC seam injects a material race before the original call and releases only the held upload result', async () => {
  const f = fixture(); f.seam.armConflict({ ...owner, attachmentRefId: 'file' })
  const params = mutation('cas', 'quote'); params.command.action.action = { type: 'remove', quoteId: 'quote' }
  await f.invoke('rovai:request', 'singleChat.composerDraft.mutate', params)
  assert.deepEqual(f.calls.map(call => call.params.command.action.type), ['remove_attachment', 'quote'])
  const gate = f.seam.holdUpload(owner), request = { commandId: 'upload', target: { kind: 'single_chat',
    conversationId: owner.conversationId, composerScope: owner.scope } }
  const foreign = { ...request, commandId: 'other-upload', target: { ...request.target, conversationId: 'other-private' } }
  assert.equal((await f.invoke('rovai:private-attachment-prepare', foreign, {})).state, 'recorded')
  assert.equal(gate.observed, null)
  let resolved = false
  const pending = f.invoke('rovai:private-attachment-prepare', request, {}).then(value => { resolved = true; return value })
  await Promise.resolve(); await Promise.resolve()
  assert.equal(gate.observed.state, 'recorded'); assert.equal(resolved, false)
  gate.release(); assert.equal((await pending).state, 'recorded')
  assert.equal(resolved, true)
})
