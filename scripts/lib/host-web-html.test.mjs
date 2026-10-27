import assert from 'node:assert/strict'
import { access, mkdtemp, realpath, readFile, writeFile, rm, mkdir, readdir, rename } from 'node:fs/promises'
import { createServer } from 'node:http'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import test from 'node:test'
import { launchHost, within } from './host-test-client.mjs'
import { launchAcceptanceBrowser } from './host-web-browser.mjs'
import { coreDataDirectoryArguments, removeEphemeralRuntimeCampFilesRoot } from './runtime-camp-files-root.mjs'

const repository = resolve(import.meta.dirname, '../..')
// Actual Rust resource authorization + production React viewer + Chrome CSP.
// No daily data, native App, model, or network dependency is used by this fixture.
test('trusted Web HTML uses native storage, forms, popups and modals while preserving source and diagnostics', { timeout: 90_000 }, async t => {
  if (process.platform !== 'darwin') { t.skip('macOS Chrome acceptance; other platforms remain separately unverified'); return }
  const executable = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
  if (!await access(executable).then(() => true, () => false)) { t.skip('Chrome unavailable'); return }
  const fixture = await realpath(await mkdtemp(join(tmpdir(), 'coaxi-web-html-')))
  const dataDir = join(fixture, 'core')
  const uploadScratch = join(fixture, 'uploads')
  await mkdir(uploadScratch)
  console.log(JSON.stringify({ channel: 'automatic_acceptance', dataDir, skillLibraryRoot: join(dataDir, 'skills'), browserProfile: join(fixture, 'chrome'), runtime: false }))
  const host = launchHost(process.env.COAXI_HOST_BIN ?? join(repository, 'target/debug/coaxi-host'), [
    ...coreDataDirectoryArguments(dataDir), '--skill-library-root', join(dataDir, 'skills'), '--mcp-config-path', join(dataDir, 'mcp.json')
  ], { cwd: repository, env: { ...process.env, TMPDIR: uploadScratch, TMP: uploadScratch, TEMP: uploadScratch } })
  let browser
  let submittedForm = ''
  const formServer = createServer(async (request, response) => {
    if (request.method === 'POST') {
      for await (const chunk of request) submittedForm += chunk
    }
    response.setHeader('Content-Type', 'text/html'); response.end('<h1>FORM_OK</h1>')
  })
  await new Promise(resolve => formServer.listen(0, '127.0.0.1', resolve))
  const formOrigin = `http://127.0.0.1:${formServer.address().port}`
  try {
    await within(host.ready)
    const profiles = await host.request('members.list')
    await host.request('camps.create', { commandId: crypto.randomUUID(), name: 'HTML attachment acceptance', workspace: null, memberAgentIds: [profiles[0].agentId], defaultLeadAgentId: profiles[0].agentId, collaborationMode: 'peer' })
    const service = await host.request('host.web.start', { listen: '127.0.0.1:0', uiDirectory: join(repository, 'out/web') })
    const html = `<!doctype html><html><head><meta charset="utf-8"><title>Interactive attachment</title><link rel="stylesheet" href="./theme.css"><style>body{margin:0}button{position:absolute;left:20px;top:60px;width:200px;height:40px}h1{font:24px sans-serif}</style></head><body><h1>Rendered HTML</h1><button id="run">Run interaction</button><p id="result"></p><form action="${formOrigin}/submit" method="post" target="form-result"><input name="value" value="native-form"><button id="submit" style="top:120px">Submit form</button></form><iframe name="form-result" hidden></iframe><script>
      const visits=Number(localStorage.getItem('preview-visits')||0)+1;
      localStorage.setItem('preview-visits',String(visits));
      sessionStorage.setItem('preview-session','native-session');
      const report=()=>parent.postMessage({type:'html-acceptance',visits,nativeStorage:localStorage instanceof Storage&&sessionStorage instanceof Storage,parentSentinel:parent.sessionStorage.getItem('acceptance-sentinel'),session:sessionStorage.getItem('preview-session'),color:getComputedStyle(document.body).backgroundColor,clicked:document.querySelector('#result').textContent},location.origin);
      document.querySelector('#run').onclick=()=>{document.querySelector('#result').textContent='INTERACTION_OK';report();const missing=new Image();missing.src='./missing-local-image.png';document.body.append(missing)};report();
    </script></body></html>`
    const file = join(fixture, 'interactive.html'); await writeFile(file, html)
    browser = await launchAcceptanceBrowser({ executable, args: ['--headless=new', `--user-data-dir=${join(fixture, 'chrome')}`, '--no-first-run', '--no-default-browser-check', '--remote-debugging-port=0', 'about:blank'] })
    const listen = `window.htmlAcceptance=[];addEventListener('message',e=>{if(e.source===document.querySelector('.file-preview-html')?.contentWindow&&e.origin===location.origin&&e.data?.type==='html-acceptance')window.htmlAcceptance.push(e.data)})`
    await browser.send('Page.addScriptToEvaluateOnNewDocument', { source: listen })
    await browser.send('Page.navigate', { url: service.origin })
    await browser.wait(`document.querySelector('#administrator-token') !== null`)
    assert.equal(await browser.evaluate(`document.querySelector('#administrator-token').placeholder`), '输入 64 位的 Token')
    await browser.click(`document.querySelector('#administrator-token')`)
    await browser.send('Input.insertText', { text: service.administratorToken })
    await browser.click(`document.querySelector('.web-login button[type=submit]')`)
    await browser.wait(`document.querySelector('.web-login-overlay') === null`)
    await browser.click(`[...document.querySelectorAll('button')].find(e=>e.textContent.includes('HTML attachment acceptance'))`)
    await browser.wait(`document.querySelector('.conversation-controls .composer-file-input:not(:disabled)') !== null`)
    await browser.evaluate(`sessionStorage.setItem('acceptance-sentinel','private')`)
    await browser.setFiles('.conversation-controls .composer-file-input', [file])
    await browser.wait(`document.querySelector('.composer-attachment-card .attachment-open:not(:disabled)') !== null`)
    const uploads = (await readdir(uploadScratch)).filter(name => name.startsWith('coaxi-web-upload-'))
    assert.equal(uploads.length, 1)
    const stylesheet = join(uploadScratch, uploads[0], 'theme.css')
    await writeFile(stylesheet, 'body{background:rgb(236,238,239)}')
    await browser.click(`document.querySelector('.composer-attachment-card .attachment-open')`)
    await browser.wait(`document.querySelector('.file-preview-html-stage')?.dataset.documentState==='loaded' && window.htmlAcceptance.length>0`)
    const actual = await browser.evaluate('window.htmlAcceptance.at(-1)')
    assert.equal(actual.visits, 1); assert.equal(actual.nativeStorage, true)
    assert.equal(actual.parentSentinel, 'private'); assert.equal(actual.session, 'native-session')
    assert.equal(actual.color, 'rgb(236, 238, 239)')
    assert.equal(await browser.evaluate(`document.querySelector('.file-preview-html').sandbox.value`), 'allow-scripts allow-same-origin allow-forms allow-popups allow-modals')
    assert.equal(await browser.evaluate(`document.querySelector('.file-preview-html-stage').dataset.channelState`), 'connected')
    await browser.evaluate('new Promise(resolve=>setTimeout(resolve,300))')
    const point = await browser.evaluate(`(()=>{const r=document.querySelector('.file-preview-html').getBoundingClientRect();return{x:r.x+120,y:r.y+80}})()`)
    await browser.send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...point })
    await browser.send('Input.dispatchMouseEvent', { type: 'mousePressed', ...point, button: 'left', clickCount: 1 })
    await browser.send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...point, button: 'left', clickCount: 1 })
    await browser.wait(`window.htmlAcceptance.at(-1)?.clicked==='INTERACTION_OK'`)
    await browser.wait(`document.querySelector('.file-preview-html-stage')?.dataset.resourceState==='partial-failure'`)
    // Exercise author form submission to another HTTP origin through a child frame.
    // This requires both allow-forms and the response's form-action policy.
    await browser.evaluate(`document.querySelector('.file-preview-html').contentDocument.querySelector('form').requestSubmit()`)
    const formDeadline = Date.now() + 5000
    while (!submittedForm && Date.now() < formDeadline) await new Promise(resolve => setTimeout(resolve, 50))
    assert.equal(submittedForm, 'value=native-form')
    const popup = await browser.send('Runtime.evaluate', { expression: `(()=>{const w=document.querySelector('.file-preview-html').contentWindow;const popup=w.open(${JSON.stringify(formOrigin)},'preview-popup');const opened=Boolean(popup);popup?.close();return opened})()`, returnByValue: true, userGesture: true })
    assert.equal(popup.result.value, true, 'trusted author scripts can open popups with user activation')
    const modal = browser.send('Runtime.evaluate', { expression: `document.querySelector('.file-preview-html').contentWindow.confirm('PREVIEW_MODAL')`, returnByValue: true, userGesture: true })
    // CDP keeps evaluate pending while the native modal is open.
    let accepted = false
    for (let attempt = 0; attempt < 40 && !accepted; attempt++) {
      await new Promise(resolve => setTimeout(resolve, 50))
      accepted = await browser.send('Page.handleJavaScriptDialog', { accept: true }).then(() => true, () => false)
    }
    assert.equal(accepted, true, 'native confirm is available inside the preview')
    assert.equal((await modal).result.value, true)
    await browser.send('Input.dispatchKeyEvent', { type: 'rawKeyDown', key: 'f', code: 'KeyF', windowsVirtualKeyCode: 70, modifiers: 4 })
    await browser.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'f', code: 'KeyF', windowsVirtualKeyCode: 70, modifiers: 4 })
    await browser.wait(`document.querySelector('input[aria-label="查找文件内容"]') !== null`)
    await browser.send('Page.bringToFront')
    await browser.click(`document.querySelector('input[aria-label="查找文件内容"]')`)
    await browser.send('Input.insertText', { text: 'Rendered HTML' })
    await browser.wait(`document.body.textContent.includes('共 1 处匹配')`)
    await browser.key('Escape')
    await browser.capture('/tmp/coaxi-web-html-rendered.png')
    const toggleSource = async label => {
      const target = await browser.evaluate(`(()=>{const r=document.querySelector('.file-preview-tab-activate').getBoundingClientRect();return{x:r.x+40,y:r.y+r.height/2}})()`)
      await browser.send('Input.dispatchMouseEvent', { type: 'mousePressed', ...target, button: 'right', clickCount: 1 })
      await browser.send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...target, button: 'right', clickCount: 1 })
      await browser.click(`[...document.querySelectorAll('[role=menuitem]')].find(e=>e.textContent===${JSON.stringify(label)})`)
    }
    await toggleSource('查看源码')
    await browser.wait(`document.querySelector('.file-preview-html-source')?.textContent.includes('Rendered HTML')`)
    assert.equal(await browser.evaluate(`document.querySelector('.file-preview-html-source').textContent.includes('data-coaxi-preview-diagnostic')`), false)
    await toggleSource('交互预览')
    assert.equal(await browser.evaluate(`window.htmlAcceptance.at(-1).clicked`), 'INTERACTION_OK', 'source toggle preserves iframe interaction state')
    await writeFile(`${stylesheet}.tmp`, 'body{background:rgb(210,220,230)}')
    await rename(`${stylesheet}.tmp`, stylesheet)
    await browser.send('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 1, mobile: true })
    await browser.evaluate('window.previewReloadMarker=true')
    await browser.send('Page.reload')
    await browser.wait('window.previewReloadMarker!==true')
    await browser.wait(`document.querySelector('.web-login-overlay')===null && document.querySelector('.composer-attachment-card .attachment-open:not(:disabled)') !== null`)
    assert.equal(await browser.evaluate(`document.querySelector('.web-login-overlay')===null`), true)
    // Composer drafts restore their exact attachment locator; file tabs may be
    // reopened explicitly without changing editor ownership or requiring Token.
    if (!await browser.evaluate(`document.querySelector('.file-preview-html')!==null`)) await browser.click(`document.querySelector('.composer-attachment-card .attachment-open')`)
    await browser.wait(`document.querySelector('.file-preview-html-stage')?.dataset.documentState==='loaded'`)
    await browser.wait(`window.htmlAcceptance.at(-1)?.color==='rgb(210, 220, 230)'`)
    assert.equal(await browser.evaluate(`localStorage.getItem('preview-visits')`), '2', 'native localStorage survives a page refresh at phone width')
    assert.equal(await browser.evaluate(`document.querySelector('.file-preview-html').contentWindow.sessionStorage.getItem('preview-session')`), 'native-session')
    assert.equal(await browser.evaluate(`document.querySelector('.file-preview-html-stage').dataset.channelState`), 'connected')
    assert.equal(await readFile(file, 'utf8'), html, 'preview never rewrites the original attachment')
    await browser.click(`document.querySelector('[aria-label="关闭 interactive.html"]')`)
    await browser.wait(`document.querySelector('.file-preview-html')===null`)
    assert.deepEqual(browser.errors, [])
    console.log(JSON.stringify({ rendered: true, realClick: true, originalSource: true, nativeStorage: true, form: true, popup: true, modal: true, mobileWidth: true, relativeCss: true, replacementSave: true, refresh: true, released: true }))
  } catch (error) {
    if (browser) console.log(JSON.stringify(await browser.evaluate(`({samples:window.htmlAcceptance,search:document.querySelector('input[aria-label="查找文件内容"]')?.value,active:document.activeElement?.outerHTML,frame:document.querySelector('.file-preview-html')?.getBoundingClientRect().toJSON()})`).catch(()=>null)))
    if (browser) await browser.capture('/tmp/coaxi-web-html-failure.png').catch(() => {})
    throw error
  } finally {
    await browser?.close(); await host.close()
    await new Promise(resolve => formServer.close(resolve))
    await removeEphemeralRuntimeCampFilesRoot(dataDir, { temporaryDirectory: fixture })
    await rm(fixture, { recursive: true, force: true })
  }
})
