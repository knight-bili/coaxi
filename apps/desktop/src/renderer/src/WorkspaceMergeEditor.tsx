import { useMemo, useState } from 'react'
import { Panel, PanelGroup, PanelResizeHandle } from 'react-resizable-panels'
import { computeMergeHunks, buildResult } from './codeg-workbench/merge-diff'
import { parseConflictMarkers } from './codeg-workbench/conflict-parser'
import { WorkspaceCodeEditor } from './WorkspaceCodeEditor'
import { useUiText } from './interface-language'

/** Codeg base/ours/result/theirs hunk model with Rovai's shared editor host. */
export function WorkspaceMergeEditor({ path, base, ours, theirs, merged, onApply, onClose }: {
  path:string; base:string; ours:string; theirs:string; merged:string; onApply(value:string):void; onClose():void
}):React.JSX.Element {
  const t=useUiText(), hunks=useMemo(()=>computeMergeHunks(base,ours,theirs),[base,ours,theirs])
  const [applied,setApplied]=useState(new Map<string,'left'|'right'>()), [value,setValue]=useState(merged), [manual,setManual]=useState(false)
  const select=(id:string, side:'left'|'right'):void=>{
    if(manual && !window.confirm(t('按选块重新生成结果会替换当前手动编辑，是否继续？')))return
    const next=new Map(applied).set(id,side);setApplied(next);setValue(buildResult(base,hunks,next));setManual(false)
  }
  const unresolved=parseConflictMarkers(value).length>0 || !manual && hunks.some(hunk=>hunk.type==='conflict'&&!applied.has(hunk.id))
  return <section className="workbench-merge">
    <div className="workbench-toolbar"><strong>{t('解决冲突：{0}',path)}</strong><button type="button" disabled={unresolved} onClick={()=>onApply(value)}>{t('使用当前结果')}</button><button type="button" onClick={onClose}>{t('返回编辑')}</button></div>
    <div className="workbench-merge-hunks">{hunks.map(hunk=><div key={hunk.id}><span>{t('第 {0} 行',String(hunk.baseStart+1))} · {hunk.type==='conflict'?t('冲突'):t('变更')}</span><button type="button" aria-pressed={applied.get(hunk.id)==='left'} onClick={()=>select(hunk.id,'left')}>{t('使用本地')}</button><button type="button" aria-pressed={applied.get(hunk.id)==='right'} onClick={()=>select(hunk.id,'right')}>{t('使用传入')}</button></div>)}</div>
    <PanelGroup direction="horizontal" className="workbench-panels">
      <Panel minSize={15}><div className="workbench-merge-column"><strong>{t('本地版本')}</strong><WorkspaceCodeEditor path={path} text={ours} readonly /></div></Panel><PanelResizeHandle className="workbench-resize" />
      <Panel minSize={25}><div className="workbench-merge-column"><strong>{t('合并结果')}</strong><WorkspaceCodeEditor path={path} text={value} onChange={text=>{setValue(text);setManual(true)}} /></div></Panel><PanelResizeHandle className="workbench-resize" />
      <Panel minSize={15}><div className="workbench-merge-column"><strong>{t('传入版本')}</strong><WorkspaceCodeEditor path={path} text={theirs} readonly /></div></Panel>
    </PanelGroup>
  </section>
}
