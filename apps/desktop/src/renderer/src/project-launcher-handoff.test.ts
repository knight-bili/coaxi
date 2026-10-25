import {describe,expect,it,vi} from 'vitest'
import type {WorkspaceInspection} from '@contracts'
import {prepareCreatedProject} from './project-launcher-handoff'
describe('新项目交接异步门禁',()=>{
  it.each(['inspect','register'] as const)('等待 %s 时打开其他新对话，不改变其目录，也不确认交接',async(stage)=>{
    let open=false,release!:()=>void
    const waiting=new Promise<void>(resolve=>{release=resolve})
    const workspace={name:'created',projectPath:'/created'} as WorkspaceInspection
    const enter=vi.fn(),register=vi.fn(async()=>{if(stage==='register')await waiting})
    const result=prepareCreatedProject({path:'/created',assertCurrent:()=>{if(open)throw new Error('当前新对话尚未处理')},inspect:async()=>{if(stage==='inspect')await waiting;return workspace},register,enter})
    await Promise.resolve();open=true;release()
    await expect(result).rejects.toThrow('当前新对话尚未处理')
    expect(enter).not.toHaveBeenCalled()
    if(stage==='inspect')expect(register).not.toHaveBeenCalled()
  })
  it('完整核验后只进入已核实的目录',async()=>{
    const workspace={name:'created',projectPath:'/canonical'} as WorkspaceInspection,enter=vi.fn(),assertCurrent=vi.fn()
    await prepareCreatedProject({path:'/selected',assertCurrent,inspect:async()=>workspace,register:async()=>{},enter})
    expect(assertCurrent).toHaveBeenCalledTimes(3);expect(enter).toHaveBeenCalledWith(workspace)
  })
})
