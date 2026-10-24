import { describe, expect, it, vi } from 'vitest'
vi.mock('electron',()=>({app:{getPath:()=>''},globalShortcut:{}}))
import { InteractionComputer } from './interaction-computer'

function monitor(scopes:Array<[string,unknown]>,request=vi.fn().mockResolvedValue({grants:[]})){
  const computer=Object.assign(Object.create(InteractionComputer.prototype),{
    ready:Promise.resolve(),core:{request},peer:{desktopPeerId:'desktop-test'},scopes:new Map(scopes),
    preferences:{enabled:false,stopShortcut:''},child:null,lastError:null,registeredShortcut:null,activity:[]
  }) as InteractionComputer
  return {computer,request}
}
describe('computer monitor scope admission',()=>{
  it('does not query grants without a known scope',async()=>{
    const {computer,request}=monitor([])
    expect((await computer.monitor()).grants).toEqual([])
    expect(request).not.toHaveBeenCalled()
  })
  it('passes the actual conversation and member for each admitted target',async()=>{
    const scope={threadId:'rvcamp_test',scopeKind:'public',conversationId:null}
    const {computer,request}=monitor([['scope-key',{scope,agentId:'agent_1'}]])
    await computer.monitor()
    expect(request).toHaveBeenCalledExactlyOnceWith('interaction.local',{action:'grant.list',payload:{scope,agentId:'agent_1',desktopPeerId:'desktop-test'}})
  })
  it('drops a revoked scope while preserving unrelated status and unexpected errors',async()=>{
    const request=vi.fn().mockRejectedValueOnce(new Error('scope_changed')).mockResolvedValue({grants:[]})
    const {computer}=monitor([['expired',{scope:{threadId:'old'},agentId:'agent_1'}],['active',{scope:{threadId:'new'},agentId:'agent_2'}]],request)
    await expect(computer.monitor()).resolves.toMatchObject({grants:[]})
    request.mockRejectedValue(new Error('storage unavailable'))
    await expect(computer.monitor()).rejects.toThrow('storage unavailable')
  })
})
