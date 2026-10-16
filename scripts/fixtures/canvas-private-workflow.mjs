import assert from 'node:assert/strict'
import { randomUUID } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { setTimeout as pause } from 'node:timers/promises'

const terminal = run => ['succeeded', 'failed', 'cancelled'].includes(run.status)
export async function waitPrivateSnapshot(rpc, conversationId, predicate, timeout = 60_000) {
  const deadline = Date.now() + timeout
  let snapshot
  do {
    snapshot = await rpc('singleChat.get', { conversationId })
    if (predicate(snapshot)) return snapshot
    await pause(100)
  } while (Date.now() < deadline)
  throw new Error(`Private workflow condition timed out: ${JSON.stringify({ conversation: snapshot?.conversation,
    runs: snapshot?.agentRuns.map(run => ({ id: run.id, status: run.status, error: run.errorCode })), pending: snapshot?.pendingInputs.items.length })}`)
}

// Shared by Native UI scenarios and their real-Core fixture preflight. All data
// is created through production commands; this never seeds pending/history rows.
export async function preparePrivateWorkflow({ rpc, config, record }) {
  const member = await rpc('members.create', { commandId: randomUUID(), command: {
    displayName: `R01 固定协议队员 ${randomUUID().slice(0, 8)}`, teamRole: '隔离页面验证',
    professionalResponsibilities: '只使用本机固定ACP协议，不连接外部服务。', personalityTraits: ['严谨'],
    workingPrinciples: '固定协议响应，测试材料仅在隔离目录。', growthTopic: '' } })
  assert.equal(member.status, 'applied')
  const agentId = member.resultEntity.entityId
  const { configureProductRuntime } = await import(config.runtime_configure_module)
  await configureProductRuntime(rpc, 'qwen-code', [agentId])
  const created = await rpc('camps.create', { commandId: randomUUID(), name: 'R01 私聊完整输入', workspace: null,
    memberAgentIds: [agentId], defaultLeadAgentId: agentId, collaborationMode: 'peer' })
  assert.equal(created.status, 'applied')
  const threadId = created.payload.threadId
  const opened = await rpc('singleChat.open', { commandId: randomUUID(), command: { threadId, agentId } })
  assert.equal(opened.status, 'applied')
  const conversationId = opened.payload.conversationId
  const primeBody = 'R01 可引用原文：甲材料对应的问题；乙材料对应的问题。'
  const initial = await rpc('singleChat.get', { conversationId })
  const sent = await rpc('singleChat.send', { commandId: randomUUID(), command: {
    threadId, conversationId, body: primeBody, draftRevision: initial.draft.revision } })
  assert.equal(sent.status, 'accepted', JSON.stringify(sent))
  const primed = await waitPrivateSnapshot(rpc, conversationId, value => value.agentRuns.length > 0 && value.agentRuns.every(terminal))
  assert.ok(primed.agentRuns.every(run => run.status === 'succeeded'), JSON.stringify(primed.agentRuns))
  const message = primed.messages.find(item => item.authorType === 'user' && item.body === primeBody)
  assert.ok(message)
  const board = await rpc('canvas.boards.create', { commandId: randomUUID(), command: { name: 'R01 完整输入与撤销' } })
  assert.equal(board.status, 'applied')
  const boardId = board.payload.board.id, scopes = []
  for (let index = 0; index < 3; index++) {
    const graph = await rpc('canvas.nodes.snapshot', { boardId })
    const pin = await rpc('canvas.nodes.apply', { commandId: randomUUID(), command: { boardId,
      expectedRevision: graph.board.revision, mutation: { type: 'create', node: { kind: 'conversation',
        title: ['甲卡完整输入', '乙卡独立输入', '只读空卡'][index], conversation: { threadId, conversationId },
        x: index * 560, y: 0, width: 520, height: 650 } } } })
    assert.equal(pin.status, 'applied')
    scopes.push({ kind: 'canvas', boardId, nodeId: pin.payload.nodeIds[0] })
  }
  record('controlled_local_acp_creates_real_private_history_and_three_unregistered_pins_without_database_seeding')
  return { threadId, conversationId, agentId, boardId, scopes, quoteMessage: message,
    primeRunIds: primed.agentRuns.map(run => run.id) }
}

