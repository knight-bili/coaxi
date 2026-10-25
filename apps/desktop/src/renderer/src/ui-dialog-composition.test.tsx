// @vitest-environment jsdom
import {afterEach,expect,it,vi} from 'vitest'
import {render,fireEvent,waitFor,cleanup} from '@testing-library/react'
import {BrowserSettingsDialog} from './BrowserSettingsDialog'
import {BackupSettings} from './BackupSettings'
import {ThreadClientProvider,type ThreadClient} from './camp-client'
import {desktopThreadClient} from './desktop-camp-client'
import type {BrowserWorkspaceApi} from '@contracts'
vi.mock('./interface-language',()=>({useUiText:()=>((value:string)=>value)}))
vi.mock('./ConfigurationSnapshotSettings',()=>({ConfigurationSnapshotSettings:()=>null}))
vi.mock('./LegacyDataSettings',()=>({LegacyDataSettings:()=>null}))
afterEach(cleanup)
it('规则编辑后取消不提交；重新打开读取已保存值，提交只发送本次有效表单',async()=>{
 const saved={hostRules:[],proxy:null,devtools:false},preferences=vi.fn(async()=>structuredClone(saved)),updatePreferences=vi.fn(async value=>value)
 const api={preferences,updatePreferences} as unknown as BrowserWorkspaceApi,changed=vi.fn()
 const view=render(<BrowserSettingsDialog api={api} onChanged={changed}/>)
 fireEvent.click(view.getByText('浏览设置'))
 const input=await view.findByLabelText('网站规则地址')
 fireEvent.change(input,{target:{value:'*.example.com'}});fireEvent.click(view.getByText('添加规则'))
 expect(view.getByText('*.example.com')).toBeTruthy()
 fireEvent.click(view.getByText('取消'));expect(updatePreferences).not.toHaveBeenCalled()
 fireEvent.click(view.getByText('浏览设置'));await view.findByLabelText('网站规则地址')
 await waitFor(()=>expect(preferences).toHaveBeenCalledTimes(2))
 expect(view.queryByText('*.example.com')).toBeNull()
 fireEvent.change(view.getByLabelText('浏览器代理地址'),{target:{value:'http://127.0.0.1:8888'}})
 fireEvent.click(view.getByText('保存'))
 await waitFor(()=>expect(updatePreferences).toHaveBeenCalledTimes(1))
 expect(updatePreferences).toHaveBeenCalledWith({hostRules:[],proxy:'http://127.0.0.1:8888',devtools:false})
 expect(changed).toHaveBeenCalledTimes(1)
})
it('统一正文与页脚后，备份口令与提交按钮仍属于同一表单，取消不会执行备份',async()=>{
 const create=vi.fn(async()=>null),client=Object.create(desktopThreadClient) as ThreadClient
 Object.defineProperty(client,'nativeBackups',{value:{create,onProgress:()=>()=>{}}})
 const view=render(<ThreadClientProvider client={client}><BackupSettings/></ThreadClientProvider>)
 fireEvent.click(view.getByText('创建加密备份'))
 const pass=view.getByLabelText('加密口令') as HTMLInputElement,repeat=view.getByLabelText(/再次输入口令/) as HTMLInputElement,submit=view.getByText('选择保存位置并备份') as HTMLButtonElement
 expect(pass.form).toBe(submit.form);expect(submit.closest('.app-dialog-footer')).not.toBeNull()
 fireEvent.change(pass,{target:{value:'fixture-password'}});fireEvent.change(repeat,{target:{value:'fixture-password'}})
 fireEvent.submit(submit.form!)
 await waitFor(()=>expect(create).toHaveBeenCalledWith('fixture-password'))
 await waitFor(()=>expect((view.getByText('取消') as HTMLButtonElement).disabled).toBe(false))
 fireEvent.click(view.getByText('取消'));expect(create).toHaveBeenCalledTimes(1)
})
