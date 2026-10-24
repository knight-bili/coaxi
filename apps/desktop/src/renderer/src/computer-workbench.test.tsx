// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { fireEvent, render, cleanup, waitFor, within } from '@testing-library/react'
import { ComputerWorkbench } from './ComputerWorkbench'
import { ThreadClientProvider, type ThreadClient } from './camp-client'
import { desktopThreadClient } from './desktop-camp-client'
vi.mock('./InteractionActivity',()=>({InteractionActivity:()=>null}))
vi.mock('./InteractionComputerPanel',()=>({InteractionComputerPanel:()=>null}))
vi.mock('./interface-language',()=>({useUiText:()=>((value:string)=>value)}))
beforeEach(()=>{vi.stubGlobal('IntersectionObserver',class{observe(){}disconnect(){}})})
afterEach(()=>{cleanup();vi.unstubAllGlobals()})
function fixture(){
  let enabled=true
  const rows=[{targetId:'one',app:'Browser',appKey:'browser',pid:12,title:'测试页面一',shareable:true,level:'none',wholeApp:false},
    {targetId:'two',app:'Browser',appKey:'browser',pid:12,title:'测试页面二',shareable:true,level:'none',wholeApp:false}]
  const request=vi.fn(async(_thread:string,input:Record<string,unknown>):Promise<unknown>=>{
    if(input.operation==='status')return {enabled,installed:true,driverVersion:'test',backend:{state:'ready'},shared:[]}
    if(input.operation==='windows')return {windows:structuredClone(rows)}
    if(input.operation==='enable'){enabled=!!input.enabled;return {}}
    if(input.operation==='share_app'){for(const row of rows){row.level=String(input.level);row.wholeApp=input.level!=='none'}return {}}
    if(input.operation==='share'){rows.find(row=>row.targetId===input.targetId)!.level=String(input.level);return {}}
    if(input.operation==='stop'){for(const row of rows){row.level='none';row.wholeApp=false}return {}}
    throw Error('unexpected operation '+input.operation)
  })
  const client=Object.create(desktopThreadClient) as ThreadClient
  Object.defineProperty(client,'computerWorkspace',{value:{request}})
  const view=render(<ThreadClientProvider client={client}><ComputerWorkbench threadId="thread-test"/></ThreadClientProvider>)
  return {view,request,rows}
}
it('进入已启用页面即读取窗口；列表和本机预览不会隐式共享',async()=>{
  const {view,request}=fixture()
  await view.findByTitle('测试页面一')
  expect(request.mock.calls.map(call=>call[1].operation)).toEqual(['status','windows'])
  expect(view.getByText('已共享 0 个')).toBeTruthy()
})
it('窗口与整个应用使用各自授权接口，撤销后恢复独立窗口权限',async()=>{
  const {view,request}=fixture()
  const window=await view.findByRole('group',{name:'测试页面一 窗口共享权限'})
  fireEvent.click(within(window).getByText('可读取'))
  await waitFor(()=>expect(within(window).getByText('可读取').getAttribute('aria-pressed')).toBe('true'))
  expect(request).toHaveBeenCalledWith('thread-test',{operation:'share',targetId:'one',level:'read'})
  fireEvent.click(within(view.getByRole('group',{name:'Browser 应用共享权限'})).getByText('可操作'))
  await waitFor(()=>expect((within(window).getByText('可操作') as HTMLButtonElement).disabled).toBe(true))
  expect(request).toHaveBeenCalledWith('thread-test',{operation:'share_app',targetId:'one',level:'control'})
  fireEvent.click(view.getByText('全部停止共享'))
  await waitFor(()=>expect((within(window).getByText('不共享') as HTMLButtonElement).disabled).toBe(false))
  expect(within(window).getByText('不共享').getAttribute('aria-pressed')).toBe('true')
})
it('关闭共享后清空旧列表，再开启会自动重新读取',async()=>{
  const {view}=fixture()
  await view.findByTitle('测试页面一')
  fireEvent.click(view.getByRole('checkbox'))
  await waitFor(()=>expect(view.queryByTitle('测试页面一')).toBeNull())
  fireEvent.click(view.getByRole('checkbox'))
  await view.findByTitle('测试页面一')
})
