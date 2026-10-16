import assert from 'node:assert/strict'
import { app } from 'electron'
import { mkdir,readFile,writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { randomUUID } from 'node:crypto'
import { setTimeout as pause } from 'node:timers/promises'
import { stripVTControlCharacters } from 'node:util'

const config=JSON.parse(await readFile(process.env.ROVAI_C06_NATIVE_CONFIG,'utf8'))
const report={source_commit:config.source_commit,scope:'production Main/preload/Core; direct Electron native input events',checks:[],status:'running',started_at:new Date().toISOString()}
await mkdir(config.evidence,{recursive:false})
let started=false
const record=(name,details={})=>{report.checks.push({name,passed:true,...details});console.log(JSON.stringify({check:name,passed:true}))}
app.on('browser-window-created',(_event,window)=>{
 if(started)return;started=true
 window.webContents.once('did-finish-load',()=>{void qualify(window)})
})
async function qualify(window){
 const evaluate=expression=>window.webContents.executeJavaScript(expression,true)
 const wait=async(expression,timeout=20_000)=>{const until=Date.now()+timeout;while(Date.now()<until){if(await evaluate(expression))return;await pause(100)}throw Error('Native condition timed out: '+expression)}
 const rpc=(method,params={})=>evaluate(`window.rovai.request(${JSON.stringify(method)},${JSON.stringify(params)})`)
 try{
  window.show();window.focus();window.webContents.focus()
  await wait(`window.rovai&&document.querySelector('button[aria-label="无限画布"]')?.disabled===false`,60_000)
  if(config.mode==='files'){
   report.scope='production Main/preload/Core; real file cards and author HTML lifecycle'
   await qualifyFiles(window,{evaluate,wait,rpc})
   report.status='passed'
   return
  }
  const boards=await rpc('canvas.boards.list'),board=boards.boards.find(board=>board.name==='C06 真实桌面画布')
  assert.ok(board)
  const graph=await rpc('canvas.nodes.snapshot',{boardId:board.id}),terminal=graph.nodes.find(node=>node.data.kind==='terminal')
  assert.ok(terminal)
  const target={boardId:board.id,nodeId:terminal.id}
  await evaluate(`(()=>{document.querySelector('button[aria-label="无限画布"]').click();return true})()`)
  await wait(`document.querySelectorAll('.react-flow__node').length===3`)
  record('production_canvas_route_and_persisted_nodes',{board_id:board.id})
  await wait(`(()=>{const button=[...document.querySelectorAll('.canvas-terminal button')].find(button=>button.textContent.trim()==='启动终端');return button&&!button.disabled})()`,15_000)
  await evaluate(`(()=>{[...document.querySelectorAll('.canvas-terminal button')].find(button=>button.textContent.trim()==='启动终端').click();return true})()`)
  await wait(`(()=>{const button=[...document.querySelectorAll('.canvas-terminal button')].find(button=>button.textContent.trim()==='粘贴');return button&&!button.disabled})()`,30_000)
  window.show();window.focus();window.webContents.focus()
  await evaluate(`(()=>{window.nativeKeys=[];for(const type of ['keydown','keypress','input'])document.addEventListener(type,event=>{if(event.target?.classList?.contains('xterm-helper-textarea'))window.nativeKeys.push({type,key:event.key,trusted:event.isTrusted})},true);document.querySelector('.canvas-terminal .xterm-helper-textarea').focus();return true})()`)
  const before=await rpc('canvas.terminals.snapshot',{target})
  for(const keyCode of '2+3'){
   window.webContents.sendInputEvent({type:'keyDown',keyCode})
   window.webContents.sendInputEvent({type:'char',keyCode})
   window.webContents.sendInputEvent({type:'keyUp',keyCode})
  }
  window.webContents.sendInputEvent({type:'keyDown',keyCode:'Enter'})
  window.webContents.sendInputEvent({type:'keyUp',keyCode:'Enter'})
  const until=Date.now()+12_000
  let after
  while(Date.now()<until){after=await rpc('canvas.terminals.snapshot',{target,afterSeq:0});const text=stripVTControlCharacters(Buffer.concat(after.chunks.map(chunk=>Buffer.from(chunk.bytesBase64,'base64'))).toString('utf8'));if(/(?:^|\r?\n)5(?:\r?\n|$)/.test(text))break;await pause(100)}
  report.native_keys=await evaluate('window.nativeKeys')
  report.input_sequence={before:before.inputSequence,after:after.inputSequence}
  report.output=stripVTControlCharacters(Buffer.concat(after.chunks.map(chunk=>Buffer.from(chunk.bytesBase64,'base64'))).toString('utf8'))
  assert.ok(after.inputSequence>before.inputSequence,JSON.stringify(report))
  assert.match(report.output,/(?:^|\r?\n)5(?:\r?\n|$)/)
  record('native_electron_keyboard_reaches_real_pty_and_calculates_five',{input_sequence:after.inputSequence})
  await wait(`Boolean(document.querySelector('.canvas-terminal .xterm-screen')?.textContent.includes('2+3'))&&[...document.querySelectorAll('.canvas-terminal .xterm-rows > div')].some(row=>row.textContent?.trim()==='5')`,8_000)
  record('real_pty_result_is_painted_in_production_terminal_reader')
  const shot=join(config.evidence,'native-output.png');await writeFile(shot,(await window.webContents.capturePage()).toPNG());report.screenshot=shot
  await evaluate(`(()=>{[...document.querySelectorAll('.canvas-terminal button')].find(button=>button.textContent.trim()==='停止终端').click();return true})()`)
  await wait(`window.rovai.request('canvas.terminals.snapshot',{target:${JSON.stringify(target)}}).then(value=>value.cleanupComplete===true)`)
  record('native_terminal_stop_completes_process_cleanup')
  report.status='passed'
 }catch(error){report.status='failed';report.error=String(error.stack??error);report.native_keys=await evaluate('window.nativeKeys').catch(()=>null);report.dom=await evaluate('document.body.innerText.slice(-2500)').catch(()=>null);if(config.mode==='files'&&(report.screenshots?.length??0)<3){const path=join(config.evidence,'failure.png');await writeFile(path,(await window.webContents.capturePage()).toPNG()).then(()=>{(report.screenshots??=[]).push(path)}).catch(()=>{})}}
 finally{report.finished_at=new Date().toISOString();await writeFile(join(config.evidence,'result.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({status:report.status,evidence:config.evidence}));process.exitCode=report.status==='passed'?0:1;app.quit()}
}
async function qualifyFiles(window,{evaluate,wait,rpc}){
 const requests=new Set(),network=[]
 window.webContents.debugger.attach('1.3')
 window.webContents.debugger.on('message',(_event,method,params)=>{
  if(method==='Network.requestWillBeSent'){
   let url;try{url=new URL(params.request.url)}catch{return}
   if(!url.hostname.startsWith('preview-')||!url.hostname.endsWith('.localhost'))return
   requests.add(params.requestId)
   network.push({method,path:url.pathname.replace(/(\/start\/)[^/]+/,'$1<local-capability>'),redirect_status:params.redirectResponse?.status})
  }else if(requests.has(params.requestId)&&method==='Network.responseReceived')network.push({method,status:params.response.status})
  else if(requests.has(params.requestId)&&method==='Network.responseReceivedExtraInfo')network.push({method,status:params.statusCode,blocked_cookie_reasons:params.blockedCookies?.map(cookie=>cookie.blockedReasons)})
  else if(requests.has(params.requestId)&&method==='Network.loadingFailed')network.push({method,error:params.errorText,blocked_reason:params.blockedReason})
 })
 report.preview_network=network
 await window.webContents.debugger.sendCommand('Network.enable')
 const created=await rpc('canvas.boards.create',{commandId:randomUUID(),command:{name:'C06 文件与 HTML'}})
 assert.equal(created.status,'applied',JSON.stringify(created))
 const boardId=created.payload.board.id,nodeIds=[]
 for(const [index,path] of [config.text_file,config.html_file].entries()){
  const graph=await rpc('canvas.nodes.snapshot',{boardId})
  const applied=await rpc('canvas.nodes.apply',{commandId:randomUUID(),command:{boardId,expectedRevision:graph.board.revision,mutation:{type:'create',node:{kind:'file',path,fileReference:{threadId:config.thread_id},x:index*560,y:0,width:520,height:520}}}})
  assert.equal(applied.status,'applied',JSON.stringify(applied));nodeIds.push(applied.payload.nodeIds[0])
 }
 report.board_id=boardId;report.node_ids=nodeIds
 await evaluate(`(()=>{document.querySelector('button[aria-label="无限画布"]').click();return true})()`)
 await wait(`Boolean(document.querySelector('.canvas-boards-toolbar select option[value="${boardId}"]'))`)
 await evaluate(`(()=>{const select=document.querySelector('.canvas-boards-toolbar select');select.value=${JSON.stringify(boardId)};select.dispatchEvent(new Event('change',{bubbles:true}));return true})()`)
 await wait(`document.querySelectorAll('.react-flow__node').length===2`)
 await evaluate(`(()=>{[...document.querySelectorAll('.canvas-surface button')].find(button=>button.textContent.trim()==='查看全部').click();return true})()`)
 const textCard=`document.querySelector('[data-id="${nodeIds[0]}"] .canvas-card')`
 const htmlCard=`document.querySelector('[data-id="${nodeIds[1]}"] .canvas-card')`
 await wait(`${textCard}?.textContent.includes('C06 真实文件权限')`,25_000)
 record('actual_canvas_text_card_preserves_unicode_and_literal_hash_file_name')
 await wait(`Boolean(${htmlCard}?.querySelector('iframe.file-preview-html')?.src)`,25_000)
 const entryUrl=await evaluate(`${htmlCard}.querySelector('iframe.file-preview-html').src`)
 report.html_entry_url=entryUrl
 const origin=new URL(entryUrl).origin
 let frame,initial
 const deadline=Date.now()+20_000
 const probe=randomUUID()
 while(Date.now()<deadline){
  const candidates=window.webContents.mainFrame.framesInSubtree.filter(item=>{try{return new URL(item.url).origin===origin}catch{return false}})
  for(const item of candidates)await item.executeJavaScript(`(()=>{if(window.__c06ProbeHandler)removeEventListener('message',window.__c06ProbeHandler);window.__c06ProbeHandler=event=>{if(event.source===parent&&event.data?.c06FrameProbe===${JSON.stringify(probe)})window.__c06FrameProbe=${JSON.stringify(probe)}};addEventListener('message',window.__c06ProbeHandler);return true})()`).catch(()=>{})
  await evaluate(`(()=>{${htmlCard}.querySelector('iframe.file-preview-html').contentWindow.postMessage({c06FrameProbe:${JSON.stringify(probe)}},${JSON.stringify(origin)});return true})()`)
  await pause(100)
  for(const item of candidates)if(await item.executeJavaScript(`window.__c06FrameProbe===${JSON.stringify(probe)}`).catch(()=>false))frame=item
  if(frame){initial=await frame.executeJavaScript(`({title:document.title,ready:document.readyState,counter:document.querySelector('#counter')?.textContent,color:document.querySelector('#counter')?getComputedStyle(document.querySelector('#counter')).color:null,bridge:typeof window.rovai,node:typeof window.require,csp:document.querySelector('meta[http-equiv="Content-Security-Policy"]')?.content})`).catch(()=>null);if(initial?.ready==='complete'&&(initial?.color==='rgb(23, 101, 155)'||initial?.title==='无法加载页面'))break}
  await pause(100)
 }
 for(const item of window.webContents.mainFrame.framesInSubtree.filter(item=>{try{return new URL(item.url).origin===origin}catch{return false}}))await item.executeJavaScript(`(()=>{removeEventListener('message',window.__c06ProbeHandler);delete window.__c06ProbeHandler;delete window.__c06FrameProbe;return true})()`).catch(()=>{})
 report.frame_urls=window.webContents.mainFrame.framesInSubtree.map(item=>item.url)
 report.frame_details=await Promise.all(window.webContents.mainFrame.framesInSubtree.filter(item=>{try{return new URL(item.url).origin===origin}catch{return false}}).map(async item=>({name:item.name,id:item.frameTreeNodeId,document:await item.executeJavaScript(`({name:window.name,title:document.title,ready:document.readyState,counter:document.querySelector('#counter')?.textContent,color:document.querySelector('#counter')?getComputedStyle(document.querySelector('#counter')).color:null})`).catch(error=>String(error))})))
 report.iframe_presentations=await evaluate(`[...document.querySelectorAll('iframe.file-preview-html')].map(frame=>({name:frame.name,inCard:Boolean(frame.closest('.canvas-file-preview')),hidden:frame.hidden,width:frame.getBoundingClientRect().width,height:frame.getBoundingClientRect().height}))`)
 report.preview_cookies=(await window.webContents.session.cookies.get({url:entryUrl})).map(cookie=>({name:cookie.name,secure:cookie.secure,httpOnly:cookie.httpOnly,sameSite:cookie.sameSite}))
 report.html_initial=initial
 assert.equal(initial?.title,'C06 HTML 保留作者页面',JSON.stringify(initial))
 assert.equal(initial.counter,'0');assert.equal(initial.color,'rgb(23, 101, 155)')
 assert.equal(initial.bridge,'undefined');assert.equal(initial.node,'undefined')
 assert.equal(initial.csp,config.author_csp)
 report.html_initial=initial
 record('actual_html_card_loads_author_css_js_and_keeps_csp_without_native_bridge')
 const documentId=frame.frameTreeNodeId
 await frame.executeJavaScript(`(()=>{document.querySelector('#increment').click();return true})()`)
 assert.equal(await frame.executeJavaScript(`document.querySelector('#counter').textContent`),'1')
 record('author_html_interaction_executes_in_actual_preview')
 const capture=async name=>{const path=join(config.evidence,name+'.png');await writeFile(path,(await window.webContents.capturePage()).toPNG());(report.screenshots??=[]).push(path)}
 await capture('01-text-and-interactive-html')
 report.host_html_state=await evaluate(`({...${htmlCard}.querySelector('.file-preview-html-stage').dataset})`)
 await wait(`${htmlCard}.querySelector('.file-preview-html-stage')?.dataset.documentState==='loaded'&&${htmlCard}.querySelector('.file-preview-html-stage')?.dataset.channelState==='connected'`,15_000)
 record('author_document_has_confirmed_loaded_state_and_connected_host_channel')
 report.card_layout=await evaluate(`(()=>{const card=${htmlCard};return {pathRows:card.querySelectorAll('.file-preview-path-row').length,exposedPaths:card.querySelectorAll('.canvas-file-path').length,headerActions:Boolean(card.querySelector('.canvas-card-handle .canvas-file-actions button')),title:card.querySelector('.canvas-card-handle strong').title,headerHeight:card.querySelector('.canvas-card-handle').getBoundingClientRect().height}})()`)
 assert.equal(report.card_layout.pathRows,0);assert.equal(report.card_layout.exposedPaths,0)
 assert.equal(report.card_layout.headerActions,true);assert.ok(report.card_layout.title.includes(config.html_file))
 record('file_card_uses_one_header_and_keeps_exact_path_available_on_hover')
 await evaluate(`(()=>{${htmlCard}.querySelector('.canvas-file-actions button[aria-label="显示源码"]').click();return true})()`)
 await wait(`${htmlCard}.querySelector('iframe.file-preview-html').hidden===true`)
 assert.equal(frame.frameTreeNodeId,documentId)
 assert.equal(await frame.executeJavaScript(`document.querySelector('#counter').textContent`),'1')
 await evaluate(`(()=>{${htmlCard}.querySelector('.canvas-file-actions button[aria-label="显示预览"]').click();return true})()`)
 await wait(`${htmlCard}.querySelector('iframe.file-preview-html').hidden===false`)
 const currentFrame=window.webContents.mainFrame.framesInSubtree.find(item=>item.frameTreeNodeId===documentId)
 assert.equal(currentFrame?.frameTreeNodeId,documentId)
 assert.equal(await currentFrame.executeJavaScript(`document.querySelector('#counter').textContent`),'1')
 assert.equal(await evaluate(`${htmlCard}.querySelector('.file-preview-html-stage').dataset.documentState`),'loaded')
 assert.equal(await evaluate(`${htmlCard}.querySelector('.file-preview-html-stage').dataset.channelState`),'connected')
 record('html_source_preview_toggle_preserves_live_document_and_author_state')
 const original=await readFile(config.text_file,'utf8')
 try{
  await writeFile(config.text_file,'C06 文件已更新，另一张 HTML 卡片保持状态。\n')
  await wait(`(()=>{const button=${textCard}.querySelector('.canvas-file-actions button[aria-label="重新读取"]');return button&&!button.disabled})()`)
  await evaluate(`(()=>{${textCard}.querySelector('.canvas-file-actions button[aria-label="重新读取"]').click();return true})()`)
  await wait(`${textCard}.textContent.includes('C06 文件已更新')`,15_000)
  assert.equal(await currentFrame.executeJavaScript(`document.querySelector('#counter').textContent`),'1')
  assert.equal(await evaluate(`${htmlCard}.querySelector('.file-preview-html-stage').dataset.documentState`),'loaded')
  assert.equal(await evaluate(`${htmlCard}.querySelector('.file-preview-html-stage').dataset.channelState`),'connected')
  record('reloading_one_actual_file_card_preserves_other_html_document')
  await capture('02-updated-text-and-retained-html')
 }finally{await writeFile(config.text_file,original)}
 await evaluate(`(()=>{${textCard}.querySelector('button[aria-label="文件节点操作"]').focus();return true})()`)
 window.webContents.sendInputEvent({type:'keyDown',keyCode:'Down'})
 window.webContents.sendInputEvent({type:'keyUp',keyCode:'Down'})
 await wait(`document.querySelectorAll('.canvas-card-menu [role="menuitem"]').length===2`)
 record('file_card_more_menu_is_reachable_with_native_keyboard')
 await evaluate(`(()=>{[...document.querySelectorAll('.canvas-card-menu [role="menuitem"]')].find(item=>item.textContent==='移除文件节点').click();return true})()`)
 await wait(`Boolean(document.querySelector('[role="dialog"] .danger-button'))`)
 await evaluate(`(()=>{document.querySelector('[role="dialog"] .danger-button').click();return true})()`)
 await wait(`!document.querySelector('[data-id="${nodeIds[0]}"]')&&Boolean(${htmlCard})`)
 assert.equal(await currentFrame.executeJavaScript(`document.querySelector('#counter').textContent`),'1')
 assert.equal(await readFile(config.text_file,'utf8'),original)
 assert.ok(!(await rpc('canvas.nodes.snapshot',{boardId})).nodes.some(node=>node.id===nodeIds[0]))
 record('core_node_deletion_removes_only_pin_and_preserves_file_and_other_html_view')
}
await import(config.production_main)
