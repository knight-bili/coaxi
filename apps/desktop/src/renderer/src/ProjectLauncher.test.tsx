// @vitest-environment jsdom
import {afterEach,describe,expect,it,vi} from 'vitest'
import {cleanup,fireEvent,render,waitFor,within} from '@testing-library/react'
import {ProjectLauncher} from './ProjectLauncher'
afterEach(()=>{cleanup();localStorage.clear()})
describe('独立项目启动器',()=>{
  it.each(['download','process'])('%s 超时显示可执行的中文提示并保留现场与日志',async(kind)=>{
    localStorage.setItem('failed-template',JSON.stringify({directory:'/chosen',commandId:'failed'}))
    const request=vi.fn(async(input:Record<string,unknown>)=>input.operation==='detect'?{tools:[]}:{commandId:'failed',state:'failed',partialPath:'/chosen/.partial',error:kind==='process'?'office.process_timeout':'office.process_failed: exit code:1',log:kind==='process'?'':'UND_ERR_CONNECT_TIMEOUT https://ui.shadcn.com'})
    const enter=vi.fn()
    const view=render(<ProjectLauncher storageKey="failed-template" request={request as never} chooseDirectory={async()=>null} onEnter={enter}/>)
    expect(await view.findByRole('alert')).toHaveProperty('textContent',expect.stringContaining('模板下载超时'))
    expect(view.getByText(/生成现场已保留：/).textContent).toContain('/chosen/.partial')
    expect(view.getByText('查看创建日志')).toBeTruthy()
    expect(enter).not.toHaveBeenCalled()
    expect(request.mock.calls.some(([input])=>input.operation==='start')).toBe(false)
  })
  it('浏览器重开恢复原目录和运行编号，不跨主机恢复，重复类型标签保留名称',async()=>{
    localStorage.setItem('host-a',JSON.stringify({directory:'/chosen',commandId:'original'}))
    const request=vi.fn(async(input:Record<string,unknown>)=>input.operation==='detect'?{tools:[]}:{commandId:input.commandId,state:'running'})
    const props={request:request as never,chooseDirectory:async()=>null,onEnter:async()=>{}}
    const view=render(<ProjectLauncher {...props} storageKey="host-a"/>)
    await waitFor(()=>expect(request).toHaveBeenCalledWith({projectPath:'/chosen',operation:'status',commandId:'original'}))
    expect(view.getByRole('button',{name:'创建项目'}).hasAttribute('disabled')).toBe(true)
    view.unmount();request.mockClear()
    const other=render(<ProjectLauncher {...props} storageKey="host-b"/>)
    await waitFor(()=>expect(request).toHaveBeenCalledWith({operation:'detect'}))
    expect(request.mock.calls.some(([input])=>input.operation==='status')).toBe(false)
    fireEvent.click(other.getByRole('button',{name:/HTML 视频/}))
    fireEvent.change(other.getByLabelText('项目名称'),{target:{value:'发布会'}})
    fireEvent.click(other.getByRole('button',{name:/HTML 视频/}))
    expect((other.getByLabelText('项目名称') as HTMLInputElement).value).toBe('发布会')
  })
  it('不依赖已有对话，选择目录、确认创建后以原命令进入新项目',async()=>{
    const calls:Array<Record<string,unknown>>=[]
    const request=vi.fn(async(input:Record<string,unknown>)=>{calls.push(input);if(input.operation==='detect')return {tools:[{name:'npm',installed:true}]};if(input.operation==='preview')return {planId:'plan-a',arguments:['template','init'],name:'my-video',parent:'',kind:'hyperframes'};if(input.operation==='start')return {commandId:input.commandId,state:'completed',path:'/projects/my-video'};throw new Error('unexpected')})
    const enter=vi.fn(async()=>{}),choose=vi.fn(async()=>'/projects')
    const view=render(<ProjectLauncher request={request as never} chooseDirectory={choose} onEnter={enter}/>)
    fireEvent.click(view.getByRole('button',{name:/HTML 视频/}))
    fireEvent.click(view.getByRole('button',{name:'选择保存目录'}))
    await waitFor(()=>expect(choose).toHaveBeenCalledOnce())
    fireEvent.click(view.getByRole('button',{name:'创建项目'}))
    const dialog=await view.findByRole('dialog')
    fireEvent.click(within(dialog).getByRole('button',{name:'核对创建'}))
    await waitFor(()=>expect(within(dialog).getByRole('button',{name:'确认创建'})).toBeTruthy())
    expect(calls.find(input=>input.operation==='preview')).toMatchObject({projectPath:'/projects',options:{kind:'hyperframes',parent:''}})
    expect(calls.every(input=>!('scope' in input))).toBe(true)
    fireEvent.click(within(dialog).getByRole('button',{name:'确认创建'}))
    fireEvent.click(await view.findByRole('button',{name:'进入项目并新建对话'}))
    await waitFor(()=>expect(enter).toHaveBeenCalledWith(expect.any(String),'/projects/my-video'))
    expect(calls.filter(input=>input.operation==='start')).toHaveLength(1)
  })
  it('创建丢回执后只读核对原编号，不重复生成',async()=>{
    const calls:Array<Record<string,unknown>>=[]
    const request=vi.fn(async(input:Record<string,unknown>)=>{calls.push(input);if(input.operation==='detect')return {tools:[{name:'npm',installed:true}]};if(input.operation==='preview')return {planId:'p',arguments:['init']};if(input.operation==='start')throw {message:'连接中断'};if(input.operation==='status')return {commandId:input.commandId,state:'completed',path:'/projects/app'};throw new Error('unexpected')})
    const view=render(<ProjectLauncher initialDirectory="/projects" request={request as never} chooseDirectory={async()=>null} onEnter={async()=>{}}/>)
    fireEvent.click(view.getByRole('button',{name:'创建项目'}))
    const dialog=await view.findByRole('dialog')
    fireEvent.click(within(dialog).getByRole('button',{name:'核对创建'}))
    fireEvent.click(await within(dialog).findByRole('button',{name:'确认创建'}))
    fireEvent.click(await view.findByRole('button',{name:'核对原操作'}))
    await view.findByRole('button',{name:'进入项目并新建对话'})
    const started=calls.find(input=>input.operation==='start')!
    expect(calls.find(input=>input.operation==='status')?.commandId).toBe(started.commandId)
    expect(calls.filter(input=>input.operation==='start')).toHaveLength(1)
  })
})
