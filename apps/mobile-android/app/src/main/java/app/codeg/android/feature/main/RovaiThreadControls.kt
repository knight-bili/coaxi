package app.codeg.android.feature.main

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.markdown.MarkdownContent
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import java.util.UUID

/** Native cards retain Core's original option IDs, versions and receipt path. */
@Composable
fun RovaiThreadControls(model:RovaiWorkspaceViewModel,state:RovaiWorkspaceState){
    val threadId=state.selectedThread?:return;val thread=state.thread?:return
    var editing by remember(threadId){mutableStateOf<JsonObject?>(null)};var creating by remember(threadId){mutableStateOf(false)}
    var managingMembers by remember(threadId){mutableStateOf(false)}
    if(managingMembers)RovaiThreadMembers(model,threadId){managingMembers=false}
    Column(verticalArrangement=Arrangement.spacedBy(10.dp)){
        TextButton(onClick={managingMembers=true}){Text("会话队员与队长")}
        thread["approvals"].coreArray().filter{it.coreText("status")=="pending"}.forEach{approval->GlassRow(Modifier.fillMaxWidth()){
            Text("需要你的许可",style=MaterialTheme.typography.titleSmall);Text(approval.coreText("actionSummary"));if(approval.coreText("reason").isNotEmpty())Text(approval.coreText("reason"),color=CodegTheme.colors.textSecondary)
            approval["options"].coreArray().forEach{option->TextButton(enabled=state.pending==null&&!state.busy,onClick={model.execute(RovaiOperation.ACTION_APPROVALS_RESOLVE,buildJsonObject{put("commandId",UUID.randomUUID().toString());put("threadId",threadId);put("approvalId",approval.coreText("id"));put("expectedVersion",approval.coreLong("version"));put("optionId",option.coreText("optionId"));put("reason","用户选择智能体原生选项：${option.coreText("optionId")}。")})}){Column{Text(option.coreText("label"));Text(option.coreText("consequence"),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}}}
        }}
        thread["agentRuns"].coreArray().map{it.coreText("conversationId")}.filter{it.isNotEmpty()}.distinct().forEach{conversation->key(threadId,conversation){RovaiInteractionCards(model,state,conversation);RovaiDelegationList(model,threadId,conversation)}}
        Row{Text("任务",Modifier.weight(1f),style=MaterialTheme.typography.titleSmall);TextButton(enabled=state.pending==null,onClick={creating=true}){Text("新建任务")}}
        thread["tasks"].coreArray().forEach{task->GlassRow(Modifier.fillMaxWidth()){
            Row{Text(task.coreText("title"),Modifier.weight(1f),style=MaterialTheme.typography.titleSmall);Text(task.coreText("status"),style=MaterialTheme.typography.labelSmall,color=CodegTheme.colors.textSecondary)}
            Text(task.coreText("description"));if(task.coreText("blockedReason").isNotEmpty())Text(task.coreText("blockedReason"),color=CodegTheme.colors.danger);if(task.coreText("completionSummary").isNotEmpty())MarkdownContent(task.coreText("completionSummary"));TextButton(enabled=state.pending==null,onClick={editing=task}){Text("编辑任务")}
        }}
    }
    if(creating||editing!=null)RovaiTaskEditor(threadId,editing,thread["members"].coreArray(),onDismiss={creating=false;editing=null}){params->model.execute(if(editing==null)RovaiOperation.TASKS_CREATE else RovaiOperation.TASKS_UPDATE,params);creating=false;editing=null}
}

@Composable fun RovaiInteractionCards(model:RovaiWorkspaceViewModel,state:RovaiWorkspaceState,conversationId:String){
    var values by remember(conversationId){mutableStateOf<List<JsonObject>>(emptyList())};var issue by remember{mutableStateOf<String?>(null)};val coroutine=rememberCoroutineScope();var cursor by remember{mutableStateOf<String?>(null)}
    suspend fun load(after:String?=null){try{val page=model.query(RovaiOperation.RUNTIME_INTERACTIONS_LIST,buildJsonObject{put("conversationId",conversationId);put("limit",50);if(after!=null)put("cursor",after)}).coreObject();val details=page["items"].coreArray().filter{it.coreText("status")=="pending"}.mapNotNull{summary->val value=model.query(RovaiOperation.RUNTIME_INTERACTIONS_GET,buildJsonObject{put("interactionId",summary.coreText("id"))}).coreObject();value.takeIf{it.coreText("conversationId")==conversationId&&it.coreText("status")=="pending"}};values=if(after==null)details else values+details;cursor=(page["nextCursor"] as? JsonPrimitive)?.contentOrNull;issue=null}catch(error:Exception){issue=error.message}}
    LaunchedEffect(conversationId,state.thread,state.lastResult){load()}
    issue?.let{Text(it,color=CodegTheme.colors.danger,style=MaterialTheme.typography.bodySmall)}
    values.forEach{interaction->key(interaction.coreText("id"),interaction.coreLong("version")){RovaiInteractionCard(interaction,state.pending!=null||state.busy){answer->model.command(RovaiOperation.RUNTIME_INTERACTIONS_RESOLVE,buildJsonObject{put("interactionId",interaction.coreText("id"));put("expectedVersion",interaction.coreLong("version"));put("answer",answer)})}}}
    cursor?.let{after->TextButton(onClick={coroutine.launch{load(after)}}){Text("更多待回答事项")}}
}

@Composable private fun RovaiInteractionCard(value:JsonObject,blocked:Boolean,onAnswer:(JsonObject)->Unit){
    val body=value["body"].coreObject();var feedback by remember{mutableStateOf("")};var answers by remember{mutableStateOf<Map<String,Set<String>>>(emptyMap())};var custom by remember{mutableStateOf<Map<String,String>>(emptyMap())}
    GlassRow(Modifier.fillMaxWidth()){
        if(value.coreText("kind")=="plan_approval"){
            Text("计划待确认",style=MaterialTheme.typography.titleSmall);MarkdownContent(body.coreText("planMarkdown"));OutlinedTextField(feedback,{feedback=it},label={Text("反馈（可选）")},modifier=Modifier.fillMaxWidth(),enabled=!blocked)
            Row{listOf("approve" to "同意计划","request_changes" to "要求修改","abandon" to "放弃计划").forEach{(decision,label)->TextButton(enabled=!blocked,onClick={onAnswer(buildJsonObject{put("kind","plan_approval");put("decision",decision);put("feedback",feedback.takeIf{it.isNotBlank()}?.let(::JsonPrimitive)?:JsonNull)})}){Text(label)}}}
        }else{
            Text("队员需要你的回答",style=MaterialTheme.typography.titleSmall)
            body["questions"].coreArray().forEach{question->val id=question.coreText("id");Text(question.coreText("question"));question["options"].coreArray().forEach{option->val label=option.coreText("label");Row{Checkbox(checked=label in answers[id].orEmpty(),enabled=!blocked,onCheckedChange={checked->val old=answers[id].orEmpty();answers=answers+(id to if(checked){if(question["multiSelect"]==JsonPrimitive(true))old+label else setOf(label)}else old-label)});Column{Text(label);Text(option.coreText("description"),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}}};OutlinedTextField(custom[id].orEmpty(),{custom=custom+(id to it)},label={Text("补充回答")},modifier=Modifier.fillMaxWidth(),enabled=!blocked)}
            fun answer(declined:Boolean)=buildJsonObject{put("kind","question");put("declined",declined);putJsonArray("answers"){if(!declined)body["questions"].coreArray().forEach{question->val id=question.coreText("id");add(buildJsonObject{put("questionId",id);put("labels",JsonArray((answers[id].orEmpty()+listOfNotNull(custom[id]?.takeIf{it.isNotBlank()})).map(::JsonPrimitive)))})}}}
            Row{TextButton(enabled=!blocked,onClick={onAnswer(answer(false))}){Text("提交回答")};TextButton(enabled=!blocked,onClick={onAnswer(answer(true))}){Text("暂不回答")}}
        }
    }
}

@Composable private fun RovaiTaskEditor(threadId:String,task:JsonObject?,members:List<JsonObject>,onDismiss:()->Unit,onSave:(JsonObject)->Unit){
    var title by remember{mutableStateOf(task?.coreText("title")?:"")};var description by remember{mutableStateOf(task?.coreText("description")?:"")};var assignee by remember{mutableStateOf(task?.coreText("assigneeAgentId")?:members.firstOrNull()?.coreText("agentId").orEmpty())};var status by remember{mutableStateOf(task?.coreText("status")?:"pending")};var reason by remember{mutableStateOf("")}
    AlertDialog(onDismissRequest=onDismiss,title={Text(if(task==null)"新建任务" else "编辑任务")},text={LazyColumn(verticalArrangement=Arrangement.spacedBy(8.dp)){item{OutlinedTextField(title,{title=it},label={Text("任务名称")})};item{OutlinedTextField(description,{description=it},label={Text("说明")},maxLines=5)};item{members.forEach{member->FilterChip(selected=assignee==member.coreText("agentId"),onClick={assignee=member.coreText("agentId")},label={Text(member.coreText("displayName"))})}};if(task!=null){item{listOf("pending","in_progress","blocked","completed","cancelled").forEach{value->FilterChip(selected=status==value,onClick={status=value},label={Text(value)})}};item{OutlinedTextField(reason,{reason=it},label={Text("完成说明 / 阻塞或取消原因")},maxLines=4)}}}},confirmButton={TextButton(enabled=title.isNotBlank()&&assignee.isNotBlank(),onClick={onSave(buildJsonObject{put("commandId",UUID.randomUUID().toString());put("threadId",threadId);put("title",title);put("description",description);if(task==null)put("assigneeAgentId",assignee)else{put("taskId",task.coreText("taskId"));put("status",status);putJsonObject("assignee"){put("operation","assign");put("agentId",assignee)};if(status=="blocked")put("blockedReason",reason);if(status=="completed")put("completionSummary",reason);if(status=="cancelled")put("cancelReason",reason)}})}){Text("保存")}},dismissButton={TextButton(onClick=onDismiss){Text("取消")}})
}
