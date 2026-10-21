package app.codeg.android.feature.main

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.markdown.MarkdownContent
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.util.UUID

private val notificationLabels=mapOf("approval_pending" to "待审批","user_mention" to "提到你","turn_completed" to "本轮完成","turn_failed" to "执行失败","turn_incomplete" to "执行未完成","round_completed" to "协作完成","single_chat_reply" to "单聊回复","mission_needs_you" to "使命需要你","mission_status_changed" to "使命状态变更","task_status_changed" to "任务状态变更")
private fun actionLabel(kind:String)=when(kind){"open_approval"->"查看审批";"open_single_chat"->"打开私聊";"open_mission"->"查看使命";"open_task"->"查看任务";"acknowledge_only"->"知道了";else->"查看来源"}
@Composable
fun RovaiNotificationInbox(model:RovaiWorkspaceViewModel,onClose:()->Unit){
    val state by model.state.collectAsStateWithLifecycle();val scope=rememberCoroutineScope()
    var page by remember{mutableStateOf(buildJsonObject{})};var cursor by remember{mutableStateOf<String?>(null)};var unread by remember{mutableStateOf(false)};var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)};var source by remember{mutableStateOf<JsonObject?>(null)}
    suspend fun load(after:String?=null){if(busy)return;busy=true;try{val value=model.query(RovaiOperation.NOTIFICATIONS_INBOX,buildJsonObject{put("filter",if(unread)"unread" else "all");put("limit",50);if(after!=null)put("cursor",after)}).coreObject();require(value.coreLong("schemaVersion")==9L){"提醒收件箱格式不兼容"};page=value;cursor=after;issue=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    LaunchedEffect(unread){load()};LaunchedEffect(state.lastResult){if(state.lastCompletedOperation==RovaiOperation.NOTIFICATIONS_ACKNOWLEDGE.wire)load(cursor)}
    Column(Modifier.fillMaxSize()){
        Row(Modifier.fillMaxWidth().padding(12.dp)){TextButton(onClick=onClose){Text("关闭")};Text("提醒 · ${page.coreLong("unreadCount")} 未读",Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(enabled=!busy,onClick={scope.launch{load()}}){Text("最新")}}
        Row(Modifier.padding(horizontal=16.dp)){FilterChip(selected=unread,onClick={unread=!unread},enabled=!busy,label={Text("只看未读")})}
        issue?.let{Text(it,color=CodegTheme.colors.danger,modifier=Modifier.padding(16.dp))}
        LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(10.dp)){
            items(page["items"].coreArray(),key={it.coreText("id")}){episode->GlassRow(Modifier.fillMaxWidth(),selected=episode["unread"]==JsonPrimitive(true)){
                Text(notificationLabels[episode.coreText("primarySemantic")]?:"提醒",style=MaterialTheme.typography.titleMedium);Text(episode["thread"].coreObject().coreText("title"),color=CodegTheme.colors.textSecondary);val mention=episode["mention"].coreObject();if(mention.coreText("summary").isNotEmpty())Text(mention.coreText("summary"));Text(episode.coreText("updatedAt"),style=MaterialTheme.typography.labelSmall)
                (listOf(episode["primaryAction"].coreObject())+episode["secondaryActions"].coreArray()).distinctBy{it.coreText("actionId")}.forEach{action->if(action["available"]==JsonPrimitive(true))Row(horizontalArrangement=Arrangement.spacedBy(8.dp)){
                    if(action.coreText("kind")!="acknowledge_only")TextButton(enabled=!busy,onClick={source=action}){Text(actionLabel(action.coreText("kind")))}
                    if((action["acknowledgementId"] as? JsonPrimitive)?.contentOrNull!=null)TextButton(enabled=state.pending==null&&!state.busy,onClick={model.command(RovaiOperation.NOTIFICATIONS_ACKNOWLEDGE,buildJsonObject{put("episodeId",episode.coreText("id"));put("observedEpisodeVersion",action.coreLong("observedEpisodeVersion"));put("acknowledgementId",action.coreText("acknowledgementId"))})}){Text(if(action.coreText("kind")=="acknowledge_only")"知道了" else "标记此条提醒已读")}
                }}
            }}
            (page["nextCursor"] as? JsonPrimitive)?.contentOrNull?.let{after->item{TextButton(enabled=!busy,onClick={scope.launch{load(after)}}){Text("更早的提醒")}}}
        }
    }
    source?.let{action->Dialog(onDismissRequest={source=null},properties=DialogProperties(usePlatformDefaultWidth=false,dismissOnBackPress=false,dismissOnClickOutside=false)){Surface(Modifier.fillMaxSize(),color=CodegTheme.colors.bgElevated){val single=action["singleChat"].coreObject();if(action.coreText("kind")=="open_single_chat"&&single.coreText("conversationId").isNotEmpty())RovaiPrivateConversation(model,action.coreText("threadId"),initialConversationId=single.coreText("conversationId")){source=null}else RovaiNotificationSource(model,action){source=null}}}}
}

/** Read a separate source projection; opening a reminder never changes the active composer. */
@Composable private fun RovaiNotificationSource(model:RovaiWorkspaceViewModel,action:JsonObject,onClose:()->Unit){
    val state by model.state.collectAsStateWithLifecycle();val lifecycle=LocalLifecycleOwner.current.lifecycle
    var snapshot by remember(action.coreText("actionId")){mutableStateOf(buildJsonObject{})};var message by remember{mutableStateOf<JsonObject?>(null)};var issue by remember{mutableStateOf<String?>(null)}
    val threadId=action.coreText("threadId")
    suspend fun load(){try{val value=model.query(RovaiOperation.THREADS_OPEN,buildJsonObject{put("threadId",threadId);put("traceId",UUID.randomUUID().toString())}).coreObject();require(value["thread"].coreObject().coreText("id")==threadId){"提醒来源已不可用"};snapshot=value
        if(action.coreText("messageId").isNotEmpty()){val around=model.query(RovaiOperation.THREAD_MESSAGES_AROUND,buildJsonObject{put("threadId",threadId);put("messageId",action.coreText("messageId"))}).coreObject();message=around["messages"].coreArray().find{it.coreText("id")==action.coreText("messageId")};if(message==null)issue="来源消息已不可用"else issue=null}else issue=null
    }catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}
    LaunchedEffect(action.coreText("actionId")){while(isActive){if(lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED))load();delay(2000)}}
    LaunchedEffect(state.lastResult){load()}
    Column(Modifier.fillMaxSize()){
        Row(Modifier.padding(12.dp)){TextButton(onClick=onClose){Text("返回提醒")};Text(snapshot["thread"].coreObject().coreText("title"),style=MaterialTheme.typography.titleMedium)}
        if(state.pending!=null)Row(Modifier.padding(12.dp)){Text("原操作待核对",Modifier.weight(1f));TextButton(onClick=model::reconcile,enabled=!state.busy){Text("核对")};TextButton(onClick=model::retry,enabled=!state.busy){Text("重试原操作")}}
        LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
            issue?.let{item{Text(it,color=CodegTheme.colors.danger)}}
            message?.let{value->item{GlassRow(Modifier.fillMaxWidth(),selected=true){Text("来源消息",style=MaterialTheme.typography.titleSmall);if(value["withdrawn"]==JsonPrimitive(true))Text("消息已撤回")else MarkdownContent(value.coreText("body"))}}}
            val subject=action["subject"].coreObject();if(subject.isNotEmpty())item{GlassRow(Modifier.fillMaxWidth()){Text(subject.coreText("title"),style=MaterialTheme.typography.titleMedium);Text(subject.coreText("status"),color=CodegTheme.colors.textSecondary)}}
            if(snapshot.isNotEmpty())item{RovaiThreadControls(model,state.copy(selectedThread=threadId,thread=snapshot))}
            items(snapshot["agentRuns"].coreArray().filter{action.coreText("agentRunId").isEmpty()||it.coreText("id")==action.coreText("agentRunId")},key={it.coreText("id")}){run->RovaiExecutionHistory(model,threadId,run,snapshot["executionEvidence"].coreArray())}
        }
    }
}
