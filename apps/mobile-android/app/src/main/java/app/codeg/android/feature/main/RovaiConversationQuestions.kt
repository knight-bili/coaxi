package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.model.QuestionAnswer
import app.codeg.android.core.model.QuestionOption
import app.codeg.android.core.model.QuestionSpec
import app.codeg.android.core.network.RovaiOperation
import app.codeg.android.feature.sessiondetail.interactive.AskQuestionCard
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import java.util.UUID

internal fun conversationScope(threadId:String,conversationId:String?,child:Boolean)=buildJsonObject{put("threadId",threadId);put("conversationId",conversationId?.let(::JsonPrimitive)?:JsonNull);put("scopeKind",if(conversationId==null)"public" else if(child)"delegation_child" else "single_chat")}
private fun questionStatus(value:String)=when(value){"awaiting_answer"->"等待你回答";"answered"->"答案已保存，等待原连接确认";"delivered","applied"->"已写回原队员连接";"cancelled"->"原问题已停止，记录保留";"outcome_unknown"->"原连接结果待核对，不自动重发";else->value}
/** 在原会话滚动容器中复用原Codeg问题卡；本地不保存答案正文。 */
@Composable fun RovaiConversationQuestions(model:RovaiWorkspaceViewModel,threadId:String,conversationId:String?=null,child:Boolean=false){
    val state by model.state.collectAsState();val scope=remember(threadId,conversationId,child){conversationScope(threadId,conversationId,child)};val generation=remember(scope){model.connectionGeneration};val coroutine=rememberCoroutineScope()
    var items by remember(scope){mutableStateOf<List<JsonObject>>(emptyList())};var next by remember(scope){mutableStateOf<String?>(null)};var issue by remember(scope){mutableStateOf<String?>(null)};var history by remember(scope){mutableStateOf(false)}
    suspend fun read(more:Boolean=false){val result=nativeUserCall(model,RovaiOperation.CONVERSATIONTOOLS_QUESTIONS_LIST,buildJsonObject{put("scope",scope);put("after",if(more)next?.let(::JsonPrimitive)?:JsonNull else JsonNull);put("limit",50)});require(generation==model.connectionGeneration);items=if(more)(items+result["items"].coreArray()).distinctBy{it.coreText("requestId")}else result["items"].coreArray();next=(result["next"] as? JsonPrimitive)?.contentOrNull;issue=null}
    LaunchedEffect(scope,generation){try{read()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}
    LaunchedEffect(scope,state.resourceVersion){delay(250);try{read()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}
    val active=items.filter{it.coreText("state") in listOf("awaiting_answer","answered","outcome_unknown")}
    if(items.isNotEmpty()||issue!=null)Column(verticalArrangement=Arrangement.spacedBy(10.dp)){
        active.forEach{row->key(row.coreText("requestId")){RovaiConversationQuestion(model,row){coroutine.launch{try{read()}catch(error:Exception){issue=error.message}}}}}
        if(items.size>active.size){TextButton(onClick={history=!history}){Text(if(history)"收起原问题记录" else "查看已结束的问题")};if(history)items.filter{it !in active}.forEach{row->key(row.coreText("requestId")){RovaiConversationQuestion(model,row){coroutine.launch{read()}}}}}
        if(next!=null)TextButton(onClick={coroutine.launch{try{read(true)}catch(error:Exception){issue=error.message}}}){Text(stringResource(R.string.rovai_fusion_9051db3a46418dcc))}
        issue?.let{Text(it,color=CodegTheme.colors.danger,style=MaterialTheme.typography.bodySmall)}
    }
}
@Composable private fun RovaiConversationQuestion(model:RovaiWorkspaceViewModel,row:JsonObject,onChanged:()->Unit){
    val id=row.coreText("requestId");val owner="conversation-question:$id";val generation=remember(id){model.connectionGeneration};val coroutine=rememberCoroutineScope()
    var pending by remember(id){mutableStateOf<JsonObject?>(null)};var result by remember(id){mutableStateOf<JsonObject?>(null)};var ready by remember(id){mutableStateOf(false)};var busy by remember(id){mutableStateOf(false)};var issue by remember(id){mutableStateOf<String?>(null)}
    val specs=remember(row){row["questions"].coreArray().map{question->val spec=question["spec"].coreObject();QuestionSpec(id=question.coreText("id"),question=spec.coreText("question"),header=spec.coreText("header"),multiSelect=spec["multiSelect"]==JsonPrimitive(true),options=spec["options"].coreArray().map{QuestionOption(it.coreText("label"),it.coreText("description"))})}}
    fun work(action:suspend()->Unit){if(busy)return;busy=true;issue=null;coroutine.launch{try{action()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(generation==model.connectionGeneration)issue=error.message}finally{if(generation==model.connectionGeneration)busy=false}}}
    suspend fun retain(value:JsonObject){model.saveBody(owner,value.toString());require(generation==model.connectionGeneration);pending=value}
    suspend fun clear(original:JsonObject){require(model.clearBodyIfMatches(owner,original.toString(),generation)){"原回答记录已变化"};pending=null}
    fun submit(answer:QuestionAnswer){if(!ready||pending!=null||busy)return;val operation=if(answer.declined)RovaiOperation.CONVERSATIONTOOLS_QUESTIONS_DECLINE else RovaiOperation.CONVERSATIONTOOLS_QUESTIONS_ANSWER;val command=UUID.randomUUID().toString();val original=buildJsonObject{put("operation",operation.wire);put("commandId",command);put("requestId",id)};val input=buildJsonObject{put("commandId",command);put("requestId",id);put("expectedRevision",row.coreLong("revision"));if(!answer.declined)put("answers",JsonArray(answer.answers.map{item->buildJsonObject{put("questionId",item.questionId);put("labels",JsonArray(item.labels.map(::JsonPrimitive)))}}))};work{retain(original);result=nativeUserCall(model,operation,input);clear(original);onChanged()}}
    fun reconcile(){work{pending?.let{original->val value=nativeUserCall(model,RovaiOperation.COMMANDS_RECONCILE,buildJsonObject{put("operation",original.coreText("operation"));putJsonObject("params"){put("commandId",original.coreText("commandId"))}});if(value.coreText("state")=="recorded"){val receipt=value["result"].coreObject();require(receipt.coreText("commandId")==original.coreText("commandId")&&receipt.coreText("commandType")==original.coreText("operation"));clear(original);if(receipt.coreText("status")=="rejected")issue=receipt.coreText("code")}else issue="原回答尚无确定回执，没有重复发送"};result=nativeUserCall(model,RovaiOperation.CONVERSATIONTOOLS_QUESTIONS_RECONCILE,buildJsonObject{put("requestId",id)});onChanged()}}
    fun cancel(){val original=buildJsonObject{put("operation","conversationTools.requests.resolve");put("commandId",UUID.randomUUID().toString());put("requestId",id)};work{retain(original);result=nativeUserCall(model,RovaiOperation.CONVERSATIONTOOLS_REQUESTS_RESOLVE,buildJsonObject{put("commandId",original.coreText("commandId"));put("requestId",id);put("action","cancel_remaining")});clear(original);onChanged()}}
    LaunchedEffect(id,generation){try{val raw=model.readBody(owner);if(raw.isNotEmpty()){require(raw.length<=1024);val original=Json.parseToJsonElement(raw).coreObject();require(original.keys==setOf("operation","commandId","requestId")&&original.coreText("requestId")==id&&original.coreText("operation") in listOf("conversationTools.questions.answer","conversationTools.questions.decline","conversationTools.requests.resolve"));UUID.fromString(original.coreText("commandId"));pending=original};ready=true}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}
    val current=if(row.coreText("state") in listOf("delivered","cancelled","outcome_unknown"))row.coreText("state")else result?.coreText("state")?.takeIf{it.isNotBlank()}?:row.coreText("state");val outcome=result?.get("outcome").coreObject();val saved=outcome["answers"].coreArray();val selections=specs.mapIndexed{index,question->question.id to ((saved.getOrNull(index)?.get("selected") as? JsonArray)?.mapNotNull{(it as? JsonPrimitive)?.contentOrNull}?:emptyList())}.toMap()
    Column(verticalArrangement=Arrangement.spacedBy(6.dp)){
        Text((model.state.value.members.firstOrNull{it.coreText("agentId")==row.coreText("agentId")}?.coreText("displayName")?:"原队员")+" · "+questionStatus(current),style=MaterialTheme.typography.labelMedium)
        key(id,row.coreLong("revision"),current){AskQuestionCard(questions=specs,onSubmit=::submit,onSkip={submit(QuestionAnswer.dismissed)},readOnly=!ready||busy||pending!=null||current!="awaiting_answer",initialSelections=selections,titleOverride=if(current=="awaiting_answer")"队员需要你的选择" else "原问题记录",subtitleOverride=questionStatus(current))}
        if(outcome["declined"]==JsonPrimitive(true))Text(stringResource(R.string.rovai_fusion_43c7b60762c11ca5),style=MaterialTheme.typography.bodySmall)
        Row{TextButton(enabled=!busy,onClick=::reconcile){Text(if(pending!=null)"核对原回答" else "查看原回执")};if(current in listOf("awaiting_answer","outcome_unknown"))TextButton(enabled=!busy,onClick=::cancel){Text(stringResource(R.string.rovai_fusion_aba882395b4545fe))}}
        issue?.let{Text(it,color=CodegTheme.colors.danger,style=MaterialTheme.typography.bodySmall)}
    }
}
