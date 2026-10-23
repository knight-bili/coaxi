package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.codeg.android.core.designsystem.markdown.MarkdownContent
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import app.codeg.android.feature.sessiondetail.rendering.ToolCallCard
import app.codeg.android.feature.sessiondetail.rendering.ToolCallState
import app.codeg.android.feature.sessiondetail.rendering.ToolCallVM
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*

private fun JsonElement?.evidenceText():String?=when(this){null,JsonNull->null;is JsonPrimitive->contentOrNull;else->Json{prettyPrint=true}.encodeToString(JsonElement.serializer(),this)}
/** The original Codeg native tool card receives Core-owned, run-scoped observations. */
@Composable
fun RovaiExecutionHistory(model:RovaiWorkspaceViewModel,threadId:String,run:JsonObject,observed:List<JsonObject>){
    val runId=run.coreText("id");val epoch=run.coreLong("executionEpoch");val scope=rememberCoroutineScope()
    var expanded by remember(threadId,runId){mutableStateOf(run.coreText("status") in listOf("running","queued","waiting"))}
    var page by remember(threadId,runId,epoch){mutableStateOf<JsonObject?>(null)};var before by remember(threadId,runId,epoch){mutableStateOf<Long?>(null)}
    var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)};var full by remember(threadId,runId,epoch){mutableStateOf<Map<String,JsonObject>>(emptyMap())}
    suspend fun load(cursor:Long?=null){if(busy)return;busy=true;try{val value=model.query(RovaiOperation.AGENTRUNEXECUTION_PAGE,buildJsonObject{put("threadId",threadId);put("agentRunId",runId);put("beforeSequence",cursor?.let(::JsonPrimitive)?:JsonNull);put("limit",50)}).coreObject();require(value.coreText("agentRunId")==runId&&value.coreText("threadId")==threadId&&value.coreLong("schemaVersion")==2L){"执行过程的归属或格式发生变化"};page=value;before=cursor;full=emptyMap();issue=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    LaunchedEffect(expanded,run.coreLong("executionEvidenceChangeSequence"),run.coreLong("executionEvidenceCount"),observed){if(expanded&&before==null)load()}
    val source=if(page!=null)page!!["evidence"].coreArray()+page!!["activeEvidence"].coreArray()else observed
    val groups=source.filter{it.coreText("agentRunId")==runId&&it.coreLong("executionEpoch")==epoch&&it.coreText("kind")!="reasoning_summary"}.sortedBy{it.coreLong("sequence")}.groupBy{it["canonical"].coreObject().coreText("operationId").ifEmpty{it.coreText("operationId").ifEmpty{it.coreText("id")}}}
    Column(verticalArrangement=Arrangement.spacedBy(8.dp)){
        Row{TextButton(onClick={expanded=!expanded}){Text((if(expanded)"收起过程" else "查看过程")+" · "+run.coreText("status"))};if(expanded&&before!=null)TextButton(enabled=!busy,onClick={scope.launch{load()}}){Text(stringResource(R.string.rovai_fusion_8367d15e3204dc62))}}
        if(expanded){
            issue?.let{Text(it,color=CodegTheme.colors.danger)}
            if(page?.get("hasMore")==JsonPrimitive(true))TextButton(enabled=!busy,onClick={val cursor=(page?.get("nextBeforeSequence") as? JsonPrimitive)?.longOrNull;if(cursor!=null)scope.launch{load(cursor)}}){Text(stringResource(R.string.rovai_fusion_71d5cd6c800c841b))}
            groups.values.forEach{versions->val evidence=versions.maxWithOrNull(compareBy<JsonObject>{it.coreLong("revision")}.thenBy{it.coreLong("changeSequence")}.thenBy{it.coreLong("sequence")})?:return@forEach
                key(evidence.coreText("id")){
                    val payload=full[evidence.coreText("id")]?:evidence["payload"].coreObject();val canonical=evidence["canonical"].coreObject();val kind=evidence.coreText("kind")
                    if(kind in listOf("narration","plan","step")&&canonical.coreText("operationId").isEmpty()){
                        val text=payload.coreText("text").ifEmpty{payload.coreText("message").ifEmpty{payload.coreText("title")}};if(text.isNotBlank())MarkdownContent(text)
                    }else{
                        val item=payload["item"].coreObject();val outcome=canonical.coreText("outcome");val phase=canonical.coreText("phase").ifEmpty{evidence.coreText("phase")}
                        val running=phase !in listOf("terminal","completed","failed")&&run.coreText("status") in listOf("running","queued","waiting")
                        val failed=outcome in listOf("failed","denied","unknown","unsettled","cancelled","not_executed")||evidence.coreText("phase")=="failed"
                        val command=item.coreText("command");val input=payload["input"].evidenceText()?:item["arguments"].evidenceText()?:command.takeIf{it.isNotEmpty()}?.let{buildJsonObject{put("command",it)}.toString()}
                        val output=payload["output"].evidenceText()?:item["aggregatedOutput"].evidenceText()?:item["output"].evidenceText()
                        val diff=canonical["diffProjection"].coreObject().takeIf{it.coreText("status")=="available"}?.get("entries").coreArray().joinToString("\n"){it.coreText("diff")}.takeIf{it.isNotBlank()}
                        ToolCallCard(ToolCallVM.of(evidence.coreText("id"),canonical.coreText("toolName").ifEmpty{payload.coreText("title").ifEmpty{item.coreText("tool").ifEmpty{canonical.coreText("presentationHint").ifEmpty{kind}}}},if(canonical.coreText("activityDomain")=="shell")"execute" else payload.coreText("kind"),if(running)ToolCallState.RUNNING else if(failed)ToolCallState.ERROR else ToolCallState.DONE,input,output,diff,failed))
                        if(outcome in listOf("unknown","unsettled","cancelled","not_executed","denied"))Text(when(outcome){"cancelled"->"已停止";"not_executed"->"未执行";"denied"->"已拒绝";else->"结果尚未确定"},style=MaterialTheme.typography.labelSmall,color=CodegTheme.colors.textSecondary)
                    }
                    if(evidence["outputTruncated"]==JsonPrimitive(true))Text(stringResource(R.string.rovai_fusion_655e6ed3e412389e),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)
                    if(evidence["contentBlobId"]!=null&&evidence["contentBlobId"]!=JsonNull&&full[evidence.coreText("id")]==null)TextButton(enabled=!busy,onClick={scope.launch{busy=true;try{val value=model.query(RovaiOperation.AGENTRUNEVIDENCE_GETCONTENT,buildJsonObject{put("threadId",threadId);put("evidenceId",evidence.coreText("id"))}).coreObject();val payload=value["payload"].coreObject();require(payload.toString().length<=4*1024*1024){"完整结果较大，请在工程中查看产物"};full=mapOf(evidence.coreText("id") to payload)}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}){Text(stringResource(R.string.rovai_fusion_fa96b64068fddf27))}
                }
            }
            if(busy)LinearProgressIndicator(Modifier.fillMaxWidth())
        }
    }
}
