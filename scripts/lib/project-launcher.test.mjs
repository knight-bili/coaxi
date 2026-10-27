import assert from 'node:assert/strict'
import {randomUUID} from 'node:crypto'
import {spawn,execFileSync} from 'node:child_process'
import {createInterface} from 'node:readline'
import {mkdir,mkdtemp,readFile,writeFile,realpath,rm} from 'node:fs/promises'
import {join,resolve,toNamespacedPath} from 'node:path'
import {tmpdir} from 'node:os'
import test from 'node:test'
const repository=resolve(import.meta.dirname,'../..')
const binary=process.env.COAXI_CORE_BIN??join(repository,'resources/bin/windows-x64/coaxi-core.exe')

test('独立启动器经真实 Core 创建中文文件、幂等恢复、目录边界和原 Thread 文件读写', {timeout:90_000,skip:process.platform!=='win32'},async()=>{
  const fixture=await realpath(await mkdtemp(join(tmpdir(),'coaxi-launcher-core-'))),authority=join(fixture,'authority'),data=join(authority,'Core'),parent=join(fixture,'projects'),tools=join(fixture,'tools'),count=join(fixture,'invocations.json')
  await Promise.all([mkdir(parent),mkdir(tools)])
  execFileSync(binary,['--prepare-windows-data-root',authority],{stdio:'pipe'})
  const generator=join(tools,'generate.cjs'),program=join(tools,'npx.cmd')
  await writeFile(generator,`const fs=require('node:fs'),p=require('node:path');const args=process.argv.slice(2),name=args[args.indexOf('-n')+1];const file=${JSON.stringify(count)};const calls=fs.existsSync(file)?JSON.parse(fs.readFileSync(file,'utf8')):[];calls.push(name);fs.writeFileSync(file,JSON.stringify(calls));fs.mkdirSync(name);fs.writeFileSync(p.join(name,'说明.md'),'# 中文工程\\n创建成功\\n');`)
  await writeFile(program,`@echo off\r\n"${process.execPath}" "${generator}" %*\r\n`)
  const child=spawn(binary,['--data-dir',data,'--skill-library-root',join(fixture,'skills'),'--mcp-config-path',join(fixture,'mcp.json'),'--runtime-camp-files-root',join(data,'runtime-files')],{cwd:repository,env:{...process.env,PATH:tools+';'+process.env.PATH},stdio:['pipe','pipe','pipe']})
  const pending=new Map();let id=0,stderr=''
  child.stderr.on('data',chunk=>stderr=(stderr+chunk).slice(-3000))
  createInterface({input:child.stdout}).on('line',line=>{let value;try{value=JSON.parse(line)}catch{return}const waiter=pending.get(value.id);if(!waiter)return;pending.delete(value.id);value.error?waiter.reject(Object.assign(new Error(value.error.message),value.error)):waiter.resolve(value.result)})
  child.on('error',error=>{for(const waiter of pending.values())waiter.reject(error)})
  const request=(method,params={})=>new Promise((resolve,reject)=>{const key=String(++id);const timer=setTimeout(()=>{pending.delete(key);reject(new Error(`${method} timeout ${stderr}`))},20000);pending.set(key,{resolve:value=>{clearTimeout(timer);resolve(value)},reject:error=>{clearTimeout(timer);reject(error)}});child.stdin.write(JSON.stringify({id:key,method,params})+'\n')})
  const launcher=input=>request('workbench.projectLauncher',input)
  try{
    assert.ok((await launcher({operation:'detect'})).tools.some(tool=>tool.name==='npm'&&tool.installed))
    for(const input of [{},{projectPath:'relative'},{projectPath:data},{projectPath:parent,scope:{threadId:'other'}}])await assert.rejects(launcher({...input,operation:'preview',options:{kind:'shadcn',name:'app',packageManager:'npm',preset:'abc'}}))
    const options={kind:'shadcn',name:'中文项目',parent:'',packageManager:'npm',template:'vite',preset:'abc'}
    const plan=await launcher({projectPath:parent,operation:'preview',options}),commandId=randomUUID()
    await launcher({projectPath:parent,operation:'start',planId:plan.planId,commandId,confirmed:true})
    let task;for(let attempt=0;attempt<60;attempt++){task=await launcher({projectPath:parent,operation:'status',commandId});if(task.state!=='running')break;await new Promise(resolve=>setTimeout(resolve,100))}
    assert.equal(task.state,'completed',JSON.stringify(task))
    assert.equal(await readFile(join(parent,'中文项目','说明.md'),'utf8'),'# 中文工程\n创建成功\n')
    assert.equal((await launcher({projectPath:parent,operation:'start',planId:plan.planId,commandId,confirmed:true})).state,'completed')
    assert.deepEqual(JSON.parse(await readFile(count,'utf8')),['中文项目'])
    await assert.rejects(launcher({projectPath:tools,operation:'status',commandId}))
    await assert.rejects(launcher({projectPath:parent,operation:'preview',options}))
    const preflight=await request('camps.creationPreflight')
    const camp=await request('camps.create',{commandId:randomUUID(),name:'工程验收',workspace:{projectPath:join(parent,'中文项目')},memberAgentIds:preflight.presentMembers.map(item=>item.agentId),defaultLeadAgentId:preflight.initialLeadAgentId,collaborationMode:'peer'})
    assert.equal(camp.status,'applied',JSON.stringify(camp))
    const scope={threadId:camp.payload.threadId},file=await request('workbench.files.read',{scope,path:'说明.md'})
    await request('workbench.files.mutate',{scope,commandId:randomUUID(),action:'save',path:'说明.md',text:'# 文件保存验证\n中文正常\n',expectedDigest:file.digest})
    assert.equal(await readFile(join(parent,'中文项目','说明.md'),'utf8'),'# 文件保存验证\n中文正常\n')
    assert.equal(toNamespacedPath((await request('workbench.files.list',{scope,path:''})).workspaceRoot),toNamespacedPath(await realpath(join(parent,'中文项目'))))
  }finally{
    child.stdin.end();await Promise.race([new Promise(resolve=>child.once('close',resolve)),new Promise(resolve=>setTimeout(resolve,3000))]);if(child.exitCode===null)child.kill()
    await rm(fixture,{recursive:true,force:true})
  }
})
