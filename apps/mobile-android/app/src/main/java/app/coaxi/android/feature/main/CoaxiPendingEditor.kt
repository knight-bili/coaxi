package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.coaxi.android.core.designsystem.component.GlassRow
import app.coaxi.android.core.designsystem.theme.CoaxiTheme
import app.coaxi.android.core.network.CoaxiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*

/** Original PrivatePendingEditor actions, with native controls and the Core edit token. */
@Composable fun CoaxiPendingEditor(model:CoaxiWorkspaceViewModel,threadId:String,conversationId:String,child:Boolean,item:JsonObject,session:JsonObject,messages:List<JsonObject>){
    val state=model.state.collectAsState().value;val scope=rememberCoroutineScope();val generation=model.connectionGeneration
    val token=session.coreText("editToken");val owner="pending-edit:$threadId:$conversationId:${item.coreText("id")}:$token"
    val operation=if(child)CoaxiOperation.DELEGATIONS_PENDINGINPUTS_EDIT else CoaxiOperation.SINGLECHAT_PENDINGINPUTS_EDIT
    var body by remember(owner){mutableStateOf("")};var ready by remember(owner){mutableStateOf(false)};var issue by remember(owner){mutableStateOf<String?>(null)}
    var discard by remember{mutableStateOf(false)};var takeover by remember{mutableStateOf(false)};var choosing by remember{mutableStateOf(false)};var quoteMessage by remember{mutableStateOf<String?>(null)};var removedQuote by remember(owner){mutableStateOf<String?>(null)}
    val recovery=session["recoveryRequired"]==JsonPrimitive(true);val disabled=!ready||state.busy||state.pending!=null||recovery
    fun document()=buildJsonObject{put("version",1);put("body",body);put("editToken",token);put("pendingInputId",item.coreText("id"))}.toString()
    fun command(action:JsonObject)=buildJsonObject{if(!child)put("threadId",threadId);put("conversationId",conversationId);put("composerScope",JsonNull);put("pendingInputId",item.coreText("id"));put("expectedRevision",session.coreLong("basePendingRevision"));put("editToken",token);put("action",action)}
    fun perform(action:JsonObject,settle:Boolean=false){val frozen=command(action);val saved=document();scope.launch{try{model.saveBody(owner,saved);require(generation==model.connectionGeneration);if(settle)model.executePrivate(operation,frozen,owner,saved)else model.command(operation,frozen)}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}}
    LaunchedEffect(owner){try{val raw=model.readBody(owner);if(raw.isBlank())body=session.coreText("workingBody")else{val saved=Json.parseToJsonElement(raw).coreObject();require(saved.coreText("editToken")==token&&saved.coreText("pendingInputId")==item.coreText("id")){"原编辑草稿归属变化"};body=saved.coreText("body")};ready=true}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}
    GlassRow(Modifier.fillMaxWidth()){
        Text(stringResource(R.string.coaxi_fusion_2017d767d04ac59a),style=MaterialTheme.typography.titleSmall)
        issue?.let{Text(it,color=CoaxiTheme.colors.danger)}
        if(recovery)Text(stringResource(R.string.coaxi_fusion_fc77398bb26cdb89),color=CoaxiTheme.colors.textSecondary)
        OutlinedTextField(body,{body=it;model.rememberBody(owner,document())},Modifier.fillMaxWidth(),enabled=!disabled,maxLines=8,label={Text(stringResource(R.string.coaxi_fusion_387278213e791913))})
        val attachments=session["workingAttachments"].coreArray()
        attachments.forEachIndexed{index,attachment->Row(Modifier.fillMaxWidth()){
            Text(attachment.coreText("displayName"),Modifier.weight(1f),style=MaterialTheme.typography.bodySmall)
            TextButton(enabled=!disabled&&index>0,onClick={val ids=attachments.map{it.coreText("id")}.toMutableList();val id=ids.removeAt(index);ids.add(index-1,id);perform(buildJsonObject{put("type","reorder_attachments");put("attachmentRefIds",JsonArray(ids.map(::JsonPrimitive)))})}){Text(stringResource(R.string.coaxi_fusion_f853a70b120418be))}
            TextButton(enabled=!disabled,onClick={perform(buildJsonObject{put("type","remove_attachment");put("attachmentRefId",attachment.coreText("id"))})}){Text(stringResource(R.string.coaxi_fusion_6135d4159e892541))}
        }}
        CoaxiAttachmentPicker(model,threadId,buildJsonObject{put("kind",if(child)"delegation_child_pending" else "single_chat_pending");put("conversationId",conversationId);put("pendingInputId",item.coreText("id"));put("editToken",token);put("composerScope",JsonNull)},item.coreLong("revision"),disabled)
        CoaxiComposerQuotes(session["workingQuotes"].coreArray(),!disabled){id->removedQuote=id;perform(buildJsonObject{put("type","quote");putJsonObject("action"){put("type","remove");put("quoteId",id)}})}
        Row{TextButton(enabled=!disabled&&messages.isNotEmpty(),onClick={choosing=true}){Text(stringResource(R.string.coaxi_fusion_9466166264c04b29))};removedQuote?.let{id->TextButton(enabled=!disabled,onClick={perform(buildJsonObject{put("type","quote");putJsonObject("action"){put("type","restore");put("quoteId",id)}});removedQuote=null}){Text(stringResource(R.string.coaxi_fusion_e80b755ef87b7d65))}}}
        Row(horizontalArrangement=Arrangement.spacedBy(8.dp)){
            if(recovery)TextButton(enabled=ready&&!state.busy&&state.pending==null,onClick={takeover=true}){Text(stringResource(R.string.coaxi_fusion_f2adb22560848ca5))}
            else Button(enabled=!disabled&&body.isNotBlank()&&body.codePointCount(0,body.length)<=100000,onClick={perform(buildJsonObject{put("type","save");put("body",body)},true)}){Text(stringResource(R.string.coaxi_fusion_e86189bf94c3d737))}
            TextButton(enabled=ready&&!state.busy&&state.pending==null,onClick={discard=true}){Text(stringResource(R.string.coaxi_fusion_3085206f1c6a6d63))}
        }
    }
    if(discard||takeover)AlertDialog(onDismissRequest={discard=false;takeover=false},title={Text(if(takeover)"从已保存版本接管？" else "放弃本次编辑？")},text={Text(if(takeover)"恢复原排队消息的已保存正文、附件和引用；本次尚未保存的输入仍保留在本机草稿。" else "保留原排队消息，放弃编辑中的附件和引用修改；本机正文草稿仅在原请求确认成功后清除。")},confirmButton={TextButton(enabled=ready&&!state.busy&&state.pending==null,onClick={val action=if(takeover)"takeover" else "cancel";perform(buildJsonObject{put("type",action)},settle=action=="cancel");discard=false;takeover=false}){Text(stringResource(R.string.coaxi_fusion_36f33adaf0942634))}},dismissButton={TextButton(onClick={discard=false;takeover=false}){Text(stringResource(R.string.coaxi_fusion_fd4b9e3b6c685bae))}})
    if(choosing)AlertDialog(onDismissRequest={choosing=false},title={Text(stringResource(R.string.coaxi_fusion_1e7d4f14b90f6f3c))},text={LazyColumn(Modifier.heightIn(max=360.dp)){items(messages,key={it.coreText("id")}){message->TextButton(enabled=!disabled,onClick={quoteMessage=message.coreText("id");choosing=false}){Text(message.coreText("body").take(90),maxLines=2)}}}},confirmButton={TextButton(onClick={choosing=false}){Text(stringResource(R.string.coaxi_fusion_3fd47edce45b3603))}})
    quoteMessage?.let{id->CoaxiQuoteSelector(model,threadId,id,onChosen={false},conversationId=conversationId,onSelection={selection->if(disabled)false else{perform(buildJsonObject{put("type","quote");putJsonObject("action"){put("type","add");put("selection",selection)}});true}}){quoteMessage=null}}
}
