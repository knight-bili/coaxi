package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.selection.SelectionContainer
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

@Composable fun RovaiNativeEvidence(model:RovaiWorkspaceViewModel,commandId:String){
    val coroutine=rememberCoroutineScope();var value by remember(commandId){mutableStateOf<JsonObject?>(null)};var issue by remember(commandId){mutableStateOf<String?>(null)};var busy by remember(commandId){mutableStateOf(false)};var open by remember(commandId){mutableStateOf(false)}
    Column(verticalArrangement=Arrangement.spacedBy(6.dp)){
        TextButton(enabled=!busy,onClick={open=!open;if(open){busy=true;issue=null;coroutine.launch{try{val result=nativeUserCall(model,RovaiOperation.RUNTIME_STEER_EVIDENCE,buildJsonObject{put("schemaVersion",1);put("commandId",commandId)});require(result["receipt"].coreObject().coreText("commandId")==commandId){"原回执归属不一致"};value=result}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}}){Text(if(open)"收起原投递记录"else "读取原投递与普通续办记录")}
        if(open){Text(stringResource(R.string.rovai_fusion_17fc39301a290764),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary);issue?.let{Text(it,color=CodegTheme.colors.danger)};value?.let{SelectionContainer{Text(Json{prettyPrint=true}.encodeToString(JsonObject.serializer(),it),style=MaterialTheme.typography.bodySmall)}}}
    }
}

@Composable fun RovaiNativeCreationRecords(model:RovaiWorkspaceViewModel,entry:JsonObject,onClose:()->Unit){
    val coroutine=rememberCoroutineScope();val generation=remember(entry){model.connectionGeneration};val state=model.state.collectAsState().value
    var rows by remember(entry){mutableStateOf<List<JsonObject>>(emptyList())};var after by remember(entry){mutableStateOf<String?>(null)};var busy by remember(entry){mutableStateOf(false)};var issue by remember(entry){mutableStateOf<String?>(null)};var working by remember(entry){mutableStateOf<JsonObject?>(null)}
    suspend fun read(more:Boolean=false){val value=nativeUserCall(model,RovaiOperation.HISTORY_EXECUTION_RECORDS,buildJsonObject{put("schemaVersion",1);put("sourceThreadId",entry.coreText("linkedThreadId"));put("importId",entry.coreText("id"));put("after",if(more)after?.let(::JsonPrimitive)?:JsonNull else JsonNull);put("limit",50)});val next=value["items"].coreArray();rows=if(more)(rows+next).distinctBy{it["preview"].coreObject().coreText("previewId")}else next;after=(value["next"]as?JsonPrimitive)?.contentOrNull}
    fun work(action:suspend()->Unit){if(busy)return;busy=true;issue=null;coroutine.launch{try{action()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}
    LaunchedEffect(entry,generation){busy=true;try{read()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    working?.let{receipt->Dialog(onDismissRequest={working=null},properties=DialogProperties(usePlatformDefaultWidth=false,dismissOnClickOutside=false)){Surface(Modifier.fillMaxSize()){RovaiPrivateConversation(model,receipt.coreText("threadId"),initialConversationId=receipt.coreText("conversationId")){working=null}}};return}
    Dialog(onDismissRequest={if(!busy)onClose()},properties=DialogProperties(usePlatformDefaultWidth=false)){Surface(Modifier.fillMaxWidth().fillMaxHeight(.94f),shape=MaterialTheme.shapes.large){Column(Modifier.padding(16.dp)){
        Row{Text(stringResource(R.string.rovai_fusion_b52874d86756ff74),Modifier.weight(1f),style=MaterialTheme.typography.titleLarge);TextButton(enabled=!busy,onClick=onClose){Text(stringResource(R.string.rovai_fusion_3fd47edce45b3603))}}
        issue?.let{Text(it,color=CodegTheme.colors.danger)}
        LazyColumn(Modifier.weight(1f),verticalArrangement=Arrangement.spacedBy(10.dp)){
            item{TextButton(enabled=!busy,onClick={work{read()}}){Text(stringResource(R.string.rovai_fusion_67fdc15fc7d8d754))}}
            items(rows,key={it["preview"].coreObject().coreText("previewId")}){row->val preview=row["preview"].coreObject();val receipt=row["receipt"].coreObject();GlassRow(Modifier.fillMaxWidth()){
                Text(preview.coreText("title"),style=MaterialTheme.typography.titleMedium);Text(when(preview.coreText("mode")){"continue"->"继续原生会话";"fork_point"->"从指定回答分叉";else->"从末尾分叉"});Text(if(receipt.isEmpty())"只有预览，尚未确认创建"else nativeCreationState(receipt));preview.coreText("reason").takeIf{it.isNotEmpty()}?.let{Text(it,color=CodegTheme.colors.textSecondary)}
                if(receipt.isNotEmpty()){SelectionContainer{Text(receipt.coreText("commandId"),style=MaterialTheme.typography.bodySmall)};receipt.coreText("reportedSessionId").takeIf{it.isNotEmpty()}?.let{SelectionContainer{Text(stringResource(R.string.rovai_fusion_37c177cb42b7a570)+it,style=MaterialTheme.typography.bodySmall)}};receipt.coreText("error").takeIf{it.isNotEmpty()}?.let{Text(it,color=CodegTheme.colors.danger)};TextButton(enabled=!busy&&generation==model.connectionGeneration,onClick={work{val result=nativeUserCall(model,RovaiOperation.HISTORY_EXECUTION_RECEIPT,buildJsonObject{put("schemaVersion",1);put("commandId",receipt.coreText("commandId"))});require(result.coreText("commandId")==receipt.coreText("commandId")){"原回执归属不一致"};rows=rows.map{old->if(old["preview"].coreObject().coreText("previewId")==preview.coreText("previewId"))JsonObject(old+mapOf("receipt"to result))else old}}){Text(stringResource(R.string.rovai_fusion_6ca6d7ced54750dc))}}
                if(receipt.coreText("state")=="binding_recorded")TextButton(enabled=!busy&&!state.busy&&state.pending==null&&generation==model.connectionGeneration,onClick={working=receipt}){Text(stringResource(R.string.rovai_fusion_7dcca1fa2d8a79aa))}
            }}
            if(rows.isEmpty()&&!busy)item{Text(stringResource(R.string.rovai_fusion_c38fa291bbe03d6e),color=CodegTheme.colors.textSecondary)}
            if(after!=null)item{TextButton(enabled=!busy,onClick={work{read(true)}}){Text(stringResource(R.string.rovai_fusion_b902b27f21363c7d))}}
        }
    }}}
}
