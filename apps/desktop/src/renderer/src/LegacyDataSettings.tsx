import { useEffect,useRef,useState } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import { FolderInput } from 'lucide-react'
import type { LegacyDataPreview } from '@contracts'
import { AppDialogBody,AppDialogContent,AppDialogFooter,AppDialogHeader } from './AppDialog'
import { useThreadClient } from './camp-client'
import { useUiText } from './interface-language'
import './interaction-workspace.css'

/** 沿用原版备份页的先预览、再明确替换与重启。 */
export function LegacyDataSettings():React.JSX.Element|null{
  const api=useThreadClient().nativeBackups,t=useUiText(),generation=useRef(0)
  const [preview,setPreview]=useState<LegacyDataPreview|null>(null),[open,setOpen]=useState(false),[busy,setBusy]=useState(false),[confirmation,setConfirmation]=useState(''),[error,setError]=useState(''),[phase,setPhase]=useState<{phase:string;done:number;total:number}|null>(null)
  useEffect(()=>{const epoch=++generation.current;const stop=api?.onProgress(value=>{if(generation.current===epoch)setPhase(value)});return()=>{generation.current++;stop?.()}},[api])
  if(!api)return null
  const inspect=async():Promise<void>=>{
    if(busy)return;const epoch=generation.current;setBusy(true);setError('');setPreview(null);setConfirmation('');setOpen(true);setPhase(null)
    try{const result=await api.inspectLegacyData();if(epoch===generation.current){setPreview(result);if(!result)setOpen(false)}}catch(error){if(epoch===generation.current)setError(String(error))}finally{if(epoch===generation.current)setBusy(false)}
  }
  const migrate=async():Promise<void>=>{
    if(busy||!preview||confirmation!=='迁入')return;const epoch=generation.current;setBusy(true);setError('');setPhase(null)
    try{await api.importLegacyData(preview.selectionId)}catch(error){if(epoch===generation.current)setError(String(error))}finally{if(epoch===generation.current)setBusy(false)}
  }
  return <section className="section-block">
    <div className="section-heading"><div><h3>{t('从旧版应用迁入数据')}</h3><p>{t('旧应用退出后，选择它的 Core 数据目录。当前工作区会先保留安全副本，旧目录仍保留。')}</p></div></div>
    <button type="button" disabled={busy} onClick={()=>void inspect()}><FolderInput size={14}/>{t('选择旧数据目录并预览')}</button>
    <p className="interaction-help">{t('原生智能体会话请使用“导入本地会话”。本入口迁入兼容 Core 工作区的会话、画布、队员、使命和目录内文件。外部工程、共享技能库与模型原生目录继续保留在原位置。')}</p>
    <Dialog.Root open={open} onOpenChange={value=>{if(!busy){setOpen(value);setConfirmation('')}}}>
      <Dialog.Portal><Dialog.Overlay className="dialog-overlay"/><AppDialogContent width="wide" tone="danger"><AppDialogHeader title={t('预览旧数据迁入')} description={t('旧数据内容和已冻结的会话材料保持原样，迁入后由现有升级流程处理兼容。请继续保留旧目录及外部工程。')} closeDisabled={busy}/>
        <AppDialogBody>{preview&&<><dl className="legacy-data-summary"><dt>{t('来源目录')}</dt><dd>{preview.source}</dd><dt>{t('原数据库')}</dt><dd>{preview.authority}</dd><dt>{t('迁入文件')}</dt><dd>{preview.fileCount} · {(preview.totalBytes/1024/1024).toFixed(1)} MB</dd></dl>
          {preview.externalRoots.length>0&&<div><p>{t('以下外部位置保留原样，不随本次目录副本替换：')}</p>{preview.externalRoots.map(root=><p key={root.id}>{root.id}: {root.path}</p>)}</div>}
          <p>{t('迁入会替换当前工作区。当前数据保留为安全副本；迁入中断时保留暂存记录，应用重新启动后继续恢复。预览后旧目录有变化会拒绝迁入，需要重新预览。')}</p>
          <label className="forge-form">{t('输入“迁入”确认替换并重启')}<input value={confirmation} disabled={busy} onChange={event=>setConfirmation(event.target.value)} autoComplete="off"/></label>
        </>}
        {error&&<p role="alert" className="workbench-error">{error}</p>}
        {phase&&busy&&<div role="status"><span>{t(phase.phase==='migration_copy'?'正在复制旧目录':'正在核对旧目录')} {phase.done}/{phase.total}</span><progress max={Math.max(1,phase.total)} value={phase.done}/></div>}
        </AppDialogBody><AppDialogFooter><button type="button" disabled={busy} onClick={()=>{setOpen(false);setConfirmation('')}}>{t('取消')}</button><button type="button" disabled={busy} onClick={()=>void inspect()}>{t('重新选择并预览')}</button><button type="button" disabled={busy||!preview||confirmation!=='迁入'} onClick={()=>void migrate()}>{t(busy?'处理中…':'迁入并重启')}</button></AppDialogFooter>
      </AppDialogContent></Dialog.Portal>
    </Dialog.Root>
  </section>
}
