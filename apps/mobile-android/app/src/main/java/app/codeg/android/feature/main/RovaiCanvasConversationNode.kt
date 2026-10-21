package app.codeg.android.feature.main

import androidx.compose.material3.*
import androidx.compose.runtime.*
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.CancellationException
import kotlinx.serialization.json.*
import java.util.UUID

/** canvas-conversation.ts: resolve the saved public/single-chat identity, without opening a new private session. */
@Composable fun RovaiCanvasConversationNode(model:RovaiWorkspaceViewModel,boardId:String,node:JsonObject,memberReference:JsonObject?=null,onClose:()->Unit){
    val generation=remember{model.connectionGeneration};val data=if(memberReference==null)node["data"].coreObject()else buildJsonObject{put("kind","conversation");put("conversation",memberReference)};val reference=data["conversation"].coreObject();val threadId=reference.coreText("threadId");val conversationId=(reference["conversationId"]as?JsonPrimitive)?.contentOrNull
    var ready by remember(node.coreText("id")){mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)};var retry by remember{mutableIntStateOf(0)}
    LaunchedEffect(node.coreText("id"),reference,retry){try{
        require(data.coreText("kind")=="conversation"&&data["objectReference"] !is JsonObject&&threadId.isNotEmpty()){"卡片不属于普通会话引用"}
        if(memberReference!=null)requireCanvasRegionReference(model,boardId,node.coreText("id"),reference)else{
        val graph=model.query(RovaiOperation.CANVAS_NODES_SNAPSHOT,buildJsonObject{put("boardId",boardId)}).coreObject()
        val saved=graph["nodes"].coreArray().firstOrNull{it.coreText("id")==node.coreText("id")}?.get("data").coreObject()
        require(saved.coreText("kind")=="conversation"&&saved["objectReference"] !is JsonObject&&saved["conversation"]==reference){"原卡片已删除或绑定已变化；引用保留"}
        }
        if(conversationId==null){
            val snapshot=model.query(RovaiOperation.THREADS_OPEN,buildJsonObject{put("threadId",threadId);put("traceId",UUID.randomUUID().toString())}).coreObject();require(snapshot["thread"].coreObject().coreText("id")==threadId){"原公开会话已不可用"};require(generation==model.connectionGeneration);model.openThread(threadId);onClose()
        }else{
            val snapshot=model.query(RovaiOperation.SINGLECHAT_GET,buildJsonObject{put("conversationId",conversationId);put("composerScope",JsonNull)}).coreObject();val conversation=snapshot["conversation"].coreObject();require(conversation.coreText("id")==conversationId&&conversation.coreText("threadId")==threadId&&conversation.coreText("status")=="active"){"原私聊已结束、删除或归属已变化；卡片引用保留"};val anchor=model.query(RovaiOperation.THREADS_OPEN,buildJsonObject{put("threadId",threadId);put("traceId",UUID.randomUUID().toString())}).coreObject();require(anchor["thread"].coreObject().coreText("id")==threadId){"原私聊的公开会话锚点已不可用"};require(generation==model.connectionGeneration);ready=true;issue=null
        }
    }catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}
    if(ready&&conversationId!=null)RovaiPrivateConversation(model,threadId,initialConversationId=conversationId,onClose=onClose)
    else AlertDialog(onDismissRequest=onClose,title={Text("打开原会话")},text={Text(issue?:"正在核对卡片绑定和原会话…")},confirmButton={TextButton(onClick=onClose){Text("关闭")}},dismissButton={if(issue!=null)TextButton(onClick={retry++}){Text("重新读取")}})
}
