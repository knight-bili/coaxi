import { useEffect,useRef,useState,type CSSProperties,type ReactNode } from 'react'
import { Columns2,Rows2,PanelLeftClose,RotateCw,X } from 'lucide-react'
import { GroupSplitHandle } from './CodegGroupSplitHandle'
import { computeRects,leafIds,makeGroupId,neighborGroupId,removeGroup,resizeSplitAt,ROOT_GROUP_ID,singleGroupLayout,splitGroup,toggleOrientation,type LayoutNode,type SplitDirection } from '../../shared/codeg-tab-group-layout'
import { NUMBERED_TAB_ACTION_IDS } from '../../shared/codeg-keyboard-shortcuts'
import { useShortcutActions } from './useShortcutActions'
import type { ShortcutActions } from './shortcut-action-driver'
import { useUiText } from './interface-language'
import './split-tab-deck.css'

export interface SplitDeckItem {id:string;label:string;dirty?:boolean}
interface Group {tabs:string[];selected:string|null}
interface State {tree:LayoutNode;groups:Record<string,Group>;active:string}
const MIME='application/x-codeg-rovai-split-tab'
function initial():State{return {tree:singleGroupLayout(),groups:{[ROOT_GROUP_ID]:{tabs:[],selected:null}},active:ROOT_GROUP_ID}}
function project(state:State,items:SplitDeckItem[],selected:string|null):State{
  const live=new Set(items.map(item=>item.id)),ids=leafIds(state.tree),active=ids.includes(state.active)?state.active:ids[0],groups:Record<string,Group>={}
  const assigned=new Set<string>()
  for(const id of ids){const saved=state.groups[id]??{tabs:[],selected:null},tabs=saved.tabs.filter(tab=>live.has(tab)&&!assigned.has(tab)&&Boolean(assigned.add(tab)));groups[id]={tabs,selected:tabs.includes(saved.selected??'')?saved.selected:tabs[0]??null}}
  groups[active].tabs.push(...items.filter(item=>!assigned.has(item.id)).map(item=>item.id))
  const focused=selected?ids.find(id=>groups[id].tabs.includes(selected)):undefined
  if(focused)groups[focused].selected=selected
  return {tree:state.tree,groups,active:focused??active}
}

