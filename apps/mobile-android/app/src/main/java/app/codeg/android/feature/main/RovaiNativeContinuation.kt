package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.security.MessageDigest
import java.util.UUID

private fun nativeForkPoint(provider:String,transcript:JsonObject,selected:String):JsonObject{
    val turns=transcript["turns"].coreArray();val index=turns.indexOfFirst{it.coreText("id")==selected&&it.coreText("role")=="assistant"};require(index>=0){"请重新选择原助手回答"}
    fun text(turn:JsonObject)=turn["blocks"].coreArray().filter{it.coreText("type")=="text"}.joinToString(""){it.coreText("text")}
    fun fingerprint(text:String)="sha256:"+MessageDigest.getInstance("SHA-256").digest(text.toByteArray(Charsets.UTF_8)).joinToString(""){"%02x".format(it)}
    val turn=turns[index];val content=text(turn);val digest=content.takeIf{it.isNotBlank()}?.let(::fingerprint);val native=turn.coreText("agent_message_id").takeIf{it.isNotBlank()}
    require(digest!=null||provider!="codex"&&native!=null){"这条回答暂时无法准确定位，请另选回答或明确从末尾分叉"}
    val id=(if(provider=="codex")selected else native?:selected).replace(Regex(":segment:\\d+$"),"")
    return buildJsonObject{put("selectedTurnId",selected);put("messageId",id);put("messageFingerprint",digest?.let(::JsonPrimitive)?:JsonNull);put("messageOccurrence",if(digest==null)JsonNull else JsonPrimitive(turns.take(index).count{it.coreText("role")=="assistant"&&fingerprint(text(it))==digest}+1))}
}
internal fun nativeCreationState(receipt:JsonObject)=when(receipt.coreText("state")){"prepared"->"已登记，正在准备原生会话";"starting"->"原程序正在恢复或分叉";"binding_recorded"->"原生会话已记录，新会话尚未发送消息";"failed"->"未完成创建，原导入快照保留";"unknown"->"创建结果待核对，不重复创建";else->receipt.coreText("state")}

