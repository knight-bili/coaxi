import { useCallback,useMemo,type ComponentProps } from 'react'
import { SafeMarkdown } from './SafeMarkdown'
import { CardFeedbackMessageScope,type FeedbackMessageSource } from './CardFeedbackFields'
import { JsonRenderScope } from './coaxi-native-content/components/ai-elements/json-render-pre'
import { MessageResponse } from './coaxi-native-content/components/ai-elements/source-message-response'
import './coaxi-native-content/source-html.css'

type Identity=Omit<FeedbackMessageSource,'cardOrdinal'>
function CoreSpecCard({source,ordinal,identity}:{source:string;ordinal:number;identity?:Identity}):React.JSX.Element{
  const value=useMemo(()=>identity?{...identity,cardOrdinal:ordinal}:null,[identity?.agentId,identity?.messageId,identity?.scope.threadId,identity?.scope.conversationId,identity?.scope.scopeKind,ordinal])
  const markdown=<div className="coaxi-native-rich"><JsonRenderScope><MessageResponse mode="static" parseIncompleteMarkdown={false}>{'```spec\n'+source+(source.endsWith('\n')?'':'\n')+'```'}</MessageResponse></JsonRenderScope></div>
  return value?<CardFeedbackMessageScope value={value}>{markdown}</CardFeedbackMessageScope>:markdown
}
/** 普通消息仍走 Coaxi 安全 Markdown，仅 spec 围栏复用原 Coaxi 卡片。 */
export function CoreFeedbackMarkdown({identity,...props}:ComponentProps<typeof SafeMarkdown>&{identity?:Identity}):React.JSX.Element{
  const source=useMemo(()=>identity,[identity?.agentId,identity?.messageId,identity?.scope.threadId,identity?.scope.conversationId,identity?.scope.scopeKind])
  const renderCode=useCallback((code:string,language?:string,ordinal?:number)=>language==='spec'&&ordinal!==undefined?<CoreSpecCard source={code} ordinal={ordinal} identity={source}/>:null,[source])
  return <SafeMarkdown {...props} renderCodeBlock={renderCode}/>
}
