import {useEffect,useRef,useState} from 'react'
import type {PortableExportReceipt,PortableExportSourceArchive} from '@contracts'
import {useThreadClient} from './camp-client'
import {useUiText} from './interface-language'
import {newCommandId} from '../../shared/command-id'
import {loadPortablePending,savePortablePending,type PortableConfigurationPending} from './portable-configuration-recovery'
import {readErrorMessage} from './error-message'

/** 原配置同步页的导出区域；秘密和口令只留在本次输入内存。 */
export function PortableConfigurationExport({sourceArchive}:{sourceArchive?:PortableExportSourceArchive}={}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),generation=useRef(0)
  const key=client.presentationScope?`portable-export:${client.presentationScope}${sourceArchive?`:original:${sourceArchive.archiveId}`:''}`:null
  const [pending,setPending]=useState<PortableConfigurationPending|null>(null),[receipt,setReceipt]=useState<PortableExportReceipt|null>(null)
  const [includeSecrets,setIncludeSecrets]=useState(false),[passphrase,setPassphrase]=useState(''),[busy,setBusy]=useState(false),[ready,setReady]=useState(false),[error,setError]=useState(''),[message,setMessage]=useState('')
  useEffect(()=>{
    const observed=++generation.current;setReady(false);setPending(null);setReceipt(null);setPassphrase('');setIncludeSecrets(false);setError('');setBusy(false);setMessage('')
    if(!key||!client.portableConfiguration)return
    void loadPortablePending(client,key).then(async value=>{
      if(observed!==generation.current)return
      if(value&&value.operation!=='configuration.export.prepare')throw new Error(t('原导出记录的类型不同，已保留。'))
      if(value&&value.targetId!==sourceArchive?.archiveId)throw new Error(t('原导出的来源不同，已保留。'))
      setPending(value)
      if(value){const result=await client.portableConfiguration!.exportReceipt(value.commandId);if(observed!==generation.current)return;setReceipt(result);setIncludeSecrets(result.includeSecrets===true)}
      setReady(true)
    }).catch(reason=>{if(observed===generation.current)setError(readErrorMessage(reason))})
    return()=>{generation.current++}
  },[client,key,t])
  const prepare=async():Promise<void>=>{
    if(!key||!client.portableConfiguration||busy||!ready)return
    const observed=generation.current;setBusy(true);setError('');setMessage('')
    try{
      const commandId=pending?.commandId??newCommandId()
      if(sourceArchive&&!includeSecrets)throw new Error(t('请先确认原文件可能包含的密钥等内容一并保留在加密副本中。'))
      if(!pending){const value:PortableConfigurationPending={operation:'configuration.export.prepare',commandId,...(sourceArchive?{targetId:sourceArchive.archiveId}:{})};await savePortablePending(client,key,value);if(observed!==generation.current)return;setPending(value)}
      const result=await client.portableConfiguration.prepareExport({operationId:commandId,includeSecrets,passphrase,...(sourceArchive?{sourceArchive}:{})})
      if(observed!==generation.current)return
      if(result.operationId!==commandId)throw new Error(t('导出回执编号不同，原请求已保留。'))
      setReceipt(result);setIncludeSecrets(result.includeSecrets===true);if(result.state==='ready')setPassphrase('')
    }catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current)setBusy(false)}
  }
  const reconcile=async():Promise<void>=>{
    if(!pending||!client.portableConfiguration||busy)return
    const observed=generation.current;setBusy(true);setError('')
    try{const result=await client.portableConfiguration.exportReceipt(pending.commandId);if(observed!==generation.current)return;if(result.operationId!==pending.commandId)throw new Error(t('导出回执编号不同。'));setReceipt(result);if(result.includeSecrets!==undefined)setIncludeSecrets(result.includeSecrets);if(result.state==='ready')setPassphrase('')}
    catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current)setBusy(false)}
  }
  const save=async():Promise<void>=>{
    if(!receipt||receipt.state!=='ready'||!client.portableConfiguration||busy)return
    const observed=generation.current;setBusy(true);setError('')
    try{
      if(client.portableConfiguration.saveExport){const saved=await client.portableConfiguration.saveExport(receipt.operationId);if(observed===generation.current&&saved.saved)setMessage(t('加密配置副本已保存。'))}
      else if(client.portableConfiguration.downloadExport){
        const blob=await client.portableConfiguration.downloadExport(receipt);if(observed!==generation.current)return
        const url=URL.createObjectURL(blob);const anchor=document.createElement('a');anchor.href=url;anchor.download='configuration.coaxi-config';anchor.click();window.setTimeout(()=>URL.revokeObjectURL(url),60000);setMessage(t('加密配置副本已交给浏览器下载。'))
      }else throw new Error(t('当前客户端没有配置文件保存入口。'))
    }catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current)setBusy(false)}
  }
  const another=async():Promise<void>=>{
    if(!key||receipt?.state!=='ready'||busy)return
    const observed=generation.current;setBusy(true);setError('')
    try{await savePortablePending(client,key,null);if(observed!==generation.current)return;setPending(null);setReceipt(null);setIncludeSecrets(false);setPassphrase('');setMessage('')}
    catch(reason){if(observed===generation.current)setError(readErrorMessage(reason))}finally{if(observed===generation.current)setBusy(false)}
  }
  return <section className="portable-configuration-export">
    <div className="section-heading"><h3>{t(sourceArchive?'保留原配置的完整副本':'导出完整配置')}</h3><p>{t(sourceArchive?'保存这份候选的原内容，包括尚不支持的字段。保存副本不会启用其中的配置。':'保存一份加密副本，到另一台主机再选择要搬入的配置和本机队员。')}</p></div>
    {receipt?.state!=='ready'&&<fieldset className="portable-provider-fields" disabled={busy||!ready}>
      <label><span>{t(sourceArchive?'确认原内容和可能包含的密钥一起加密保存':'同时包含密钥')}</span><input className="workspace-check" type="checkbox" checked={includeSecrets} disabled={!!pending&&receipt?.state!=='unknown'} onChange={event=>setIncludeSecrets(event.target.checked)}/></label>
      <label><span>{t('副本加密口令')}</span><input className="workspace-field" type="password" autoComplete="new-password" value={passphrase} onChange={event=>setPassphrase(event.target.value)} placeholder={t('至少8个字符')}/></label>
    </fieldset>}
    <p className="muted">{t(sourceArchive?'原副本可能含原应用保存的敏感设置；需要你明确勾选后才导出，始终使用加密文件。':'默认不含密钥；包含密钥时也只放进加密副本。原生登录、服务令牌和本机安装锁不随配置迁移。')}</p>
    <div className="portable-provider-actions">
      {receipt?.state!=='ready'&&<button className="quiet-button" type="button" disabled={busy||!ready||Array.from(passphrase).length<8||Boolean(sourceArchive&&!includeSecrets)} onClick={()=>void prepare()}>{t(pending?'继续准备原副本':'准备加密副本')}</button>}
      {pending&&<button className="quiet-button" type="button" disabled={busy} onClick={()=>void reconcile()}>{t('核对原导出')}</button>}
      {receipt?.state==='ready'&&<><button className="quiet-button" type="button" disabled={busy} onClick={()=>void save()}>{t('保存加密副本')}</button><button className="quiet-button" type="button" disabled={busy} onClick={()=>void another()}>{t('另建一份副本')}</button></>}
    </div>
    {receipt&&<p role="status">{t(receipt.state==='ready'?'副本已准备好。':receipt.state==='prepared'?'原副本仍待完成，请使用原口令继续。':'尚未查到原导出结果，未自动重发。')}</p>}
    {message&&<p role="status">{message}</p>}{error&&<p className="inline-error" role="alert">{error}</p>}
  </section>
}
