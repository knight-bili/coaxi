package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.coaxi.android.core.designsystem.component.GlassRow
import app.coaxi.android.core.designsystem.theme.CoaxiTheme
import app.coaxi.android.core.network.CoaxiOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import java.util.UUID

/** 嵌入原编辑表单，复用其滚动区域；只维护新计划附带的动作和时区。 */
@Composable fun CoaxiAutomationConversationDetails(model:CoaxiWorkspaceViewModel,automationId:String,enabled:Boolean,onVersionChange:(Long)->Unit,onOpenThread:(String)->Unit){
    if(runCatching{UUID.fromString(automationId)}.isFailure)return
    val coroutine=rememberCoroutineScope();val generation=remember(automationId){model.connectionGeneration};val storage="conversation-automation:$automationId"
    var owner by remember(automationId){mutableStateOf<JsonObject?>(null)};var action by remember(automationId){mutableStateOf("launch_session")};var timezone by remember(automationId){mutableStateOf("")}
    var rows by remember(automationId){mutableStateOf<List<JsonObject>>(emptyList())};var next by remember(automationId){mutableStateOf<String?>(null)};var pending by remember(automationId){mutableStateOf<JsonObject?>(null)};var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)}
    fun current(){require(generation==model.connectionGeneration){"工作区已切换"}}
    suspend fun call(op:CoaxiOperation,input:JsonObject):JsonObject{current();val value=nativeUserCall(model,op,input);current();return value}
    suspend fun load(more:Boolean=false){val value=call(CoaxiOperation.AUTOMATIONS_GET,buildJsonObject{put("automationId",automationId);if(more)put("queuedMissionAfter",next)});if(!more){owner=value["conversationToolOwner"] as? JsonObject;owner?.let{action=it.coreText("action");timezone=it.coreText("timezone");onVersionChange(it.coreLong("automationVersion"))}};val page=value["queuedMissions"].coreObject();rows=if(more)(rows+page["items"].coreArray()).distinctBy{it.coreText("occurrenceId")}else page["items"].coreArray();next=(page["next"] as? JsonPrimitive)?.contentOrNull}
    fun work(task:suspend()->Unit){if(busy)return;busy=true;issue=null;coroutine.launch{try{task()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(generation==model.connectionGeneration)issue=error.message}finally{if(generation==model.connectionGeneration)busy=false}}}
    suspend fun clear(original:JsonObject){require(model.clearBodyIfMatches(storage,original.toString(),generation)){"原保存请求已变化"};pending=null}
    LaunchedEffect(automationId,generation){busy=true;try{val raw=model.readBody(storage);current();if(raw.isNotEmpty()){require(raw.length<=1024);val value=Json.parseToJsonElement(raw).coreObject();require(value.keys==setOf("operation","commandId")&&value.coreText("operation")=="conversationTools.automationOwner.set");UUID.fromString(value.coreText("commandId"));pending=value};load()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    if(owner!=null||pending!=null||issue!=null)Column(verticalArrangement=Arrangement.spacedBy(8.dp)){
        Text(stringResource(R.string.coaxi_fusion_4941561ff102f4fd),style=MaterialTheme.typography.titleMedium)
        owner?.let{original->
            listOf("launch_session" to "启动队员会话","enqueue_task" to "生成待启动的使命").forEach{(value,label)->Row{RadioButton(selected=action==value,enabled=enabled&&!busy&&pending==null,onClick={action=value});Text(label)}}
            OutlinedTextField(timezone,{timezone=it},label={Text(stringResource(R.string.coaxi_fusion_340db31cc0805ee6))},enabled=enabled&&!busy&&pending==null,modifier=Modifier.fillMaxWidth())
            Text(stringResource(R.string.coaxi_fusion_c5bb807c5bd1ec50),style=MaterialTheme.typography.bodySmall,color=CoaxiTheme.colors.textSecondary)
            if(pending==null)TextButton(enabled=enabled&&!busy&&timezone.isNotBlank(),onClick={work{val request=buildJsonObject{put("operation","conversationTools.automationOwner.set");put("commandId",UUID.randomUUID().toString())};model.saveBody(storage,request.toString());current();pending=request;val value=call(CoaxiOperation.CONVERSATIONTOOLS_AUTOMATIONOWNER_SET,buildJsonObject{put("commandId",request.coreText("commandId"));put("automationId",automationId);put("expectedAutomationVersion",original.coreLong("automationVersion"));put("expectedOwnerRevision",original.coreLong("revision"));put("action",action);put("timezone",timezone)});clear(request);owner=value;onVersionChange(value.coreLong("automationVersion"));load()}}){Text(stringResource(R.string.coaxi_fusion_57bd983570e1e698))}
        }
        pending?.let{original->TextButton(enabled=!busy,onClick={work{val value=call(CoaxiOperation.COMMANDS_RECONCILE,buildJsonObject{put("operation",original.coreText("operation"));putJsonObject("params"){put("commandId",original.coreText("commandId"))}});val result=value["result"].coreObject();require(value.coreText("state")=="recorded"&&result.coreText("commandId")==original.coreText("commandId")&&result.coreText("commandType")==original.coreText("operation")){"原保存请求没有确定回执，没有再次修改计划"};clear(original);require(result.coreText("status")!="rejected"){result.coreText("code")};load()}}){Text(stringResource(R.string.coaxi_fusion_38ec197a5eb4bec8))}}
        TextButton(enabled=!busy&&pending==null,onClick={work{load()}}){Text(stringResource(R.string.coaxi_fusion_cf56144e84d1eecb))}
        rows.forEach{row->key(row.coreText("occurrenceId")){GlassRow(Modifier.fillMaxWidth()){Text(row.coreText("title"));Text(stringResource(R.string.coaxi_fusion_eb277618d43981b9)+row.coreText("scheduledAtUtc"),style=MaterialTheme.typography.bodySmall);TextButton(onClick={onOpenThread(row.coreText("threadId"))}){Text(stringResource(R.string.coaxi_fusion_176ff7a83df97f55))}}}}
        if(next!=null)TextButton(enabled=!busy,onClick={work{load(true)}}){Text(stringResource(R.string.coaxi_fusion_7442942abbb698d6))}
        issue?.let{Text(it,color=CoaxiTheme.colors.danger)}
    }
}
