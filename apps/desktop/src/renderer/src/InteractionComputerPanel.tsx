import { useEffect,useRef,useState } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import { Monitor,RefreshCw,Square } from 'lucide-react'
import { AppDialogBody,AppDialogContent,AppDialogFooter,AppDialogHeader } from './AppDialog'
import { InteractionShareControl } from './InteractionShareControl'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import './interaction-workspace.css'

interface Preferences{revision:string;enabled:boolean;launch:boolean;clipboard:boolean;foreground:boolean;screen:boolean}
interface Target{id:string;agentId:string;displayName:string;createdAt:string;scope:{threadId:string;conversationId:string|null;scopeKind:'public'|'single_chat'|'delegation_child'}}
interface NativeWindow{targetId:string;appId:string|null;appName:string;title:string;shareable:boolean}
const rows:Array<{key:Exclude<keyof Preferences,'revision'>;name:string;description:string}>=[
  {key:'enabled',name:'队员电脑操作',description:'仅为本机启用电脑执行器，队员还须配备工具并得到具体会话授权。'},
  {key:'launch',name:'启动应用与调整窗口',description:'允许已配备对应工具的队员请求启动应用或调整共享窗口。'},
  {key:'clipboard',name:'剪贴板',description:'单独允许剪贴板工具；不会读取与此范围无关的原有剪贴板内容。'},
  {key:'foreground',name:'前台操作',description:'允许操作时把共享窗口带到前台。'},
  {key:'screen',name:'整屏共享选项',description:'在支持的平台提供整屏选项，选中后仍须单独共享。'},
]
/** Codeg ComputerSettings 的独立开关和 ComputerWindowPicker 的窗口/应用分组。 */
export function InteractionComputerPanel({threadId}:{threadId:string}):React.JSX.Element|null{
  const client=useThreadClient(),api=client.interactionComputer,resources=client.interactionWorkspace,t=useUiText(),epoch=useRef(0)
  const [open,setOpen]=useState(false),[preferences,setPreferences]=useState<Preferences|null>(null),[saved,setSaved]=useState<Preferences|null>(null),[targets,setTargets]=useState<Target[]>([]),[selected,setSelected]=useState(''),[windows,setWindows]=useState<NativeWindow[]>([]),[screenId,setScreenId]=useState(''),[busy,setBusy]=useState(false),[error,setError]=useState('')
  useEffect(()=>{const generation=++epoch.current;setWindows([]);setSelected('');setTargets([]);setError('');if(open&&api&&resources){void api.request<Preferences&{lastError?:string}>('preferences.get',{}).then(value=>{if(epoch.current!==generation)return;const next:Preferences={revision:value.revision,enabled:value.enabled,launch:value.launch,clipboard:value.clipboard,foreground:value.foreground,screen:value.screen};setPreferences(next);setSaved(next);if(value.lastError)setError(value.lastError)}).catch(error=>{if(epoch.current===generation)setError(String(error))});void resources.request<{targets:Target[];next:string|null}>('sharing.targets',{threadId,after:null}).then(value=>{if(epoch.current===generation)setTargets(value.targets)}).catch(error=>{if(epoch.current===generation)setError(String(error))})}return()=>{epoch.current++}},[api,resources,open,threadId])
  if(!api||!resources)return null
  const run=async(action:()=>Promise<void>):Promise<void>=>{if(busy)return;const generation=epoch.current;setBusy(true);setError('');try{await action()}catch(error){if(epoch.current===generation)setError(String(error))}finally{if(epoch.current===generation)setBusy(false)}}
  const read=async():Promise<void>=>{const target=targets.find(target=>target.id===selected);if(!target)return;const generation=epoch.current;const result=await api.request<{windows:NativeWindow[];screenId:string}>('picker',{scope:target.scope,agentId:target.agentId});if(generation===epoch.current){setWindows(result.windows);setScreenId(result.screenId)}}
  const groups=Map.groupBy(windows,window=>window.appName)
  return <Dialog.Root open={open} onOpenChange={value=>{if(!busy)setOpen(value)}}><Dialog.Trigger asChild><button type="button"><Monitor size={14}/>{t('队员电脑权限')}</button></Dialog.Trigger>
    <Dialog.Portal><Dialog.Overlay className="dialog-overlay"/><AppDialogContent tone="neutral" width="wide"><AppDialogHeader title={t('队员电脑权限')} description={t('先开启本机能力，再按会话与队员选择窗口、应用或整屏。停止后要重新共享。')} closeDisabled={busy}/>
      <AppDialogBody>{preferences&&<div className="interaction-setting-rows">{rows.map(row=><label className="interaction-setting-row" key={row.key}><span><strong>{t(row.name)}</strong><small>{t(row.description)}</small></span><input type="checkbox" role="switch" checked={preferences[row.key]} disabled={busy} onChange={event=>setPreferences({...preferences,[row.key]:event.target.checked})}/></label>)}</div>}
        <div className="interaction-actions"><button type="button" disabled={busy||!preferences||JSON.stringify(preferences)===JSON.stringify(saved)} onClick={()=>void run(async()=>{if(!preferences)return;const value=await api.request<Preferences>('preferences.set',{...preferences});setPreferences(value);setSaved(value);setWindows([])})}>{t('保存本机设置')}</button><button type="button" disabled={busy} onClick={()=>void run(async()=>{await api.request('stop',{});setWindows([])})}><Square size={13}/>{t('立即停止全部电脑操作')}</button></div>
        <div className="interaction-share-body"><label>{t('查看此会话可使用的电脑资源')}<select value={selected} disabled={busy||!saved?.enabled} onChange={event=>{setSelected(event.target.value);setWindows([])}}><option value="">{t('请选择会话与队员')}</option>{targets.map(target=><option value={target.id} key={target.id}>{target.displayName} · {t(target.scope.scopeKind==='public'?'公共会话':target.scope.scopeKind==='single_chat'?'私聊':'独立帮手')}{target.scope.scopeKind!=='public'?` · ${new Date(target.createdAt).toLocaleString()}`:''}</option>)}</select></label><button type="button" disabled={busy||!selected||!saved?.enabled} onClick={()=>void run(read)}><RefreshCw size={14}/>{t('选择本机窗口')}</button></div>
        {error&&<p className="environment-error" role="alert">{error}</p>}
        <div className="workbench-window-groups">{[...groups].map(([name,items])=><section key={name}><h3>{name}</h3>{items[0]?.appId&&<InteractionShareControl threadId={threadId} targetId={items[0].appId} targetKind="computer_app"/>}<div className="workbench-window-grid">{items.map(window=><article className="workbench-window-tile" key={window.targetId}><Monitor size={24}/><strong title={window.title}>{window.title||window.appName}</strong><InteractionShareControl threadId={threadId} targetId={window.targetId} targetKind="computer_window" disabled={!window.shareable}/></article>)}</div></section>)}</div>
        {saved?.screen&&screenId&&<div className="interaction-actions"><span>{t('整个桌面')}</span><InteractionShareControl threadId={threadId} targetId={screenId} targetKind="computer_desktop"/></div>}
      </AppDialogBody><AppDialogFooter><Dialog.Close asChild><button type="button" disabled={busy}>{t('完成')}</button></Dialog.Close></AppDialogFooter>
    </AppDialogContent></Dialog.Portal>
  </Dialog.Root>
}
