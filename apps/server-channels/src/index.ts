import {ChannelCorePipe} from './core-pipe'
import {ChannelSettingsService} from '../../desktop/src/main/channel-settings'
import {ChannelSettingsCoordinator} from '../../desktop/src/main/channel-settings-coordinator'
import {DingTalkChannelSettingsService,DINGTALK_REQUIRED_SCOPE_VALUES} from '../../desktop/src/main/dingtalk-channel-settings'
import {SqliteChannelCredentialStore,SqliteChannelDeveloperSessionStore} from '../../desktop/src/main/channel-credential-store'
import {ElectronFeishuDeveloperSessionService} from '../../desktop/src/main/feishu-developer-session'
import {ElectronDingTalkDeveloperSessionService} from '../../desktop/src/main/dingtalk-developer-session'
import {FeishuWebSessionMemberBotProvisioner} from '../../desktop/src/main/feishu-member-bot-provisioner'
import {DingTalkOpenPlatformMemberBotProvisioner} from '../../desktop/src/main/dingtalk-member-bot-provisioner'
import {DingTalkDeveloperGateway} from '../../desktop/src/main/dingtalk-developer-gateway'
import {FEISHU_PROVIDER_PROFILE,LARK_PROVIDER_PROFILE} from '../../desktop/src/main/channel-provider-profile'
import {ControlledMemberBotAvatarSourceResolver} from '../../desktop/src/main/member-bot-avatar-source'
import {createHostChannelHandler,parseHostChannelRequest,projectHostChannels} from '../../desktop/src/main/host-channels'
import type {ChannelKind,ChannelSettingsSnapshot} from '@contracts'

// stdout belongs exclusively to the parent pipe. No administrator token or
// credential is accepted through command-line arguments or environment.
for(const key of ['log','info','warn','error','debug'] as const)console[key]=(...args:unknown[])=>process.stderr.write(`[channels] ${args.map(value=>typeof value==='string'?value:JSON.stringify(value)).join(' ')}\n`)
let service:ChannelSettingsCoordinator
const pipe=new ChannelCorePipe(async frame=>{
  if(frame.operation==='start'){await service.start();return {ready:true}}
  if(frame.operation==='stop'){await service.stop();return {stopped:true}}
  const request=frame.request as Record<string,unknown>|undefined
  if(!request)throw new Error('channel_request_invalid')
  const parsed=parseHostChannelRequest(request)
  if(parsed?.operation==='get')return {result:present(await service.get()),error:null}
  if(parsed)return createHostChannelHandler(service)(parsed)
  const kind=request.kind as ChannelKind
  if(!['feishu','lark','dingtalk'].includes(kind)&&!['cancelLogin','refreshLogin'].includes(String(request.operation)))throw new Error('channel_request_invalid')
  if(request.operation==='connect'){
    void service.connect(kind).catch(error=>console.warn(error instanceof Error?error.message:'channel_connect_failed'))
    return {result:present(await service.get()),error:null}
  }
  const value=request.operation==='disconnect'?await service.disconnect(kind)
    :request.operation==='cancelLogin'&&typeof request.attemptId==='string'?await service.cancelQrAttempt(request.attemptId)
    :request.operation==='refreshLogin'&&typeof request.attemptId==='string'?(await service.refreshLoginQr(request.attemptId),await service.get())
    :null
  if(!value)throw new Error('channel_request_invalid')
  return {result:present(value),error:null}
},async()=>{await service?.stop();service?.dispose()})
const core={request:<T,>(method:string,params:unknown={}):Promise<T>=>pipe.request<T>(method,params)}
const credentials=new SqliteChannelCredentialStore(core)
const sessions=new SqliteChannelDeveloperSessionStore(core)
const avatars=new ControlledMemberBotAvatarSourceResolver({read:async(avatarRef,rendition)=>{
  const value=await core.request<{base64:string;width:number;height:number;mediaType:'image/png'}|null>('memberAvatars.read',{avatarRef,rendition})
  return value?{...value,bytes:Buffer.from(value.base64,'base64')}:null
}})
function openPlatform(profile:typeof FEISHU_PROVIDER_PROFILE):ChannelSettingsService{
  const session=new ElectronFeishuDeveloperSessionService(sessions,()=>null,{loginProfile:profile.login})
  return new ChannelSettingsService({core,profile,credentialStore:credentials.forProvider(profile.kind),developerSession:session,memberBotProvisioner:new FeishuWebSessionMemberBotProvisioner(session,{sdkDomain:profile.sdkDomain}),memberBotAvatarSource:avatars})
}
const dingtalkSession=new ElectronDingTalkDeveloperSessionService({store:sessions})
service=new ChannelSettingsCoordinator({feishu:openPlatform(FEISHU_PROVIDER_PROFILE),lark:openPlatform(LARK_PROVIDER_PROFILE),dingtalk:new DingTalkChannelSettingsService({core,credentialStore:credentials,developerSession:dingtalkSession,provisioner:new DingTalkOpenPlatformMemberBotProvisioner({developerApi:new DingTalkDeveloperGateway({session:dingtalkSession}),developerSession:dingtalkSession}),avatarSource:avatars,requiredScopeValues:DINGTALK_REQUIRED_SCOPE_VALUES})})
pipe.onEvent(event=>service.handleCoreEvent(event))
service.onChanged(snapshot=>pipe.write({event:{method:'channels.changed',params:present(snapshot)}}))
function present(snapshot:ChannelSettingsSnapshot):ChannelSettingsSnapshot{
  const value=projectHostChannels(snapshot)
  // A QR is a one-attempt login presentation. Cookie/session snapshots remain
  // exclusively in the existing Core developer-session store.
  value.activeQrAttempt=snapshot.activeQrAttempt?structuredClone(snapshot.activeQrAttempt):null
  return value
}
pipe.write({event:{method:'channels.transport.ready',params:{protocol:1,providers:['feishu','lark','dingtalk']}}})
