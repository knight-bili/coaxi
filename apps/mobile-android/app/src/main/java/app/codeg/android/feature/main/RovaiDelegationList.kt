package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*

private data class HelperAction(val operation:RovaiOperation,val command:JsonObject,val title:String,val detail:String)
@Composable
fun RovaiDelegationList(model:RovaiWorkspaceViewModel,threadId:String,parentConversationId:String){
    val state by model.state.collectAsStateWithLifecycle();val scope=rememberCoroutineScope()
    var helpers by remember(parentConversationId){mutableStateOf<List<JsonObject>>(emptyList())};var cursor by remember(parentConversationId){mutableStateOf<String?>(null)}
    var issue by remember{mutableStateOf<String?>(null)};var opened by remember{mutableStateOf<JsonObject?>(null)};var action by remember{mutableStateOf<HelperAction?>(null)};var loading by remember{mutableStateOf(false)}
    suspend fun load(after:String?=null){if(loading)return;loading=true;try{val page=model.query(RovaiOperation.DELEGATIONS_LIST,buildJsonObject{put("parentConversationId",parentConversationId);put("limit",50);if(after!=null)put("cursor",after)}).coreObject();val values=page["items"].coreArray().filter{it.coreText("threadId")==threadId&&it.coreText("parentConversationId")==parentConversationId};helpers=if(after==null)values else (helpers+values).distinctBy{it.coreText("delegationId")};cursor=(page["nextCursor"] as? JsonPrimitive)?.contentOrNull;issue=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{loading=false}}
    LaunchedEffect(parentConversationId,state.thread,state.lastResult){load()}
    val disabled=state.pending!=null||state.busy||loading
    Column(verticalArrangement=Arrangement.spacedBy(8.dp)){
        issue?.let{Text(it,color=CodegTheme.colors.danger)}
        helpers.forEach{helper->key(helper.coreText("delegationId")){GlassRow(Modifier.fillMaxWidth()){
            Text(stringResource(R.string.rovai_fusion_5c171fe232ed4688)+helper.coreText("agentType"),style=MaterialTheme.typography.titleSmall);Text(helper.coreText("status")+" · 第 ${helper.coreLong("currentAttemptNo")} 次",color=CodegTheme.colors.textSecondary)
            Row(horizontalArrangement=Arrangement.spacedBy(6.dp)){
                TextButton(enabled=!loading,onClick={opened=helper}){Text(stringResource(R.string.rovai_fusion_db8db0530432bd15))}
                if(helper["takenOver"]!=JsonPrimitive(true))TextButton(enabled=!disabled,onClick={action=HelperAction(RovaiOperation.DELEGATIONS_TAKEOVER,buildJsonObject{put("delegationId",helper.coreText("delegationId"));put("expectedControlVersion",helper.coreLong("controlVersion"))},"接管独立帮手","接管后由你直接发消息，原队员不再继续控制这位帮手。历史记录保留。")}){Text(stringResource(R.string.rovai_fusion_ea878d5d090510da))}
                if(helper.coreText("status") in listOf("queued","running","waiting","waiting_permission","waiting_question","waiting_plan_approval"))TextButton(enabled=!disabled,onClick={action=HelperAction(RovaiOperation.DELEGATIONS_CANCEL,buildJsonObject{put("delegationId",helper.coreText("delegationId"));put("attemptNo",helper.coreLong("currentAttemptNo"))},"停止这次帮手执行","停止当前尝试，保留记录和实际输出。")}){Text(stringResource(R.string.rovai_fusion_ca4d973c0b006b75))}
                else TextButton(enabled=!disabled,onClick={action=HelperAction(RovaiOperation.DELEGATIONS_RESUME,buildJsonObject{put("delegationId",helper.coreText("delegationId"));put("expectedAttemptNo",helper.coreLong("currentAttemptNo"));put("reason","用户从移动客户端请求继续独立帮手。")},"继续独立帮手","使用Core保存的材料开始下一次尝试，已完成的尝试保留。")}){Text(stringResource(R.string.rovai_fusion_7c9691192f1b7340))}
            }
        }}}
        cursor?.let{after->TextButton(enabled=!loading,onClick={scope.launch{load(after)}}){Text(stringResource(R.string.rovai_fusion_18496e832ae3c378))}}
    }
    action?.let{target->AlertDialog(onDismissRequest={if(!disabled)action=null},title={Text(target.title)},text={Text(target.detail)},confirmButton={TextButton(enabled=!disabled,onClick={model.command(target.operation,target.command);action=null}){Text(stringResource(R.string.rovai_fusion_36f33adaf0942634))}},dismissButton={TextButton(enabled=!disabled,onClick={action=null}){Text(stringResource(R.string.rovai_fusion_2cd0f3be8738a86c))}})}
    opened?.let{target->Dialog(onDismissRequest={opened=null},properties=DialogProperties(usePlatformDefaultWidth=false,dismissOnBackPress=false,dismissOnClickOutside=false)){Surface(Modifier.fillMaxSize(),color=CodegTheme.colors.bgElevated){RovaiPrivateConversation(model,threadId,target.coreText("conversationId")){opened=null}}}}
}
