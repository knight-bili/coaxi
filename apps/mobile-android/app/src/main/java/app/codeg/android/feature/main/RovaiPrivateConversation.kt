package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

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
fun RovaiPrivateConversation(model:RovaiWorkspaceViewModel,threadId:String,childId:String?=null,initialConversationId:String?=null,onClose:()->Unit){
    val state by model.state.collectAsStateWithLifecycle();val scope=rememberCoroutineScope();val child=childId!=null
    var conversations by remember(threadId){mutableStateOf<List<JsonObject>>(emptyList())};var selected by remember(threadId,childId,initialConversationId){mutableStateOf(childId?:initialConversationId)}
    var snapshot by remember(threadId,childId){mutableStateOf(buildJsonObject{})};var body by remember(selected){mutableStateOf("")};var bodyReady by remember(selected){mutableStateOf(false)}
    var retained by remember(selected){mutableStateOf<List<JsonObject>>(emptyList())};var cursor by remember(selected){mutableStateOf<String?>(null)}
    var quoteSelection by remember(selected){mutableStateOf<Pair<String,Long>?>(null)};var removedQuoteId by remember(selected){mutableStateOf<String?>(null)}
    var issue by remember{mutableStateOf<String?>(null)};var loading by remember{mutableStateOf(false)};var ending by remember{mutableStateOf(false)};var openingCommand by remember{mutableStateOf<String?>(null)}
    val get=if(child)RovaiOperation.DELEGATIONS_CONVERSATION_GET else RovaiOperation.SINGLECHAT_GET
    val send=if(child)RovaiOperation.DELEGATIONS_MESSAGE_SEND else RovaiOperation.SINGLECHAT_SEND
    val edit=if(child)RovaiOperation.DELEGATIONS_PENDINGINPUTS_EDIT else RovaiOperation.SINGLECHAT_PENDINGINPUTS_EDIT
    val mutate=if(child)RovaiOperation.DELEGATIONS_COMPOSER_MUTATE else RovaiOperation.SINGLECHAT_COMPOSERDRAFT_MUTATE
    val recovery=if(child)RovaiOperation.DELEGATIONS_COMPOSER_RECOVERY else RovaiOperation.SINGLECHAT_COMPOSERDRAFT_RECOVERY
    val owner="private:$threadId:${selected.orEmpty()}:${if(child)"delegation_child" else "single_chat"}"
    val disabled=loading||state.pending!=null||state.busy||!bodyReady
    val active=snapshot["conversation"].coreObject().coreText("status")=="active"
    suspend fun load(){val id=selected;loading=true;try{
        if(!child)conversations=model.query(RovaiOperation.SINGLECHAT_LIST,buildJsonObject{put("threadId",threadId)}).coreArray()
        if(id!=null){val next=model.query(get,buildJsonObject{put("conversationId",id);put("composerScope",JsonNull)}).coreObject();val conversation=next["conversation"].coreObject();val draft=next["draft"].coreObject();require(conversation.coreText("id")==id&&conversation.coreText("threadId")==threadId&&draft.coreLong("scopeVersion")==1L&&draft.coreText("scopeKey").isEmpty()&&draft["composerScope"]==JsonNull){"该私聊材料不属于当前输入框"};if(selected==id)snapshot=next};issue=null
    }catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{loading=false}}
    suspend fun retainedPage(after:String?=null){val id=selected?:return;try{val page=model.query(recovery,buildJsonObject{if(!child)put("threadId",threadId);put("conversationId",id);put("limit",50);if(child)put("composerScope",JsonNull);if(after!=null)put("cursor",after)}).coreObject();val rows=page["items"].coreArray().filter{it.coreText("kind")=="retained"}.map{it["input"].coreObject()}.filter{it.coreText("conversationId")==id&&it.coreText("scopeKey").isEmpty()};if(selected==id){retained=if(after==null)rows else (retained+rows).distinctBy{it.coreText("id")};cursor=(page["nextCursor"] as? JsonPrimitive)?.contentOrNull}}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}
    suspend fun close(){try{if(bodyReady&&selected!=null)model.saveBody(owner,body);onClose()}catch(error:Exception){issue=error.message}}
    fun base(action:JsonObject)=buildJsonObject{put("threadId",threadId);put("conversationId",selected);put("composerScope",JsonNull);put("expectedDraftRevision",snapshot["draft"].coreObject().coreLong("revision"));put("action",action)}
    fun pendingAction(item:JsonObject,action:JsonObject,replace:Boolean=false){val command=buildJsonObject{if(!child)put("threadId",threadId);put("conversationId",selected);put("composerScope",JsonNull);put("pendingInputId",item.coreText("id"));put("expectedRevision",item.coreLong("revision"));put("editToken",snapshot["pendingInputs"].coreObject()["editSession"].coreObject().takeIf{it.coreText("pendingInputId")==item.coreText("id")}?.get("editToken")?:JsonNull);put("action",action)};if(replace)model.executePrivate(edit,command,owner,body,true)else model.command(edit,command)}
    LaunchedEffect(threadId,selected){snapshot=buildJsonObject{};bodyReady=false;try{if(selected!=null){body=model.readBody(owner);bodyReady=true};load();if(selected!=null)retainedPage()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}
    LaunchedEffect(state.thread){load()}
    LaunchedEffect(state.lastResult){
        val receipt=state.lastResult.coreObject()["commandResult"].coreObject().ifEmpty{state.lastResult.coreObject()}
        if(!child&&state.lastCompletedOperation==RovaiOperation.SINGLECHAT_OPEN.wire&&receipt.coreText("commandId")==openingCommand&&receipt.coreText("status")=="applied"){
            val id=receipt["payload"].coreObject().coreText("conversationId")
            if(id.isNotEmpty()){val opened=model.query(get,buildJsonObject{put("conversationId",id);put("composerScope",JsonNull)}).coreObject();if(opened["conversation"].coreObject().coreText("threadId")==threadId)selected=id}
        }else{load();if(selected!=null){retainedPage();if(state.lastCompletedOperation in listOf(send.wire,edit.wire,mutate.wire)&&state.pending==null){body=model.readBody(owner);bodyReady=true}}}
    }

    BackHandler{scope.launch{close()}}
    var feedback by remember(selected){mutableStateOf(false)}
    if(feedback&&selected!=null)RovaiUserFeedback(model,threadId,selected,child,snapshot["conversation"].coreObject().coreText("agentId"),normalInput={
        require(!disabled&&body.isEmpty()){ "请先处理普通输入中的草稿和材料" }
        val current=model.query(get,buildJsonObject{put("conversationId",selected);put("composerScope",JsonNull)}).coreObject();val draft=current["draft"].coreObject()
        require(current["conversation"].coreObject().coreText("id")==selected&&draft["attachments"].coreArray().isEmpty()&&draft["quotes"].coreArray().isEmpty()){ "请先处理普通输入中的草稿和材料" }
        buildJsonObject{put("draftRevision",draft.coreLong("revision"))}
    },nativeMaterials=snapshot["draft"].coreObject()["attachments"].coreArray()){feedback=false;scope.launch{load()}}
    Column(Modifier.fillMaxSize()){
        Row(Modifier.fillMaxWidth().padding(horizontal=12.dp)){TextButton(onClick={scope.launch{close()}}){Text(stringResource(R.string.rovai_fusion_39755d00a27cc1d3))};Text(if(child)"独立帮手" else "队员私聊",Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);if(selected!=null)TextButton(onClick={feedback=true}){Text(stringResource(R.string.rovai_fusion_a1a2795f53d03bde))};TextButton(onClick={scope.launch{load();retainedPage()}},enabled=!loading){Text(stringResource(R.string.rovai_fusion_aee88743413144a2))}}
        issue?.let{Text(it,color=CodegTheme.colors.danger,modifier=Modifier.padding(horizontal=16.dp))};if(state.pending!=null)Row(Modifier.padding(horizontal=12.dp)){Text(stringResource(R.string.rovai_fusion_5a692cd92a0632c9),Modifier.weight(1f));TextButton(enabled=!state.busy,onClick=model::reconcile){Text(stringResource(R.string.rovai_fusion_2e81b3dff5809023))};TextButton(enabled=!state.busy,onClick=model::retry){Text(stringResource(R.string.rovai_fusion_04a02c897ac919f0))}}
        if(selected==null){LazyColumn(contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(10.dp)){
            items(state.thread?.get("members").coreArray().filter{it.coreText("membershipStatus")=="active"&&it.coreText("profilePresence")=="present"},key={it.coreText("agentId")}){member->GlassRow(Modifier.fillMaxWidth()){Text(member.coreText("displayName"),style=MaterialTheme.typography.titleMedium);TextButton(enabled=state.pending==null&&!state.busy,onClick={run{val id=java.util.UUID.randomUUID().toString();openingCommand=id;model.execute(RovaiOperation.SINGLECHAT_OPEN,buildJsonObject{put("commandId",id);putJsonObject("command"){put("threadId",threadId);put("agentId",member.coreText("agentId"))}})}}){Text(stringResource(R.string.rovai_fusion_cd1d5d18372ce6af))}}}
            items(conversations,key={it.coreText("id")}){conversation->GlassRow(Modifier.fillMaxWidth().clickable{selected=conversation.coreText("id")}){Text(state.members.find{it.coreText("agentId")==conversation.coreText("agentId")}?.coreText("displayName")?:conversation.coreText("agentId"));Text(conversation.coreText("updatedAt"),style=MaterialTheme.typography.labelSmall)}}
        }}else{
            LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
                item{Row{if(!child)TextButton(enabled=!disabled,onClick={scope.launch{model.saveBody(owner,body);selected=null}}){Text(stringResource(R.string.rovai_fusion_d8bb3c61b0b729bf))};TextButton(enabled=!disabled&&active,onClick={ending=true}){Text(stringResource(R.string.rovai_fusion_3eaf8d1b8148fff8))}}}
                item{RovaiAttachmentPicker(model,threadId,buildJsonObject{put("kind",if(child)"delegation_child" else "single_chat");put("conversationId",selected);put("composerScope",JsonNull)},snapshot["draft"].coreObject().coreLong("revision"),disabled||!active);RovaiInteractionCards(model,state,selected!!);RovaiDelegationList(model,threadId,selected!!)}
                item{if(selected!=null)key(selected,model.connectionGeneration){RovaiConversationSources(model,threadId,selected,child,enabled=!disabled&&active){reference->if(disabled||!active)false else{if(!body.contains(reference)){body=if(body.isEmpty())reference else body+"\n"+reference;model.rememberBody(owner,body)};true}}}}
                item{if(selected!=null)key(selected,model.connectionGeneration){RovaiConversationQuestions(model,threadId,selected,child)}}
                items(snapshot["messages"].coreArray(),key={it.coreText("id")}){message->GlassRow(Modifier.fillMaxWidth(),selected=message.coreText("authorType")=="user"){Text(if(message.coreText("authorType")=="user")"你" else message.coreText("authorType"),style=MaterialTheme.typography.labelMedium,color=CodegTheme.colors.textSecondary);RovaiMessageBody(model,threadId,message,selected,child);TextButton(enabled=!disabled&&active,onClick={quoteSelection=message.coreText("id") to snapshot["draft"].coreObject().coreLong("revision")}){Text(stringResource(R.string.rovai_fusion_cda0a153bacd4b29))}}}
                items(snapshot["agentRuns"].coreArray(),key={"run:"+it.coreText("id")}){run->GlassRow(Modifier.fillMaxWidth()){RovaiExecutionHistory(model,threadId,run,snapshot["executionEvidence"].coreArray());Text(run.coreText("status"),style=MaterialTheme.typography.labelMedium);if(run.coreText("lastErrorCode").isNotEmpty())Text(run.coreText("lastErrorCode"),color=CodegTheme.colors.danger);if(run.coreText("status") in listOf("running","queued","waiting"))TextButton(enabled=!disabled,onClick={model.command(RovaiOperation.AGENTRUNS_CANCEL,buildJsonObject{put("threadId",threadId);put("agentRunId",run.coreText("id"));put("expectedVersion",run.coreLong("version"))})}){Text(stringResource(R.string.rovai_fusion_ca4d973c0b006b75))}}}
                items(snapshot["pendingInputs"].coreObject()["items"].coreArray(),key={"pending:"+it.coreText("id")}){pending->GlassRow(Modifier.fillMaxWidth()){Text(stringResource(R.string.rovai_fusion_47d7dafd43ad2512),style=MaterialTheme.typography.labelMedium);MarkdownContent(pending.coreText("body"));Text("${pending["attachments"].coreArray().size} 个附件 · ${pending["quotes"].coreArray().size} 条引用");Row{TextButton(enabled=!disabled,onClick={pendingAction(pending,buildJsonObject{put("type","begin")})}){Text(stringResource(R.string.rovai_fusion_051836569928a9f9))};TextButton(enabled=!disabled,onClick={scope.launch{model.saveBody(owner,body);pendingAction(pending,buildJsonObject{put("type","return_to_composer");put("expectedDraftRevision",snapshot["draft"].coreObject().coreLong("revision"));put("targetBody",body)},true)}}){Text(stringResource(R.string.rovai_fusion_7c9c35906f7eb6bd))};TextButton(enabled=!disabled,onClick={pendingAction(pending,buildJsonObject{put("type","delete")})}){Text(stringResource(R.string.rovai_fusion_2f9daa828907b93f))}}}}
                snapshot["pendingInputs"].coreObject()["editSession"].coreObject().takeIf{it.coreText("editToken").isNotEmpty()}?.let{session->snapshot["pendingInputs"].coreObject()["items"].coreArray().find{it.coreText("id")==session.coreText("pendingInputId")}?.let{item->item{RovaiPendingEditor(model,threadId,selected!!,child,item,session,snapshot["messages"].coreArray())}}}
                items(retained,key={"retained:"+it.coreText("id")}){input->GlassRow(Modifier.fillMaxWidth()){Text(stringResource(R.string.rovai_fusion_bffd6ac06552158e),style=MaterialTheme.typography.labelMedium);MarkdownContent(input.coreText("body"));Text("${input["attachments"].coreArray().size} 个附件 · ${input["quotes"].coreArray().size} 条引用");TextButton(enabled=!disabled,onClick={scope.launch{model.saveBody(owner,body);model.executePrivate(mutate,base(buildJsonObject{put("type","swap_retained");put("retainedInputId",input.coreText("id"));put("expectedRetainedRevision",input.coreLong("revision"));put("currentBody",body)}),owner,body,true)}}){Text(stringResource(R.string.rovai_fusion_a940ffcad28cbd64))}}}
                cursor?.let{after->item{TextButton(onClick={scope.launch{retainedPage(after)}}){Text(stringResource(R.string.rovai_fusion_1a03c539078069eb))}}}
                item{RovaiComposerQuotes(snapshot["draft"].coreObject()["quotes"].coreArray(),!disabled&&active){quoteId->removedQuoteId=quoteId;model.command(mutate,base(buildJsonObject{put("type","quote");putJsonObject("action"){put("type","remove");put("quoteId",quoteId)}}))};removedQuoteId?.let{quoteId->TextButton(enabled=!disabled&&active,onClick={model.command(mutate,base(buildJsonObject{put("type","quote");putJsonObject("action"){put("type","restore");put("quoteId",quoteId)}}));removedQuoteId=null}){Text(stringResource(R.string.rovai_fusion_e80b755ef87b7d65))}}}
                items(snapshot["draft"].coreObject()["attachments"].coreArray(),key={"draft:"+it.coreText("id")}){attachment->Row{Text(stringResource(R.string.rovai_fusion_f90da1272c11a23f)+attachment.coreText("displayName"),Modifier.weight(1f));TextButton(enabled=!disabled,onClick={model.command(mutate,base(buildJsonObject{put("type","remove_attachment");put("attachmentRefId",attachment.coreText("id"))}))}){Text(stringResource(R.string.rovai_fusion_6135d4159e892541))}}}
            }
            Row(Modifier.fillMaxWidth().imePadding().padding(12.dp),horizontalArrangement=Arrangement.spacedBy(8.dp)){OutlinedTextField(body,{next->body=next;scope.launch{try{model.saveBody(owner,next)}catch(error:Exception){issue=error.message}}},Modifier.weight(1f),maxLines=6,enabled=!disabled&&active,placeholder={Text(stringResource(R.string.rovai_fusion_91585f61daddc419))});Button(enabled=!disabled&&active&&body.isNotBlank(),onClick={scope.launch{try{model.saveBody(owner,body);val draft=snapshot["draft"].coreObject();model.executePrivate(send,buildJsonObject{if(!child)put("threadId",threadId);put("conversationId",selected);if(child)put("expectedRevision",draft.coreLong("revision"))else put("draftRevision",draft.coreLong("revision"));put("body",body.trim());put("composerScope",JsonNull)},owner,body)}catch(error:Exception){issue=error.message}}}){Text(stringResource(R.string.rovai_fusion_edecf0ae6e5144f9))}}
        }
    }
    quoteSelection?.let{(messageId,revision)->RovaiQuoteSelector(model,threadId,messageId,onChosen={false},conversationId=selected,onSelection={selection->if(disabled||!active)false else{val command=JsonObject(base(buildJsonObject{put("type","quote");putJsonObject("action"){put("type","add");put("selection",selection)}}).toMutableMap().apply{put("expectedDraftRevision",JsonPrimitive(revision))});scope.launch{try{model.saveBody(owner,body);model.command(mutate,command)}catch(error:Exception){issue=error.message}};true}}){quoteSelection=null}}
    if(ending)AlertDialog(onDismissRequest={ending=false},title={Text(stringResource(R.string.rovai_fusion_a08d443d494ff2cf))},text={Text(stringResource(R.string.rovai_fusion_f83b56edb9ac104b))},confirmButton={TextButton(enabled=!disabled,onClick={model.command(if(child)RovaiOperation.DELEGATIONS_CONVERSATION_END else RovaiOperation.SINGLECHAT_END,buildJsonObject{if(!child)put("threadId",threadId);put("conversationId",selected)});ending=false}){Text(stringResource(R.string.rovai_fusion_c7b24e7997e96eaa))}},dismissButton={TextButton(onClick={ending=false}){Text(stringResource(R.string.rovai_fusion_2cd0f3be8738a86c))}})
}
