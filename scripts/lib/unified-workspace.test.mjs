import assert from 'node:assert/strict'
import {spawn} from 'node:child_process'
import {once} from 'node:events'
import {mkdir,mkdtemp,writeFile} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join,resolve} from 'node:path'
import test from 'node:test'
import electron from 'electron'
import react from '@vitejs/plugin-react'
import {build} from 'vite'
import {admitElectronIntegrationTest} from './electron-sandbox-capability.mjs'
const root=resolve(import.meta.dirname,'../..'),source=join(root,'scripts/fixtures/unified-workspace')
test('统一工作区原生网页、编辑器、菜单和授权操作回归',{timeout:180000},async t=>{
 if(!admitElectronIntegrationTest(t))return
 const artifactDir=process.env.COAXI_UNIFIED_WORKSPACE_ARTIFACTS?resolve(process.env.COAXI_UNIFIED_WORKSPACE_ARTIFACTS):await mkdtemp(join(tmpdir(),'coaxi-unified-'))
 await mkdir(artifactDir,{recursive:true})
 await writeFile(join(artifactDir,'preload.cjs'),"const {contextBridge,ipcRenderer}=require('electron');contextBridge.exposeInMainWorld('unifiedFixture',{request:(m,p)=>ipcRenderer.invoke('unified-request',m,p),interaction:(m,p)=>ipcRenderer.invoke('unified-interaction',m,p),office:{open:(threadId,path,editable,embedded)=>ipcRenderer.invoke('unified-office',{threadId,path,editable,embedded}),present:input=>ipcRenderer.invoke('unified-office',{...input,operation:'present'}),inspect:input=>ipcRenderer.invoke('unified-office',{...input,operation:'inspect'}),close:id=>ipcRenderer.invoke('unified-office',{id,operation:'close'})},browser:Object.fromEntries(['present','inspect','list','open','control','preferences','updatePreferences','profiles','profileControl','downloads','downloadControl'].map(key=>[key,(...args)=>ipcRenderer.invoke('unified-browser',key,key==='open'?{threadId:args[0],url:args[1],profileId:args[2]}:args[0])]))});")
 await build({configFile:false,root:source,base:'./',logLevel:'error',plugins:[react()],resolve:{alias:{'@contracts':join(root,'packages/contracts/src/index.ts')}},build:{outDir:join(artifactDir,'renderer'),minify:false}})
 await build({configFile:false,logLevel:'error',build:{ssr:join(root,'apps/desktop/src/main/browser-workspace.ts'),outDir:join(artifactDir,'main'),minify:false,rollupOptions:{external:['electron'],output:{format:'cjs',entryFileNames:'browser.cjs'}}}})
 await build({configFile:false,logLevel:'error',build:{ssr:join(root,'apps/desktop/src/main/office-workspace.ts'),outDir:join(artifactDir,'office'),minify:false,rollupOptions:{external:['electron'],output:{format:'cjs',entryFileNames:'office.cjs'}}}})
 const env={...process.env,ELECTRON_DISABLE_SECURITY_WARNINGS:'true'};delete env.ELECTRON_RUN_AS_NODE
 const child=spawn(electron,[join(source,'main.cjs'),join(artifactDir,'renderer/index.html'),artifactDir,join(artifactDir,'main/browser.cjs')],{env,stdio:['ignore','pipe','pipe']})
 let output='';child.stdout.on('data',chunk=>{output+=chunk});child.stderr.on('data',chunk=>{output+=chunk})
 const timer=setTimeout(()=>child.kill('SIGKILL'),140000)
 try{const [code]=await once(child,'close');await writeFile(join(artifactDir,'electron.log'),output);assert.equal(code,0,output.slice(-9000));const report=output.split('\n').find(row=>row.startsWith('{"ok":'));assert.ok(report,'missing report');assert.equal(JSON.parse(report).ok,true);console.log('Artifacts: '+artifactDir)}finally{clearTimeout(timer);if(child.exitCode===null&&child.signalCode===null)child.kill('SIGKILL')}
})
