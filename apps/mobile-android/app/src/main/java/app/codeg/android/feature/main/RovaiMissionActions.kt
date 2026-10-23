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
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.util.UUID

@Composable fun RovaiMissionActions(model:RovaiWorkspaceViewModel,mission:JsonObject,onDeleted:()->Unit){
    val state=model.state.collectAsState().value;val coroutine=rememberCoroutineScope();val id=mission.coreText("missionId");val threadId=mission.coreText("threadId");val generation=model.connectionGeneration
    var editing by remember(id){mutableStateOf(false)};var lead by remember(id){mutableStateOf(false)};var confirmation by remember(id){mutableStateOf<String?>(null)};var delivery by remember{mutableStateOf<JsonObject?>(null)};var cleanup by remember{mutableStateOf(false)};var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)}
    val blocked=state.pending!=null||state.busy||busy
    fun prepare(kind:String){if(blocked)return;confirmation=kind;delivery=null;cleanup=false;busy=true;issue=null;coroutine.launch{try{val current=model.query(RovaiOperation.MISSIONS_DELIVERY,buildJsonObject{put("missionId",id)}).coreObject();require(generation==model.connectionGeneration){"工作区已切换"};delivery=current}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}
    Column(verticalArrangement=Arrangement.spacedBy(8.dp)){
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState())){rovaiMissionStatuses.forEach{(value,title)->FilterChip(selected=mission.coreText("status")==value,enabled=!blocked,onClick={model.command(RovaiOperation.MISSIONS_STATUS,buildJsonObject{put("missionId",id);put("status",value);put("sourceMessageId",JsonNull)})},label={Text(title)})}}
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState())){TextButton(enabled=!blocked,onClick={editing=true}){Text(stringResource(R.string.rovai_fusion_71311e4c8ed3af89))};TextButton(enabled=!blocked,onClick={lead=true}){Text(stringResource(R.string.rovai_fusion_d9ab862d7a96b2e6))};if(mission["cleanupAvailable"]==JsonPrimitive(true)||mission["workspaceCleanup"].coreObject().coreText("state")=="failed")TextButton(enabled=!blocked,onClick={prepare("cleanup")}){Text(stringResource(R.string.rovai_fusion_d79d46848ebc35f8))};TextButton(enabled=!blocked,onClick={prepare("delete")}){Text(stringResource(R.string.rovai_fusion_e11ac69338138fb5))}}
        issue?.let{Text(it,color=CodegTheme.colors.danger)}
    }
    if(editing)RovaiMissionEdit(model,mission){editing=false}
    if(lead)AlertDialog(onDismissRequest={if(!busy)lead=false},title={Text(stringResource(R.string.rovai_fusion_4c138bf38913196f))},text={Column{val ids=(mission["memberAgentIds"]as?JsonArray)?.mapNotNull{(it as?JsonPrimitive)?.contentOrNull}.orEmpty();state.members.filter{it.coreText("agentId") in ids}.forEach{member->val memberId=member.coreText("agentId");TextButton(enabled=!blocked&&memberId!=mission.coreText("defaultLeadAgentId"),onClick={busy=true;coroutine.launch{try{val current=model.query(RovaiOperation.THREADS_OPEN,buildJsonObject{put("threadId",threadId);put("traceId",UUID.randomUUID().toString())}).coreObject();require(generation==model.connectionGeneration){"工作区已切换"};model.command(RovaiOperation.THREADS_CHANGEDEFAULTLEAD,buildJsonObject{put("threadId",threadId);put("successorAgentId",memberId);put("expectedVersion",current["thread"].coreObject().coreLong("version"))});lead=false}catch(error:Exception){issue=error.message}finally{busy=false}}}){Text(member.coreText("displayName")+(if(memberId==mission.coreText("defaultLeadAgentId"))" · 当前队长" else ""))}}}},confirmButton={TextButton(enabled=!busy,onClick={lead=false}){Text(stringResource(R.string.rovai_fusion_3fd47edce45b3603))}})
    confirmation?.let{kind->AlertDialog(onDismissRequest={if(!busy)confirmation=null},title={Text(if(kind=="delete")"删除使命？" else "清理使命Worktree？")},text={Column(verticalArrangement=Arrangement.spacedBy(8.dp)){
        Text(if(kind=="delete")"使命、会话和交付文件将一并删除，正在执行的队员会停止。此操作无法撤销。" else "将清理此使命的Worktree和本地分支。主工程保留。")
        delivery?.get("workspace").coreObject().let{workspace->if(workspace.isNotEmpty()){Text(workspace.coreText("worktreePath"),style=MaterialTheme.typography.bodySmall);Text(workspace.coreText("managedBranch"),style=MaterialTheme.typography.bodySmall)}}
        if(kind=="delete"&&mission["workspaceEverCreated"]==JsonPrimitive(true))Row{Checkbox(cleanup,{cleanup=it},enabled=mission["workspaceResourcesPresent"]==JsonPrimitive(true)&&!busy);Text(stringResource(R.string.rovai_fusion_6223b4e828720346))}
        if(kind=="delete"&&!cleanup)Text(stringResource(R.string.rovai_fusion_7d05cb0716fc42c8),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary);issue?.let{Text(it,color=CodegTheme.colors.danger)}
    }},confirmButton={TextButton(enabled=!blocked&&delivery!=null&&(kind!="cleanup"||delivery?.get("workspace")is JsonObject),onClick={if(kind=="cleanup"){model.command(RovaiOperation.MISSIONS_WORKSPACE_CLEANUP,buildJsonObject{put("missionId",id)});confirmation=null}else{busy=true;coroutine.launch{try{val current=model.query(RovaiOperation.THREADS_OPEN,buildJsonObject{put("threadId",threadId);put("traceId",UUID.randomUUID().toString())}).coreObject();require(generation==model.connectionGeneration){"工作区已切换"};model.command(RovaiOperation.THREADS_DELETE,buildJsonObject{put("threadId",threadId);put("expectedVersion",current["thread"].coreObject().coreLong("version"));put("force",true);put("workspaceDisposition",if(cleanup)"cleanup" else "retain")});confirmation=null;onDeleted()}catch(error:Exception){issue=error.message}finally{busy=false}}}}){Text(if(kind=="delete")"确认删除使命" else "确认清理")}},dismissButton={TextButton(enabled=!busy,onClick={confirmation=null}){Text(stringResource(R.string.rovai_fusion_2cd0f3be8738a86c))}})}
}
