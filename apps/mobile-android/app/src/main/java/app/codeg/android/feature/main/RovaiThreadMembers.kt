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
import java.util.UUID

/** Original Camp member inspector; Core owns roster, exact lifetimes and removal impact. */
@Composable fun RovaiThreadMembers(model:RovaiWorkspaceViewModel,threadId:String,onClose:()->Unit){
    val state=model.state.collectAsState().value;val scope=rememberCoroutineScope()
    val generation=remember(threadId){model.connectionGeneration}
    var snapshot by remember(threadId){mutableStateOf<JsonObject?>(null)}
    var search by remember(threadId){mutableStateOf("")}
    var busy by remember(threadId){mutableStateOf(false)}
    var issue by remember(threadId){mutableStateOf<String?>(null)}
    var confirmation by remember(threadId){mutableStateOf<JsonObject?>(null)}
    var submitted by remember(threadId){mutableStateOf<String?>(null)}
    val blocked=busy||state.busy||state.pending!=null||generation!=model.connectionGeneration
    val members=snapshot?.get("members").coreArray().filter{it.coreText("membershipStatus")=="active"}
    val present=members.count{it.coreText("profilePresence")=="present"&&(it["leaveRequestedAt"]==null||it["leaveRequestedAt"]==JsonNull)}
    val active=members.map{it.coreText("agentId")}.toSet()
    val candidates=state.members.filter{it.coreText("presence")=="present"&&it.coreText("agentId") !in active&&(it.coreText("displayName")+" "+it.coreText("teamRole")).contains(search,true)}
    suspend fun load(){
        busy=true
        try{val value=model.query(RovaiOperation.THREADS_OPEN,buildJsonObject{put("threadId",threadId);put("traceId",UUID.randomUUID().toString())}).coreObject();require(generation==model.connectionGeneration){"工作区已切换"};snapshot=value;issue=null}
        catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}
    }
    fun prepared(kind:String,command:JsonObject,preview:JsonObject?=null)=buildJsonObject{put("kind",kind);put("commandId",UUID.randomUUID().toString());put("command",command);if(preview!=null)put("preview",preview)}
    LaunchedEffect(threadId,state.resourceVersion,state.lastResult){load()}
    LaunchedEffect(state.lastResult){
        val receipt=state.lastResult.coreObject()["commandResult"].coreObject().ifEmpty{state.lastResult.coreObject()}
        if(submitted!=null&&receipt.coreText("commandId")==submitted){
            if(receipt.coreText("status")in listOf("applied","accepted")){confirmation=null;submitted=null;load()}
            else if(receipt.coreText("status")=="rejected"){confirmation=null;submitted=null;issue=receipt.coreText("code")+"；请重新读取名册或移出影响后再确认。"}
        }
    }
    Dialog(onDismissRequest={if(!busy)onClose()},properties=DialogProperties(usePlatformDefaultWidth=false,dismissOnClickOutside=false)){
        Surface(Modifier.fillMaxSize(),color=CodegTheme.colors.bgElevated){Column(Modifier.fillMaxSize()){
            Row(Modifier.fillMaxWidth().padding(12.dp)){Text("会话队员",Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(enabled=!busy,onClick=onClose){Text("关闭")}}
            LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(10.dp)){
                item{issue?.let{Text(it,color=CodegTheme.colors.danger)};Text("$present 位在队 · ${members.size-present} 位暂离",color=CodegTheme.colors.textSecondary);TextButton(enabled=!blocked,onClick={scope.launch{load()}}){Text("重新读取名册")}}
                items(snapshot?.get("membershipReconciliations").coreArray(),key={it.coreText("id")}){value->Text("正在收拢已开始工作 · ${value.coreLong("settledRunCount")}/${value.coreLong("targetRunCount")}；新消息和工具写入已停止。",color=CodegTheme.colors.textSecondary)}
                items(members,key={it.coreText("agentId")}){member->GlassRow(Modifier.fillMaxWidth()){
                    val agentId=member.coreText("agentId");val profile=state.members.firstOrNull{it.coreText("agentId")==agentId};val lead=member["isDefaultLead"]==JsonPrimitive(true)
                    Text(member.coreText("displayName")+if(lead)" · 队长" else "",style=MaterialTheme.typography.titleSmall)
                    Text(member.coreText("teamRole"),color=CodegTheme.colors.textSecondary)
                    Text(if(member["leaveRequestedAt"]is JsonPrimitive&&member["leaveRequestedAt"]!=JsonNull)"正在暂离" else if(member.coreText("profilePresence")=="present")"在队" else "暂离",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)
                    profile?.let{Text(it["runtime"].coreObject().coreText("adapterKind").ifEmpty{"未选择运行程序"},style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}
                    (member["fast"]as?JsonObject)?.let{fast->Row(Modifier.fillMaxWidth()){
                        Column(Modifier.weight(1f)){Text("Fast");Text("用于之后的新执行；实际档位以运行程序反馈为准。",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}
                        val checked=(fast["fastOverride"]as?JsonPrimitive)?.booleanOrNull?:(fast["runtimeDefaultFast"]as?JsonPrimitive)?.booleanOrNull?:false
                        Switch(checked=checked,enabled=!blocked&&fast.coreText("runtimeBindingRevision").isNotEmpty(),onCheckedChange={selected->model.command(RovaiOperation.THREADS_MEMBERS_FAST_SET,buildJsonObject{put("threadId",threadId);put("agentId",agentId);put("expectedRuntimeBindingRevision",fast["runtimeBindingRevision"]!!);put("fastOverride",selected)})})
                    }}
                    Row{
                        TextButton(enabled=!blocked&&!lead&&member.coreText("profilePresence")=="present"&&(member["leaveRequestedAt"]==null||member["leaveRequestedAt"]==JsonNull),onClick={confirmation=prepared("lead",buildJsonObject{put("threadId",threadId);put("successorAgentId",agentId);put("expectedVersion",snapshot?.get("thread").coreObject().coreLong("version"))},member)}){Text("设为队长")}
                        TextButton(enabled=!blocked&&members.size>1,onClick={busy=true;scope.launch{try{
                            val preview=model.query(RovaiOperation.THREADS_MEMBERS_REMOVALPREVIEW,buildJsonObject{put("threadId",threadId);put("agentId",agentId)}).coreObject();require(generation==model.connectionGeneration){"工作区已切换"}
                            confirmation=prepared("remove",buildJsonObject{put("threadId",threadId);put("agentId",agentId);put("expectedMembershipGeneration",preview.coreLong("membershipGeneration"));put("expectedMembershipVersion",preview.coreLong("membershipVersion"));if(preview["isDefaultLead"]==JsonPrimitive(true))put("replacementDefaultLeadAgentId",preview["nextDefaultLeadAgentId"]?:JsonNull)},preview)
                        }catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}){Text(if(members.size<=1)"至少保留一位队员" else "移出会话")}
                    }
                }}
                item{Text("邀请队员",style=MaterialTheme.typography.titleSmall);OutlinedTextField(search,{search=it},Modifier.fillMaxWidth(),label={Text("搜索姓名或职责")},singleLine=true);Text("加入将在后续新执行中生效；不会启动队员或恢复旧执行。",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}
                items(candidates,key={it.coreText("agentId")}){candidate->Row{Column(Modifier.weight(1f)){Text(candidate.coreText("displayName"));Text(candidate.coreText("teamRole"),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)};TextButton(enabled=!blocked&&snapshot!=null,onClick={confirmation=prepared("add",buildJsonObject{put("threadId",threadId);put("agentId",candidate.coreText("agentId"));put("expectedMembershipGeneration",snapshot?.get("thread").coreObject().coreLong("membershipGeneration"))},candidate)}){Text("邀请")}}}
                if(candidates.isEmpty())item{Text("没有符合条件的可邀请队员",color=CodegTheme.colors.textSecondary)}
            }
        }}
    }
    confirmation?.let{frozen->val kind=frozen.coreText("kind");val preview=frozen["preview"].coreObject();AlertDialog(onDismissRequest={if(!blocked)confirmation=null},title={Text(when(kind){"add"->"邀请${preview.coreText("displayName")}？";"lead"->"更换会话队长？";else->"将${preview.coreText("displayName")}移出会话？"})},text={Column(verticalArrangement=Arrangement.spacedBy(8.dp)){
        when(kind){
            "remove"->{Text("影响按Core当前名册读取：${preview.coreLong("nonTerminalAgentRunCount")}个执行、${preview.coreLong("openAssignedTaskCount")}项开放任务、${preview.coreLong("pendingDeliveryCount")+preview.coreLong("runningDeliveryCount")}份投递。");if(preview["isDefaultLead"]==JsonPrimitive(true)){val next=preview.coreText("nextDefaultLeadAgentId");Text("接任队长 · "+members.firstOrNull{it.coreText("agentId")==next}?.coreText("displayName").orEmpty().ifEmpty{"暂缺，等现有成员归队后恢复"})};Text("历史记录保留；旧参与期的执行停止，开放任务回到待分配。重新邀请不会恢复旧执行。");if(preview["removable"]!=JsonPrimitive(true))Text(preview.coreText("blockerCode"),color=CodegTheme.colors.danger)}
            "lead"->Text("将${preview.coreText("displayName")}设为本会话队长，不改变其他会话。")
            else->Text("邀请${preview.coreText("displayName")}加入当前会话。保存名册后不自动发消息或启动运行。")
        }
        if(state.pending!=null)Text("原请求结果待核对，可关闭本页后从工作区核对。",color=CodegTheme.colors.textSecondary)
    }},confirmButton={TextButton(enabled=!blocked&&submitted==null&&(kind!="remove"||preview["removable"]==JsonPrimitive(true)),onClick={submitted=frozen.coreText("commandId");model.execute(when(kind){"add"->RovaiOperation.THREADS_MEMBERS_ADD;"lead"->RovaiOperation.THREADS_CHANGEDEFAULTLEAD;else->RovaiOperation.THREADS_MEMBERS_REMOVE},buildJsonObject{put("commandId",frozen.coreText("commandId"));put("command",frozen["command"]!!)} )}){Text(if(submitted!=null)"原请求待核对" else "确认")}},dismissButton={TextButton(enabled=!busy,onClick={confirmation=null}){Text("关闭")}})}
}
