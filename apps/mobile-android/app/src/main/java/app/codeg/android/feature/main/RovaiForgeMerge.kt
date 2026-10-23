package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.util.UUID

@Composable fun RovaiForgeMerge(model:RovaiWorkspaceViewModel,threadId:String,accountId:String,number:Long,repository:JsonObject,detail:JsonObject,onChanged:()->Unit){
    val scope=rememberCoroutineScope();val generation=model.connectionGeneration;val owner="forge-merge:$threadId:$accountId:$number"
    var open by remember(threadId,accountId,number){mutableStateOf(false)};var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)};var methods by remember{mutableStateOf<List<String>>(emptyList())};var method by remember{mutableStateOf("merge")};var strategy by remember{mutableStateOf("")};var head by remember{mutableStateOf("")};var commandId by remember{mutableStateOf("")};var sent by remember{mutableStateOf<JsonObject?>(null)}
    fun params(action:String)=buildJsonObject{putJsonObject("scope"){put("threadId",threadId)};put("action",action);put("number",number);if(accountId.isNotEmpty())put("accountId",accountId)}
    fun record()=buildJsonObject{put("repositoryKey",repository.coreText("repositoryKey"));put("head",head);put("method",method);put("methods",JsonArray(methods.map(::JsonPrimitive)));put("strategy",strategy);put("commandId",commandId);put("sent",sent?:JsonNull)}.toString()
    fun prepare(){if(busy)return;val capturedHead=detail.coreText("head_sha");busy=true;issue=null;open=true;scope.launch{try{val previous=model.readBody(owner);if(previous.isNotEmpty()){val value=Json.parseToJsonElement(previous).coreObject();require(value.coreText("repositoryKey")==repository.coreText("repositoryKey")){"仓库身份已变化，已保留原合并请求"};head=value.coreText("head");method=value.coreText("method");methods=(value["methods"]as?JsonArray)?.mapNotNull{(it as?JsonPrimitive)?.contentOrNull}.orEmpty();strategy=value.coreText("strategy");commandId=value.coreText("commandId");sent=value["sent"]as?JsonObject}else{require(capturedHead.matches(Regex("[0-9a-fA-F]{40,64}"))){"请先读取当前PR的提交"};val result=model.query(RovaiOperation.FORGE_OPERATION,params("merge_options")).coreObject();require(result["success"]==JsonPrimitive(true)&&generation==model.connectionGeneration){"无法读取合并选项"};val options=result["data"].coreObject();methods=(options["methods"]as?JsonArray)?.mapNotNull{(it as?JsonPrimitive)?.contentOrNull}?.filter{it in listOf("merge","squash","rebase")}.orEmpty().ifEmpty{listOf("merge")};method=options.coreText("default_method").takeIf{it in methods}?:methods.first();strategy=options.coreText("merge_strategy");head=capturedHead;commandId=UUID.randomUUID().toString()}}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}
    TextButton(enabled=!busy&&detail.coreText("head_sha").isNotEmpty()&&detail.coreText("state")=="open"&&detail["draft"]!=JsonPrimitive(true)&&model.state.collectAsState().value.pending==null,onClick=::prepare){Text(stringResource(R.string.rovai_fusion_8718874941b7e349))}
    if(open)AlertDialog(onDismissRequest={if(!busy)open=false},title={Text(stringResource(R.string.rovai_fusion_4c6856d232187496))},text={Column(verticalArrangement=Arrangement.spacedBy(8.dp)){
        Text(repository.coreText("ownerRepo")+" #$number");Text(stringResource(R.string.rovai_fusion_b44633c57a946c0f),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary);Text(head,style=MaterialTheme.typography.bodySmall.copy(fontFamily=androidx.compose.ui.text.font.FontFamily.Monospace));issue?.let{Text(it,color=CodegTheme.colors.danger)}
        methods.forEach{value->FilterChip(selected=method==value,enabled=!busy&&sent==null,onClick={method=value},label={Text(when(value){"squash"->"压缩提交";"rebase"->"变基合并";else->when(strategy){"fast_forward"->"快进合并";"rebase_merge"->"变基后合并";else->"创建合并提交"}})})}
        if(sent!=null)Text(stringResource(R.string.rovai_fusion_b44bf73315025d56),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)
    }},confirmButton={TextButton(enabled=!busy&&head.isNotEmpty()&&methods.isNotEmpty()&&issue==null,onClick={busy=true;scope.launch{try{val command=sent?:buildJsonObject{params("merge_pr").forEach{(key,value)->put(key,value)};put("headSha",head);put("mergeMethod",method);put("confirmed",true);put("expectedRepositoryKey",repository.coreText("repositoryKey"));put("commandId",commandId)};sent=command;val saved=record();model.rememberBody(owner,saved);model.saveBody(owner,saved);val result=model.query(RovaiOperation.FORGE_OPERATION,command).coreObject();require(result["success"]==JsonPrimitive(true)&&generation==model.connectionGeneration){"合并结果需核对，原请求仍保留"};model.rememberBody(owner,"");open=false;onChanged()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}){Text(if(sent==null)"确认合并" else "核对原合并请求")}},dismissButton={TextButton(enabled=!busy,onClick={open=false}){Text(stringResource(R.string.rovai_fusion_3fd47edce45b3603))}})
}
