import assert from 'node:assert/strict'
import {randomUUID,createHash} from 'node:crypto'
import {readFile,writeFile,mkdir} from 'node:fs/promises'
import {join,dirname} from 'node:path'
import {launchHost,within} from '../lib/host-test-client.mjs'
import {coreDataDirectoryArguments} from '../lib/runtime-camp-files-root.mjs'
import {launchAcceptanceBrowser,pause} from '../lib/host-web-browser.mjs'
import {stripVTControlCharacters} from 'node:util'

// 新App退出后，实际Host/Web继续访问同一隔离Core事实；不复制数据库或伪造权限回执。
export async function qualifyCanvasFinalWeb({binary,uiDirectory,chrome,output,checkpoint,repositoryRoot,source}){
 const context=JSON.parse(await readFile(checkpoint,'utf8')),dataDir=join(context.dataRoot,'Core'),profile=join(context.dataRoot,'R10-isolated-browser-'+output.split(/[\\/]/).at(-1)),home=join(dirname(context.dataRoot),'home')
 await mkdir(output,{recursive:false})
 const env={};for(const key of ['SystemRoot','WINDIR','COMSPEC','PATHEXT'])if(process.env[key])env[key]=process.env[key]
 Object.assign(env,{PATH:[dirname(process.execPath),'E:/software/encoding/common/Git/bin',join(process.env.SystemRoot,'System32')].join(';'),HOME:home,USERPROFILE:home,APPDATA:join(home,'AppData/Roaming'),LOCALAPPDATA:join(home,'AppData/Local'),TEMP:join(dirname(context.dataRoot),'tmp'),TMP:join(dirname(context.dataRoot),'tmp'),SHELL:process.execPath,ROVAI_DISABLE_AUTO_UPDATE_CHECKS:'1'})
 const report={status:'running',channel:'automatic_acceptance',source,dataDir,skillLibraryRoot:join(dataDir,'managed-skill-library'),mcpConfig:join(dataDir,'mcp.json'),profile,checks:[],startedAt:new Date().toISOString(),externalModelCalls:0}
 let host,browser,stopEvents,browserBearer,fileReadBody
 const record=async name=>{report.checks.push({name,passed:true});await writeFile(join(output,'progress.json'),JSON.stringify({checks:report.checks,at:new Date().toISOString()},null,2)+'\n');console.log(JSON.stringify({check:name,passed:true}))}
 const until=async predicate=>{const end=Date.now()+20000;while(Date.now()<end){const value=await predicate();if(value)return value;await pause(70)}throw Error('Actual Host state did not settle')}
 try{
  host=launchHost(binary,[...coreDataDirectoryArguments(dataDir,{homeDirectory:home}),'--skill-library-root',report.skillLibraryRoot,'--mcp-config-path',report.mcpConfig],{cwd:repositoryRoot,env,windowsHide:true});report.hostPid=host.child.pid;await within(host.ready)
  const web=await host.request('host.web.start',{listen:'127.0.0.1:0',uiDirectory});report.origin=web.origin
  const login=async()=>{const r=await fetch(web.origin+'/api/v1/login',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({protocolVersion:4,administratorToken:web.administratorToken}),signal:AbortSignal.timeout(15000)});assert.equal(r.status,200);return r.json()}
  const healthy=await login()
  const raw=(token,operation,params={})=>fetch(web.origin+'/api/v1/request',{method:'POST',headers:{'Content-Type':'application/json',...(token?{Authorization:'Bearer '+token}:{})},body:JSON.stringify({operation,params}),signal:AbortSignal.timeout(15000)})
  const call=async(token,operation,params={})=>{const r=await raw(token,operation,params);assert.equal(r.status,200,operation);const value=await r.json();assert.equal(value.error,null,operation);return value.result}
  const snapshot=()=>call(healthy.token,'canvas.nodes.snapshot',{boardId:context.boardId})
  let primary=await snapshot()
  // 同一隔离数据中保留前一失败批次已经合法提交的改名/便签移动；不能倒写旧证据。
  // 原Native冷恢复已精确比较全部字段，此处核对身份与不可变绑定，随后以当前布局为重载基线。
  assert.equal(primary.nodes.length,context.primary.nodes.length)
  for(const original of context.primary.nodes){const current=primary.nodes.find(n=>n.id===original.id);assert.ok(current)
   if(original.data.kind==='note'){const {x,y,...data}=current.data,{x:oldX,y:oldY,...expected}=original.data;assert.deepEqual(data,expected);assert.equal(current.boardId,original.boardId)}else assert.deepEqual(current,original)
  }
  const {name:currentName,revision:currentRevision,updatedAt:currentUpdated,...boardIdentity}=primary.board
  const {name:originalName,revision:originalRevision,updatedAt:originalUpdated,...expectedIdentity}=context.primary.board
  assert.deepEqual(boardIdentity,expectedIdentity);assert.ok([originalName,'R10 Web正式检查后保留原画板'].includes(currentName))
  const other=await call(healthy.token,'canvas.nodes.snapshot',{boardId:context.otherBoardId});assert.deepEqual(other.nodes,context.secondary.nodes)
  await record('current_host_http_reads_same_eight_native_created_nodes_both_boards_and_immutable_bindings_without_copying_authority')
  browser=await launchAcceptanceBrowser({executable:chrome,env,args:['--headless=new','--no-first-run','--disable-sync','--disable-background-networking','--disable-component-update','--disable-default-apps','--disable-extensions','--remote-debugging-port=0',`--user-data-dir=${profile}`,'about:blank']})
  stopEvents=browser.onProtocolEvent(message=>{
   if(message.method!=='Network.requestWillBeSent'||!message.params.request.url.startsWith(web.origin+'/api/v1/'))return
   const request=message.params.request,auth=Object.entries(request.headers).find(([key])=>key.toLowerCase()==='authorization')?.[1]
   if(typeof auth==='string'&&auth.startsWith('Bearer '))browserBearer=auth.slice(7)
   if(new URL(request.url).pathname==='/api/v1/files'&&request.postData)fileReadBody=request.postData
  })
  await browser.send('Page.addScriptToEvaluateOnNewDocument',{source:"window.r10WindowErrors=[];window.addEventListener('error',event=>window.r10WindowErrors.push({message:event.message,stack:String(event.error?.stack??'')}));"})
  await browser.send('Page.navigate',{url:web.origin});await browser.wait('Boolean(document.querySelector("#administrator-token"))',30000)
  await browser.click('document.querySelector("#administrator-token")');await browser.send('Input.insertText',{text:web.administratorToken});await browser.click('document.querySelector(".web-login button[type=submit]")')
  await browser.wait('!document.querySelector(".web-login-overlay")&&Boolean(document.querySelector("button[aria-label=无限画布]"))',30000)
  const open=async boardId=>{
   await browser.click('document.querySelector("button[aria-label=无限画布]")')
   await browser.wait('Boolean(document.querySelector(".canvas-breadcrumb-root:not(:disabled)"))',30000)
   if(!await browser.evaluate(`Boolean(document.querySelector('.canvas-board-title[data-board-id="${boardId}"]'))`)){
    await browser.click('document.querySelector(".canvas-breadcrumb-root")');await browser.wait(`Boolean(document.querySelector('.canvas-board-tile[data-board-id="${boardId}"]:not(:disabled)'))`)
    await browser.click(`document.querySelector('.canvas-board-tile[data-board-id="${boardId}"]')`)
   }
   await browser.wait(`Boolean(document.querySelector('.canvas-board-title[data-board-id="${boardId}"]:not(:disabled)'))`)
  }
  await open(context.boardId)
  for(const n of context.nodes)await browser.wait(`Boolean(document.querySelector('.react-flow__node[data-id="${n.id}"] article'))`,30000)
  await browser.wait(`document.querySelector('.canvas-card-file')?.textContent.includes(${JSON.stringify(context.sourceText.trim())})`,30000)
  assert.ok(browserBearer&&browserBearer!==healthy.token);assert.ok(fileReadBody,'capture an actual authorized file read body for the later revocation check')
  await browser.click('document.querySelector(\'.canvas-dock-panel button[aria-label="查看全部"]\')');await pause(500);await browser.capture(join(output,'web-all-eight.png'))
  report.deviceLocalGroupBoundary='Native设备分组不复制到新浏览器；原缺失分组提示保持，不能伪造Core共享分组。'
  await record('production_web_entry_real_login_renders_eight_types_and_exact_original_file_with_device_scoped_group_boundary')
  const rename=await call(browserBearer,'canvas.boards.update',{commandId:randomUUID(),command:{boardId:context.boardId,expectedRevision:primary.board.revision,name:'R10 Web正式检查后保留原画板'}});assert.equal(rename.status,'applied')
  primary=await snapshot();const note=primary.nodes.find(n=>n.data.kind==='note'),oldRevision=primary.board.revision
  const move={type:'update',nodeId:note.id,patch:{x:note.data.x+16,y:note.data.y+8}}
  assert.equal((await call(browserBearer,'canvas.nodes.apply',{commandId:randomUUID(),command:{boardId:context.boardId,expectedRevision:oldRevision,mutation:move}})).status,'applied')
  const beforeConflict=await snapshot(),rejected=await call(browserBearer,'canvas.nodes.apply',{commandId:randomUUID(),command:{boardId:context.boardId,expectedRevision:oldRevision,mutation:{type:'update',nodeId:note.id,patch:{title:'不得保存的旧版本'}}}})
  assert.equal(rejected.status,'rejected');assert.equal(rejected.code,'canvas.nodes.conflict');const afterConflict=await snapshot();assert.deepEqual(afterConflict.board,beforeConflict.board);assert.deepEqual(afterConflict.nodes,beforeConflict.nodes)
  await record('real_web_board_rename_node_move_and_stale_revision_rejection_preserve_other_nodes_and_original_body')
  await browser.reload();await browser.wait('!document.querySelector(".web-login-overlay")&&Boolean(document.querySelector("button[aria-label=无限画布]"))',30000);await open(context.boardId)
  await browser.wait(`document.querySelector('.canvas-card-file')?.textContent.includes(${JSON.stringify(context.sourceText.trim())})`,30000)
  assert.deepEqual((await snapshot()).nodes,beforeConflict.nodes);assert.deepEqual((await call(healthy.token,'canvas.nodes.snapshot',{boardId:context.otherBoardId})).nodes,context.secondary.nodes)
  await record('production_web_reload_preserves_same_board_nodes_file_binding_and_independent_second_board')
  const target={boardId:context.boardId,nodeId:context.nodes.find(n=>n.data.kind==='terminal').id}
  let term=await call(browserBearer,'canvas.terminals.snapshot',{target,afterSeq:0});assert.equal(term.exists,false)
  term=await call(browserBearer,'canvas.terminals.start',{target,coreGeneration:term.coreGeneration,requestId:randomUUID(),expectedGeneration:null,cols:80,rows:20})
  await until(async()=>{term=await call(healthy.token,'canvas.terminals.snapshot',{target,afterSeq:0});return term.phase==='running'})
  const marker='R10_WEB_'+randomUUID().replaceAll('-',''),input={target,generation:term.generation,inputId:randomUUID(),expectedInputSequence:term.inputSequence,bytesBase64:Buffer.from(`console.log(${JSON.stringify(marker)})\r`).toString('base64')}
  await call(browserBearer,'canvas.terminals.write',input)
  term=await until(async()=>{const s=await call(healthy.token,'canvas.terminals.snapshot',{target,afterSeq:0}),out=stripVTControlCharacters(Buffer.concat(s.chunks.map(c=>Buffer.from(c.bytesBase64,'base64'))).toString('utf8'));return out.split(/\r?\n/).some(x=>x.trim()===marker)?s:false})
  await browser.wait(`document.querySelector('.canvas-terminal')?.textContent.includes(${JSON.stringify(marker)})`,30000)
  await record('actual_host_web_pty_generation_and_exact_output_reach_the_original_browser_terminal')
  const baseline=await snapshot(),sequence=term.inputSequence,logout=await fetch(web.origin+'/api/v1/logout',{method:'POST',headers:{Authorization:'Bearer '+browserBearer},signal:AbortSignal.timeout(15000)});assert.equal(logout.status,204)
  const forbidden=[['canvas.boards.list',{}],['canvas.boards.update',{commandId:randomUUID(),command:{boardId:context.boardId,expectedRevision:baseline.board.revision,name:'不得越权改名'}}],['canvas.nodes.snapshot',{boardId:context.boardId}],['canvas.nodes.apply',{commandId:randomUUID(),command:{boardId:context.boardId,expectedRevision:baseline.board.revision,mutation:move}}],['canvas.terminals.write',{...input,inputId:randomUUID(),expectedInputSequence:sequence}],['canvas.terminals.stop',{target,generation:term.generation}]]
  for(const [operation,params] of forbidden)assert.equal((await raw(browserBearer,operation,params)).status,401,operation)
  assert.equal((await fetch(web.origin+'/api/v1/files',{method:'POST',headers:{Authorization:'Bearer '+browserBearer,'Content-Type':'application/json'},body:fileReadBody,signal:AbortSignal.timeout(15000)})).status,401)
  assert.equal((await raw(null,'canvas.boards.list')).status,401);const afterRevocation=await snapshot();assert.deepEqual(afterRevocation.board,baseline.board);assert.deepEqual(afterRevocation.nodes,baseline.nodes)
  const afterDenied=await call(healthy.token,'canvas.terminals.snapshot',{target,afterSeq:0});assert.equal(afterDenied.generation,term.generation);assert.equal(afterDenied.inputSequence,sequence)
  assert.equal(createHash('sha256').update(await readFile(context.sourceFile)).digest('hex'),context.sourceSha256)
  report.revocation={rejectedOperations:forbidden.map(([name])=>name),exactPreviouslyAuthorizedFileReadRejected:true,unauthenticatedRejected:true,graphUnchanged:true,inputSequenceUnchanged:true,sourceBytesUnchanged:true}
  await record('revoked_real_browser_session_cannot_read_or_mutate_boards_nodes_files_or_active_pty_while_independent_session_remains_valid')
  await browser.reload();await browser.wait('Boolean(document.querySelector("#administrator-token"))',30000);await browser.capture(join(output,'web-revoked-login.png'))
  await call(healthy.token,'canvas.terminals.stop',{target,generation:term.generation});await until(async()=>{const s=await call(healthy.token,'canvas.terminals.snapshot',{target,afterSeq:0});return s.cleanupComplete})
  report.browserErrors=browser.errors;report.windowErrors=await browser.evaluate('window.r10WindowErrors');assert.deepEqual(browser.errors,[])
  report.networkStatuses=browser.responses;await record('revoked_production_web_returns_to_real_login_and_remaining_authorized_owner_reaps_only_its_terminal')
  report.status='passed'
 }catch(error){report.status='failed';report.error=String(error.stack??error);if(browser){report.browserErrors=browser.errors;report.windowErrors=await browser.evaluate('window.r10WindowErrors').catch(()=>null);report.dom=await browser.evaluate('document.body.innerText.slice(-2200)').catch(()=>null);await browser.capture(join(output,'failure.png')).catch(()=>{})}}
 finally{
  stopEvents?.();if(browser)try{await browser.close()}catch(error){report.status='failed';report.browserCloseError=String(error)}
  if(host){try{await host.close({timeoutMs:15000})}catch(error){report.status='failed';report.hostCloseError=String(error)}report.hostExit={code:host.child.exitCode,signal:host.child.signalCode};if(host.child.exitCode!==0||host.child.signalCode!==null)report.status='failed'}
  report.finishedAt=new Date().toISOString();await writeFile(join(output,'result.json'),JSON.stringify(report,null,2)+'\n')
 }
 return report
}