@Composable fun RovaiNativeContinuation(model:RovaiWorkspaceViewModel,entry:JsonObject,transcript:JsonObject,mode:String,selectedTurnId:String?=null,onClose:()->Unit){
    val coroutine=rememberCoroutineScope();val generation=remember(entry){model.connectionGeneration};val owner="native-continuation:${entry.coreText("linkedThreadId")}:${entry.coreText("id")}"
    val state=model.state.collectAsState().value;var members by remember(entry){mutableStateOf<List<JsonObject>>(emptyList())};var member by remember(entry){mutableStateOf("")};var title by remember(entry){mutableStateOf(entry["summary"].coreObject().coreText("title").ifBlank{entry.coreText("sourceName")})}
    var packet by remember(entry){mutableStateOf<JsonObject?>(null)};var receipt by remember(entry){mutableStateOf<JsonObject?>(null)};var ready by remember(entry){mutableStateOf(false)};var busy by remember(entry){mutableStateOf(false)};var issue by remember(entry){mutableStateOf<String?>(null)}
    val disabled=busy||!ready||state.busy||state.pending!=null||generation!=model.connectionGeneration
    var working by remember(entry){mutableStateOf(false)}
    suspend fun keep(value:JsonObject){require(generation==model.connectionGeneration){"工作区已切换，原请求保留"};model.saveBody(owner,value.toString());packet=value}
    suspend fun reconcile(original:JsonObject){val value=nativeUserCall(model,RovaiOperation.HISTORY_EXECUTION_RECEIPT,buildJsonObject{put("schemaVersion",1);put("commandId",original.coreText("commandId"))});require(value.coreText("commandId")==original.coreText("commandId")){"原回执归属不一致"};receipt=value}
    fun work(action:suspend()->Unit){if(busy)return;busy=true;issue=null;coroutine.launch{try{action()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}
    LaunchedEffect(entry,generation){busy=true;try{val raw=model.readBody(owner);if(raw.isNotEmpty()){packet=Json.parseToJsonElement(raw).coreObject();val original=packet?.get("original").coreObject();if(original.isNotEmpty())reconcile(original)};members=nativeUserCallList(model);if(members.size==1)member=members.first().coreText("agentId");ready=true}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    if(working&&receipt?.coreText("state")=="binding_recorded"){
        Dialog(onDismissRequest={working=false},properties=DialogProperties(usePlatformDefaultWidth=false,dismissOnClickOutside=false)){Surface(Modifier.fillMaxSize()){RovaiPrivateConversation(model,receipt!!.coreText("threadId"),initialConversationId=receipt!!.coreText("conversationId")){working=false}}};return
    }
    Dialog(onDismissRequest={if(!busy)onClose()},properties=DialogProperties(usePlatformDefaultWidth=false)){Surface(Modifier.fillMaxWidth().fillMaxHeight(.94f),shape=MaterialTheme.shapes.large){Column(Modifier.padding(16.dp)){
        Row{Text(if(mode=="continue")"从原生会话继续"else if(mode=="fork_point")"从这条回答分叉"else "从会话末尾分叉",Modifier.weight(1f),style=MaterialTheme.typography.titleLarge);TextButton(enabled=!busy,onClick=onClose){Text(stringResource(R.string.rovai_fusion_1b22eb19b0591f21))}}
        issue?.let{Text(it,color=CodegTheme.colors.danger)}
        LazyColumn(Modifier.weight(1f),verticalArrangement=Arrangement.spacedBy(10.dp)){
            val preview=packet?.get("preview").coreObject();val original=packet?.get("original").coreObject()
            if(preview.isEmpty()){
                items(members,key={it.coreText("agentId")}){candidate->Row{RadioButton(selected=member==candidate.coreText("agentId"),onClick={member=candidate.coreText("agentId")},enabled=!disabled);Text(candidate.coreText("displayName"))}}
                item{OutlinedTextField(title,{title=it},Modifier.fillMaxWidth(),enabled=!disabled,label={Text(stringResource(R.string.rovai_fusion_1c962aa994bb160b))});Text(stringResource(R.string.rovai_fusion_1c27a71e332c685c),color=CodegTheme.colors.textSecondary)}
                item{Button(enabled=!disabled&&member.isNotEmpty()&&title.codePointCount(0,title.length)<=80,onClick={work{val selected=members.first{it.coreText("agentId")==member};val next=nativeUserCall(model,RovaiOperation.HISTORY_EXECUTION_PREVIEW,buildJsonObject{put("schemaVersion",1);put("sourceThreadId",entry.coreText("linkedThreadId"));put("importId",entry.coreText("id"));put("expectedImportRevision",entry.coreText("revision"));put("agentId",member);put("expectedAgentVersion",selected.coreLong("version"));put("mode",mode);put("point",if(mode=="fork_point")nativeForkPoint(entry.coreText("provider"),transcript,selectedTurnId.orEmpty())else JsonNull);put("title",title)});keep(buildJsonObject{put("preview",next)})}}){Text(stringResource(R.string.rovai_fusion_f1f74a8beb921bcf))}}
            }else{
                item{GlassRow(Modifier.fillMaxWidth()){Text(preview.coreText("title"),style=MaterialTheme.typography.titleMedium);Text(stringResource(R.string.rovai_fusion_6181ed6f97342ce2)+preview["source"].coreObject().coreText("provider"));Text(stringResource(R.string.rovai_fusion_31fea8ed8898ddf3)+preview.coreLong("historyMessageCount"));Text(stringResource(R.string.rovai_fusion_1ec111e62128cc0e)+(members.find{it.coreText("agentId")==preview.coreText("agentId")}?.coreText("displayName")?:preview.coreText("agentId")));if(preview["point"]!=JsonNull)Text(stringResource(R.string.rovai_fusion_be70dbebf7b76977)+preview["point"].coreObject().coreText("selectedTurnId"));preview.coreText("reason").takeIf{it.isNotEmpty()}?.let{Text(it)};Text(stringResource(R.string.rovai_fusion_3100ae6b56cb3e35),color=CodegTheme.colors.textSecondary)}}
                receipt?.let{value->item{Text(nativeCreationState(value));Text(value.coreText("commandId"),style=MaterialTheme.typography.bodySmall);value.coreText("reportedSessionId").takeIf{it.isNotEmpty()}?.let{Text(stringResource(R.string.rovai_fusion_37c177cb42b7a570)+it,style=MaterialTheme.typography.bodySmall)};value.coreText("error").takeIf{it.isNotEmpty()}?.let{Text(it,color=CodegTheme.colors.danger)};if(value.coreText("state")=="binding_recorded")TextButton(enabled=!disabled,onClick={working=true}){Text(stringResource(R.string.rovai_fusion_7dcca1fa2d8a79aa))}}}
                item{Button(enabled=!disabled&&(original.isNotEmpty()||preview.coreText("capabilityStatus")=="available"),onClick={work{if(original.isNotEmpty())reconcile(original)else{val request=buildJsonObject{put("schemaVersion",1);put("commandId",UUID.randomUUID().toString());put("previewId",preview.coreText("previewId"));put("previewDigest",preview.coreText("previewDigest"))};keep(buildJsonObject{put("preview",preview);put("original",request)});val result=nativeUserCall(model,RovaiOperation.HISTORY_EXECUTION_CONFIRM,request);require(result.coreText("commandId")==request.coreText("commandId")){"原回执归属不一致"};receipt=result}}}){Text(if(original.isNotEmpty())"核对原创建结果"else "确认创建，不发送消息")}}
                item{Text(stringResource(R.string.rovai_fusion_c348078f1130667e),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}
            }
        }
    }}}
}
private suspend fun nativeUserCallList(model:RovaiWorkspaceViewModel):List<JsonObject>{val generation=model.connectionGeneration;val value=model.query(RovaiOperation.MEMBERS_LIST,buildJsonObject{}).coreArray();require(generation==model.connectionGeneration){"工作区已切换"};return value.filter{it.coreText("presence")=="present"&&it["runtimeConfiguration"]!=null&&it["runtimeConfiguration"]!=JsonNull}}
