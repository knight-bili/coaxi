package app.codeg.android.feature.main

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import java.util.UUID

@Composable
fun RovaiMemberLifecycle(model:RovaiWorkspaceViewModel,member:JsonObject){
    val state by model.state.collectAsStateWithLifecycle();val coroutine=rememberCoroutineScope()
    var preview by remember(member.coreText("agentId")){mutableStateOf<JsonObject?>(null)};var name by remember{mutableStateOf("")};var issue by remember{mutableStateOf<String?>(null)};var loading by remember{mutableStateOf(false)};var submitted by remember{mutableStateOf<String?>(null)}
    val disabled=loading||state.busy||state.pending!=null
    LaunchedEffect(state.lastResult){val receipt=state.lastResult.coreObject()["commandResult"].coreObject().ifEmpty{state.lastResult.coreObject()};if(receipt.coreText("commandId")==submitted){if(receipt.coreText("status")=="applied"){preview=null;name=""}else issue=receipt.coreText("code")}}
    Row(horizontalArrangement=Arrangement.spacedBy(8.dp)){
        TextButton(enabled=!disabled&&member.coreText("presence")!="removed",onClick={model.command(RovaiOperation.MEMBERS_PRESENCE_SET,buildJsonObject{put("agentId",member.coreText("agentId"));put("expectedVersion",member.coreLong("version"));put("presence",if(member.coreText("presence")=="present")"away" else "present")})}){Text(if(member.coreText("presence")=="present")"暂离" else "归队")}
        TextButton(enabled=!disabled,onClick={coroutine.launch{loading=true;try{val result=model.query(RovaiOperation.MEMBERS_REMOVALPREVIEW,buildJsonObject{put("agentId",member.coreText("agentId"))}).coreObject();require(result.coreText("agentId")==member.coreText("agentId")){"队员归属发生变化"};preview=result;name="";issue=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{loading=false}}}){Text("移除")}
    }
    issue?.let{Text(it,color=CodegTheme.colors.danger)}
    preview?.let{value->AlertDialog(onDismissRequest={if(!disabled)preview=null},title={Text("移除队员 · "+value.coreText("displayName"))},text={Column(verticalArrangement=Arrangement.spacedBy(10.dp)){
        Text("正在执行：${value.coreLong("nonTerminalAgentRunCount")}\n所在对话：${value.coreLong("currentThreadMembershipCount")}\n未完成责任任务：${value.coreLong("openAssignedTaskCount")}\n担任负责人：${value.coreLong("defaultLeadThreadCount")}")
        Text("移除后历史身份和记录保留。",color=CodegTheme.colors.textSecondary)
        if(value["removable"]!=JsonPrimitive(true))Text("当前仍有责任或执行需要处理，请先完成对应交接。",color=CodegTheme.colors.danger)
        OutlinedTextField(value=name,onValueChange={name=it},label={Text("输入队员名称确认")},enabled=!disabled)
    }},confirmButton={TextButton(enabled=!disabled&&value["removable"]==JsonPrimitive(true)&&name==value.coreText("displayName"),onClick={val id=UUID.randomUUID().toString();submitted=id;model.execute(RovaiOperation.MEMBERS_REMOVE,buildJsonObject{put("commandId",id);putJsonObject("command"){put("agentId",value.coreText("agentId"));put("expectedVersion",value.coreLong("version"));put("confirmationName",name)}})}){Text("确认移除")}},dismissButton={TextButton(enabled=!disabled,onClick={preview=null}){Text("取消")}})}
}
