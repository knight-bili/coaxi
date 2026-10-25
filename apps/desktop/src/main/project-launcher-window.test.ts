import {beforeEach,describe,expect,it,vi} from 'vitest'
const fixture=vi.hoisted(()=>({windows:[] as any[],choose:vi.fn(),write:vi.fn()}))
vi.mock('node:fs',()=>({readFileSync:()=>{throw new Error('not found')},writeFileSync:fixture.write}))
vi.mock('electron',()=>({app:{getPath:()=>'/fixture/user'},dialog:{showOpenDialog:fixture.choose},BrowserWindow:class{
  webContents={mainFrame:{},setWindowOpenHandler:vi.fn(),on:vi.fn(),send:vi.fn()};handlers=new Map();destroyed=false;show=vi.fn();hide=vi.fn();focus=vi.fn();restore=vi.fn()
  constructor(){fixture.windows.push(this)}isDestroyed(){return this.destroyed}isMinimized(){return false}setMenu(){}on(name:string,callback:unknown){this.handlers.set(name,callback)}once(){}loadFile(){return Promise.resolve()}loadURL(){return Promise.resolve()}destroy(){this.destroyed=true;this.handlers.get('closed')?.()}
}}))
import {BrowserWindow} from 'electron'
import {ProjectLauncherWindow} from './project-launcher-window'
beforeEach(()=>{fixture.windows.length=0;vi.clearAllMocks()})
const event=(window:any,frame=window.webContents.mainFrame)=>({sender:window.webContents,senderFrame:frame}) as any
describe('启动器窗口归属',()=>{
  it('仅本窗口主 frame 可选目录，模板请求始终绑定原生已选择目录',async()=>{
    const main=new BrowserWindow(),calls:any[]=[]
    const request=vi.fn(async(method:string,input:any)=>{calls.push([method,input]);return method==='workspaces.inspect'?{projectPath:'/verified'}:{tools:[]}})
    const launcher=new ProjectLauncherWindow(request as never,()=>main)
    await expect(launcher.invoke(event(main,{}),'open')).rejects.toThrow('local_owner')
    await launcher.invoke(event(main),'open')
    const window=fixture.windows[1]
    await expect(launcher.invoke(event(main),'request',{operation:'detect'})).rejects.toThrow('window_owner')
    await expect(launcher.invoke(event(window,{}),'chooseDirectory')).rejects.toThrow('window_owner')
    fixture.choose.mockResolvedValue({canceled:false,filePaths:['/chosen']})
    expect(await launcher.invoke(event(window),'chooseDirectory')).toBe('/verified')
    await launcher.invoke(event(window),'request',{operation:'preview',projectPath:'/injected',scope:{threadId:'other'},options:{kind:'shadcn',scope:{threadId:'other'}}})
    expect(calls.at(-1)[1]).toMatchObject({projectPath:'/verified',scope:undefined,options:{scope:undefined}})
    await expect(launcher.invoke(event(window),'request',{operation:'delete'})).rejects.toThrow('not_admitted')
    await expect(launcher.invoke(event(window),'peek')).rejects.toThrow('local_owner')
    const preventDefault=vi.fn();window.handlers.get('close')({preventDefault})
    expect(preventDefault).toHaveBeenCalledOnce();expect(window.hide).toHaveBeenCalledOnce()
  })
  it('未完成不能交接，成功后只允许主窗口读取和确认结果',async()=>{
    const main=new BrowserWindow();let completed=false
    const request=vi.fn(async(method:string)=>method==='workspaces.inspect'?{projectPath:'/created'}:{state:completed?'completed':'running',path:'/created'})
    const launcher=new ProjectLauncherWindow(request as never,()=>main)
    await launcher.invoke(event(main),'open');const window=fixture.windows[1]
    await expect(launcher.invoke(event(window),'enter','task-a')).rejects.toThrow('尚未创建完成')
    completed=true;await launcher.invoke(event(window),'enter','task-a')
    const pending=await launcher.invoke(event(main),'peek') as {id:string;path:string}
    expect(pending.path).toBe('/created')
    await launcher.invoke(event(main),'acknowledge','other');expect(await launcher.invoke(event(main),'peek')).toEqual(pending)
    await launcher.invoke(event(main),'acknowledge',pending.id);expect(await launcher.invoke(event(main),'peek')).toBeNull()
    launcher.closeAll();await launcher.invoke(event(main),'open')
    const reopened=fixture.windows.at(-1),preventDefault=vi.fn();reopened.handlers.get('close')({preventDefault});expect(preventDefault).toHaveBeenCalledOnce()
  })
})
