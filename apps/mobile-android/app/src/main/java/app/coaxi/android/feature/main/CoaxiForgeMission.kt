package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import app.coaxi.android.core.designsystem.theme.CoaxiTheme
import app.coaxi.android.core.network.CoaxiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.util.UUID

@Composable fun CoaxiForgeMission(model:CoaxiWorkspaceViewModel,threadId:String,accountId:String,sourceKind:String,number:Long,repository:JsonObject,onSaved:()->Unit){
    var open by remember(threadId,accountId,sourceKind,number){mutableStateOf(false)}
    TextButton(onClick={open=true},enabled=model.state.collectAsState().value.pending==null){Text(stringResource(R.string.coaxi_fusion_bf52f518ddcdc457))}
    if(open)key(threadId,accountId,sourceKind,number){CoaxiForgeMissionEditor(model,threadId,accountId,sourceKind,number,repository,onClose={open=false},onSaved={open=false;onSaved()})}
}
@Composable private fun CoaxiForgeMissionEditor(model:CoaxiWorkspaceViewModel,threadId:String,accountId:String,sourceKind:String,number:Long,repository:JsonObject,onClose:()->Unit,onSaved:()->Unit){
    val coroutine=rememberCoroutineScope();val generation=model.connectionGeneration;val owner="forge-mission:$threadId:$accountId:$sourceKind:$number"
    var title by remember{mutableStateOf("")};var description by remember{mutableStateOf("")};var sourceUrl by remember{mutableStateOf("")};var project by remember{mutableStateOf("")};var members by remember{mutableStateOf<List<JsonObject>>(emptyList())};var selected by remember{mutableStateOf<Set<String>>(emptySet())};var lead by remember{mutableStateOf("")}
    var commandId by remember{mutableStateOf(UUID.randomUUID().toString())};var sent by remember{mutableStateOf<JsonObject?>(null)};var link by remember{mutableStateOf<JsonObject?>(null)};var created by remember{mutableStateOf("")};var ready by remember{mutableStateOf(false)};var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)}
    fun base()=buildJsonObject{putJsonObject("scope"){put("threadId",threadId)};put("sourceKind",sourceKind);put("number",number);if(accountId.isNotEmpty())put("accountId",accountId)}
    fun record()=buildJsonObject{put("repositoryKey",repository.coreText("repositoryKey"));put("title",title);put("description",description);put("sourceUrl",sourceUrl);put("project",project);put("members",JsonArray(members));put("selected",JsonArray(selected.map(::JsonPrimitive)));put("lead",lead);put("commandId",commandId);put("sent",sent?:JsonNull);put("link",link?:JsonNull);put("createdId",created)}.toString()
    LaunchedEffect(owner,generation){busy=true;try{val saved=model.readBody(owner);if(saved.isNotEmpty()){val value=Json.parseToJsonElement(saved).coreObject();require(value.coreText("repositoryKey")==repository.coreText("repositoryKey")){"远程仓库已变化，请在原仓库核对已保留的使命操作"};title=value.coreText("title");description=value.coreText("description");sourceUrl=value.coreText("sourceUrl");project=value.coreText("project");members=value["members"].coreArray();selected=(value["selected"]as?JsonArray)?.mapNotNull{(it as?JsonPrimitive)?.contentOrNull}?.toSet().orEmpty();lead=value.coreText("lead");commandId=value.coreText("commandId");sent=value["sent"]as?JsonObject;link=value["link"]as?JsonObject;created=value.coreText("createdId")}
        else{val current=model.query(CoaxiOperation.THREADS_OPEN,buildJsonObject{put("threadId",threadId);put("traceId",UUID.randomUUID().toString())}).coreObject();val info=current["thread"].coreObject();require(info.coreText("projectBindingKind")=="directory"&&info.coreText("projectPath").isNotEmpty()){"此对话没有可关联的目录工程"};project=info.coreText("projectPath");members=current["members"].coreArray();selected=members.map{it.coreText("agentId")}.filter{it.isNotEmpty()}.toSet();lead=info.coreText("defaultLeadAgentId").takeIf{it in selected}?:selected.firstOrNull().orEmpty();val source=model.query(CoaxiOperation.FORGE_OPERATION,buildJsonObject{base().forEach{(key,value)->if(key!="sourceKind")put(key,value)};put("action","mission_draft_$sourceKind")}).coreObject();require(source["success"]==JsonPrimitive(true)&&source["repository"].coreObject().coreText("repositoryKey")==repository.coreText("repositoryKey")){"仓库记录来源发生变化"};val draft=source["missionDraft"].coreObject();title=draft.coreText("title");description=draft.coreText("description");sourceUrl=draft.coreText("sourceUrl")};require(generation==model.connectionGeneration){"工作区已切换"};ready=true
    }catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    val document=record();LaunchedEffect(document,ready){if(ready&&!busy){delay(350);model.rememberBody(owner,document)}}
    fun submit(){if(!ready||busy)return;busy=true;issue=null;coroutine.launch{try{
        val request=sent?:buildJsonObject{put("commandId",commandId);putJsonObject("command"){put("title",title);put("description",description);put("projectPath",project);put("projectBindingKind","directory");put("memberAgentIds",JsonArray(selected.sorted().map(::JsonPrimitive)));put("defaultLeadAgentId",lead);put("tags",JsonArray(listOf(JsonPrimitive("forge:${repository.coreText("provider")}"))))}}
        sent=request;model.rememberBody(owner,record());model.saveBody(owner,record())
        if(created.isEmpty()){val result=model.query(CoaxiOperation.MISSIONS_CREATE,request).coreObject();val receipt=result["commandResult"].coreObject().ifEmpty{result};require(receipt.coreText("status")in listOf("applied","accepted")&&generation==model.connectionGeneration){"使命创建结果需核对，原请求已保留"};val payload=receipt["payload"].coreObject();created=payload.coreText("missionId").ifEmpty{payload["mission"].coreObject().coreText("id")};require(created.isNotEmpty()){"使命已返回，但尚未核对到使命编号"};model.rememberBody(owner,record());model.saveBody(owner,record())}
        val linkRequest=link?:run{val page=model.query(CoaxiOperation.FORGE_LINKS,buildJsonObject{base().forEach{(key,value)->put(key,value)};put("operation","list")}).coreObject();buildJsonObject{base().forEach{(key,value)->put(key,value)};put("operation","link");put("targetKind","mission");put("targetId",created);put("expectedRevision",page.coreText("revision"));put("expectedRepositoryKey",repository.coreText("repositoryKey"));put("commandId",UUID.randomUUID().toString())}}
        link=linkRequest;model.rememberBody(owner,record());model.saveBody(owner,record());val result=model.query(CoaxiOperation.FORGE_LINKS,linkRequest).coreObject();require(result["applied"]==JsonPrimitive(true)&&generation==model.connectionGeneration){"使命已保留，来源关联仍需核对"};model.rememberBody(owner,"");ready=false;model.refreshNow();onSaved()
    }catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}
    Dialog(onDismissRequest={if(!busy)onClose()},properties=DialogProperties(usePlatformDefaultWidth=false)){Surface(Modifier.fillMaxSize(),color=CoaxiTheme.colors.bgElevated){Column(Modifier.fillMaxSize().padding(16.dp),verticalArrangement=Arrangement.spacedBy(10.dp)){
        Row{Text(stringResource(R.string.coaxi_fusion_a65c7961075bf328),Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(enabled=!busy,onClick=onClose){Text(stringResource(R.string.coaxi_fusion_3dfe7b6b28cb0c80))}};Text(sourceUrl,style=MaterialTheme.typography.bodySmall,color=CoaxiTheme.colors.textSecondary);issue?.let{Text(it,color=CoaxiTheme.colors.danger)}
        LazyColumn(Modifier.weight(1f),verticalArrangement=Arrangement.spacedBy(10.dp)){item{OutlinedTextField(title,{title=it},enabled=ready&&!busy&&sent==null,label={Text(stringResource(R.string.coaxi_fusion_476c7ace3e8f7e4b))},modifier=Modifier.fillMaxWidth())};item{OutlinedTextField(description,{description=it},enabled=ready&&!busy&&sent==null,label={Text(stringResource(R.string.coaxi_fusion_a26c64a6aaea9dd8))},modifier=Modifier.fillMaxWidth(),minLines=5,maxLines=15)};item{Text("当前工程 · $project",style=MaterialTheme.typography.bodySmall)};item{Text(stringResource(R.string.coaxi_fusion_eec1f8f154144e4f),style=MaterialTheme.typography.titleSmall);members.forEach{member->val id=member.coreText("agentId");Row{Checkbox(checked=id in selected,enabled=ready&&!busy&&sent==null,onCheckedChange={checked->selected=if(checked)selected+id else selected-id;if(lead !in selected)lead=selected.firstOrNull().orEmpty()});TextButton(enabled=ready&&!busy&&sent==null&&id in selected,onClick={lead=id}){Text(member.coreText("displayName")+(if(lead==id)" · 负责人" else ""))}}}};item{Text(if(created.isEmpty())"先保存使命和来源关系，由你决定何时开始。" else "使命已保存，继续核对原来源关联。",color=CoaxiTheme.colors.textSecondary)}}
        Button(enabled=ready&&!busy&&title.isNotBlank()&&lead in selected&&selected.isNotEmpty(),onClick=::submit,modifier=Modifier.fillMaxWidth()){Text(if(sent==null)"确认保存使命" else "核对原创建和关联")}
    }}}
}
