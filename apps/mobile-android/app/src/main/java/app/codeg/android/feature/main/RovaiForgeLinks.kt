package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.util.UUID

@Composable fun RovaiForgeLinks(model:RovaiWorkspaceViewModel,threadId:String,accountId:String,sourceKind:String,number:Long,repositoryKey:String){
    val coroutine=rememberCoroutineScope();val generation=model.connectionGeneration;val owner="forge-link:$threadId:$accountId:$sourceKind:$number"
    var page by remember(owner){mutableStateOf<JsonObject?>(null)};var missions by remember(owner){mutableStateOf<List<JsonObject>>(emptyList())};var tasks by remember(owner){mutableStateOf<List<JsonObject>>(emptyList())};var kind by remember(owner){mutableStateOf("thread")};var target by remember(owner){mutableStateOf(threadId)};var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)};var pending by remember(owner){mutableStateOf<JsonObject?>(null)};var unlinking by remember{mutableStateOf<JsonObject?>(null)}
    fun args(operation:String)=buildJsonObject{putJsonObject("scope"){put("threadId",threadId)};put("operation",operation);put("sourceKind",sourceKind);put("number",number);if(accountId.isNotEmpty())put("accountId",accountId)}
    suspend fun load(){page=model.query(RovaiOperation.FORGE_LINKS,args("list")).coreObject()}
    LaunchedEffect(owner,generation,repositoryKey){busy=true;try{val saved=model.readBody(owner);if(saved.isNotEmpty()){val value=Json.parseToJsonElement(saved).coreObject();require(value.coreText("expectedRepositoryKey")==repositoryKey){"仓库已变化，请在原仓库核对原关联"};pending=value};val thread=model.query(RovaiOperation.THREADS_OPEN,buildJsonObject{put("threadId",threadId);put("traceId",UUID.randomUUID().toString())}).coreObject()["thread"].coreObject();val project=thread.coreText("projectPath");missions=model.query(RovaiOperation.MISSIONS_LIST).coreArray().filter{it.coreText("projectPath")==project};tasks=model.query(RovaiOperation.TASKS_LIST,buildJsonObject{put("threadId",threadId);put("limit",200)}).coreObject()["tasks"].coreArray();load();require(generation==model.connectionGeneration){"工作区已切换"}}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    fun apply(operation:String,targetKind:String=kind,targetId:String=target){if(busy||page==null||targetId.isEmpty())return;busy=true;issue=null;coroutine.launch{try{val request=pending?:buildJsonObject{args(operation).forEach{(key,value)->put(key,value)};put("targetKind",targetKind);put("targetId",targetId);put("expectedRevision",page!!.coreText("revision"));put("expectedRepositoryKey",repositoryKey);put("commandId",UUID.randomUUID().toString())};pending=request;model.rememberBody(owner,request.toString());model.saveBody(owner,request.toString());val result=model.query(RovaiOperation.FORGE_LINKS,request).coreObject();require(generation==model.connectionGeneration&&result["applied"]==JsonPrimitive(true)){"关联结果尚未核对，原请求仍保留"};model.rememberBody(owner,"");pending=null;unlinking=null;load()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}
    Column(verticalArrangement=Arrangement.spacedBy(8.dp)){
        Text(stringResource(R.string.rovai_fusion_f6387b298d51c152),style=MaterialTheme.typography.titleSmall);issue?.let{Text(it,color=CodegTheme.colors.danger)}
        page?.get("links").coreArray().forEach{link->GlassRow(Modifier.fillMaxWidth()){
            val targetKind=link.coreText("targetKind");val targetId=link.coreText("targetId");val title=when(targetKind){"mission"->missions.firstOrNull{it.coreText("missionId")==targetId}?.coreText("title")?:"原使命暂不可用";"task"->tasks.firstOrNull{it.coreText("taskId")==targetId}?.coreText("title")?:"原任务暂不可用";else->"当前对话"};Text(title);TextButton(enabled=!busy&&pending==null,onClick={unlinking=link}){Text(stringResource(R.string.rovai_fusion_2d32ab1cdcd0506a))}
        }}
        Row{listOf("thread" to "当前对话","mission" to "使命","task" to "任务").forEach{(id,title)->FilterChip(selected=kind==id,enabled=!busy&&pending==null,onClick={kind=id;target=when(id){"mission"->missions.firstOrNull()?.coreText("missionId").orEmpty();"task"->tasks.firstOrNull()?.coreText("taskId").orEmpty();else->threadId}},label={Text(title)})}}
        if(kind!="thread")Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState())){(if(kind=="mission")missions else tasks).forEach{row->val id=row.coreText(if(kind=="mission")"missionId" else "taskId");FilterChip(selected=target==id,enabled=!busy&&pending==null,onClick={target=id},label={Text(row.coreText("title"))})}}
        TextButton(enabled=!busy&&page!=null&&(pending!=null||target.isNotEmpty()),onClick={apply("link")}){Text(if(pending==null)"关联" else "核对原关联")}
    }
    unlinking?.let{link->AlertDialog(onDismissRequest={if(!busy)unlinking=null},title={Text(stringResource(R.string.rovai_fusion_ee828b7ce27e4b0a))},text={Text(stringResource(R.string.rovai_fusion_3bf4ba47273faf80))},confirmButton={TextButton(enabled=!busy,onClick={apply("unlink",link.coreText("targetKind"),link.coreText("targetId"))}){Text(stringResource(R.string.rovai_fusion_2d32ab1cdcd0506a))}},dismissButton={TextButton(enabled=!busy,onClick={unlinking=null}){Text(stringResource(R.string.rovai_fusion_2cd0f3be8738a86c))}})}
}
