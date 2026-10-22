package app.codeg.android.feature.main

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.util.UUID

@Composable fun RovaiNativeNormalContinue(model:RovaiWorkspaceViewModel,sourceCommandId:String,composerScope:JsonElement=JsonNull,onClose:()->Unit){
    val coroutine=rememberCoroutineScope();val generation=remember(sourceCommandId){model.connectionGeneration};val state=model.state.collectAsState().value;val owner="native-normal:$sourceCommandId"
    var packet by remember(sourceCommandId){mutableStateOf<JsonObject?>(null)};var receipt by remember(sourceCommandId){mutableStateOf<JsonObject?>(null)};var ready by remember(sourceCommandId){mutableStateOf(false)};var busy by remember(sourceCommandId){mutableStateOf(false)};var issue by remember(sourceCommandId){mutableStateOf<String?>(null)}
    val disabled=busy||!ready||state.busy||state.pending!=null||generation!=model.connectionGeneration
    suspend fun keep(value:JsonObject){require(generation==model.connectionGeneration){"工作区已切换"};model.saveBody(owner,value.toString());packet=value}
    suspend fun reconcile(original:JsonObject){val value=nativeUserCall(model,RovaiOperation.RUNTIME_STEER_NORMAL_RECEIPT,buildJsonObject{put("schemaVersion",1);put("commandId",original.coreText("commandId"))});require(value.coreText("commandId")==original.coreText("commandId")){"原回执归属不一致"};receipt=value}
    fun work(action:suspend()->Unit){if(busy)return;busy=true;issue=null;coroutine.launch{try{action()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}
    LaunchedEffect(sourceCommandId,generation){busy=true;try{
        val raw=model.readBody(owner);if(raw.isNotEmpty()){val saved=Json.parseToJsonElement(raw).coreObject();require(saved["preview"].coreObject().coreText("sourceCommandId")==sourceCommandId){"原请求记录归属不一致"};packet=saved;saved["original"].coreObject().takeIf{it.isNotEmpty()}?.let{reconcile(it)}}
        if(packet==null){val value=nativeUserCall(model,RovaiOperation.RUNTIME_STEER_NORMAL_PREVIEW,buildJsonObject{put("schemaVersion",1);put("sourceCommandId",sourceCommandId)});keep(buildJsonObject{put("preview",value)})};ready=true
    }catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    Dialog(onDismissRequest={if(!busy)onClose()},properties=DialogProperties(usePlatformDefaultWidth=false)){Surface(Modifier.fillMaxWidth().fillMaxHeight(.94f),shape=MaterialTheme.shapes.large){Column(Modifier.padding(16.dp)){
        Row{Text("整份补充按普通下一轮发送",Modifier.weight(1f),style=MaterialTheme.typography.titleLarge);TextButton(enabled=!busy,onClick=onClose){Text("关闭并保留")}}
        issue?.let{Text(it,color=CodegTheme.colors.danger)}
        val preview=packet?.get("preview").coreObject();val original=packet?.get("original").coreObject()
        LazyColumn(Modifier.weight(1f),verticalArrangement=Arrangement.spacedBy(10.dp)){
            item{Text("原程序已明确未接收。本次另行确认后按原会话的普通发送和排队规则处理。",color=CodegTheme.colors.textSecondary)}
            if(preview.isNotEmpty())item{Text(preview["input"].coreObject().coreText("text"));preview["input"].coreObject()["materials"].coreArray().forEach{material->Text(material.coreText("displayName")+" · "+material.coreLong("byteSize")+" B")};Text("当前普通输入框的正文、材料和引用保持原状。本次只发送以上整份输入。",color=CodegTheme.colors.textSecondary)}
            receipt?.let{value->item{val result=value["commandResult"].coreObject();Text(when(value.coreText("state")){"recorded"->if(result.coreText("status")=="rejected")"本次未发送，完整输入保留"else "已按普通消息提交或排队";"failed"->"未完成提交，完整输入保留";"prepared"->"已登记，正在处理原请求";else->"原结果尚未确定，只核对同一编号"});Text(value.coreText("commandId"),style=MaterialTheme.typography.bodySmall);if(result.coreText("status")=="rejected")Text(result.coreText("code"),color=CodegTheme.colors.danger);value.coreText("error").takeIf{it.isNotEmpty()}?.let{Text(it,color=CodegTheme.colors.danger)}}}
        }
        if(receipt?.coreText("state")!in listOf("recorded","failed"))Button(enabled=!disabled&&preview.isNotEmpty(),onClick={work{
            if(original.isNotEmpty()){reconcile(original);return@work}
            val scope=preview["scope"].coreObject();var draftRevision:Long?=null
            if(scope.coreText("scopeKind")!="public"){
                val get=if(scope.coreText("scopeKind")=="delegation_child")RovaiOperation.DELEGATIONS_CONVERSATION_GET else RovaiOperation.SINGLECHAT_GET
                val current=nativeUserCall(model,get,buildJsonObject{put("conversationId",scope.coreText("conversationId"));put("composerScope",composerScope)})
                require(current["conversation"].coreObject().coreText("id")==scope.coreText("conversationId")){"当前私聊已变化，完整输入保留"};draftRevision=current["draft"].coreObject().coreLong("revision")
            }
            val request=buildJsonObject{put("schemaVersion",1);put("commandId",UUID.randomUUID().toString());putJsonObject("reference"){put("sourceCommandId",sourceCommandId);put("previewId",preview.coreText("previewId"));put("previewDigest",preview.coreText("previewDigest"))};draftRevision?.let{put("draftRevision",it);put("composerScope",composerScope)}}
            keep(buildJsonObject{put("preview",preview);put("original",request)});val value=nativeUserCall(model,RovaiOperation.RUNTIME_STEER_NORMAL_CONFIRM,request);require(value.coreText("commandId")==request.coreText("commandId")){"原回执归属不一致"};receipt=value
        }}){Text(if(original.isNotEmpty())"核对原普通提交"else "确认发送整份输入")}
    }}}
}
