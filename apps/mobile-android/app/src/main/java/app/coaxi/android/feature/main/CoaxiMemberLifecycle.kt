package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.coaxi.android.core.designsystem.theme.CoaxiTheme
import app.coaxi.android.core.network.CoaxiOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import java.util.UUID

@Composable
fun CoaxiMemberLifecycle(model:CoaxiWorkspaceViewModel,member:JsonObject){
    val state by model.state.collectAsStateWithLifecycle();val coroutine=rememberCoroutineScope()
    var preview by remember(member.coreText("agentId")){mutableStateOf<JsonObject?>(null)};var name by remember{mutableStateOf("")};var issue by remember{mutableStateOf<String?>(null)};var loading by remember{mutableStateOf(false)};var submitted by remember{mutableStateOf<String?>(null)}
    val disabled=loading||state.busy||state.pending!=null
    LaunchedEffect(state.lastResult){val receipt=state.lastResult.coreObject()["commandResult"].coreObject().ifEmpty{state.lastResult.coreObject()};if(receipt.coreText("commandId")==submitted){if(receipt.coreText("status")=="applied"){preview=null;name=""}else issue=receipt.coreText("code")}}
    Row(horizontalArrangement=Arrangement.spacedBy(8.dp)){
        TextButton(enabled=!disabled&&member.coreText("presence")!="removed",onClick={model.command(CoaxiOperation.MEMBERS_PRESENCE_SET,buildJsonObject{put("agentId",member.coreText("agentId"));put("expectedVersion",member.coreLong("version"));put("presence",if(member.coreText("presence")=="present")"away" else "present")})}){Text(if(member.coreText("presence")=="present")"暂离" else "归队")}
        TextButton(enabled=!disabled,onClick={coroutine.launch{loading=true;try{val result=model.query(CoaxiOperation.MEMBERS_REMOVALPREVIEW,buildJsonObject{put("agentId",member.coreText("agentId"))}).coreObject();require(result.coreText("agentId")==member.coreText("agentId")){"队员归属发生变化"};preview=result;name="";issue=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{loading=false}}}){Text(stringResource(R.string.coaxi_fusion_6135d4159e892541))}
    }
    issue?.let{Text(it,color=CoaxiTheme.colors.danger)}
    preview?.let{value->AlertDialog(onDismissRequest={if(!disabled)preview=null},title={Text(stringResource(R.string.coaxi_fusion_5784bb0ad713c0e6)+value.coreText("displayName"))},text={Column(verticalArrangement=Arrangement.spacedBy(10.dp)){
        Text("正在执行：${value.coreLong("nonTerminalAgentRunCount")}\n所在对话：${value.coreLong("currentThreadMembershipCount")}\n未完成责任任务：${value.coreLong("openAssignedTaskCount")}\n担任负责人：${value.coreLong("defaultLeadThreadCount")}")
        Text(stringResource(R.string.coaxi_fusion_807edec42570faac),color=CoaxiTheme.colors.textSecondary)
        if(value["removable"]!=JsonPrimitive(true))Text(stringResource(R.string.coaxi_fusion_60d04e86e7f63b80),color=CoaxiTheme.colors.danger)
        OutlinedTextField(value=name,onValueChange={name=it},label={Text(stringResource(R.string.coaxi_fusion_1d36f3fdadd5af08))},enabled=!disabled)
    }},confirmButton={TextButton(enabled=!disabled&&value["removable"]==JsonPrimitive(true)&&name==value.coreText("displayName"),onClick={val id=UUID.randomUUID().toString();submitted=id;model.execute(CoaxiOperation.MEMBERS_REMOVE,buildJsonObject{put("commandId",id);putJsonObject("command"){put("agentId",value.coreText("agentId"));put("expectedVersion",value.coreLong("version"));put("confirmationName",name)}})}){Text(stringResource(R.string.coaxi_fusion_ee7dea070d88bb3c))}},dismissButton={TextButton(enabled=!disabled,onClick={preview=null}){Text(stringResource(R.string.coaxi_fusion_2cd0f3be8738a86c))}})}
}
