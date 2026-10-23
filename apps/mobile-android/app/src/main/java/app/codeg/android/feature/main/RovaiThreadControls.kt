package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

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
        TextButton(onClick={managingMembers=true}){Text(stringResource(R.string.rovai_fusion_adfcecb7ef86f7ab))}
        RovaiExecutionMetrics(model,threadId,thread)
        thread["approvals"].coreArray().filter{it.coreText("status")=="pending"}.forEach{approval->GlassRow(Modifier.fillMaxWidth()){
            Text(stringResource(R.string.rovai_fusion_efb1c70e206655b8),style=MaterialTheme.typography.titleSmall);Text(approval.coreText("actionSummary"));if(approval.coreText("reason").isNotEmpty())Text(approval.coreText("reason"),color=CodegTheme.colors.textSecondary)
            approval["options"].coreArray().forEach{option->TextButton(enabled=state.pending==null&&!state.busy,onClick={model.execute(RovaiOperation.ACTION_APPROVALS_RESOLVE,buildJsonObject{put("commandId",UUID.randomUUID().toString());put("threadId",threadId);put("approvalId",approval.coreText("id"));put("expectedVersion",approval.coreLong("version"));put("optionId",option.coreText("optionId"));put("reason","用户选择智能体原生选项：${option.coreText("optionId")}。")})}){Column{Text(option.coreText("label"));Text(option.coreText("consequence"),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}}}
        }}
        thread["agentRuns"].coreArray().map{it.coreText("conversationId")}.filter{it.isNotEmpty()}.distinct().forEach{conversation->key(threadId,conversation){RovaiInteractionCards(model,state,conversation);RovaiDelegationList(model,threadId,conversation)}}
        Row{Text(stringResource(R.string.rovai_fusion_5253040db8643c85),Modifier.weight(1f),style=MaterialTheme.typography.titleSmall);TextButton(enabled=state.pending==null,onClick={creating=true}){Text(stringResource(R.string.rovai_fusion_6bee2372805a2fd7))}}
        thread["tasks"].coreArray().forEach{task->GlassRow(Modifier.fillMaxWidth()){
            Row{Text(task.coreText("title"),Modifier.weight(1f),style=MaterialTheme.typography.titleSmall);Text(taskStatusName(task.coreText("status")),style=MaterialTheme.typography.labelSmall,color=CodegTheme.colors.textSecondary)}
            Text(task.coreText("description"));if(task.coreText("blockedReason").isNotEmpty())Text(task.coreText("blockedReason"),color=CodegTheme.colors.danger);if(task.coreText("completionSummary").isNotEmpty())MarkdownContent(task.coreText("completionSummary"));TextButton(enabled=state.pending==null,onClick={editing=task}){Text(stringResource(R.string.rovai_fusion_56b3909705f355fc))}
        }}
    }
    if(creating||editing!=null)RovaiTaskEditor(model,threadId,editing,projection=thread){creating=false;editing=null}
}

@Composable fun RovaiInteractionCards(model:RovaiWorkspaceViewModel,state:RovaiWorkspaceState,conversationId:String){
    var values by remember(conversationId){mutableStateOf<List<JsonObject>>(emptyList())};var issue by remember{mutableStateOf<String?>(null)};val coroutine=rememberCoroutineScope();var cursor by remember{mutableStateOf<String?>(null)}
    suspend fun load(after:String?=null){try{val page=model.query(RovaiOperation.RUNTIME_INTERACTIONS_LIST,buildJsonObject{put("conversationId",conversationId);put("limit",50);if(after!=null)put("cursor",after)}).coreObject();val details=page["items"].coreArray().filter{it.coreText("status")=="pending"}.mapNotNull{summary->val value=model.query(RovaiOperation.RUNTIME_INTERACTIONS_GET,buildJsonObject{put("interactionId",summary.coreText("id"))}).coreObject();value.takeIf{it.coreText("conversationId")==conversationId&&it.coreText("status")=="pending"}};values=if(after==null)details else values+details;cursor=(page["nextCursor"] as? JsonPrimitive)?.contentOrNull;issue=null}catch(error:Exception){issue=error.message}}
    LaunchedEffect(conversationId,state.thread,state.lastResult){load()}
    issue?.let{Text(it,color=CodegTheme.colors.danger,style=MaterialTheme.typography.bodySmall)}
    values.forEach{interaction->key(interaction.coreText("id"),interaction.coreLong("version")){RovaiInteractionCard(interaction,state.pending!=null||state.busy){answer->model.command(RovaiOperation.RUNTIME_INTERACTIONS_RESOLVE,buildJsonObject{put("interactionId",interaction.coreText("id"));put("expectedVersion",interaction.coreLong("version"));put("answer",answer)})}}}
    cursor?.let{after->TextButton(onClick={coroutine.launch{load(after)}}){Text(stringResource(R.string.rovai_fusion_739e12e08dbbab34))}}
}

