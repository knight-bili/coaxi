package app.codeg.android.feature.main

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
import java.util.UUID

internal suspend fun nativeUserCall(model:RovaiWorkspaceViewModel,operation:RovaiOperation,input:JsonObject):JsonObject{
    val generation=model.connectionGeneration;val value=model.query(operation,input).coreObject()
    require(generation==model.connectionGeneration){"工作区已切换，原请求保留"};return value
}
@Composable fun RovaiMemberNativeSteerSwitch(model:RovaiWorkspaceViewModel,agentId:String){
    val coroutine=rememberCoroutineScope();val state=model.state.collectAsState().value;val generation=remember(agentId){model.connectionGeneration};val owner="native-steer-policy:$agentId"
    var policy by remember(agentId){mutableStateOf<JsonObject?>(null)};var original by remember(agentId){mutableStateOf<JsonObject?>(null)};var busy by remember(agentId){mutableStateOf(false)};var issue by remember(agentId){mutableStateOf<String?>(null)}
    suspend fun read(){policy=nativeUserCall(model,RovaiOperation.RUNTIME_STEER_POLICY_GET,buildJsonObject{put("schemaVersion",1)})}
    suspend fun accept(result:JsonObject,request:JsonObject){
        require(result.coreText("commandId")==request.coreText("commandId")&&result.coreText("commandType")=="runtime.steer.policy.set"){"原回执归属不一致"}
        require(result.coreText("status") in listOf("applied","accepted","rejected")){"原请求尚无确定回执"}
        model.clearBodyIfMatches(owner,request.toString(),generation);original=null
        if(result.coreText("status")=="rejected"){read();error("本次设置未保存："+result.coreText("code"))}
        policy=result["payload"].coreObject()
    }
    fun work(action:suspend()->Unit){if(busy)return;busy=true;issue=null;coroutine.launch{try{action()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}
    LaunchedEffect(agentId,generation){busy=true;try{val raw=model.readBody(owner);if(raw.isNotEmpty())original=Json.parseToJsonElement(raw).coreObject();read()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    val enabled=policy?.get("members").coreArray().any{it.coreText("agentId")==agentId&&it["enabled"]==JsonPrimitive(true)}
    Column(verticalArrangement=Arrangement.spacedBy(6.dp)){
        Row{Column(Modifier.weight(1f)){Text("原生插话");Text("默认关闭；仅之后的新任务生效，仍需实际连接支持。",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)};Switch(checked=enabled,enabled=!busy&&policy!=null&&original==null&&!state.busy&&state.pending==null&&generation==model.connectionGeneration,onCheckedChange={checked->val current=policy?:return@Switch;val request=buildJsonObject{put("schemaVersion",1);put("commandId",UUID.randomUUID().toString());put("expectedRevision",current.coreLong("revision"));put("members",JsonArray(current["members"].coreArray().filter{it.coreText("agentId")!=agentId}+buildJsonObject{put("agentId",agentId);put("enabled",checked)}))};work{model.saveBody(owner,request.toString());original=request;accept(nativeUserCall(model,RovaiOperation.RUNTIME_STEER_POLICY_SET,request),request)}})}
        original?.let{request->TextButton(enabled=!busy&&generation==model.connectionGeneration,onClick={work{val value=nativeUserCall(model,RovaiOperation.COMMANDS_RECONCILE,buildJsonObject{put("operation","runtime.steer.policy.set");putJsonObject("params"){put("commandId",request.coreText("commandId"))}});require(value.coreText("state")=="recorded"){"原请求尚无确定回执，未重复发送"};accept(value["result"].coreObject(),request)}}){Text("核对原保存请求")}}
        issue?.let{Text(it,style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.danger)}
    }
}

internal fun nativeSteerState(receipt:JsonObject)=when(receipt.coreText("status")){"prepared"->"已登记，正在发送原请求";"connection_written"->"已写入原生连接";"native_acknowledged"->"原程序已明确接收";"not_consumed"->"原程序未接收，整份输入保留";"requires_user_review"->"原程序已接收但另开轮次，需要核对";"unknown"->"结果待核对，不重复发送";else->receipt.coreText("status")}

/** 复用原生 GlassRow/表单，只接受当前卡片提供的附件 metadata。 */
@Composable fun RovaiNativeSteer(model:RovaiWorkspaceViewModel,scope:JsonObject,agentId:String?=null,text:String,materials:List<JsonObject> = emptyList(),composerScope:JsonElement=JsonNull,onClose:()->Unit){
    val coroutine=rememberCoroutineScope();val generation=remember(scope){model.connectionGeneration};val state=model.state.collectAsState().value;val owner="native-steer:$scope:${agentId.orEmpty()}"
    var targets by remember(scope){mutableStateOf<List<JsonObject>>(emptyList())};var run by remember(scope){mutableStateOf("")};var checked by remember(scope){mutableStateOf<Set<String>>(emptySet())};var packet by remember(scope){mutableStateOf<JsonObject?>(null)};var receipt by remember(scope){mutableStateOf<JsonObject?>(null)}
    var history by remember(scope){mutableStateOf<List<JsonObject>>(emptyList())};var after by remember(scope){mutableStateOf<String?>(null)};var busy by remember(scope){mutableStateOf(false)};var ready by remember(scope){mutableStateOf(false)};var issue by remember(scope){mutableStateOf<String?>(null)}
    val disabled=busy||!ready||state.busy||state.pending!=null||generation!=model.connectionGeneration
    var normalOpen by remember(scope){mutableStateOf(false)}
    suspend fun keep(value:JsonObject){model.saveBody(owner,value.toString());packet=value}
    suspend fun records(more:Boolean=false){val page=nativeUserCall(model,RovaiOperation.RUNTIME_STEER_PREVIEWS,buildJsonObject{put("schemaVersion",1);put("scope",scope);put("limit",50);put("after",if(more)after?.let(::JsonPrimitive)?:JsonNull else JsonNull)});val rows=page["items"].coreArray();history=if(more)(history+rows).distinctBy{it["preview"].coreObject().coreText("previewId")}else rows;after=(page["next"]as?JsonPrimitive)?.contentOrNull}
    fun work(action:suspend()->Unit){if(busy)return;busy=true;issue=null;coroutine.launch{try{action()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}
    suspend fun reconcile(request:JsonObject){val value=nativeUserCall(model,RovaiOperation.RUNTIME_STEER_RECEIPT,buildJsonObject{put("schemaVersion",1);put("commandId",request.coreText("commandId"))});require(value.coreText("commandId")==request.coreText("commandId")){"原回执归属不一致"};receipt=value}
    LaunchedEffect(scope,generation){busy=true;try{
        val raw=model.readBody(owner);if(raw.isNotEmpty()){packet=Json.parseToJsonElement(raw).coreObject();packet?.get("original")?.coreObject()?.takeIf{it.isNotEmpty()}?.let{reconcile(it)}}
        val value=nativeUserCall(model,RovaiOperation.RUNTIME_STEER_TARGETS,buildJsonObject{put("schemaVersion",1);put("scope",scope)});targets=value["targets"].coreArray().filter{agentId==null||it.coreText("agentId")==agentId};if(targets.size==1)run=targets.first().coreText("agentRunId");records();ready=true
    }catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    if(normalOpen&&receipt!=null){RovaiNativeNormalContinue(model,receipt!!.coreText("commandId"),composerScope){normalOpen=false};return}
    Dialog(onDismissRequest={if(!busy)onClose()},properties=DialogProperties(usePlatformDefaultWidth=false)){Surface(Modifier.fillMaxWidth().fillMaxHeight(.94f),shape=MaterialTheme.shapes.large){Column(Modifier.padding(16.dp)){
        Row{Text("完整原生插话",Modifier.weight(1f),style=MaterialTheme.typography.titleLarge);TextButton(enabled=!busy,onClick=onClose){Text("关闭并保留")}}
        issue?.let{Text(it,color=CodegTheme.colors.danger)}
        LazyColumn(Modifier.weight(1f),verticalArrangement=Arrangement.spacedBy(10.dp)){
            val preview=packet?.get("preview").coreObject();val original=packet?.get("original").coreObject()
            if(preview.isEmpty()){
                items(targets,key={it.coreText("agentRunId")}){target->Row{RadioButton(selected=run==target.coreText("agentRunId"),onClick={run=target.coreText("agentRunId");checked=emptySet()},enabled=!disabled);Text((state.members.find{it.coreText("agentId")==target.coreText("agentId")}?.coreText("displayName")?:target.coreText("agentId"))+" · "+target.coreText("agentRunId"))}}
                item{SelectionContainer{Text(text)}}
                items(materials,key={it.coreText("id")}){material->Row{Checkbox(checked=material.coreText("id")in checked,enabled=!disabled&&material.coreText("kind")=="file"&&material.coreText("availability")in listOf("","unknown","available"),onCheckedChange={selected->checked=if(selected)checked+material.coreText("id")else checked-material.coreText("id")});Text(material.coreText("displayName"))}}
                item{Button(enabled=!disabled&&targets.any{it.coreText("agentRunId")==run}&&(text.isNotBlank()||checked.isNotEmpty())&&text.codePointCount(0,text.length)<=4096,onClick={work{
                    val target=targets.first{it.coreText("agentRunId")==run};val frozen=if(checked.isEmpty())JsonArray(emptyList())else nativeUserCall(model,RovaiOperation.RUNTIME_STEER_MATERIALS_PREPARE,buildJsonObject{put("schemaVersion",1);put("target",target);put("sourceIds",JsonArray(checked.map(::JsonPrimitive)));put("composerScope",composerScope)})["materials"]?:JsonArray(emptyList())
                    val next=nativeUserCall(model,RovaiOperation.RUNTIME_STEER_PREVIEW,buildJsonObject{put("schemaVersion",1);put("target",target);putJsonObject("input"){put("text",text);put("materials",frozen)}});keep(buildJsonObject{put("preview",next)});receipt=null
                }}){Text("预览整份文字和材料")};if(targets.isEmpty())Text("当前没有实际支持原生插话且已启用的任务。")}
            }else{
                item{GlassRow(Modifier.fillMaxWidth()){Text("将发送的完整内容",style=MaterialTheme.typography.titleMedium);Text(preview["input"].coreObject().coreText("text"));preview["input"].coreObject()["materials"].coreArray().forEach{material->Text(material.coreText("displayName")+" · "+material.coreLong("byteSize")+" B")};Text("普通输入和其他材料保留。",color=CodegTheme.colors.textSecondary)}}
                receipt?.let{value->item{Text(nativeSteerState(value));Text(value.coreText("commandId"),style=MaterialTheme.typography.bodySmall);if(value.coreText("nativeOutcome")=="startedNewTurn")Text("输入已被原程序消耗，不要再次排队；该连接的直接插话已停用。");if(value.coreText("status")=="not_consumed")Text("整份输入仍保存在原预览，作为普通下一轮发送须另外确认。")}}
                if(receipt?.coreText("status")=="not_consumed")item{TextButton(enabled=!disabled,onClick={normalOpen=true}){Text("预览整份普通下一轮")}}
                receipt?.let{value->item{RovaiNativeEvidence(model,value.coreText("commandId"))}}
                item{Button(enabled=!disabled&&(receipt==null||original.isNotEmpty()),onClick={work{
                    if(original.isNotEmpty())reconcile(original)else{
                        val request=buildJsonObject{put("schemaVersion",1);put("commandId",UUID.randomUUID().toString());put("previewId",preview.coreText("previewId"));put("previewDigest",preview.coreText("previewDigest"))};keep(buildJsonObject{put("preview",preview);put("original",request)})
                        val value=nativeUserCall(model,RovaiOperation.RUNTIME_STEER_CONFIRM,request);require(value.coreText("commandId")==request.coreText("commandId")){"原回执归属不一致"};receipt=value
                    }
                }}){Text(if(original.isNotEmpty())"核对原提交"else "确认发送完整补充")}}
            }
            item{Text("原生插话记录",style=MaterialTheme.typography.titleMedium)}
            items(history.filter{agentId==null||it["preview"].coreObject()["target"].coreObject().coreText("agentId")==agentId},key={it["preview"].coreObject().coreText("previewId")}){row->GlassRow(Modifier.fillMaxWidth()){Text(row["preview"].coreObject()["input"].coreObject().coreText("text"));Text(nativeSteerState(row["receipt"].coreObject()));TextButton(enabled=!disabled,onClick={work{val old=row["receipt"].coreObject();keep(buildJsonObject{put("preview",row["preview"]!!);if(old.isNotEmpty())put("original",buildJsonObject{put("schemaVersion",1);put("commandId",old.coreText("commandId"));put("previewId",row["preview"].coreObject().coreText("previewId"));put("previewDigest",row["preview"].coreObject().coreText("previewDigest"))})});receipt=old.takeIf{it.isNotEmpty()}}}){Text("查看完整内容和原回执")}}}
            if(after!=null)item{TextButton(enabled=!disabled,onClick={work{records(true)}}){Text("更多记录")}}
        }
    }}}
}
