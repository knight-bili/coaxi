// @vitest-environment jsdom
import {afterEach,expect,it,vi} from 'vitest'
import {cleanup,fireEvent,render,waitFor} from '@testing-library/react'
import {ThreadClientProvider,type ThreadClient} from './camp-client'
import {PortableConfigurationExport} from './PortableConfigurationExport'

afterEach(cleanup)
it('普通重绘与编辑不会重读恢复记录、禁用控件或清空口令',async()=>{
 const get=vi.fn(async()=>null)
 const client={presentationScope:'isolated-test',portableConfiguration:{},workbenchEditorRecovery:{get}} as unknown as ThreadClient
 const element=()=> <ThreadClientProvider client={client}><PortableConfigurationExport/></ThreadClientProvider>
 const view=render(element())
 const checkbox=view.getByRole('checkbox') as HTMLInputElement
 const password=view.getByPlaceholderText('至少8个字符') as HTMLInputElement
 await waitFor(()=>expect(checkbox.disabled||checkbox.closest('fieldset')!.disabled).toBe(false))
 fireEvent.click(checkbox);fireEvent.change(password,{target:{value:'test-password'}})
 view.rerender(element())
 await waitFor(()=>expect(view.getByRole('button',{name:'准备加密副本'}).hasAttribute('disabled')).toBe(false))
 expect(checkbox.checked).toBe(true);expect(password.value).toBe('test-password');expect(get).toHaveBeenCalledTimes(1)
})
