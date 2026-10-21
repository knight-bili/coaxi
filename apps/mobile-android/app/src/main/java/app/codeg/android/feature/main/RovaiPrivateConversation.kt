package app.codeg.android.feature.main

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.markdown.MarkdownContent
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*

@Composable
fun RovaiPrivateConversation(model:RovaiWorkspaceViewModel,threadId:String,childId:String?=null,onClose:()->Unit){
    val state by model.state.collectAsStateWithLifecycle();val scope=rememberCoroutineScope();val child=childId!=null
    var conversations by remember(threadId){mutableStateOf<List<JsonObject>>(emptyList())};var selected by remember(threadId,childId){mutableStateOf(childId)}
    var snapshot by remember(threadId,childId){mutableStateOf(buildJsonObject{})};var body by remember(selected){mutableStateOf("")};var bodyReady by remember(selected){mutableStateOf(false)}
    var retained by remember(selected){mutableStateOf<List<JsonObject>>(emptyList())};var cursor by remember(selected){mutableStateOf<String?>(null)}
    var issue by remember{mutableStateOf<String?>(null)};var loading by remember{mutableStateOf(false)};var ending by remember{mutableStateOf(false)}
    val get=if(child)RovaiOperation.DELEGATIONS_CONVERSATION_GET else RovaiOperation.SINGLECHAT_GET
    val send=if(child)RovaiOperation.DELEGATIONS_MESSAGE_SEND else RovaiOperation.SINGLECHAT_SEND
    val edit=if(child)RovaiOperation.DELEGATIONS_PENDINGINPUTS_EDIT else RovaiOperation.SINGLECHAT_PENDINGINPUTS_EDIT
    val mutate=if(child)RovaiOperation.DELEGATIONS_COMPOSER_MUTATE else RovaiOperation.SINGLECHAT_COMPOSERDRAFT_MUTATE
    val recovery=if(child)RovaiOperation.DELEGATIONS_COMPOSER_RECOVERY else RovaiOperation.SINGLECHAT_COMPOSERDRAFT_RECOVERY
    val owner="private:$threadId:${selected.orEmpty()}:${if(child)"delegation_child" else "single_chat"}"
    val disabled=loading||state.pending!=null||state.busy||!bodyReady
    suspend fun load(){val id=selected;loading=true;try{
        if(!child)conversations=model.query(RovaiOperation.SINGLECHAT_LIST,buildJsonObject{put("threadId",threadId)}).coreArray()
        if(id!=null){val next=model.query(get,buildJsonObject{put("conversationId",id);put("composerScope",JsonNull)}).coreObject();val conversation=next["conversation"].coreObject();val draft=next["draft"].coreObject();require(conversation.coreText("id")==id&&conversation.coreText("threadId")==threadId&&draft.coreLong("scopeVersion")==1L&&draft.coreText("scopeKey").isEmpty()&&draft["composerScope"]==JsonNull){"该私聊材料不属于当前输入框"};if(selected==id)snapshot=next};issue=null
    }catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{loading=false}}
    suspend fun retainedPage(after:String?=null){val id=selected?:return;try{val page=model.query(recovery,buildJsonObject{if(!child)put("threadId",threadId);put("conversationId",id);put("limit",50);if(child)put("composerScope",JsonNull);if(after!=null)put("cursor",after)}).coreObject();val rows=page["items"].coreArray().filter{it.coreText("kind")=="retained"}.map{it["input"].coreObject()}.filter{it.coreText("conversationId")==id&&it.coreText("scopeKey").isEmpty()};if(selected==id){retained=if(after==null)rows else (retained+rows).distinctBy{it.coreText("id")};cursor=(page["nextCursor"] as? JsonPrimitive)?.contentOrNull}}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}
    suspend fun close(){try{if(bodyReady&&selected!=null)model.saveBody(owner,body);onClose()}catch(error:Exception){issue=error.message}}
    fun base(action:JsonObject)=buildJsonObject{put("threadId",threadId);put("conversationId",selected);put("composerScope",JsonNull);put("expectedDraftRevision",snapshot["draft"].coreObject().coreLong("revision"));put("action",action)}
    fun pendingAction(item:JsonObject,action:JsonObject,replace:Boolean=false){val command=buildJsonObject{if(!child)put("threadId",threadId);put("conversationId",selected);put("composerScope",JsonNull);put("pendingInputId",item.coreText("id"));put("expectedRevision",item.coreLong("revision"));put("editToken",snapshot["pendingInputs"].coreObject()["editSession"].coreObject().takeIf{it.coreText("pendingInputId")==item.coreText("id")}?.get("editToken")?:JsonNull);put("action",action)};if(replace)model.executePrivate(edit,command,owner,body,true)else model.command(edit,command)}
    LaunchedEffect(threadId,selected){snapshot=buildJsonObject{};bodyReady=false;try{if(selected!=null){body=model.readBody(owner);bodyReady=true};load();if(selected!=null)retainedPage()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}
    LaunchedEffect(state.thread,state.lastResult){val receipt=state.lastResult.coreObject()["commandResult"].coreObject().ifEmpty{state.lastResult.coreObject()};if(state.lastCompletedOperation==RovaiOperation.SINGLECHAT_OPEN.wire&&receipt.coreText("status")=="applied"&&receipt["payload"].coreObject().coreText("conversationId").isNotEmpty())selected=receipt["payload"].coreObject().coreText("conversationId");else{load();if(selected!=null){retainedPage();if(state.lastCompletedOperation in listOf(send.wire,edit.wire,mutate.wire)&&state.pending==null){body=model.readBody(owner);bodyReady=true}}}}
    BackHandler{scope.launch{close()}}
    Column(Modifier.fillMaxSize()){
        Row(Modifier.fillMaxWidth().padding(horizontal=12.dp)){TextButton(onClick={scope.launch{close()}}){Text("返回对话")};Text(if(child)"独立帮手" else "队员私聊",Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(onClick={scope.launch{load();retainedPage()}},enabled=!loading){Text("刷新")}}
        issue?.let{Text(it,color=CodegTheme.colors.danger,modifier=Modifier.padding(horizontal=16.dp))};if(state.pending!=null)Row(Modifier.padding(horizontal=12.dp)){Text("原操作待核对",Modifier.weight(1f));TextButton(enabled=!state.busy,onClick=model::reconcile){Text("核对")};TextButton(enabled=!state.busy,onClick=model::retry){Text("重试原操作")}}
        if(selected==null){LazyColumn(contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(10.dp)){
            items(state.thread?.get("members").coreArray().filter{it.coreText("membershipStatus")=="active"&&it.coreText("profilePresence")=="present"},key={it.coreText("agentId")}){member->GlassRow(Modifier.fillMaxWidth()){Text(member.coreText("displayName"),style=MaterialTheme.typography.titleMedium);TextButton(enabled=state.pending==null&&!state.busy,onClick={model.command(RovaiOperation.SINGLECHAT_OPEN,buildJsonObject{put("threadId",threadId);put("agentId",member.coreText("agentId"))})}){Text("开始私聊")}}}
            items(conversations,key={it.coreText("id")}){conversation->GlassRow(Modifier.fillMaxWidth().clickable{selected=conversation.coreText("id")}){Text(state.members.find{it.coreText("agentId")==conversation.coreText("agentId")}?.coreText("displayName")?:conversation.coreText("agentId"));Text(conversation.coreText("updatedAt"),style=MaterialTheme.typography.labelSmall)}}
        }}else{
            val active=snapshot["conversation"].coreObject().coreText("status")=="active"
            LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
                item{Row{if(!child)TextButton(enabled=!disabled,onClick={scope.launch{model.saveBody(owner,body);selected=null}}){Text("其他私聊")};TextButton(enabled=!disabled&&active,onClick={ending=true}){Text("结束会话")}}}
                item{RovaiInteractionCards(model,state,selected!!);RovaiDelegationList(model,threadId,selected!!)}
                items(snapshot["messages"].coreArray(),key={it.coreText("id")}){message->GlassRow(Modifier.fillMaxWidth(),selected=message.coreText("authorType")=="user"){Text(if(message.coreText("authorType")=="user")"你" else message.coreText("authorType"),style=MaterialTheme.typography.labelMedium,color=CodegTheme.colors.textSecondary);message["quotes"].coreArray().forEach{quote->Text("引用",style=MaterialTheme.typography.labelSmall);MarkdownContent(quote.coreText("body"))};MarkdownContent(message.coreText("body"));message["attachments"].coreArray().forEach{attachment->Text("附件 · "+attachment.coreText("displayName"),style=MaterialTheme.typography.bodySmall)}}}
                items(snapshot["agentRuns"].coreArray(),key={"run:"+it.coreText("id")}){run->GlassRow(Modifier.fillMaxWidth()){Text(run.coreText("status"),style=MaterialTheme.typography.labelMedium);if(run.coreText("lastErrorCode").isNotEmpty())Text(run.coreText("lastErrorCode"),color=CodegTheme.colors.danger);if(run.coreText("status") in listOf("running","queued","waiting"))TextButton(enabled=!disabled,onClick={model.command(RovaiOperation.AGENTRUNS_CANCEL,buildJsonObject{put("threadId",threadId);put("agentRunId",run.coreText("id"));put("expectedVersion",run.coreLong("version"))})}){Text("停止")}}}
                items(snapshot["pendingInputs"].coreObject()["items"].coreArray(),key={"pending:"+it.coreText("id")}){pending->GlassRow(Modifier.fillMaxWidth()){Text("排队输入",style=MaterialTheme.typography.labelMedium);MarkdownContent(pending.coreText("body"));Text("${pending["attachments"].coreArray().size} 个附件 · ${pending["quotes"].coreArray().size} 条引用");Row{TextButton(enabled=!disabled,onClick={scope.launch{model.saveBody(owner,body);pendingAction(pending,buildJsonObject{put("type","return_to_composer");put("expectedDraftRevision",snapshot["draft"].coreObject().coreLong("revision"));put("targetBody",body)},true)}}){Text("移回输入框（保留当前输入）")};TextButton(enabled=!disabled,onClick={pendingAction(pending,buildJsonObject{put("type","delete")})}){Text("删除")}}}}
                items(retained,key={"retained:"+it.coreText("id")}){input->GlassRow(Modifier.fillMaxWidth()){Text("保留的完整输入",style=MaterialTheme.typography.labelMedium);MarkdownContent(input.coreText("body"));Text("${input["attachments"].coreArray().size} 个附件 · ${input["quotes"].coreArray().size} 条引用");TextButton(enabled=!disabled,onClick={scope.launch{model.saveBody(owner,body);model.executePrivate(mutate,base(buildJsonObject{put("type","swap_retained");put("retainedInputId",input.coreText("id"));put("expectedRetainedRevision",input.coreLong("revision"));put("currentBody",body)}),owner,body,true)}}){Text("恢复到输入框（保留当前输入）")}}}
                cursor?.let{after->item{TextButton(onClick={scope.launch{retainedPage(after)}}){Text("更多保留输入")}}}
                items(snapshot["draft"].coreObject()["attachments"].coreArray(),key={"draft:"+it.coreText("attachmentRefId")}){attachment->Row{Text("附件 · "+attachment.coreText("displayName"),Modifier.weight(1f));TextButton(enabled=!disabled,onClick={model.command(mutate,base(buildJsonObject{put("type","remove_attachment");put("attachmentRefId",attachment.coreText("attachmentRefId"))}))}){Text("移除")}}}
            }
            Row(Modifier.fillMaxWidth().imePadding().padding(12.dp),horizontalArrangement=Arrangement.spacedBy(8.dp)){OutlinedTextField(body,{next->body=next;scope.launch{try{model.saveBody(owner,next)}catch(error:Exception){issue=error.message}}},Modifier.weight(1f),maxLines=6,enabled=!disabled&&active,placeholder={Text("发送私聊消息…")});Button(enabled=!disabled&&active&&body.isNotBlank(),onClick={scope.launch{try{model.saveBody(owner,body);val draft=snapshot["draft"].coreObject();model.executePrivate(send,buildJsonObject{if(!child)put("threadId",threadId);put("conversationId",selected);if(child)put("expectedRevision",draft.coreLong("revision"))else put("draftRevision",draft.coreLong("revision"));put("body",body.trim());put("composerScope",JsonNull)},owner,body)}catch(error:Exception){issue=error.message}}}){Text("发送")}}
        }
    }
    if(ending)AlertDialog(onDismissRequest={ending=false},title={Text("结束当前私聊？")},text={Text("保留消息和历史记录；正在执行的工作由Core处理结束。")},confirmButton={TextButton(enabled=!disabled,onClick={model.command(if(child)RovaiOperation.DELEGATIONS_CONVERSATION_END else RovaiOperation.SINGLECHAT_END,buildJsonObject{if(!child)put("threadId",threadId);put("conversationId",selected)});ending=false}){Text("结束")}},dismissButton={TextButton(onClick={ending=false}){Text("取消")}})
}
