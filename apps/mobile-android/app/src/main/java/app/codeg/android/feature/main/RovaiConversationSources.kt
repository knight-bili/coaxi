package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

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
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import java.time.Instant
import java.util.UUID

internal fun conversationSourceReference(source:JsonObject):String {
    val title=source.coreText("title").ifEmpty{"旧讨论"}.replace(Regex("[\\r\\n\\t]+")," ").replace("\\","\\\\").replace("[","\\[").replace("]","\\]")
    return "[参考：$title](rovai-source:${source.coreText("sourceSessionId")})"
}

/** 复用原生历史列表和资料行，授权后只加入普通输入，不启动队员。 */
@Composable fun RovaiConversationSources(model:RovaiWorkspaceViewModel,threadId:String,conversationId:String?=null,child:Boolean=false,enabled:Boolean=true,onInsert:(String)->Boolean){
    var open by remember{mutableStateOf(false)}
    val insert by rememberUpdatedState(onInsert)
    TextButton(enabled=enabled,onClick={open=true}){Text(stringResource(R.string.rovai_fusion_2386ba253ada87ba))}
    if(open)Dialog(onDismissRequest={open=false},properties=DialogProperties(usePlatformDefaultWidth=false)){
        key(threadId,conversationId,child,model.connectionGeneration){
            RovaiConversationSourcePicker(model,conversationScope(threadId,conversationId,child),enabled,{reference->insert(reference)},{open=false})
        }
    }
}

