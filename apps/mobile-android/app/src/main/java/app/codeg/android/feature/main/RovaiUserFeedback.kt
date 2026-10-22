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
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*

private suspend fun feedbackCall(model:RovaiWorkspaceViewModel,action:String,payload:JsonObject):JsonObject{
    val generation=model.connectionGeneration
    val result=model.query(RovaiOperation.FEEDBACK_OPERATION,buildJsonObject{put("action",action);put("payload",payload)}).coreObject()
    require(generation==model.connectionGeneration){"工作区已切换，保留原记录"};return result
}
fun feedbackScope(threadId:String,conversationId:String?=null,child:Boolean=false)=buildJsonObject{put("threadId",threadId);put("conversationId",conversationId?.let(::JsonPrimitive)?:JsonNull);put("scopeKind",if(conversationId==null)"public" else if(child)"delegation_child" else "single_chat")}
private fun feedbackStatus(value:JsonObject)=when(value.coreText("status")){"pending"->"等待队员检查反馈";"response_prepared"->"已准备，正在核对写出";"connection_written"->"已送到智能体连接";"unknown"->"结果待核对";"requires_user_review"->"任务已变化，需要重新选择";"continued_as_message"->if(value["continuedMessageId"]==JsonNull)"已作为普通消息排队" else "已作为普通消息续办";else->value.coreText("status")}

