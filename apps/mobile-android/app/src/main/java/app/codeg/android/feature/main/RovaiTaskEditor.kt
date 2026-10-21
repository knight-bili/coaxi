package app.codeg.android.feature.main

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.util.UUID

val rovaiTaskStatuses=listOf("pending" to "待处理","in_progress" to "进行中","blocked" to "已阻塞","completed" to "已完成","cancelled" to "已取消")
fun taskStatusName(value:String)=rovaiTaskStatuses.firstOrNull{it.first==value}?.second?:value

/** TaskPanel's sparse field patches and terminal rules; tasks have no object-version field. */
@Composable fun RovaiTaskEditor(model:RovaiWorkspaceViewModel,threadId:String,task:JsonObject?,projection:JsonObject?=null,onClose:()->Unit){
    val state=model.state.collectAsState().value;val scope=rememberCoroutineScope();val generation=remember(threadId){model.connectionGeneration}
    val key=task?.coreText("taskId")?:"new";val owner="task-edit:$threadId:$key"
    var baseline by remember(key){mutableStateOf(task)}
    var title by remember(key){mutableStateOf(task?.coreText("title").orEmpty())};var detail by remember(key){mutableStateOf(task?.coreText("description").orEmpty())};var assignee by remember(key){mutableStateOf(task?.coreText("assigneeAgentId").orEmpty())}
    var status by remember(key){mutableStateOf(task?.coreText("status")?:"pending")};var blockedReason by remember(key){mutableStateOf(task?.coreText("blockedReason").orEmpty())};var completion by remember(key){mutableStateOf(task?.coreText("completionSummary").orEmpty())};var cancelReason by remember(key){mutableStateOf("")}
    var commandId by remember(key){mutableStateOf(UUID.randomUUID().toString())};var ready by remember(key){mutableStateOf(false)};var busy by remember(key){mutableStateOf(false)};var cancelling by remember(key){mutableStateOf(false)};var issue by remember(key){mutableStateOf<String?>(null)}
    val source=projection?:state.thread?.takeIf{it["thread"].coreObject().coreText("id")==threadId}
    val currentTask=source?.get("tasks").coreArray().firstOrNull{it.coreText("taskId")==key}
    val terminal=baseline?.coreText("status")in listOf("completed","cancelled")||currentTask?.coreText("status")in listOf("completed","cancelled")
    val locked=!ready||terminal||busy||state.busy||state.pending!=null||generation!=model.connectionGeneration
    val members=source?.get("members").coreArray().filter{it.coreText("membershipStatus")=="active"}
    fun record()=buildJsonObject{put("version",1);put("threadId",threadId);put("taskKey",key);put("baseline",baseline?:JsonNull);put("title",title);put("description",detail);put("assignee",assignee);put("status",status);put("blockedReason",blockedReason);put("completionSummary",completion);put("cancelReason",cancelReason);put("commandId",commandId)}.toString()
    fun persist(){if(ready&&generation==model.connectionGeneration)model.rememberBody(owner,record())}
    fun close(){if(busy)return;if(!ready||generation!=model.connectionGeneration){onClose();return};val saved=record();scope.launch{try{model.saveBody(owner,saved);model.rememberBody(owner,saved);onClose()}catch(error:Exception){issue=error.message}}}
    fun patch(cancel:Boolean)=buildJsonObject{
        put("commandId",commandId);put("threadId",threadId)
        if(task==null){put("title",title.trim());put("description",detail.trim());put("assigneeAgentId",assignee)}
        else{put("taskId",key);if(cancel){put("status","cancelled");put("cancelReason",cancelReason.trim())}else{
            if(title.trim()!=baseline?.coreText("title"))put("title",title.trim())
            if(detail.trim()!=baseline?.coreText("description"))put("description",detail.trim())
            if(status!=baseline?.coreText("status"))put("status",status)
            if(assignee!=baseline?.coreText("assigneeAgentId"))putJsonObject("assignee"){if(assignee.isEmpty())put("operation","clear")else{put("operation","assign");put("agentId",assignee)}}
            if(status=="blocked"&&blockedReason.trim()!=baseline?.coreText("blockedReason"))put("blockedReason",blockedReason.trim())
            if(status=="completed"&&completion.trim()!=baseline?.coreText("completionSummary"))put("completionSummary",completion.trim())
        }}
    }
    val valid=title.trim().isNotEmpty()&&title.trim().codePointCount(0,title.trim().length)<=160&&detail.codePointCount(0,detail.length)<=16000&&blockedReason.codePointCount(0,blockedReason.length)<=4000&&completion.codePointCount(0,completion.length)<=4000&&(task!=null||assignee.isNotEmpty())&&(status !in listOf("in_progress","blocked")||assignee.isNotEmpty())&&(status!="blocked"||blockedReason.isNotBlank())&&(status!="completed"||completion.isNotBlank())
    fun submit(cancel:Boolean){if(locked)return;val request=patch(cancel);if(!cancel&&task!=null&&request.size<=3){issue="没有需要提交的修改";return};val saved=record();busy=true;scope.launch{try{model.saveBody(owner,saved);model.rememberBody(owner,saved);require(generation==model.connectionGeneration);model.executeOwned(if(task==null)RovaiOperation.TASKS_CREATE else RovaiOperation.TASKS_UPDATE,request,owner,saved)}catch(error:Exception){issue=error.message}finally{busy=false}}}
    LaunchedEffect(key,generation){try{val raw=model.readBody(owner);if(raw.isNotEmpty()){
        val value=Json.parseToJsonElement(raw).coreObject();require(value.coreLong("version")==1L&&value.coreText("threadId")==threadId&&value.coreText("taskKey")==key){"原任务草稿无法读取，原记录保留"};baseline=value["baseline"]as?JsonObject;title=value.coreText("title");detail=value.coreText("description");assignee=value.coreText("assignee");status=value.coreText("status");blockedReason=value.coreText("blockedReason");completion=value.coreText("completionSummary");cancelReason=value.coreText("cancelReason");commandId=value.coreText("commandId").ifEmpty{UUID.randomUUID().toString()}
    };ready=true}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}
    LaunchedEffect(state.lastResult){val result=state.lastResult.coreObject()["commandResult"].coreObject().ifEmpty{state.lastResult.coreObject()};if(state.lastInputOwner==owner&&result.coreText("commandId")==commandId){if(result.coreText("status")in listOf("applied","accepted")){ready=false;onClose()}else if(result.coreText("status")=="rejected"){commandId=UUID.randomUUID().toString();issue=result.coreText("code");persist()}}}
    Dialog(onDismissRequest={close()},properties=DialogProperties(usePlatformDefaultWidth=false,dismissOnBackPress=!busy,dismissOnClickOutside=false)){
        Surface(Modifier.fillMaxSize(),color=CodegTheme.colors.bgElevated){Column(Modifier.fillMaxSize()){
            Row(Modifier.fillMaxWidth().padding(12.dp)){Text(if(task==null)"新建任务" else "任务详情",Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(enabled=!busy,onClick={close()}){Text("保存草稿并关闭")}}
            LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(10.dp)){
                item{issue?.let{Text(it,color=CodegTheme.colors.danger)};if(state.pending!=null)Text("原请求待核对，完整草稿保留。",color=CodegTheme.colors.textSecondary);if(terminal)Text("终态任务不可修改。",color=CodegTheme.colors.textSecondary)}
                item{OutlinedTextField(title,{title=it;persist()},Modifier.fillMaxWidth(),enabled=!locked,label={Text("标题")});OutlinedTextField(detail,{detail=it;persist()},Modifier.fillMaxWidth(),enabled=!locked,label={Text("责任范围与要求")},minLines=5)}
                item{Text("负责人",style=MaterialTheme.typography.titleSmall);var open by remember{mutableStateOf(false)};Box{OutlinedButton(enabled=!locked,onClick={open=true}){Text(members.firstOrNull{it.coreText("agentId")==assignee}?.coreText("displayName")?:if(assignee.isEmpty())"未分配" else "原负责人已不可用")};DropdownMenu(expanded=open,onDismissRequest={open=false}){if(task!=null)DropdownMenuItem(text={Text("未分配")},onClick={assignee="";open=false;persist()});members.forEach{member->DropdownMenuItem(text={Text(member.coreText("displayName"))},onClick={assignee=member.coreText("agentId");open=false;persist()})}}}}
                if(task!=null)item{Text("状态",style=MaterialTheme.typography.titleSmall);rovaiTaskStatuses.filter{it.first!="cancelled"}.forEach{(key,label)->FilterChip(selected=status==key,enabled=!locked,onClick={status=key;persist()},label={Text(label)})}}
                if(status=="blocked")item{OutlinedTextField(blockedReason,{blockedReason=it;persist()},Modifier.fillMaxWidth(),enabled=!locked,label={Text("阻塞原因")},minLines=3)}
                if(status=="completed")item{OutlinedTextField(completion,{completion=it;persist()},Modifier.fillMaxWidth(),enabled=!locked,label={Text("完成摘要")},minLines=3)}
                baseline?.let{base->item{Text("创建 · "+base.coreText("createdAt")+" · "+base.coreText("createdByType"),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary);Text("更新 · "+base.coreText("updatedAt"),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary);if(base.coreText("closedAt").isNotEmpty())Text("关闭 · "+base.coreText("closedAt"),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary);if(base.coreText("cancelReason").isNotEmpty())Text("取消原因 · "+base.coreText("cancelReason"))}}
                if(task!=null){
                    item{Text("责任与审计",style=MaterialTheme.typography.titleSmall);Text("任务 · "+key,style=MaterialTheme.typography.bodySmall);Text("创建者 · "+baseline?.coreText("createdByType").orEmpty()+" · "+baseline?.coreText("createdById").orEmpty(),style=MaterialTheme.typography.bodySmall);Text("来源执行 · "+baseline?.coreText("sourceAgentRunId").orEmpty().ifEmpty{"无"},style=MaterialTheme.typography.bodySmall);Text("结束者 · "+baseline?.coreText("closedByType").orEmpty()+" · "+baseline?.coreText("closedById").orEmpty(),style=MaterialTheme.typography.bodySmall)}
                    val runs=source?.get("agentRuns").coreArray().filter{it.coreText("taskId")==key}
                    item{Text("关联执行",style=MaterialTheme.typography.titleSmall);Text("${runs.size}个执行 · ${source?.get("messageDeliveries").coreArray().count{it.coreText("taskId")==key}}份消息投递",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}
                    runs.forEach{run->item(key="task-run:"+run.coreText("id")){RovaiExecutionHistory(model,threadId,run,source?.get("executionEvidence").coreArray())}}
                }
                if(task!=null&&!terminal)item{TextButton(enabled=!locked,onClick={cancelling=true}){Text("取消任务",color=CodegTheme.colors.danger)}}
            }
            if(!terminal)Button(enabled=!locked&&valid,modifier=Modifier.fillMaxWidth().padding(12.dp),onClick={submit(false)}){Text(if(task==null)"创建任务" else "保存修改")}
        }}
    }
    if(cancelling)AlertDialog(onDismissRequest={if(!busy)cancelling=false},title={Text("取消这个任务？")},text={Column{Text("取消是终态操作；正文草稿中的其他修改不会随取消一起提交。");OutlinedTextField(cancelReason,{cancelReason=it;persist()},label={Text("取消原因")},enabled=!locked,minLines=3)}},confirmButton={TextButton(enabled=!locked&&cancelReason.isNotBlank()&&cancelReason.codePointCount(0,cancelReason.length)<=4000,onClick={submit(true)}){Text("确认取消")}},dismissButton={TextButton(enabled=!busy,onClick={cancelling=false}){Text("保留任务")}})
}
