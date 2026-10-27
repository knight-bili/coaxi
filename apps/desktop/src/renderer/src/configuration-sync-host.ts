import {useMemo} from 'react'
import type {PortableSyncRequest,CoaxiApi} from '@contracts'
import {useThreadClient} from './camp-client'

type Workspace=NonNullable<CoaxiApi['webdavWorkspace']>
type Input=Parameters<Workspace['request']>[0]

/** 同一设置页使用原 Main 所有者或认证后的私有输入；不向网络开放主机路径。 */
export function useConfigurationSyncHost():Workspace|undefined{
  const client=useThreadClient()
  return useMemo(()=>{
    if(client.webdavWorkspace)return client.webdavWorkspace
    const api=client.portableConfiguration;if(!api?.sync)return undefined
    return {request:async<T,>(input:Input):Promise<T|null>=>{
      const operation=input.operation.startsWith('config_')?input.operation.slice(7):''
      if(!['get','set','upload','cancel','resume','archive','cache_preview','cache_remove','cache_status'].includes(operation))throw new Error('此入口只处理主机配置同步。')
      return api.sync<T>({...input,operation:operation as PortableSyncRequest['operation']})
    }}
  },[client,client.presentationScope])
}
