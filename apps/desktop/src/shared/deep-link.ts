import type {DesktopDeepLink,SettingsSection} from '@contracts'
import {isThreadId} from '@contracts'

const settings:readonly SettingsSection[]=['remote','general','skills','toolbox','mcp','runtime','channels','appearance','notifications','monitoring','diagnostics','about']
const tabs=['threads','missions','members','canvas','automations','memory','settings']
/** Navigation only. URLs never carry a host address, credentials, commands or input. */
export function parseDesktopDeepLink(raw:string):Omit<DesktopDeepLink,'requestId'>|null{
  if(raw.length>2048||/[\u0000-\u0020\u007f]/.test(raw))return null
  try{
    const url=new URL(raw)
    if(!['coaxi:','rovai:'].includes(url.protocol)||url.username||url.password||url.port||url.search||url.hash)return null
    const kind=url.hostname.toLowerCase(),parts=url.pathname.split('/')
    if(parts.length!==2||!parts[1])return null
    const value=decodeURIComponent(parts[1])
    if(value.length>128||/[\u0000-\u001f\u007f/\\]/.test(value))return null
    if(kind==='settings'&&settings.includes(value as SettingsSection))return {kind,value}
    if(kind==='tab'&&tabs.includes(value))return {kind,value}
    const uuid=/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/.test(value)
    if(kind==='mission'&&(uuid||/^rvm_[a-f0-9]{32}$/.test(value)))return {kind,value}
    if(kind==='thread'&&isThreadId(value))return {kind,value}
    if(kind==='canvas'&&uuid)return {kind,value}
    return null
  }catch{return null}
}
