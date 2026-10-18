const assert = require('node:assert/strict')
const { writeFileSync } = require('node:fs')
const { join, dirname, isAbsolute } = require('node:path')
const { app, BrowserWindow } = require('electron')
const [renderer, userData] = process.argv.slice(2)
assert.ok(isAbsolute(renderer) && isAbsolute(userData))
app.setPath('userData', userData)
app.setPath('sessionData', join(userData, 'session'))
// Renderer-only automatic verification. No Core, Runtime, SQLite, Skill Library or daily data.
app.whenReady().then(async () => {
  const window = new BrowserWindow({ show: false, width: 1000, height: 920, useContentSize: true,
    webPreferences: { contextIsolation: true, sandbox: true, nodeIntegration: false, backgroundThrottling: false } })
  const errors = []
  window.webContents.on('console-message', event => { if (event.level === 'error' || event.level === 3) { errors.push(event.message); console.error(event.message) } })
  await window.loadFile(renderer)
  const run = code => window.webContents.executeJavaScript(code, true)
  const settle = async () => { await run('window.interactionTest.settle()'); return run('window.interactionTest.snapshot()') }
  const click = async selector => { await run(`window.interactionTest.query(${JSON.stringify(selector)}).click()`); return settle() }
  const button = async text => { await run(`(() => { const button=window.interactionTest.all('button').find(node=>node.textContent===${JSON.stringify(text)}); if (!button) throw Error('Missing button'); button.click() })()`); return settle() }
  const type = async (selector, value) => {
    await run(`(() => { const node=window.interactionTest.query(${JSON.stringify(selector)}); Object.getOwnPropertyDescriptor(${selector.includes('textarea') ? 'HTMLTextAreaElement' : 'HTMLInputElement'}.prototype,'value').set.call(node,${JSON.stringify(value)}); node.dispatchEvent(new Event('input',{bubbles:true})) })()`)
    return settle()
  }
  const capture = async name => writeFileSync(join(dirname(userData), name + '.png'), (await window.webContents.capturePage()).toPNG())
  try {
    let state = await settle()
    assert.equal(state.requests.filter(call => call.method === 'runtime.interactions.resolve').length, 0)
    assert.ok(state.actions.find(action => action.text.startsWith('提交回答')).disabled)
    const initialGeometry = state.questionBody
    assert.ok(initialGeometry.scrollHeight <= initialGeometry.height + 1, 'short options fit without a redundant scrollbar')
    await capture('fused-question-day-wide')
    await run("window.interactionTest.theme('night')"); await settle(); await capture('fused-question-night-wide')
    await click('.native-option input[value="0"]')
    assert.ok(await run("window.interactionTest.query('[role=tab][data-state=active]').textContent.includes('验证方式')"))
    await click('.native-option input[value="0"]')
    await click('.native-option input[value="__other__"]')
    await type('.native-answer-text', '  回归, 保留原文  ')
    await click('[aria-label="收起提问"]')
    assert.equal(await run('window.interactionTest.all(".native-option").length'), 0)
    await click('[aria-label="展开提问"]')
    assert.equal((await settle()).input, '  回归, 保留原文  ')
    await run("window.interactionTest.query('[role=tab]').focus()")
    window.webContents.sendInputEvent({ type: 'keyDown', keyCode: 'ArrowRight' })
    window.webContents.sendInputEvent({ type: 'keyUp', keyCode: 'ArrowRight' })
    await settle()
    // Return to the second question explicitly; answers survive keyboard/tab changes.
    await run("window.interactionTest.all('[role=tab]')[1].click()")
    await settle()
    state = await button('提交回答 (2)')
    await button('提交回答 (2)')
    assert.equal(state.requests.filter(call => call.method === 'runtime.interactions.resolve').length, 1)
    assert.deepEqual(state.intent.command.answer, { kind: 'question', declined: false, answers: [
      { questionId: 'scope', labels: ['先修复当前问题 (Recommended)'] }, { questionId: 'checks', labels: ['行为测试', '回归, 保留原文'] }
    ] })
    await run('window.interactionTest.fail()'); state = await settle()
    assert.ok(state.text.includes('原回答已保留'))
    await button('核对原提交')
    assert.equal((await settle()).requests.filter(call => call.method === 'runtime.interactions.resolve').length, 1)
    await run("window.interactionTest.record('resolved')")
    state = await button('核对原提交')
    assert.ok(state.text.includes('尚未确认队员收到'))
    assert.ok(!state.text.includes('答案已发送'))
    await run("window.interactionTest.record('sent')"); state = await settle()
    assert.ok(state.text.includes('答案已发送'))

    await run('window.interactionTest.replace()'); state = await settle()
    assert.equal(state.checked.length, 0)
    await click('.native-option input[value="0"]')
    await run('window.interactionTest.host()'); state = await settle()
    assert.equal(state.checked.length, 0, 'same IDs on another authenticated Host cannot carry selections')
    await run('window.interactionTest.hide()'); state = await settle(); assert.equal(state.listeners, 0)
    await run('window.interactionTest.show()'); await settle()

    // A replacement ID remounts independently; re-click clears a single choice.
    await run("window.interactionTest.reset('question', true)"); await settle()
    await click('.native-option input[value="1"]'); state = await settle()
    assert.deepEqual(state.checked, ['1'], 'real __other__ label must not activate host Other')
    assert.equal(state.input, undefined)
    await click('.native-option input[value="1"]'); state = await settle()
    assert.deepEqual(state.checked, [])
    await click('.native-option input[value="__other__"]'); await type('.native-answer-text', '自由回答')
    await run('window.interactionTest.replace()'); state = await settle()
    assert.equal(state.input, undefined)

    await run("window.interactionTest.reset('plan')"); state = await settle()
    assert.ok(state.body.scrollHeight > state.body.height, 'long plan remains complete in its own scroll area')
    await button('要求修改'); await type('textarea[aria-label="修改意见"]', '  请保留每个旧入口。  ')
    await button('取消'); state = await button('要求修改'); assert.equal(state.feedback, '  请保留每个旧入口。  ')
    state = await button('发送修改意见')
    assert.deepEqual(state.intent.command.answer, { kind: 'plan_approval', decision: 'request_changes', feedback: '请保留每个旧入口。' })
    await run("window.interactionTest.record('resolved'); window.interactionTest.complete()")
    state = await settle(); assert.ok(state.text.includes('尚未确认队员收到'))
    assert.ok(state.text.includes('请保留每个旧入口。'))
    await capture('fused-plan-feedback-night-wide')
    await run("window.interactionTest.reset('empty-plan')"); await settle(); await capture('fused-empty-plan-night-wide')
    state = await button('批准计划'); assert.equal(state.intent.command.answer.decision, 'approve')
    await run('window.interactionTest.replace()'); await settle()
    state = await button('跳过'); assert.equal(state.intent.command.answer.declined, true)

    await run("window.interactionTest.reset('question', true)"); await settle()
    await click('.native-option input[value="__other__"]'); await type('.native-answer-text', '第一份未提交回答')
    await run('window.interactionTest.queue()'); await settle()
    const chooseRecord = async id => {
      await run(`(() => { const select=window.interactionTest.query('select'); select.value=${JSON.stringify(id)}; select.dispatchEvent(new Event('change',{bubbles:true})) })()`)
      return settle()
    }
    await chooseRecord('question-queued'); await click('.native-option input[value="__other__"]'); await type('.native-answer-text', '第二份独立回答')
    state = await chooseRecord('question-1'); assert.equal(state.input, '第一份未提交回答')
    await run('window.interactionTest.hide()'); await settle(); await run('window.interactionTest.show()'); state = await settle()
    assert.equal(state.input, '第一份未提交回答', 'hiding a card keeps the unsubmitted answer without an event listener')
    state = await chooseRecord('question-queued'); assert.equal(state.input, '第二份独立回答')

    await run("window.interactionTest.reset('question', true)"); await settle()
    await button('跳过'); await run('window.interactionTest.fail(); window.interactionTest.deleteRequest()'); state = await settle()
    assert.ok(state.text.includes('仍可核对原提交'), 'target deletion cannot hide an unresolved original command')
    await run('window.interactionTest.nextAfterDelete()'); await settle()
    await button('核对原提交')
    assert.equal((await settle()).requests.filter(call => call.method === 'runtime.interactions.resolve').length, 1)

    for (const theme of ['day', 'night']) {
      console.log('Narrow layout: ' + theme)
      await run(`window.interactionTest.theme(${JSON.stringify(theme)}); window.interactionTest.reset('question')`)
      window.setContentSize(390, 720); await settle()
      state = await settle(); assert.equal(state.pageOverflowX, false)
      assert.ok(state.bounds.right <= 390 && state.bounds.x >= 0)
      assert.ok(state.actions.every(action => action.bounds.bottom <= 720), 'all question actions remain reachable')
      await capture(`fused-question-${theme}-narrow`)
      await run("window.interactionTest.reset('empty-plan')"); await settle()
      state = await button('要求修改'); assert.ok(state.actions.find(action => action.text === '发送修改意见').disabled)
      assert.ok(state.actions.every(action => action.bounds.bottom <= 720))
      await capture(`fused-empty-plan-${theme}-narrow`)
      await button('取消'); state = await button('放弃计划'); assert.equal(state.intent.command.answer.decision, 'abandon')
    }
    assert.deepEqual(errors, [])
    console.log(JSON.stringify({ ok: true, channel: 'isolated_renderer', core: false, runtime: false, provider: false, initialGeometry,
      checks: ['exact_answer_double_click', 'unknown_no_automatic_retry', 'saved_vs_sent', 'host_conversation_isolation', 'replacement_and_hide', 'single_multiselect_other_tabs_collapse', 'all_plan_decisions_feedback_retained', 'per_request_unsent_answers_survive_switch_and_hide', 'deleted_target_keeps_original_reconciliation', 'day_night_wide_narrow_geometry'] }))
    app.exit(0)
  } catch (error) { console.error(error); app.exit(1) }
})
