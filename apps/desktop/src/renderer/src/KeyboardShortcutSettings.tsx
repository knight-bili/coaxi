import { useEffect,useState } from 'react'
import { Keyboard,RotateCcw } from 'lucide-react'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import { useKeyboardShortcuts } from './useKeyboardShortcuts'
import { DEFAULT_SHORTCUTS,INPUT_SHORTCUT_IDS,SHORTCUT_DEFINITIONS,canShareShortcut,formatShortcutLabel,setShortcutRecorderArmed,shortcutFromKeyboardEvent,shortcutsConflict,type ShortcutActionId } from '../../shared/codeg-keyboard-shortcuts'

const labels:Record<ShortcutActionId,string>={toggle_search:'搜索会话',toggle_sidebar:'显示或隐藏侧栏',toggle_terminal:'显示或隐藏终端',new_terminal_tab:'新建终端标签',close_current_terminal_tab:'关闭当前终端',toggle_aux_panel:'显示或隐藏辅助面板',new_conversation:'新建会话',open_folder:'打开文件夹',open_settings:'打开设置',close_current_tab:'关闭当前文件标签',reopen_last_closed_tab:'重新打开关闭的标签',close_all_file_tabs:'关闭全部文件标签',next_tab:'下一个标签',prev_tab:'上一个标签',switch_tab_1:'切换到标签1',switch_tab_2:'切换到标签2',switch_tab_3:'切换到标签3',switch_tab_4:'切换到标签4',switch_tab_5:'切换到标签5',switch_tab_6:'切换到标签6',switch_tab_7:'切换到标签7',switch_tab_8:'切换到标签8',switch_tab_9:'切换到标签9',send_message:'发送消息',newline_in_message:'消息换行',toggle_custom_style:'启用或停用自定义样式',zoom_in:'放大界面',zoom_out:'缩小界面',zoom_reset:'恢复默认缩放'}
/** 原 Codeg shortcut-settings 的录制、冲突拒绝和逐项重置；容器沿用 Rovai。 */
export function KeyboardShortcutSettings():React.JSX.Element{
  const state=useKeyboardShortcuts(),client=useThreadClient(),t=useUiText()
  const [recording,setRecording]=useState<ShortcutActionId|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState('')
  const isMac=Boolean(client.keyboardShortcuts)&&client.platform==='darwin'
  const save=async(action:ShortcutActionId|null,shortcut:string):Promise<void>=>{if(busy)return;setBusy(true);setError('');try{await state.save(action?{...state.bindings,[action]:shortcut}:{...DEFAULT_SHORTCUTS});setRecording(null)}catch(error){setError(String(error))}finally{setBusy(false)}}
  useEffect(()=>{setShortcutRecorderArmed(Boolean(recording));void client.keyboardShortcuts?.recording?.(Boolean(recording)).catch(error=>setError(String(error)));return()=>{setShortcutRecorderArmed(false);void client.keyboardShortcuts?.recording?.(false).catch(()=>undefined)}},[recording,client.keyboardShortcuts])
  useEffect(()=>{if(!recording||busy)return;const record=(event:KeyboardEvent):void=>{
    event.preventDefault();event.stopPropagation();event.stopImmediatePropagation();if(event.repeat||event.isComposing)return
    if(event.key==='Escape'){setRecording(null);return}
    const value=shortcutFromKeyboardEvent(event,INPUT_SHORTCUT_IDS.has(recording));if(!value)return
    const conflict=SHORTCUT_DEFINITIONS.find(item=>item.id!==recording&&!canShareShortcut(item.id,recording)&&shortcutsConflict(state.bindings[item.id],value))
    if(conflict){setError(`${t('与以下操作冲突：')}${t(labels[conflict.id])}`);return}
    void save(recording,value)
  };window.addEventListener('keydown',record,true);return()=>window.removeEventListener('keydown',record,true)},[recording,busy,state.bindings])
  return <section className="section-block general-settings-section settings-data-section"><div className="section-heading"><div><h2><Keyboard size={17}/>{t('快捷键')}</h2><p>{t('点击一个按键开始录制；按 Escape 取消。终端和消息输入分别处理各自的快捷键。')}</p></div><button className="quiet-button" type="button" disabled={busy||!state.loaded} onClick={()=>void save(null,'')}><RotateCcw size={14}/>{t('恢复默认')}</button></div>
    {(error||state.error)&&<p role="alert" className="environment-error">{error||state.error}<button className="quiet-button" type="button" disabled={busy} onClick={()=>void state.reload()}>{t('重新读取')}</button></p>}
    <div className="keyboard-shortcut-rows">{SHORTCUT_DEFINITIONS.map(({id})=><div className="setting-row" key={id}><div className="setting-copy"><label>{t(labels[id])}</label></div><div className="setting-action"><button className="quiet-button" type="button" disabled={busy||!state.loaded} aria-label={`${t(labels[id])} ${t('录制快捷键')}`} aria-pressed={recording===id} onClick={()=>{setError('');setRecording(recording===id?null:id)}}><kbd>{recording===id?t('请按下组合键…'):state.bindings[id]?formatShortcutLabel(state.bindings[id],isMac):t('未设置')}</kbd></button><button className="quiet-button" type="button" disabled={busy||!state.loaded||!state.bindings[id]} onClick={()=>void save(id,'')}>{t('清除')}</button>{state.bindings[id]!==DEFAULT_SHORTCUTS[id]&&<button className="quiet-button" type="button" disabled={busy||!state.loaded} onClick={()=>void save(id,DEFAULT_SHORTCUTS[id])}>{t('重置')}</button>}</div></div>)}</div>
  </section>
}
