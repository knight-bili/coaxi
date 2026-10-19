import { describe, expect, it, vi } from 'vitest'
import type { FilePreviewApi, ResolvedFilePreview } from '@contracts'
import { withNativeHtmlDocument } from './native-html-source'

const bytes=new TextEncoder().encode('<!doctype html><p>完整页面😀</p>')
const file=(handleId:string,path='/source/docs/report.html',data=bytes):ResolvedFilePreview=>({handleId,reopenToken:'never-persist',previewKey:handleId,fileName:path.split('/').at(-1)!,displayPath:path,absolutePath:path,pathPresentation:'external',sourceRelativePath:'docs/report.html',size:data.length,mime:'text/html',extension:'.html',kind:'html',hasExternalUpdate:false,contentVersion:{size:data.length,mtimeMs:1},contentGeneration:handleId,capabilities:['read','read_child']})
function fixture(){
 let active=true
 const api={open:vi.fn(async()=>({ok:true as const,value:{kind:'file_preview' as const,file:file('parent')}})),
  readFileBytes:vi.fn(async(input:{handleId:string;expectedGeneration:string})=>({ok:true as const,value:{bytes,mime:'application/octet-stream',contentVersion:{size:bytes.length,mtimeMs:1},contentGeneration:input.expectedGeneration}})),
  release:vi.fn(async(_input:{handleId:string})=>({released:true as const}))}
 return{api,owner:{api:api as unknown as FilePreviewApi,source:{threadId:'thread',readId:'reader'},current:()=>active},hide:()=>{active=false}}
}
describe('Core-scoped native HTML document reads',()=>{
 it('reads complete bytes through the exact reader, keeps the parent only during resource assembly and then releases it',async()=>{
  const {api,owner}=fixture()
  await withNativeHtmlDocument(owner,'docs/report.html',4*1024*1024,async document=>{
   expect(new TextDecoder().decode(Uint8Array.from(atob(document.base64),c=>c.charCodeAt(0)))).toBe(new TextDecoder().decode(bytes))
   expect(api.open).toHaveBeenCalledWith({kind:'native_child_file',threadId:'thread',readId:'reader',rawReference:'docs/report.html'})
   expect(api.release).not.toHaveBeenCalled()
  })
  expect(api.release).toHaveBeenCalledExactlyOnceWith({handleId:'parent'})
 })
 it('resolves local resources only through a live child grant and rejects canonical symlink escapes before reading bytes',async()=>{
  const {api,owner}=fixture();api.open.mockResolvedValueOnce({ok:true,value:{kind:'file_preview',file:file('parent')}})
   .mockResolvedValueOnce({ok:true,value:{kind:'file_preview',file:file('child','/source/docs/site.css')}})
   .mockResolvedValueOnce({ok:true,value:{kind:'file_preview',file:file('escaped','/source/other/secret.css')}})
  await withNativeHtmlDocument(owner,'docs/report.html',4*1024*1024,async document=>{
   await document.readResource('/untrusted/frontend/root','site.css')
   expect(api.open).toHaveBeenLastCalledWith({kind:'child_of_handle',parentHandleId:'parent',rawReference:'site.css',allowSystemOpen:false})
   await expect(document.readResource('/ignored','link.css')).rejects.toThrow('outside_authorized_root')
  })
  expect(api.readFileBytes.mock.calls.map(([input])=>input.handleId)).toEqual(['parent','child'])
  expect(api.release.mock.calls.map(([input])=>input.handleId)).toEqual(['child','escaped','parent'])
 })
 it('rejects a changed generation and always releases the parent grant',async()=>{
  const {api,owner}=fixture();api.readFileBytes.mockResolvedValueOnce({ok:true,value:{bytes,mime:'text/html',contentVersion:{size:bytes.length,mtimeMs:1},contentGeneration:'wrong'}})
  await expect(withNativeHtmlDocument(owner,'doc.html',4*1024*1024,vi.fn())).rejects.toThrow('read_failed')
  expect(api.release).toHaveBeenCalledWith({handleId:'parent'})
 })
 it('rejects an oversized document before reading and does not truncate it',async()=>{
  const {api,owner}=fixture();api.open.mockResolvedValueOnce({ok:true,value:{kind:'file_preview',file:{...file('parent'),size:4*1024*1024+1}}})
  await expect(withNativeHtmlDocument(owner,'doc.html',4*1024*1024,vi.fn())).rejects.toThrow('file_too_large')
  expect(api.readFileBytes).not.toHaveBeenCalled();expect(api.release).toHaveBeenCalledWith({handleId:'parent'})
 })
 it('releases a late source response after the owner changes, without reading its bytes',async()=>{
  const {api,owner,hide}=fixture();let finish!:(value:Awaited<ReturnType<typeof api.open>>)=>void
  api.open.mockReturnValueOnce(new Promise(resolve=>{finish=resolve}))
  const pending=withNativeHtmlDocument(owner,'doc.html',4*1024*1024,vi.fn());hide();finish({ok:true,value:{kind:'file_preview',file:file('late')}})
  await expect(pending).rejects.toThrow('scope_changed');expect(api.readFileBytes).not.toHaveBeenCalled();expect(api.release).toHaveBeenCalledWith({handleId:'late'})
 })
 it('keeps hidden paths private and refuses sibling resource reads without canonical directory metadata',async()=>{
  const {api,owner}=fixture();api.open.mockResolvedValueOnce({ok:true,value:{kind:'file_preview',file:{...file('parent'),pathPresentation:'file_name_only'}}})
  await withNativeHtmlDocument(owner,'doc.html',4*1024*1024,async document=>{
   expect(document.path).toBe('/__rovai_native_html__/document.html')
   await expect(document.readResource('/ignored','site.css')).rejects.toThrow('source_not_authorized')
  })
  expect(api.open).toHaveBeenCalledTimes(1);expect(api.release).toHaveBeenCalledTimes(1)
 })
})
