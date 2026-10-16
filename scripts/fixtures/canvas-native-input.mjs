import assert from 'node:assert/strict'
import { app } from 'electron'
import { mkdir,readFile,writeFile } from 'node:fs/promises'
import { join } from 'node:path'
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
 }catch(error){report.status='failed';report.error=String(error.stack??error);report.native_keys=await evaluate('window.nativeKeys').catch(()=>null);report.dom=await evaluate('document.body.innerText.slice(-2500)').catch(()=>null)}
 finally{report.finished_at=new Date().toISOString();await writeFile(join(config.evidence,'result.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({status:report.status,evidence:config.evidence}));process.exitCode=report.status==='passed'?0:1;app.quit()}
}
await import(config.production_main)