/** Codeg 原分屏树与平铺兄弟节点：移动标签只改变坐标，不重挂编辑器。 */
export function SplitTabDeck({items,selected,onSelect,onClose,renderBody,disabled=false,scopeKey}:{
  items:SplitDeckItem[];selected:string|null;onSelect(id:string):void;onClose(id:string):void;
  renderBody(item:SplitDeckItem,visible:boolean):ReactNode;disabled?:boolean;scopeKey:string
}):React.JSX.Element{
  const t=useUiText(),root=useRef<HTMLDivElement>(null),[state,setState]=useState(initial),drag=useRef<string|null>(null)
  const view=project(state,items,selected),rects=computeRects(view.tree),ids=leafIds(view.tree),active=view.groups[view.active]
  useEffect(()=>{setState(current=>project(current,items,selected))},[items,selected])
  const select=(groupId:string,id:string):void=>{setState({...view,active:groupId,groups:{...view.groups,[groupId]:{...view.groups[groupId],selected:id}}});onSelect(id)}
  const split=(groupId:string,direction:SplitDirection):void=>{
    if(disabled||ids.length>=8)return
    const group=view.groups[groupId],id=group.selected,newId=makeGroupId()
    const next={...view,tree:splitGroup(view.tree,groupId,direction,newId),active:newId,groups:{...view.groups,[groupId]:{tabs:group.tabs.filter(tab=>tab!==id),selected:group.tabs.find(tab=>tab!==id)??null},[newId]:{tabs:id?[id]:[],selected:id}}}
    setState(next);if(id)onSelect(id)
  }
  const merge=(groupId:string):void=>{
    if(disabled)return;const neighbor=neighborGroupId(view.tree,groupId);if(!neighbor)return
    const groups={...view.groups,[neighbor]:{...view.groups[neighbor],tabs:[...view.groups[neighbor].tabs,...view.groups[groupId].tabs]}};delete groups[groupId]
    setState({tree:removeGroup(view.tree,groupId),groups,active:neighbor});if(groups[neighbor].selected)onSelect(groups[neighbor].selected!)
  }
  const move=(tabId:string,target:string,before?:string):void=>{
    if(disabled||!items.some(item=>item.id===tabId)||!view.groups[target])return
    const groups=Object.fromEntries(Object.entries(view.groups).map(([id,group])=>{const tabs=group.tabs.filter(tab=>tab!==tabId);return [id,{tabs,selected:group.selected===tabId?tabs[0]??null:group.selected}]}))
    const group=groups[target],index=before?group.tabs.indexOf(before):-1;group.tabs.splice(index<0?group.tabs.length:index,0,tabId);group.selected=tabId
    setState({...view,groups,active:target});onSelect(tabId)
  }
  const drop=(event:React.DragEvent,target:string,before?:string):void=>{
    if(!event.dataTransfer.types.includes(MIME)||disabled)return;event.preventDefault();event.stopPropagation()
    const raw=event.dataTransfer.getData(MIME);if(raw.length>8192)return
    try{const input=JSON.parse(raw)as{scopeKey:string;tabId:string};if(input.scopeKey===scopeKey&&input.tabId===drag.current)move(input.tabId,target,before)}catch{/* 不接受外部文件或另一会话的拖动。 */}finally{drag.current=null}
  }
  const next=(delta:number):void=>{if(active.tabs.length){const index=active.tabs.indexOf(active.selected??'');select(view.active,active.tabs[(Math.max(0,index)+delta+active.tabs.length)%active.tabs.length])}}
  useShortcutActions({close_current_tab:()=>{if(active.selected&&!disabled)onClose(active.selected)},next_tab:()=>next(1),prev_tab:()=>next(-1),...Object.fromEntries(NUMBERED_TAB_ACTION_IDS.map((id,index)=>[id,()=>{const tab=active.tabs[index];if(tab)select(view.active,tab)}])) as ShortcutActions},{priority:75,active:()=>Boolean(root.current?.offsetWidth)&&!disabled&&(!document.querySelector('[role="dialog"][aria-modal="true"]')||[...document.querySelectorAll('[role="dialog"][data-state="open"]')].at(-1)?.contains(root.current??null)===true)})
  return <div className="split-tab-deck" ref={root}>{ids.map(id=>{const group=view.groups[id],rect=rects.groups.get(id)!,position:CSSProperties={left:`${rect.x}%`,top:`${rect.y}%`,width:`${rect.w}%`,height:32};return <div className={`split-deck-header${view.active===id?' is-active':''}`} key={id} style={position} onDragOver={event=>{if(event.dataTransfer.types.includes(MIME)){event.preventDefault();event.dataTransfer.dropEffect='move'}}} onDrop={event=>drop(event,id)}>
    <div role="tablist" aria-label={t('分屏文件标签')} className="split-deck-tabs">{group.tabs.map(tabId=>{const item=items.find(item=>item.id===tabId);if(!item)return null;return <div className={`split-deck-tab${group.selected===tabId?' is-active':''}`} key={tabId} draggable={!disabled} onDragStart={event=>{drag.current=tabId;event.dataTransfer.effectAllowed='move';event.dataTransfer.setData(MIME,JSON.stringify({scopeKey,tabId}))}} onDragEnd={()=>{drag.current=null}} onDrop={event=>drop(event,id,tabId)}><button type="button" role="tab" aria-selected={group.selected===tabId} title={item.label} onClick={()=>select(id,tabId)}>{item.label}{item.dirty?' •':''}</button><button type="button" disabled={disabled} title={t('关闭标签')} onClick={()=>onClose(tabId)}><X size={12}/></button></div>})}</div>
    <div className="split-deck-actions"><button type="button" disabled={disabled||ids.length>=8} title={t('向右分屏')} onClick={()=>split(id,'right')}><Columns2 size={14}/></button><button type="button" disabled={disabled||ids.length>=8} title={t('向下分屏')} onClick={()=>split(id,'down')}><Rows2 size={14}/></button>{ids.length>1&&<><button type="button" disabled={disabled} title={t('切换分屏方向')} onClick={()=>setState({...view,tree:toggleOrientation(view.tree,id)})}><RotateCw size={13}/></button><button type="button" disabled={disabled} title={t('合并此分屏并保留全部标签')} onClick={()=>merge(id)}><PanelLeftClose size={14}/></button></>}</div>
  </div>})}
  {items.map(item=>{const groupId=ids.find(id=>view.groups[id].tabs.includes(item.id))??view.active,rect=rects.groups.get(groupId)!,visible=view.groups[groupId].selected===item.id;return <div key={item.id} className="split-deck-body" hidden={!visible} style={{left:`${rect.x}%`,top:`calc(${rect.y}% + 32px)`,width:`${rect.w}%`,height:`calc(${rect.h}% - 32px)`}} onFocusCapture={()=>{if(view.active!==groupId||selected!==item.id)select(groupId,item.id)}}>{renderBody(item,visible)}</div>})}
  {ids.filter(id=>!view.groups[id].tabs.length).map(id=>{const rect=rects.groups.get(id)!;return <div key={`empty:${id}`} className="split-deck-empty" style={{left:`${rect.x}%`,top:`calc(${rect.y}% + 32px)`,width:`${rect.w}%`,height:`calc(${rect.h}% - 32px)`}} onDragOver={event=>{if(event.dataTransfer.types.includes(MIME))event.preventDefault()}} onDrop={event=>drop(event,id)}>{t('将标签拖到这里，或从文件树打开文件。')}</div>})}
  {rects.handles.map(handle=><GroupSplitHandle key={`${handle.splitId}:${handle.index}`} handle={handle} containerRef={root} onResize={(splitId,index,fraction)=>{if(!disabled)setState({...view,tree:resizeSplitAt(view.tree,splitId,index,fraction)})}}/>)}
  </div>
}
