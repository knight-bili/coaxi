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
async function openCanvasBoard(evaluate,wait,boardId){
 await evaluate(`(()=>{document.querySelector('button[aria-label="无限画布"]').click();return true})()`)
 await wait(`Boolean(document.querySelector('.canvas-breadcrumb-root:not(:disabled)'))`)
 await evaluate(`(()=>{document.querySelector('.canvas-breadcrumb-root').click();return true})()`)
 await wait(`Boolean(document.querySelector('.canvas-board-tile[data-board-id="${boardId}"]:not(:disabled)'))`,40_000)
 await evaluate(`(()=>{document.querySelector('.canvas-board-tile[data-board-id="${boardId}"]').click();return true})()`)
 await wait(`Boolean(document.querySelector('.canvas-dock-panel button[aria-label="查看全部"]'))`)
}
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
  if(config.mode==='materials'){
   report.scope='production Main/preload/Core; private composer scopes and Native byte preparation; no model'
   await qualifyMaterials({evaluate,rpc})
   report.status='passed'
   return
  }
  if(config.mode==='materials-reopen'){
   report.scope='production Main/preload/Core; cold private material recovery and authorized file reads; no model'
   await qualifyMaterialReopen({evaluate,rpc})
   report.status='passed'
   return
  }
  if(config.mode==='objects'){
   report.scope='production Main/preload/Core; exact Mission/Task/Diff/file workspace navigation; no model'
   await qualifyObjects(window,{evaluate,wait,rpc})
   report.status='passed'
   return
  }
  if(config.mode==='private-ui'){
   report.scope='production Main/preload/Core; actual scoped attachment card opens exact source in canvas; no model'
   await qualifyPrivateUi(window,{evaluate,wait,rpc})
   report.status='passed'
   return
  }
  if(config.mode==='visual'){
   report.scope='production Main/preload/Core; Codeg canvas creation and rendered visual states; no model'
   await qualifyCanvasVisual(window,{evaluate,wait,rpc})
   report.status='passed'
   return
  }
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
 }catch(error){report.status='failed';report.error=String(error.stack??error);report.native_keys=await evaluate('window.nativeKeys').catch(()=>null);report.dom=await evaluate('document.body.innerText.slice(-2500)').catch(()=>null);if((report.screenshots?.length??0)<3){const path=join(config.evidence,'failure.png');await writeFile(path,(await window.webContents.capturePage()).toPNG()).then(()=>{(report.screenshots??=[]).push(path)}).catch(()=>{})}}
 finally{report.finished_at=new Date().toISOString();await writeFile(join(config.evidence,'result.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({status:report.status,evidence:config.evidence}));process.exitCode=report.status==='passed'?0:1;app.quit()}
}
async function qualifyCanvasVisual(window,{evaluate,wait,rpc}){
 window.unmaximize();window.setContentSize(1440,920)
 window.webContents.setZoomFactor(1)
 await evaluate(`(()=>{document.querySelector('button[aria-label="无限画布"]').click();return true})()`)
 await wait(`Boolean(document.querySelector('.canvas-breadcrumb-root:not(:disabled)'))`)
 await evaluate(`(()=>{document.querySelector('.canvas-breadcrumb-root').click();return true})()`)
 await wait(`Boolean(document.querySelector('.canvas-boards-home .canvas-primary-button:not(:disabled)'))`)
 report.visual_states=[]
 const capture=async name=>{
  await evaluate(`new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))`)
  await pause(150)
  const metrics=await evaluate(`(()=>{const box=selector=>document.querySelector(selector)?.getBoundingClientRect().toJSON();return {viewport:{width:innerWidth,height:innerHeight,dpr:devicePixelRatio},font:{family:getComputedStyle(document.body).fontFamily,rootSize:getComputedStyle(document.documentElement).fontSize},theme:document.documentElement.dataset.theme,workspace:box('.canvas-workspace'),dialog:box('.canvas-board-dialog'),canvas:box('.canvas-flow-stage'),dock:box('.canvas-toolbar'),zoom:box('.canvas-zoom-pill')}})()`)
  const path=join(config.evidence,name+'.png');await writeFile(path,(await window.webContents.capturePage()).toPNG())
  ;(report.screenshots??=[]).push(path);report.visual_states.push({state:name,path,metrics})
 }
 const before=await rpc('canvas.boards.list'),beforeIds=new Set(before.boards.map(board=>board.id))
 report.home_state=before.boards.length?'existing_boards':'no_boards'
 report.container_fit=await evaluate(`(()=>{const main=document.querySelector('.content.canvas-content'),work=document.querySelector('.canvas-workspace'),style=getComputedStyle(main),a=main.getBoundingClientRect(),b=work.getBoundingClientRect();return {padding:[style.paddingTop,style.paddingRight,style.paddingBottom,style.paddingLeft],extraScrollX:main.scrollWidth-main.clientWidth,extraScrollY:main.scrollHeight-main.clientHeight,widthGap:Math.abs(a.width-b.width),heightGap:Math.abs(a.height-b.height)}})()`)
 assert.deepEqual(report.container_fit.padding,['0px','0px','0px','0px'])
 assert.ok(report.container_fit.extraScrollX<=1&&report.container_fit.extraScrollY<=1&&report.container_fit.widthGap<=1&&report.container_fit.heightGap<=1)
 await capture('00-board-home')
 record('real_canvas_home_keeps_existing_boards_and_opens_creation',{boards:before.boards.length})
 window.setContentSize(1040,700)
 await evaluate(`new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))`)
 report.small_window=await evaluate(`(()=>{const main=document.querySelector('.content.canvas-content'),grid=document.querySelector('.canvas-board-grid'),last=grid.lastElementChild;grid.scrollTop=grid.scrollHeight;return {viewport:{width:innerWidth,height:innerHeight},outerExtraX:main.scrollWidth-main.clientWidth,outerExtraY:main.scrollHeight-main.clientHeight,listExtraY:grid.scrollHeight-grid.clientHeight,listScrollTop:grid.scrollTop,lastVisible:last.getBoundingClientRect().bottom<=grid.getBoundingClientRect().bottom+1}})()`)
 assert.ok(report.small_window.outerExtraX<=1&&report.small_window.outerExtraY<=1)
 assert.ok(report.small_window.listExtraY>0&&report.small_window.listScrollTop>0&&report.small_window.lastVisible)
 record('small_window_keeps_one_real_scroll_owner_and_last_board_reachable',report.small_window)
 window.setContentSize(1440,920)
 await evaluate(`new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))`)
 await evaluate(`(()=>{document.querySelector('.canvas-boards-home .canvas-primary-button').click();return true})()`)
 await wait(`Boolean(document.querySelector('.canvas-board-dialog input[data-dialog-autofocus]'))`)
 assert.equal(await evaluate(`document.querySelectorAll('.canvas-board-dialog .canvas-color-choice').length`),12)
 report.dialog_style=await evaluate(`(()=>{const panel=document.querySelector('.canvas-board-dialog'),input=panel.querySelector('input'),style=getComputedStyle(panel);return {width:panel.getBoundingClientRect().width,padding:style.paddingTop,radius:style.borderRadius,inputRadius:getComputedStyle(input).borderRadius}})()`)
 assert.equal(report.dialog_style.width,448);assert.equal(report.dialog_style.padding,'24px');assert.equal(report.dialog_style.radius,'28px');assert.equal(report.dialog_style.inputRadius,'999px')
 await capture('01-board-dialog')
 await evaluate(`(()=>{document.querySelector('.canvas-board-dialog .canvas-color-choice[data-color="blue"]').click();return true})()`)
 await wait(`document.querySelector('.canvas-board-dialog .canvas-color-choice[data-color="blue"]').getAttribute('aria-pressed')==='true'`)
 await evaluate(`(()=>{document.querySelector('.canvas-board-dialog .canvas-color-choice[data-color="blue"]').click();document.querySelector('.canvas-board-dialog input[data-dialog-autofocus]').focus();return true})()`)
 await wait(`document.querySelector('.canvas-board-dialog .canvas-color-choice[data-color="blue"]').getAttribute('aria-pressed')==='false'`)
 await window.webContents.insertText('test')
 await wait(`document.querySelector('.canvas-board-dialog input[data-dialog-autofocus]').value==='test'`)
 record('real_creation_form_has_twelve_swatches_and_repicking_clears_color')
 await evaluate(`(()=>{document.querySelector('.canvas-board-dialog button[type="submit"]').click();return true})()`)
 await wait(`!document.querySelector('.canvas-board-dialog')&&document.querySelector('.canvas-empty-hint h3')?.textContent==='画布还是空的'`)
 const after=await rpc('canvas.boards.list'),created=after.boards.filter(board=>!beforeIds.has(board.id))
 assert.equal(created.length,1);assert.equal(created[0].name,'test');assert.ok(!created[0].color)
 await wait(`document.querySelector('.canvas-board-title')?.dataset.boardId===${JSON.stringify(created[0].id)}`)
 const graph=await rpc('canvas.nodes.snapshot',{boardId:created[0].id});assert.equal(graph.nodes.length,0)
 report.visual_board_id=created[0].id
 await capture('02-empty-board')
 record('real_creation_persists_one_empty_board_and_displays_codeg_canvas_chrome',{board_id:created[0].id})
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
 await openCanvasBoard(evaluate,wait,boardId)
 await wait(`document.querySelectorAll('.react-flow__node').length===2`)
 await evaluate(`(()=>{document.querySelector('.canvas-dock-panel button[aria-label="查看全部"]').click();return true})()`)
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
 report.card_layout=await evaluate(`(()=>{const card=${htmlCard},header=card.querySelector('.canvas-card-handle').getBoundingClientRect(),icon=card.querySelector('.canvas-card-handle > .resource-reference-icon').getBoundingClientRect();return {pathRows:card.querySelectorAll('.file-preview-path-row').length,exposedPaths:card.querySelectorAll('.canvas-file-path').length,headerActions:Boolean(card.querySelector('.canvas-card-handle .canvas-file-actions button')),title:card.querySelector('.canvas-card-handle strong').title,headerHeight:header.height,iconCenterDelta:Math.abs(icon.y+icon.height/2-header.y-header.height/2)}})()`)
 assert.equal(report.card_layout.pathRows,0);assert.equal(report.card_layout.exposedPaths,0)
 assert.equal(report.card_layout.headerActions,true);assert.ok(report.card_layout.title.includes(config.html_file))
 assert.ok(report.card_layout.iconCenterDelta<=1,JSON.stringify(report.card_layout))
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
async function qualifyMaterials({evaluate,rpc}){
 const threadId=config.thread_id
 const opened=await rpc('singleChat.open',{commandId:randomUUID(),command:{threadId,agentId:config.agent_id}})
 assert.equal(opened.status,'applied',JSON.stringify(opened))
 const conversationId=opened.payload.conversationId
 assert.equal(typeof conversationId,'string')
 const board=await rpc('canvas.boards.create',{commandId:randomUUID(),command:{name:'C06 私聊材料隔离'}})
 assert.equal(board.status,'applied',JSON.stringify(board))
 const boardId=board.payload.board.id,scopes=[]
 for(let index=0;index<3;index++){
  const graph=await rpc('canvas.nodes.snapshot',{boardId})
  const created=await rpc('canvas.nodes.apply',{commandId:randomUUID(),command:{boardId,expectedRevision:graph.board.revision,mutation:{type:'create',node:{kind:'conversation',conversation:{threadId,conversationId},x:index*400,y:0,width:360,height:300}}}})
  assert.equal(created.status,'applied',JSON.stringify(created))
  scopes.push({kind:'canvas',boardId,nodeId:created.payload.nodeIds[0]})
 }
 report.private_context={thread_id:threadId,conversation_id:conversationId,board_id:boardId,scopes}
 const get=scope=>rpc('singleChat.get',{conversationId,composerScope:scope})
 const mutation=(scope,revision,action,commandId=randomUUID())=>({commandId,command:{threadId,conversationId,composerScope:scope,expectedDraftRevision:revision,action}})
 const mutate=async(scope,revision,action)=>rpc('singleChat.composerDraft.mutate',mutation(scope,revision,action))
 const names=draft=>draft.attachments.map(item=>item.displayName)
 const reject=async(method,params)=>{
  let response
  try{response=await rpc(method,params)}catch(error){return {rejected:true,error:String(error).slice(0,800)}}
  assert.equal(response.status,'rejected',JSON.stringify(response))
  return {rejected:true,result:response}
 }
 assert.equal((await get(scopes[2])).draft.revision,0)
 const readonly=await rpc('singleChat.composerDraft.recovery',{threadId,conversationId})
 assert.ok(!readonly.items.some(item=>item.kind==='scope'&&item.scope.composerScope?.nodeId===scopes[2].nodeId))
 record('readonly_empty_scope_does_not_register_a_recovery_scope')
 for(const scope of scopes.slice(0,2))assert.equal((await mutate(scope,0,{type:'register_scope'})).status,'applied')
 const bogus={kind:'canvas',boardId,nodeId:randomUUID()}
 report.invalid_scope=bogus
 report.invalid_scope_rejection=await reject('singleChat.composerDraft.mutate',mutation(bogus,0,{type:'register_scope'}))
 record('forged_node_scope_cannot_register_private_materials')
 const prepare=async(scope,name,text)=>{
  const current=await get(scope),request={commandId:randomUUID(),threadId,expectedRevision:current.draft.revision,target:{kind:'single_chat',conversationId,composerScope:scope}}
  const prepared=await evaluate(`window.rovai.singleChatAttachments.prepareInput(${JSON.stringify(request)},new File([${JSON.stringify(text)}],${JSON.stringify(name)},{type:'text/plain'}))`)
  assert.equal(prepared.state,'recorded',JSON.stringify(prepared));assert.equal(prepared.receipt.status,'applied',JSON.stringify(prepared))
  const reconciled=await evaluate(`window.rovai.singleChatAttachments.reconcileInput(${JSON.stringify(request)})`)
  assert.deepEqual(reconciled,prepared)
  report.preparations??=[];report.preparations.push({request,result:prepared})
  return get(scope)
 }
 let alpha=await prepare(scopes[0],'甲材料一 #中文.txt','甲卡第一份专用材料')
 alpha=await prepare(scopes[0],'甲材料二.txt','甲卡第二份专用材料')
 const beta=await prepare(scopes[1],'乙材料.txt','乙卡专用材料')
 assert.deepEqual(names(alpha.draft),['甲材料一 #中文.txt','甲材料二.txt'])
 assert.deepEqual(names(beta.draft),['乙材料.txt'])
 assert.deepEqual(names((await get(null)).draft),[])
 record('native_byte_uploads_and_receipt_lookup_keep_two_cards_and_default_draft_isolated')
 report.cross_scope_remove=await reject('singleChat.composerDraft.mutate',mutation(scopes[0],alpha.draft.revision,{type:'remove_attachment',attachmentRefId:beta.draft.attachments[0].id}))
 assert.deepEqual((await get(scopes[0])).draft,alpha.draft)
 assert.deepEqual((await get(scopes[1])).draft,beta.draft)
 record('cross_scope_remove_is_rejected_without_mutating_either_draft')
 const ids=alpha.draft.attachments.map(item=>item.id)
 const concurrent=await Promise.all([rpc('singleChat.composerDraft.mutate',mutation(scopes[0],alpha.draft.revision,{type:'reorder_attachments',attachmentRefIds:[...ids].reverse()})),rpc('singleChat.composerDraft.mutate',mutation(scopes[0],alpha.draft.revision,{type:'reorder_attachments',attachmentRefIds:ids}))])
 assert.equal(concurrent.filter(x=>x.status==='applied').length,1)
 assert.equal(concurrent.filter(x=>x.status==='rejected').length,1)
 assert.deepEqual((await get(scopes[1])).draft,beta.draft)
 record('concurrent_same_revision_mutations_apply_once_and_leave_other_card_unchanged')
 const legacy=await prepare(null,'原工作区材料.txt','原默认位置待迁入材料')
 alpha=await get(scopes[0])
 const adopted=await mutate(scopes[0],alpha.draft.revision,{type:'adopt_legacy_materials',expectedSourceRevision:legacy.draft.revision,targetBody:'甲卡原有问题正文'})
 assert.equal(adopted.status,'applied',JSON.stringify(adopted))
 assert.deepEqual(names((await get(scopes[0])).draft),names(alpha.draft))
 assert.deepEqual(names((await get(null)).draft),[])
 const recovery=await rpc('singleChat.composerDraft.recovery',{threadId,conversationId})
 const targetScopeKey=(await get(scopes[0])).draft.scopeKey
 const retained=recovery.items.find(item=>item.kind==='retained'&&item.input.scopeKey===targetScopeKey)
 assert.ok(retained,JSON.stringify(recovery))
 assert.deepEqual(names(retained.input),['原工作区材料.txt'])
 record('legacy_materials_move_to_a_complete_retained_input_when_target_is_occupied')
 alpha=await get(scopes[0])
 const swap=mutation(scopes[0],alpha.draft.revision,{type:'swap_retained',retainedInputId:retained.input.id,expectedRetainedRevision:retained.input.revision,currentBody:'甲卡原有问题正文'})
 const first=await rpc('singleChat.composerDraft.mutate',swap)
 assert.equal(first.status,'applied',JSON.stringify(first))
 const replay=await rpc('singleChat.composerDraft.mutate',swap)
 assert.deepEqual(replay,first)
 assert.deepEqual(names((await get(scopes[0])).draft),['原工作区材料.txt'])
 assert.deepEqual((await get(scopes[1])).draft,beta.draft)
 record('retained_swap_replay_preserves_original_receipt_and_other_card_materials')
 const graph=await rpc('canvas.nodes.snapshot',{boardId})
 const removed=await rpc('canvas.nodes.apply',{commandId:randomUUID(),command:{boardId,expectedRevision:graph.board.revision,mutation:{type:'delete',nodeIds:[scopes[0].nodeId]}}})
 assert.equal(removed.status,'applied',JSON.stringify(removed))
 const orphan=await rpc('singleChat.composerDraft.recovery',{threadId,conversationId})
 assert.ok(orphan.items.some(item=>item.kind==='scope'&&item.scope.composerScope?.nodeId===scopes[0].nodeId&&item.scope.orphaned))
 assert.deepEqual(names((await get(scopes[0])).draft),['原工作区材料.txt'])
 report.final_recovery=orphan
 record('deleted_pin_keeps_private_materials_recoverable_under_original_conversation')
}
async function qualifyMaterialReopen({evaluate,rpc}){
 const previous=JSON.parse(await readFile(config.previous_result,'utf8'))
 assert.equal(previous.status,'passed')
 const {thread_id:threadId,conversation_id:conversationId,scopes}=previous.private_context
 const recovery=await rpc('singleChat.composerDraft.recovery',{threadId,conversationId})
 assert.deepEqual(recovery.items,previous.final_recovery.items)
 record('cold_reopen_preserves_complete_private_material_and_orphan_recovery_entries')
 const alpha=await rpc('singleChat.get',{conversationId,composerScope:scopes[0]})
 const beta=await rpc('singleChat.get',{conversationId,composerScope:scopes[1]})
 assert.deepEqual(alpha.draft.attachments.map(x=>x.displayName),['原工作区材料.txt'])
 assert.deepEqual(beta.draft.attachments.map(x=>x.displayName),['乙材料.txt'])
 assert.deepEqual(alpha.agentRuns,[]);assert.deepEqual(alpha.messages,[])
 for(const prepared of previous.preparations){
  const receipt=await evaluate(`window.rovai.singleChatAttachments.reconcileInput(${JSON.stringify(prepared.request)})`)
  assert.deepEqual(receipt,prepared.result)
 }
 record('native_preparation_receipts_survive_process_restart_without_rebinding_or_execution')
 await evaluate(`window.rovai.filePreview.bindThread(${JSON.stringify(threadId)})`)
 const preview=async(locator,expected)=>{
  const opened=await evaluate(`window.rovai.filePreview.restore(${JSON.stringify({kind:'attachment',threadId,locator})})`)
  assert.equal(opened.ok,true,JSON.stringify(opened));assert.equal(opened.value.kind,'file_preview')
  const file=opened.value.file
  const read=await evaluate(`window.rovai.filePreview.readText(${JSON.stringify({handleId:file.handleId,expectedGeneration:file.contentGeneration})})`)
  assert.equal(read.ok,true,JSON.stringify(read));assert.equal(read.value.text,expected)
  await evaluate(`window.rovai.filePreview.release(${JSON.stringify({handleId:file.handleId})})`)
  return {kind:file.kind,fileName:file.fileName}
 }
 report.orphan_file=await preview({owner:'single_chat_scoped_composer',threadId,conversationId,composerScope:scopes[0],attachmentRefId:alpha.draft.attachments[0].id},'原默认位置待迁入材料')
 report.beta_file=await preview({owner:'single_chat_scoped_composer',threadId,conversationId,composerScope:scopes[1],attachmentRefId:beta.draft.attachments[0].id},'乙卡专用材料')
 record('production_file_reader_resolves_exact_card_owner_after_cold_reopen')
 const retained=recovery.items.find(x=>x.kind==='retained'&&x.input.scopeKey===alpha.draft.scopeKey)
 assert.ok(retained);assert.equal(retained.input.body,'甲卡原有问题正文')
 const expected={'甲材料一 #中文.txt':'甲卡第一份专用材料','甲材料二.txt':'甲卡第二份专用材料'}
 for(const attachment of retained.input.attachments)await preview({owner:'single_chat_retained',threadId,conversationId,composerScope:scopes[0],retainedInputId:retained.input.id,attachmentRefId:attachment.id},expected[attachment.displayName])
 assert.equal(retained.input.attachments.length,2)
 record('retained_input_keeps_original_body_and_each_authorized_attachment_content')
 const crossed=await evaluate(`window.rovai.filePreview.restore(${JSON.stringify({kind:'attachment',threadId,locator:{owner:'single_chat_scoped_composer',threadId,conversationId,composerScope:scopes[0],attachmentRefId:beta.draft.attachments[0].id}})})`)
 assert.equal(crossed.ok,false,JSON.stringify(crossed));report.crossed_file=crossed
 record('production_file_reader_rejects_attachment_from_a_different_card')
 assert.deepEqual((await rpc('singleChat.composerDraft.recovery',{threadId,conversationId})).items,recovery.items)
 report.private_context=previous.private_context
}
async function qualifyObjects(window,{evaluate,wait,rpc}){
 const mission=await rpc('missions.create',{commandId:randomUUID(),command:{title:'C06 原使命定位',description:'只保存使命，不启动模型。',projectBindingKind:'directory',projectPath:config.workspace,memberAgentIds:[config.agent_id],defaultLeadAgentId:config.agent_id,tags:[]}})
 assert.equal(mission.status,'applied',JSON.stringify(mission))
 const {missionId,threadId}=mission.payload
 assert.ok(missionId&&threadId,JSON.stringify(mission))
 const task=await rpc('tasks.create',{commandId:randomUUID(),threadId,title:'C06 原历史任务定位',description:'通过原对象卡片恢复这条已取消任务。',assigneeAgentId:config.agent_id})
 assert.equal(task.status,'applied',JSON.stringify(task))
 const taskId=task.payload.taskId
 assert.ok(taskId,JSON.stringify(task))
 const ended=await rpc('tasks.update',{commandId:randomUUID(),threadId,taskId,status:'cancelled',cancelReason:'本地历史任务定位检查'})
 assert.equal(ended.status,'applied',JSON.stringify(ended))
 const created=await rpc('canvas.boards.create',{commandId:randomUUID(),command:{name:'C06 原对象与工作区'}})
 assert.equal(created.status,'applied',JSON.stringify(created))
 const boardId=created.payload.board.id,nodes={}
 let toolbarChecked=false
 for(const [index,[kind,objectReference]] of [['mission',{kind:'mission',missionId}],['task',{kind:'task',taskId}],['mission_diff',{kind:'mission_diff',missionId,path:'不存在的精确路径.txt'}]].entries()){
  const graph=await rpc('canvas.nodes.snapshot',{boardId})
  const applied=await rpc('canvas.nodes.apply',{commandId:randomUUID(),command:{boardId,expectedRevision:graph.board.revision,mutation:{type:'create',node:{kind:'conversation',conversation:{threadId,conversationId:null},objectReference,x:index*380,y:0,width:360,height:240}}}})
  assert.equal(applied.status,'applied',JSON.stringify(applied));nodes[kind]=applied.payload.nodeIds[0]
 }
 const graph=await rpc('canvas.nodes.snapshot',{boardId})
 const file=await rpc('canvas.nodes.apply',{commandId:randomUUID(),command:{boardId,expectedRevision:graph.board.revision,mutation:{type:'create',node:{kind:'file',path:config.text_file,fileReference:{threadId:config.thread_id},x:0,y:300,width:520,height:400}}}})
 assert.equal(file.status,'applied',JSON.stringify(file));nodes.file=file.payload.nodeIds[0]
 report.objects={board_id:boardId,mission_id:missionId,thread_id:threadId,task_id:taskId,nodes}
 const board=async()=>{
  await openCanvasBoard(evaluate,wait,boardId)
  await wait(`document.querySelectorAll('.react-flow__node').length===4`)
  await evaluate(`(()=>{document.querySelector('.canvas-dock-panel button[aria-label="查看全部"]').click();return true})()`)
  if(config.expect_toolbar&&!toolbarChecked){
   report.toolbar_layout=await evaluate(`(()=>{const host=document.querySelector('.canvas-boards-toolbar'),toolbar=document.querySelector('.canvas-toolbar'),stage=document.querySelector('.canvas-flow-stage'),pill=document.querySelector('.canvas-zoom-pill'),map=document.querySelector('.canvas-minimap');return {headerHeight:host.getBoundingClientRect().height,dockBottom:stage.getBoundingClientRect().bottom-toolbar.getBoundingClientRect().bottom,dockOutsideHeader:!host.contains(toolbar),mapWidth:map?.getBoundingClientRect().width,pillWidth:pill.getBoundingClientRect().width}})()`)
   assert.ok(report.toolbar_layout.headerHeight<=44&&report.toolbar_layout.dockOutsideHeader&&report.toolbar_layout.dockBottom>=8&&report.toolbar_layout.dockBottom<=24)
   assert.ok(Math.abs(report.toolbar_layout.mapWidth-report.toolbar_layout.pillWidth)<2)
   const menu=async label=>{
    await evaluate(`(()=>{document.querySelector('button[aria-label="${label}"]').focus();return true})()`)
    window.webContents.sendInputEvent({type:'keyDown',keyCode:'Down'});window.webContents.sendInputEvent({type:'keyUp',keyCode:'Down'})
    await wait(`Boolean(document.querySelector('.canvas-card-menu [role="menuitem"]'))`)
   }
   await menu('画板操作')
   report.board_menu=await evaluate(`[...document.querySelectorAll('.canvas-card-menu [role="menuitem"]')].map(item=>item.textContent)`)
   assert.ok(report.board_menu.includes('编辑画板')&&report.board_menu.includes('删除画板'))
   window.webContents.sendInputEvent({type:'keyDown',keyCode:'Escape'});window.webContents.sendInputEvent({type:'keyUp',keyCode:'Escape'})
   await wait(`!document.querySelector('.canvas-card-menu')`)
   await menu('画布操作')
   report.canvas_menu=await evaluate(`[...document.querySelectorAll('.canvas-card-menu [role="menuitem"]')].map(item=>item.textContent)`)
   for(const label of ['刷新关联','选中对话分组','删除所选','自动排列'])assert.ok(report.canvas_menu.includes(label))
   const visible=await evaluate(`Boolean(document.querySelector('.react-flow__minimap'))`)
   await evaluate(`(()=>{document.querySelector('.canvas-card-menu [role="menuitemcheckbox"]').click();return true})()`)
   await wait(`Boolean(document.querySelector('.react-flow__minimap'))===${!visible}`)
   await menu('画布操作')
   await evaluate(`(()=>{document.querySelector('.canvas-card-menu [role="menuitemcheckbox"]').click();return true})()`)
   await wait(`Boolean(document.querySelector('.react-flow__minimap'))===${visible}`)
   await evaluate(`(()=>{document.querySelector('button[aria-label="恢复到 100%"]').click();return true})()`)
   await wait(`document.querySelector('.canvas-zoom-readout')?.textContent==='100%'`)
   await evaluate(`(()=>{document.querySelector('.canvas-dock-panel button[aria-label="查看全部"]').click();return true})()`)
   toolbarChecked=true;record('codeg_canvas_dock_viewport_keyboard_menus_and_minimap_toggle')
   await writeFile(join(config.evidence,'00-codeg-canvas-dock.png'),(await window.webContents.capturePage()).toPNG())
  }
 }
 const inspect=async kind=>{
  await board()
  await evaluate(`(()=>{[...document.querySelector('[data-id="${nodes[kind]}"]').querySelectorAll('button')].find(button=>button.textContent==='读取详情').click();return true})()`)
  await wait(`Boolean(document.querySelector('.canvas-object-dialog .canvas-object-details'))`)
 }
 const workspace=async()=>{
  await evaluate(`(()=>{[...document.querySelector('.canvas-object-dialog').querySelectorAll('button')].find(button=>button.textContent==='在工作区中操作').click();return true})()`)
  await wait(`!document.querySelector('.canvas-workspace')`)
 }
 await inspect('task')
 assert.equal(await evaluate(`document.querySelector('.canvas-object-details').dataset.taskId`),taskId)
 assert.ok(await evaluate(`document.querySelector('.canvas-object-details').textContent.includes('已取消')`))
 record('actual_canvas_task_pin_reads_exact_cancelled_task')
 await workspace()
 await wait(`(()=>{const task=document.querySelector('.task-detail[data-task-id="${taskId}"]');return task&&task.getBoundingClientRect().height>0})()`)
 record('actual_history_task_pin_opens_original_workspace_and_exact_task_detail')
 await inspect('mission')
 assert.equal(await evaluate(`document.querySelector('.canvas-object-details').dataset.missionId`),missionId)
 record('actual_mission_pin_reads_original_mission_id')
 await workspace()
 await wait(`(()=>{const mission=document.querySelector('.mission-intro[data-mission-id="${missionId}"]');return mission&&mission.getBoundingClientRect().height>0})()`)
 record('actual_mission_pin_opens_original_mission_workspace')
 report.delivery=await rpc('missions.delivery',{missionId})
 assert.equal(report.delivery.threadId,threadId,JSON.stringify(report.delivery))
 assert.equal(Object.hasOwn(report.delivery,'campId'),false)
 record('mission_delivery_uses_canonical_thread_identity')
 await inspect('mission_diff')
 assert.equal(await evaluate(`document.querySelector('.canvas-object-details').dataset.missionId`),missionId)
 await wait(`document.querySelector('.canvas-object-details').textContent.includes('当前使命尚无可用的 Git 工作区。')`)
 record('diff_pin_without_prepared_workspace_reports_unavailability_without_substituting_a_file')
 await evaluate(`(()=>{[...document.querySelector('.canvas-object-dialog').querySelectorAll('button')].find(button=>button.textContent==='关闭').click();return true})()`)
 await wait(`!document.querySelector('.canvas-object-dialog')`)
 const fileCard=`document.querySelector('[data-id="${nodes.file}"]')`
 await wait(`Boolean(${fileCard}?.querySelector('.canvas-file-actions button[aria-label="在工作区中操作"]'))`,25_000)
 await evaluate(`(()=>{${fileCard}.querySelector('.canvas-file-actions button[aria-label="在工作区中操作"]').click();return true})()`)
 await wait(`!document.querySelector('.canvas-workspace')&&Boolean(document.querySelector('.camp-workspace[aria-label]'))`)
 assert.ok(await evaluate(`document.querySelector('.camp-workspace[aria-label]').getAttribute('aria-label').includes('文件工程A')`))
 await wait(`Boolean([...document.querySelectorAll('.file-preview-path-row')].find(row=>row.getBoundingClientRect().height>0&&row.textContent.includes('说明 #中文.txt')))`)
 record('actual_file_pin_opens_its_original_thread_and_exact_file_tab')
 assert.equal((await rpc('tasks.get',{threadId,taskId})).status,'cancelled')
 assert.equal((await rpc('missions.get',{missionId})).missionId,missionId)
 report.no_model=true
 await writeFile(join(config.evidence,'01-original-file-workspace.png'),(await window.webContents.capturePage()).toPNG())
}
async function qualifyPrivateUi(window,{evaluate,wait,rpc}){
 const prior=JSON.parse(await readFile(config.previous_result,'utf8'))
 assert.equal(prior.status,'passed')
 const {board_id:boardId,conversation_id:conversationId,scopes}=prior.private_context
 await openCanvasBoard(evaluate,wait,boardId)
 await wait(`Boolean(document.querySelector('[data-id="${scopes[1].nodeId}"]'))`)
 await evaluate(`(()=>{document.querySelector('.canvas-dock-panel button[aria-label="查看全部"]').click();return true})()`)
 await evaluate(`(()=>{[...document.querySelector('[data-id="${scopes[1].nodeId}"]').querySelectorAll('button')].find(button=>button.textContent==='展开对话').click();return true})()`)
 const frame=`document.querySelector('.canvas-conversation-frame')`
 await wait(`Boolean(${frame}?.querySelector('.single-chat-composer-box .attachment-open[aria-label="打开文件预览 乙材料.txt"]'))`,25_000)
 assert.ok(!(await evaluate(`${frame}.querySelector('.single-chat-composer-box').textContent`)).includes('甲材料'))
 record('actual_scoped_composer_renders_only_its_own_attachment_card')
 await wait(`${frame}.querySelector('.single-chat-composer-box .attachment-open[aria-label="打开文件预览 乙材料.txt"]').disabled===false`)
 await evaluate(`(()=>{${frame}.querySelector('.single-chat-composer-box .attachment-open[aria-label="打开文件预览 乙材料.txt"]').click();return true})()`)
 await wait(`Boolean([...document.querySelectorAll('.file-preview-content')].find(element=>element.getBoundingClientRect().height>0&&element.textContent.includes('乙卡专用材料')))`,20_000)
 const after=await rpc('singleChat.get',{conversationId,composerScope:scopes[1]})
 assert.deepEqual(after.agentRuns,[]);assert.deepEqual(after.messages,[])
 assert.deepEqual(after.draft.attachments.map(x=>x.displayName),['乙材料.txt'])
 record('actual_scoped_attachment_button_opens_authorized_file_without_sending_or_consuming_input')
 await pause(100)
 await writeFile(join(config.evidence,'01-private-attachment-preview.png'),(await window.webContents.capturePage()).toPNG())
 report.private_context=prior.private_context
}
await import(config.production_main)