@Composable private fun RovaiConversationSourcePicker(model:RovaiWorkspaceViewModel,scope:JsonObject,enabled:Boolean,onInsert:(String)->Boolean,onClose:()->Unit){
    val coroutine=rememberCoroutineScope();val generation=remember{model.connectionGeneration};val owner="conversation-sources:$scope"
    var rows by remember{mutableStateOf<List<JsonObject>>(emptyList())};var next by remember{mutableStateOf<String?>(null)}
    var grants by remember{mutableStateOf<List<JsonObject>>(emptyList())};var grantNext by remember{mutableStateOf<String?>(null)}
    var imports by remember{mutableStateOf<List<JsonObject>>(emptyList())};var importLimit by remember{mutableIntStateOf(25)}
    var selected by remember{mutableStateOf<JsonObject?>(null)};var preview by remember{mutableStateOf<JsonObject?>(null)};var confirmed by remember{mutableStateOf<JsonObject?>(null)}
    var pending by remember{mutableStateOf<JsonObject?>(null)};var busy by remember{mutableStateOf(false)};var ready by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)}
    fun current(){require(generation==model.connectionGeneration){"工作区已切换"}}
    suspend fun call(op:RovaiOperation,input:JsonObject):JsonObject{current();val value=nativeUserCall(model,op,input);current();return value}
    suspend fun load(more:Boolean=false){
        val value=call(RovaiOperation.HISTORY_LIBRARY,buildJsonObject{putJsonObject("scope"){put("threadId",scope.coreText("threadId"))};put("operation","sources");put("targetScope",scope);put("after",if(more)next?.let(::JsonPrimitive)?:JsonNull else JsonNull);put("limit",50)})
        rows=if(more)(rows+value["items"].coreArray()).distinctBy{it.coreText("sourceSessionId")}else value["items"].coreArray();next=(value["next"] as? JsonPrimitive)?.contentOrNull
        val page=call(RovaiOperation.CONVERSATIONTOOLS_GRANTS_LIST,buildJsonObject{put("scope",scope);put("after",JsonNull);put("limit",100)});grants=page["items"].coreArray();grantNext=(page["next"] as? JsonPrimitive)?.contentOrNull
    }
    fun work(action:suspend()->Unit){if(busy)return;busy=true;issue=null;coroutine.launch{try{action()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(generation==model.connectionGeneration)issue=error.message}finally{if(generation==model.connectionGeneration)busy=false}}}
    suspend fun choose(source:JsonObject){val value=call(RovaiOperation.CONVERSATIONTOOLS_GRANTS_PREVIEW,buildJsonObject{put("scope",scope);put("sourceSessionId",source.coreText("sourceSessionId"));put("sourceDigest",source.coreText("sourceDigest"));put("throughSequence",source["throughSequence"]?:JsonNull)});selected=source;preview=value;confirmed=null}
    suspend fun retain(operation:String):JsonObject{val original=buildJsonObject{put("operation",operation);put("commandId",UUID.randomUUID().toString())};model.saveBody(owner,original.toString());current();pending=original;return original}
    suspend fun clear(original:JsonObject){require(model.clearBodyIfMatches(owner,original.toString(),generation)){"原授权请求记录已变化"};pending=null}
    fun insert(grant:JsonObject){
        current();require(enabled&&grant["scope"]==scope&&Instant.parse(grant.coreText("expiresAt")).isAfter(Instant.now())){"这份授权已过期或当前输入已变化，请重新选择"}
        val source=selected?.takeIf{it.coreText("sourceSessionId")==grant.coreText("sourceSessionId")}?:rows.firstOrNull{it.coreText("sourceSessionId")==grant.coreText("sourceSessionId")}?:buildJsonObject{put("title","旧讨论");put("sourceSessionId",grant.coreText("sourceSessionId"))}
        require(onInsert(conversationSourceReference(source))){"当前输入不可编辑，授权记录保留"};onClose()
    }
    LaunchedEffect(scope,generation){busy=true;try{val raw=model.readBody(owner);current();if(raw.isNotEmpty()){require(raw.length<=1024);val original=Json.parseToJsonElement(raw).coreObject();require(original.keys==setOf("operation","commandId")&&original.coreText("operation") in listOf("conversationTools.grants.confirm","conversationTools.grants.revoke"));UUID.fromString(original.coreText("commandId"));pending=original};load();ready=true}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    Surface(Modifier.fillMaxSize(),color=CodegTheme.colors.bg){Column(Modifier.fillMaxSize()){
        Row(Modifier.fillMaxWidth().padding(16.dp)){Text(stringResource(R.string.rovai_fusion_915b6ed7eb247939),Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(onClick=onClose){Text(stringResource(R.string.rovai_fusion_3fd47edce45b3603))}}
        LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
            item{Text(if(scope.coreText("scopeKind")=="public")"团队讨论只列出可公开参考的来源。私聊和私人导入资料请在对应私聊中选择。" else "资料只授权给当前私聊或帮手讨论，不会开放给整个团队。");Text(stringResource(R.string.rovai_fusion_31d46e6fb623ad81),style=MaterialTheme.typography.bodySmall)}
            items(rows,key={it.coreText("sourceSessionId")}){source->GlassRow(Modifier.fillMaxWidth()){Text(source.coreText("title"));TextButton(enabled=ready&&!busy&&pending==null,onClick={work{choose(source)}}){Text(stringResource(R.string.rovai_fusion_108aeed433f9d8bf))}}}
            if(next!=null)item{TextButton(enabled=!busy&&pending==null,onClick={work{load(true)}}){Text(stringResource(R.string.rovai_fusion_e8c0d54035a298a6))}}
            if(scope.coreText("scopeKind")!="public"){
                item{TextButton(enabled=!busy&&pending==null,onClick={work{val value=call(RovaiOperation.HISTORY_LIBRARY,buildJsonObject{putJsonObject("scope"){put("threadId",scope.coreText("threadId"))};put("operation","list");put("includeArchived",false)});imports=value["entries"].coreArray()}}){Text(stringResource(R.string.rovai_fusion_be67fc05b388d12b))}}
                items(imports.take(importLimit),key={"import:"+it.coreText("id")}){entry->GlassRow{Text(entry["summary"].coreObject().coreText("title").ifEmpty{"历史讨论"});TextButton(enabled=!busy&&pending==null,onClick={work{require(entry.coreText("linkedThreadId").isNotEmpty()&&entry.coreText("revision").isNotEmpty()){"导入缺少原归属或修订"};val value=call(RovaiOperation.HISTORY_LIBRARY,buildJsonObject{putJsonObject("scope"){put("threadId",entry.coreText("linkedThreadId"))};put("operation","source.prepare");put("commandId",UUID.randomUUID().toString());put("importId",entry.coreText("id"));put("expectedRevision",entry.coreText("revision"))});choose(value["source"].coreObject())}}){Text(stringResource(R.string.rovai_fusion_0ceee98d5266249b))}}}
                if(imports.size>importLimit)item{TextButton(onClick={importLimit+=25}){Text(stringResource(R.string.rovai_fusion_483caaee37b4867d))}}
            }
            preview?.let{value->item{GlassRow{Text(selected?.coreText("title")?:"旧讨论");Text(stringResource(R.string.rovai_fusion_e8660299111b7e71)+value.coreText("expiresAt"));Text(stringResource(R.string.rovai_fusion_a0e257c83211db63),style=MaterialTheme.typography.bodySmall)}}}
            items(grants,key={"grant:"+it.coreText("grantId")}){grant->GlassRow{Text(stringResource(R.string.rovai_fusion_8fc24172bd7c743d)+(rows.firstOrNull{it.coreText("sourceSessionId")==grant.coreText("sourceSessionId")}?.coreText("title")?:"旧讨论"));Text(stringResource(R.string.rovai_fusion_8a8835014f8fc799)+grant.coreText("expiresAt"),style=MaterialTheme.typography.bodySmall);TextButton(enabled=!busy&&pending==null,onClick={work{val original=retain("conversationTools.grants.revoke");call(RovaiOperation.CONVERSATIONTOOLS_GRANTS_REVOKE,buildJsonObject{put("commandId",original.coreText("commandId"));put("grantId",grant.coreText("grantId"));put("expectedRevision",grant.coreLong("revision"))});clear(original);if(confirmed?.coreText("grantId")==grant.coreText("grantId"))confirmed=null;load()}}){Text(stringResource(R.string.rovai_fusion_fdbab2900fcfa849))}}}
            if(grantNext!=null)item{TextButton(enabled=!busy&&pending==null,onClick={work{val value=call(RovaiOperation.CONVERSATIONTOOLS_GRANTS_LIST,buildJsonObject{put("scope",scope);put("after",grantNext);put("limit",100)});grants=(grants+value["items"].coreArray()).distinctBy{it.coreText("grantId")};grantNext=(value["next"] as? JsonPrimitive)?.contentOrNull}}){Text(stringResource(R.string.rovai_fusion_65012b7f0f204956))}}
            issue?.let{item{Text(it,color=CodegTheme.colors.danger)}}
        }
        Row(Modifier.fillMaxWidth().padding(16.dp),horizontalArrangement=Arrangement.End){
            when{pending!=null->TextButton(enabled=!busy,onClick={work{val original=pending?:return@work;val value=call(RovaiOperation.COMMANDS_RECONCILE,buildJsonObject{put("operation",original.coreText("operation"));putJsonObject("params"){put("commandId",original.coreText("commandId"))}});val result=value["result"].coreObject();require(value.coreText("state")=="recorded"&&result.coreText("commandId")==original.coreText("commandId")&&result.coreText("commandType")==original.coreText("operation")){"原授权请求没有确定回执，没有重复授权"};clear(original);require(result.coreText("status")!="rejected"){result.coreText("code")};if(original.coreText("operation")=="conversationTools.grants.confirm")confirmed=result["payload"].coreObject();load()}}){Text(stringResource(R.string.rovai_fusion_43e10b7f6ea49f0f))}
                confirmed!=null->Button(enabled=!busy&&enabled,onClick={work{insert(confirmed!!)}}){Text(stringResource(R.string.rovai_fusion_a42e8e0484f88d4d))}
                else->Button(enabled=ready&&!busy&&enabled&&preview!=null,onClick={work{val value=preview?:return@work;val original=retain("conversationTools.grants.confirm");val grant=call(RovaiOperation.CONVERSATIONTOOLS_GRANTS_CONFIRM,buildJsonObject{put("commandId",original.coreText("commandId"));put("previewId",value.coreText("previewId"));put("previewDigest",value.coreText("previewDigest"))});confirmed=grant;clear(original);insert(grant)}}){Text(stringResource(R.string.rovai_fusion_d55f0278900a29d2))}
            }
        }
    }}
}
