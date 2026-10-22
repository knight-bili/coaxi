import { useCallback,useMemo,type ComponentProps } from 'react'
import { SafeMarkdown } from './SafeMarkdown'
import { CardFeedbackMessageScope,type FeedbackMessageSource } from './CardFeedbackFields'
import { JsonRenderScope } from './codeg-native-content/components/ai-elements/json-render-pre'
import { MessageResponse } from './codeg-native-content/components/ai-elements/source-message-response'
import './codeg-native-content/source-html.css'

type Identity=Omit<FeedbackMessageSource,'cardOrdinal'>
function CoreSpecCard({source,ordinal,identity}:{source:string;ordinal:number;identity?:Identity}):React.JSX.Element{
  const value=useMemo(()=>identity?{...identity,cardOrdinal:ordinal}:null,[identity?.agentId,identity?.messageId,identity?.scope.threadId,identity?.scope.conversationId,identity?.scope.scopeKind,ordinal])
  const markdown=<div className="codeg-native-rich"><JsonRenderScope><MessageResponse mode="static" parseIncompleteMarkdown={false}>{'```spec\n'+source+(source.endsWith('\n')?'':'\n')+'```'}</MessageResponse></JsonRenderScope></div>
  return value?<CardFeedbackMessageScope value={value}>{markdown}</CardFeedbackMessageScope>:markdown
}
/** 普通消息仍走 Rovai 安全 Markdown，仅 spec 围栏复用原 Codeg 卡片。 */
export function CoreFeedbackMarkdown({identity,...props}:ComponentProps<typeof SafeMarkdown>&{identity?:Identity}):React.JSX.Element{
  const renderCode=useCallback((code:string,language?:string,ordinal?:number)=>language==='spec'&&ordinal!==undefined?<CoreSpecCard source={code} ordinal={ordinal} identity={identity}/>:null,[identity])
  return <SafeMarkdown {...props} renderCodeBlock={renderCode}/>
}