export async function beginHeldPrivateRun(rpc, context, capture) {
  const { threadId, conversationId } = context
  const snapshot = await rpc('singleChat.get', { conversationId })
  const body = `ROVAI_SKILL_RECOVERY_HELD_INPUT R01 ${randomUUID()}`
  const sent = await rpc('singleChat.send', { commandId: randomUUID(), command: {
    threadId, conversationId, body, draftRevision: snapshot.draft.revision } })
  assert.equal(sent.status, 'accepted', JSON.stringify(sent))
  const held = await waitPrivateSnapshot(rpc, conversationId, value => value.agentRuns.some(run => run.status === 'running'))
  const deadline = Date.now() + 30_000
  while (!(await readFile(capture, 'utf8')).includes(body)) {
    assert.ok(Date.now() < deadline, 'the owned ACP must actually receive the held input')
    await pause(100)
  }
  return held.agentRuns.find(run => run.id === held.conversation.activeAgentRunId)?.id
}

export async function qualifyCanvasPrivateWorkflow(window, { rpc, evaluate, wait, openCanvasBoard, config, report, record }) {
  assert.ok(['private-roundtrip', 'private-queue'].includes(config.mode))
  window.unmaximize(); window.setContentSize(1680, 1000); window.webContents.setZoomFactor(1)
  const context = await preparePrivateWorkflow({ rpc, config, record })
  report.private_workflow_context = context
  const { threadId, conversationId, boardId, scopes, quoteMessage } = context
  const frame = index => `document.querySelector('.canvas-conversation-frame[data-canvas-conversation-key*="${scopes[index].nodeId}"]')`
  const input = index => `${frame(index)}?.querySelector('.single-chat-composer textarea')`
  const get = scope => rpc('singleChat.get', { conversationId, composerScope: scope })
  const mutate = async (scope, action) => {
    const current = await get(scope)
    return rpc('singleChat.composerDraft.mutate', { commandId: randomUUID(), command: {
      threadId, conversationId, composerScope: scope, expectedDraftRevision: current.draft.revision, action } })
  }
  const names = value => value.attachments.map(item => item.displayName)
  const materials = value => ({ attachments: value.attachments, quotes: value.quotes })
  const addMaterial = async (scope, name, quoteText) => {
    const current = await get(scope)
    const request = { commandId: randomUUID(), threadId, expectedRevision: current.draft.revision,
      target: { kind: 'single_chat', conversationId, composerScope: scope } }
    const added = await evaluate(`window.rovai.singleChatAttachments.prepareInput(${JSON.stringify(request)},new File([${JSON.stringify(name)}],${JSON.stringify(name)},{type:'text/plain'}))`)
    assert.equal(added.state, 'recorded'); assert.equal(added.receipt.status, 'applied')
    const startScalar = [...quoteMessage.body.slice(0, quoteMessage.body.indexOf(quoteText))].length
    assert.ok(quoteMessage.body.includes(quoteText))
    const quoted = await mutate(scope, { type: 'quote', action: { type: 'add', selection: {
      messageId: quoteMessage.id, bodyAtSelection: quoteMessage.body, startScalar,
      endScalar: startScalar + [...quoteText].length, text: quoteText } } })
    assert.equal(quoted.status, 'applied', JSON.stringify(quoted))
    return get(scope)
  }
  const clickNative = async expression => {
    await wait(`Boolean((${expression})&&!(${expression}).disabled&&(${expression}).checkVisibility())`)
    const point = await evaluate(`(()=>{const element=${expression},r=element.getBoundingClientRect(),x=r.x+r.width/2,y=r.y+r.height/2,hit=document.elementFromPoint(x,y);if(!hit||!(hit===element||element.contains(hit)))throw Error('真实输入目标被遮挡');return {x:Math.round(x),y:Math.round(y)}})()`)
    for (const type of ['mouseMove', 'mouseDown', 'mouseUp']) window.webContents.sendInputEvent({ type, ...point,
      ...(type === 'mouseMove' ? {} : { button: 'left', clickCount: 1 }) })
  }
  const typeBody = async (index, text, selector = input(index)) => {
    await wait(`Boolean(${selector}&&!${selector}.disabled&&!${selector}.readOnly)`)
    await evaluate(`(()=>{const input=${selector};input.focus();input.select();window.r01TrustedInputs=[];input.addEventListener('input',event=>window.r01TrustedInputs.push(event.isTrusted),{once:true});return true})()`)
    await window.webContents.insertText(text)
    await wait(`${selector}.value===${JSON.stringify(text)}`)
    assert.ok((await evaluate('window.r01TrustedInputs')).includes(true))
  }
  const open = async index => {
    const button = `document.querySelector('[data-id="${scopes[index].nodeId}"] button[aria-label="展开对话"]')`
    await wait(`Boolean(${button}&&!${button}.disabled)`)
    await evaluate(`(()=>{${button}.click();return true})()`)
    await wait(`Boolean(${input(index)}&&!${input(index)}.disabled)`)
  }
  try {
    await openCanvasBoard(evaluate, wait, boardId)
    await open(2)
    const recovery = await rpc('singleChat.composerDraft.recovery', { threadId, conversationId })
    assert.ok(!recovery.items.some(item => item.kind === 'scope' && item.scope.composerScope?.nodeId === scopes[2].nodeId))
    assert.equal((await get(scopes[2])).draft.revision, 0)
    assert.equal((await get(null)).agentRuns.length, context.primeRunIds.length)
    record('actual_opening_of_empty_private_pin_reads_history_without_registering_scope_or_starting_a_run')
    await evaluate(`(()=>{document.querySelector('[data-id="${scopes[2].nodeId}"] button[aria-label="收起对话"]').click();return true})()`)
    await wait(`!${frame(2)}`)
    await open(0); await open(1)
    await evaluate(`(()=>{document.querySelector('.canvas-dock-panel button[aria-label="查看全部"]').click();return true})()`)
    if (config.mode === 'private-roundtrip') {
      await addMaterial(scopes[0], 'R01 甲卡材料.txt', '甲材料对应的问题')
      const text = '甲卡保持的未发送正文\n撤销与重做探针'
      await typeBody(0, text)
      await typeBody(1, '乙卡另一份未发送正文')
      await wait(`${frame(0)}.querySelector('.single-chat-composer .message-quotes-trigger')?.textContent.includes('1')`)
      const before = await get(scopes[0]), beta = await get(scopes[1])
      await evaluate(`(()=>{window.r01Editor=${input(0)};${input(0)}.focus();return true})()`)
      window.webContents.undo()
      await wait(`${input(0)}.value!==${JSON.stringify(text)}`)
      const undoValue = await evaluate(`${input(0)}.value`)
      window.webContents.redo(); await wait(`${input(0)}.value===${JSON.stringify(text)}`)
      await clickNative(`document.querySelector('[data-id="${scopes[0].nodeId}"] .canvas-card-handle button[aria-label="移至侧边"]')`)
      await wait(`Boolean(${frame(0)}?.querySelector('button[aria-label="放回画布"]'))`)
      assert.ok(await evaluate(`${input(0)}===window.r01Editor`))
      assert.deepEqual((await get(scopes[0])).draft, before.draft)
      await evaluate(`(()=>{${input(0)}.focus();return true})()`)
      window.webContents.undo(); await wait(`${input(0)}.value===${JSON.stringify(undoValue)}`)
      window.webContents.redo(); await wait(`${input(0)}.value===${JSON.stringify(text)}`)
      await clickNative(`${frame(0)}.querySelector('.canvas-conversation-heading button[aria-label="放回画布"]')`)
      await wait(`Boolean(${frame(0)}?.classList.contains('is-inline'))`)
      assert.ok(await evaluate(`${input(0)}===window.r01Editor&&${input(1)}.value==='乙卡另一份未发送正文'`))
      assert.deepEqual((await get(scopes[0])).draft, before.draft)
      assert.deepEqual((await get(scopes[1])).draft, beta.draft)
      await evaluate(`(()=>{${input(0)}.focus();return true})()`)
      window.webContents.undo(); await wait(`${input(0)}.value===${JSON.stringify(undoValue)}`)
      window.webContents.redo(); await wait(`${input(0)}.value===${JSON.stringify(text)}`)
      report.roundtrip = { revision: before.draft.revision, quoteIds: before.draft.quotes.map(quote => quote.quoteId),
        attachments: names(before.draft), undoValue, restoredBody: text }
      record('same_native_inline_drawer_roundtrip_preserves_body_attachments_quote_order_revision_editor_and_preexisting_undo_redo')
      await writeFile(join(config.evidence, '01-private-full-input-roundtrip.png'), (await window.webContents.capturePage()).toPNG())
      return
    }
    const activeRunId = await beginHeldPrivateRun(rpc, context, config.runtime_capture)
    assert.ok(activeRunId)
    const queue = []
    for (const [index, name, quote] of [[0, 'R01 排队甲材料.txt', '甲材料对应的问题'], [1, 'R01 排队乙材料.txt', '乙材料对应的问题']]) {
      const ready = await addMaterial(scopes[0], name, quote), body = `第${index + 1}份完整排队正文`
      const sent = await rpc('singleChat.send', { commandId: randomUUID(), command: {
        threadId, conversationId, composerScope: scopes[0], body, draftRevision: ready.draft.revision } })
      assert.equal(sent.status, 'accepted', JSON.stringify(sent))
      const snapshot = await get(scopes[0]), item = snapshot.pendingInputs.items.find(item => item.body === body)
      assert.ok(item); assert.deepEqual(materials(item), materials(ready.draft)); queue.push(item)
    }
    const row = (index, body) => `[...${frame(index)}.querySelectorAll('.single-chat-pending-queue .pending-input-row')].find(row=>row.querySelector('.pending-input-preview')?.getAttribute('title')===${JSON.stringify(body)})`
    await clickNative(`(${row(0, queue[0].body)})?.querySelector('button[aria-label="编辑待发送消息"]')`)
    await wait(`${input(0)}.value===${JSON.stringify(queue[0].body)}`)
    let current = await get(scopes[0])
    assert.deepEqual(materials(current.draft), materials(queue[0]))
    assert.ok(!current.pendingInputs.items.some(item => item.id === queue[0].id))
    record('real_pending_return_to_empty_card_restores_one_complete_body_attachment_and_quote_input')
    const original = current.draft
    await clickNative(`(${row(0, queue[1].body)})?.querySelector('button[aria-label="编辑待发送消息"]')`)
    await wait(`Boolean(${frame(0)}.querySelector('.single-chat-retained-inputs select'))`)
    current = await get(scopes[0]); assert.deepEqual(current.draft, original)
    assert.equal(await evaluate(`${input(0)}.value`), queue[0].body)
    const recovered = await rpc('singleChat.composerDraft.recovery', { threadId, conversationId })
    const saved = recovered.items.find(item => item.kind === 'retained' && item.input.body === queue[1].body)?.input
    assert.ok(saved); assert.deepEqual(materials(saved), materials(queue[1]))
    record('return_to_occupied_card_keeps_existing_complete_input_and_retains_the_other_without_mixing')
    const swap = `[...${frame(0)}.querySelectorAll('.single-chat-retained-inputs button')].find(button=>button.textContent==='切换完整输入')`
    await clickNative(swap); await wait(`${input(0)}.value===${JSON.stringify(queue[1].body)}`)
    assert.deepEqual(materials((await get(scopes[0])).draft), materials(queue[1]))
    const back = await rpc('singleChat.composerDraft.recovery', { threadId, conversationId })
    assert.ok(back.items.some(item => item.kind === 'retained' && item.input.body === queue[0].body
      && JSON.stringify(materials(item.input)) === JSON.stringify(materials(queue[0]))))
    assert.equal((await get(scopes[0])).conversation.activeAgentRunId, activeRunId)
    assert.equal(await evaluate(`${input(1)}.value`), '')
    record('actual_full_input_switch_moves_body_and_materials_together_preserving_other_card_and_active_run')
    report.queue_inputs = queue
    await writeFile(join(config.evidence, '01-private-pending-complete-inputs.png'), (await window.webContents.capturePage()).toPNG())
    const leaseBody = '第三份排队消息：用于明确接管', ready = await addMaterial(scopes[1], 'R01 租约材料.txt', '甲材料对应的问题')
    const leaseSent = await rpc('singleChat.send', { commandId: randomUUID(), command: {
      threadId, conversationId, composerScope: scopes[1], body: leaseBody, draftRevision: ready.draft.revision } })
    assert.equal(leaseSent.status, 'accepted')
    const leaseItem = (await get(scopes[1])).pendingInputs.items.find(item => item.body === leaseBody)
    assert.ok(leaseItem)
    const edit = (scope, token, action) => rpc('singleChat.pendingInputs.edit', { commandId: randomUUID(), command: {
      threadId, conversationId, composerScope: scope, pendingInputId: leaseItem.id, expectedRevision: leaseItem.revision, editToken: token, action } })
    assert.equal((await edit(scopes[0], null, { type: 'begin' })).status, 'applied')
    const oldToken = (await get(scopes[0])).pendingInputs.editSession.editToken
    const working = index => `${frame(index)}?.querySelector('.single-chat-pending-editor textarea')`
    await wait(`${working(0)}?.value===${JSON.stringify(leaseBody)}`)
    await typeBody(0, '甲卡尚未保存的排队编辑', working(0))
    const other = await get(scopes[1])
    assert.equal(other.pendingInputs.editSession, null); assert.equal(other.pendingInputs.editingElsewhere, true)
    const takeover = `[...(${row(1, leaseBody)}).querySelectorAll('button')].find(button=>button.textContent==='从已保存版本接管')`
    await clickNative(takeover)
    await wait(`${working(1)}?.value===${JSON.stringify(leaseBody)}&&!${working(0)}`)
    const taken = (await get(scopes[1])).pendingInputs.editSession
    assert.ok(taken && taken.editToken !== oldToken)
    assert.equal((await edit(scopes[0], oldToken, { type: 'save', body: '旧令牌不能覆盖' })).status, 'rejected')
    assert.equal((await get(scopes[1])).pendingInputs.items.find(item => item.id === leaseItem.id).body, leaseBody)
    await typeBody(1, '乙卡接管后保存的正文', working(1))
    await clickNative(`[...${frame(1)}.querySelectorAll('.single-chat-pending-editor button')].find(button=>button.textContent==='保存排队消息')`)
    await wait(`!${working(1)}`)
    const savedPending = (await get(scopes[1])).pendingInputs.items.find(item => item.id === leaseItem.id)
    assert.equal(savedPending.body, '乙卡接管后保存的正文')
    assert.deepEqual(materials(savedPending), materials(leaseItem))
    record('actual_other_card_takeover_uses_saved_working_input_and_new_token_then_rejects_old_writer_and_saves_its_own_edit')
    await writeFile(join(config.evidence, '02-private-pending-takeover.png'), (await window.webContents.capturePage()).toPNG())
  } finally {
    const ended = await rpc('singleChat.end', { commandId: randomUUID(), command: { threadId, conversationId } })
    assert.equal(ended.status, 'applied', JSON.stringify(ended))
    await waitPrivateSnapshot(rpc, conversationId, value => value.agentRuns.every(terminal))
    record('owned_private_workflow_ends_its_conversation_and_settles_only_its_controlled_run')
  }
}