/** 沿用原生模型设置表单；独立开关不改写队员身份或旧任务。 */
@Composable fun RovaiMemberFeedbackSwitch(model:RovaiWorkspaceViewModel,agentId:String){
    val coroutine=rememberCoroutineScope();var policy by remember(agentId){mutableStateOf<JsonObject?>(null)};var original by remember(agentId){mutableStateOf<JsonObject?>(null)};var busy by remember(agentId){mutableStateOf(false)};var issue by remember(agentId){mutableStateOf<String?>(null)}
    val generation=remember(agentId){model.connectionGeneration};val owner="feedback-policy:$agentId";val state=model.state.collectAsState().value
    suspend fun read(){policy=feedbackCall(model,"policy.get",buildJsonObject{})}
    suspend fun save(input:JsonObject){busy=true;try{model.saveBody(owner,input.toString());val value=feedbackCall(model,"policy.set",input);require(generation==model.connectionGeneration);policy=value;original=null;model.clearBodyIfMatches(owner,input.toString(),generation);issue=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    LaunchedEffect(agentId){busy=true;try{val raw=model.readBody(owner);if(raw.isNotEmpty())original=Json.parseToJsonElement(raw).coreObject();read()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    val enabled=policy?.get("members").coreArray().find{it.coreText("agentId")==agentId}?.get("enabled")==JsonPrimitive(true)
    Column(verticalArrangement=Arrangement.spacedBy(6.dp)){
        Row{Column(Modifier.weight(1f)){Text("工作中接收补充");Text("默认关闭；开启后从新任务生效，队员主动检查时才会收到。",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)};Switch(enabled=!busy&&policy!=null&&original==null&&!state.busy&&state.pending==null&&generation==model.connectionGeneration,checked=enabled,onCheckedChange={selected->
            val current=policy?:return@Switch;val members=current["members"].coreArray().filter{it.coreText("agentId")!=agentId}+buildJsonObject{put("agentId",agentId);put("enabled",selected)}
            val input=buildJsonObject{put("schemaVersion",1);put("expectedRevision",current.coreLong("revision"));put("members",JsonArray(members))};original=input;coroutine.launch{save(input)}
        })}
        issue?.let{Text(it,style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.danger)}
        original?.let{pending->TextButton(enabled=!busy&&generation==model.connectionGeneration,onClick={coroutine.launch{save(pending)}}){Text("核对原设置")}}
    }
}

/** 独立原生补充面板：普通消息输入及其材料不参与这份反馈。 */
@Composable fun RovaiUserFeedback(model:RovaiWorkspaceViewModel,threadId:String,conversationId:String?=null,child:Boolean=false,agentId:String?=null,normalInput:(suspend()->JsonObject)?=null,nativeMaterials:List<JsonObject>?=null,onClose:()->Unit){
    val coroutine=rememberCoroutineScope();val state=model.state.collectAsState().value;val scope=remember(threadId,conversationId,child){feedbackScope(threadId,conversationId,child)};val generation=remember(scope){model.connectionGeneration}
    val owner="feedback:${scope}";var body by remember(scope){mutableStateOf("")};var selected by remember(scope){mutableStateOf(agentId.orEmpty())};var targets by remember(scope){mutableStateOf<List<JsonObject>>(emptyList())};var run by remember(scope){mutableStateOf("")}
    var records by remember(scope){mutableStateOf<List<JsonObject>>(emptyList())};var previews by remember(scope){mutableStateOf<List<JsonObject>>(emptyList())};var next by remember(scope){mutableStateOf<String?>(null)};var previewNext by remember(scope){mutableStateOf<String?>(null)}
    var packet by remember(scope){mutableStateOf<JsonObject?>(null)};var receipt by remember(scope){mutableStateOf<JsonObject?>(null)};var uncertain by remember(scope){mutableStateOf(false)};var busy by remember(scope){mutableStateOf(false)};var ready by remember(scope){mutableStateOf(false)};var issue by remember(scope){mutableStateOf<String?>(null)}
    val disabled=busy||!ready||state.busy||state.pending!=null||generation!=model.connectionGeneration
    var nativeOpen by remember(scope){mutableStateOf(false)}
    if(nativeOpen){RovaiNativeSteer(model,scope,agentId,body,nativeMaterials?:if(conversationId==null)state.publicSources else emptyList()){nativeOpen=false};return}
    suspend fun keep(value:JsonObject?){model.saveBody("$owner:request",value?.toString().orEmpty());packet=value}
    suspend fun read(more:Boolean=false){
        val value=feedbackCall(model,"targets",buildJsonObject{put("scope",scope)});targets=value["targets"].coreArray().filter{agentId==null||it.coreText("agentId")==agentId}
        if(run.isEmpty()&&targets.size==1){run=targets.first().coreText("agentRunId");selected=targets.first().coreText("agentId")}
        if(selected.isNotEmpty()){
            val first=feedbackCall(model,"list",buildJsonObject{put("scope",scope);put("agentId",selected);put("after",if(more)next?.let(::JsonPrimitive)?:JsonNull else JsonNull)})
            val second=feedbackCall(model,"previews.list",buildJsonObject{put("scope",scope);put("agentId",selected);put("after",if(more)previewNext?.let(::JsonPrimitive)?:JsonNull else JsonNull)})
            records=if(more)(records+first["items"].coreArray()).distinctBy{it["receipt"].coreObject().coreText("feedbackId")}else first["items"].coreArray();previews=if(more)(previews+second["items"].coreArray()).distinctBy{it["preview"].coreObject().coreText("previewId")}else second["items"].coreArray()
            next=(first["next"]as?JsonPrimitive)?.contentOrNull;previewNext=(second["next"]as?JsonPrimitive)?.contentOrNull
        }
    }
    fun work(block:suspend()->Unit){if(busy)return;busy=true;coroutine.launch{try{block();issue=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}
    LaunchedEffect(scope){busy=true;try{body=model.readBody("$owner:text");val raw=model.readBody("$owner:request");if(raw.isNotEmpty()){packet=Json.parseToJsonElement(raw).coreObject();uncertain=packet?.get("original")is JsonObject};read();ready=true}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    LaunchedEffect(selected){if(ready)work{read()}}
    Dialog(onDismissRequest={if(!busy)onClose()},properties=DialogProperties(usePlatformDefaultWidth=false,dismissOnClickOutside=false)){
        Surface(Modifier.fillMaxSize(),color=CodegTheme.colors.bgElevated){Column(Modifier.fillMaxSize()){
            Row(Modifier.fillMaxWidth().padding(12.dp)){Text("工作中补充和反馈记录",Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(enabled=!busy,onClick=onClose){Text("关闭")}}
            LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
                item{issue?.let{Text(it,color=CodegTheme.colors.danger)};Text("先预览全文，再交给队员。送到连接不代表模型已阅读。",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}
                if(packet==null){
                    if(agentId==null)item{Text("查看哪位队员的反馈");state.members.filter{member->state.thread?.get("members").coreArray().any{it.coreText("agentId")==member.coreText("agentId")}}.forEach{member->TextButton(enabled=!disabled,onClick={selected=member.coreText("agentId");run=""}){Text((if(selected==member.coreText("agentId"))"✓ " else "")+member.coreText("displayName"))}}}
                    item{OutlinedTextField(body,{text->body=text;model.rememberBody("$owner:text",text)},Modifier.fillMaxWidth(),label={Text("补充说明")},enabled=!disabled,maxLines=8);Text("${body.codePointCount(0,body.length)}/4096",style=MaterialTheme.typography.bodySmall);TextButton(enabled=!disabled,onClick={nativeOpen=true}){Text("预览原生插话及当前材料")}}
                    items(targets.filter{selected.isEmpty()||it.coreText("agentId")==selected},key={it.coreText("agentRunId")}){target->Row{RadioButton(selected=run==target.coreText("agentRunId"),onClick={run=target.coreText("agentRunId");selected=target.coreText("agentId")},enabled=!disabled);Column{Text(target.coreText("displayName"));Text(target.coreText("agentRunId"),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}}}
                    item{val target=targets.find{it.coreText("agentRunId")==run};Button(enabled=!disabled&&target?.get("feedbackEnabled")==JsonPrimitive(true)&&target["nativeBindingReady"]==JsonPrimitive(true)&&body.isNotBlank()&&body.codePointCount(0,body.length)<=4096,onClick={work{
                        val request=buildJsonObject{put("schemaVersion",1);put("scope",scope);put("agentId",selected);put("agentRunId",run);put("executionEpoch",target!!["executionEpoch"]!!);putJsonObject("source"){put("kind","live_note")};put("text",body);put("title","");put("fields",JsonArray(emptyList()))}
                        val preview=feedbackCall(model,"preview",request);keep(buildJsonObject{put("kind","feedback");put("preview",preview)});receipt=null;uncertain=false
                    }}){Text("查看完整内容")};if(targets.isEmpty())Text("当前没有可接收反馈的运行任务。",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary);TextButton(enabled=!disabled,onClick={work{read()}}){Text("核对记录")}}
                    items(previews.filter{it["receipt"]==null||it["receipt"]==JsonNull},key={it["preview"].coreObject().coreText("previewId")}){row->GlassRow(Modifier.fillMaxWidth()){Text("原预览仍保留");Text(row["preview"].coreObject().coreText("text"));TextButton(enabled=!disabled,onClick={work{keep(buildJsonObject{put("kind","feedback");put("preview",row["preview"]!!)});receipt=null;uncertain=false}}){Text("继续核对原预览")}}}
                    items(records,key={it["receipt"].coreObject().coreText("feedbackId")}){row->val record=row["receipt"].coreObject();GlassRow(Modifier.fillMaxWidth()){
                        Text(feedbackStatus(record));Text(row.coreText("text"));Text(record.coreText("feedbackId"),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)
                        if(record.coreText("status")in listOf("pending","requires_user_review")&&record.coreText("deliveryChannel")=="none"&&(conversationId==null||normalInput!=null))TextButton(enabled=!disabled,onClick={work{val value=feedbackCall(model,"normal.preview",buildJsonObject{put("scope",scope);put("agentId",record.coreText("agentId"));put("feedbackId",record.coreText("feedbackId"))});keep(buildJsonObject{put("kind","normal");put("preview",value["preview"]!!);put("reference",value["reference"]!!)});receipt=null;uncertain=false}}){Text("预览普通消息续办")}
                        if(record.coreText("status")in listOf("unknown","response_prepared"))Text("原结果尚未确定，不重复发送或改投其他任务。",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)
                    }}
                    if(next!=null||previewNext!=null)item{TextButton(enabled=!disabled,onClick={work{read(true)}}){Text("更多记录")}}
                }else item{
                    val frozen=packet!!;val preview=frozen["preview"].coreObject();val normal=frozen.coreText("kind")=="normal"
                    Text(if(normal)"确认按普通消息继续" else "将交给队员的完整内容",style=MaterialTheme.typography.titleSmall);Text(preview.coreText("text"));Text(preview.coreText("agentRunId"),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)
                    receipt?.let{Text(feedbackStatus(it))};if(uncertain)Text("提交结果待核对；原文和编号保留。",color=CodegTheme.colors.textSecondary)
                    Row{TextButton(enabled=!disabled,onClick={work{keep(null);receipt=null;uncertain=false;read()}}){Text("返回并保留原文")};Button(enabled=!disabled&&receipt==null,onClick={work{
                        val prior=frozen["original"]as?JsonObject
                        val original=prior?:if(normal)buildJsonObject{put("commandId",preview.coreText("previewId"));put("scope",scope);put("agentId",preview.coreText("agentId"));put("reference",frozen["reference"]!!);put("body",preview.coreText("text"));if(conversationId!=null){val input=normalInput?.invoke()?:error("请先处理普通输入中的材料");input.forEach{(key,value)->put(key,value)}}}else buildJsonObject{put("schemaVersion",1);put("commandId",preview.coreText("previewId"));put("previewId",preview.coreText("previewId"));put("previewDigest",preview.coreText("previewDigest"))}
                        keep(JsonObject(frozen.toMutableMap().apply{put("original",original)}));uncertain=true
                        val result=feedbackCall(model,if(normal){if(prior==null)"normal.send" else "normal.reconcile"}else "submit",original)
                        if(normal){require(result.coreText("state")=="recorded"){"还没有查到原请求结果，请稍后继续核对"};val command=result["commandResult"].coreObject();require(command.coreText("status")!="rejected"){command.coreText("code")};receipt=result["receipt"].coreObject()}else{receipt=result;if(preview.coreText("text")==body){model.clearBodyIfMatches("$owner:text",body,generation);body=""}}
                        uncertain=false;keep(null);read()
                    }}){Text(if(uncertain)"核对原请求" else if(normal)"确认作为普通消息发送" else "确认交给队员")}}
                }
            }
        }}
    }
}