@Composable private fun RovaiInteractionCard(value:JsonObject,blocked:Boolean,onAnswer:(JsonObject)->Unit){
    val body=value["body"].coreObject();var feedback by remember{mutableStateOf("")};var answers by remember{mutableStateOf<Map<String,Set<String>>>(emptyMap())};var custom by remember{mutableStateOf<Map<String,String>>(emptyMap())}
    GlassRow(Modifier.fillMaxWidth()){
        if(value.coreText("kind")=="plan_approval"){
            Text(stringResource(R.string.rovai_fusion_46196ae1e6b1fde7),style=MaterialTheme.typography.titleSmall);MarkdownContent(body.coreText("planMarkdown"));OutlinedTextField(feedback,{feedback=it},label={Text(stringResource(R.string.rovai_fusion_5bb844c185179b88))},modifier=Modifier.fillMaxWidth(),enabled=!blocked)
            Row{listOf("approve" to "同意计划","request_changes" to "要求修改","abandon" to "放弃计划").forEach{(decision,label)->TextButton(enabled=!blocked,onClick={onAnswer(buildJsonObject{put("kind","plan_approval");put("decision",decision);put("feedback",feedback.takeIf{it.isNotBlank()}?.let(::JsonPrimitive)?:JsonNull)})}){Text(label)}}}
        }else{
            Text(stringResource(R.string.rovai_fusion_73247c497f9507a6),style=MaterialTheme.typography.titleSmall)
            body["questions"].coreArray().forEach{question->val id=question.coreText("id");Text(question.coreText("question"));question["options"].coreArray().forEach{option->val label=option.coreText("label");Row{Checkbox(checked=label in answers[id].orEmpty(),enabled=!blocked,onCheckedChange={checked->val old=answers[id].orEmpty();answers=answers+(id to if(checked){if(question["multiSelect"]==JsonPrimitive(true))old+label else setOf(label)}else old-label)});Column{Text(label);Text(option.coreText("description"),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}}};OutlinedTextField(custom[id].orEmpty(),{custom=custom+(id to it)},label={Text(stringResource(R.string.rovai_fusion_c19264da383f95a8))},modifier=Modifier.fillMaxWidth(),enabled=!blocked)}
            fun answer(declined:Boolean)=buildJsonObject{put("kind","question");put("declined",declined);putJsonArray("answers"){if(!declined)body["questions"].coreArray().forEach{question->val id=question.coreText("id");add(buildJsonObject{put("questionId",id);put("labels",JsonArray((answers[id].orEmpty()+listOfNotNull(custom[id]?.takeIf{it.isNotBlank()})).map(::JsonPrimitive)))})}}}
            Row{TextButton(enabled=!blocked,onClick={onAnswer(answer(false))}){Text(stringResource(R.string.rovai_fusion_0c1762dddc4faa68))};TextButton(enabled=!blocked,onClick={onAnswer(answer(true))}){Text(stringResource(R.string.rovai_fusion_2f330c03f5845711))}}
        }
    }
}
