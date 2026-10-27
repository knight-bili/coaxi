package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.coaxi.android.core.designsystem.component.GlassRow
import app.coaxi.android.core.designsystem.diff.DiffView
import app.coaxi.android.core.designsystem.diff.UnifiedDiff
import app.coaxi.android.core.designsystem.markdown.MarkdownContent
import app.coaxi.android.core.designsystem.theme.CoaxiTheme
import app.coaxi.android.core.network.CoaxiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*

fun JsonObject.forgeNumber():Long=coreLong("iid").takeIf{it>0}?:coreLong("number").takeIf{it>0}?:coreLong("index")
private fun JsonObject.forgeBody()=coreText("body").ifEmpty{coreText("description")}

/** Coaxi Forge issue rows/detail sheet, adapted to the existing native GlassRow. */
@Composable fun CoaxiForgeWorkspace(model:CoaxiWorkspaceViewModel,threadId:String){
    val coroutine=rememberCoroutineScope();val generation=model.connectionGeneration
    var accounts by remember(threadId){mutableStateOf<List<JsonObject>>(emptyList())};var account by remember(threadId){mutableStateOf("")};var repository by remember(threadId){mutableStateOf<JsonObject?>(null)}
    var kind by remember(threadId){mutableStateOf("issues")};var status by remember{mutableStateOf("open")};var search by remember{mutableStateOf("")};var sort by remember{mutableStateOf("newest")};var assigned by remember{mutableStateOf(false)};var labelText by remember{mutableStateOf("")};var frozenFilters by remember{mutableStateOf<JsonObject?>(null)}
    var rows by remember(threadId){mutableStateOf<List<JsonObject>>(emptyList())};var page by remember{mutableIntStateOf(1)};var next by remember{mutableStateOf<Int?>(null)};var total by remember{mutableStateOf<Long?>(null)};var incomplete by remember{mutableStateOf(false)};var reachable by remember{mutableStateOf<Long?>(null)}
    var selected by remember(threadId){mutableStateOf<JsonObject?>(null)};var comments by remember{mutableStateOf<List<JsonObject>>(emptyList())};var commentNext by remember{mutableStateOf<Int?>(null)};var detail by remember{mutableStateOf<JsonObject?>(null)};var files by remember{mutableStateOf<List<JsonObject>>(emptyList())};var fileNext by remember{mutableStateOf<Int?>(1)}
    var refreshRevision by remember{mutableIntStateOf(0)}
    var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)}
    fun work(block:suspend ()->Unit){if(busy)return;busy=true;issue=null;coroutine.launch{try{block()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(generation==model.connectionGeneration)issue=error.message}finally{busy=false}}}
    suspend fun call(action:String,extra:JsonObject=buildJsonObject{}):JsonObject{
        val result=model.query(CoaxiOperation.FORGE_OPERATION,buildJsonObject{putJsonObject("scope"){put("threadId",threadId)};put("action",action);if(account.isNotEmpty())put("accountId",account);extra.forEach{(key,value)->put(key,value)}}).coreObject()
        require(generation==model.connectionGeneration){"工作区已切换"};require(result["success"]==JsonPrimitive(true)){"仓库服务返回状态 ${result.coreLong("status")}，请核对远端结果和权限"};repository=result["repository"].coreObject();return result
    }
    suspend fun load(current:Int=1){val filters=if(current==1)buildJsonObject{put("state",status);put("search",search);put("sort",sort);put("assignedMe",assigned);put("labels",JsonArray(labelText.split(',','，').map{it.trim()}.filter{it.isNotEmpty()}.distinct().map(::JsonPrimitive)))}else frozenFilters?:buildJsonObject{};if(current==1)frozenFilters=filters
        val result=call(kind,buildJsonObject{filters.forEach{(key,value)->put(key,value)};put("page",current)});rows=result["data"].coreArray();page=current;next=(result["nextPage"]as?JsonPrimitive)?.intOrNull;total=(result["totalCount"]as?JsonPrimitive)?.longOrNull;reachable=(result["reachableCount"]as?JsonPrimitive)?.longOrNull;incomplete=result["incomplete"]==JsonPrimitive(true);selected=null;comments=emptyList();detail=null;files=emptyList();fileNext=1
    }
    suspend fun loadComments(number:Long,current:Int=1){val result=call(if(kind=="pulls")"comments_pr" else "comments_issue",buildJsonObject{put("number",number);put("page",current)});comments=if(current==1)result["data"].coreArray()else comments+result["data"].coreArray();commentNext=(result["nextPage"]as?JsonPrimitive)?.intOrNull}
    LaunchedEffect(threadId,generation,account,kind,refreshRevision){busy=true;issue=null;try{accounts=model.query(CoaxiOperation.FORGE_ACCOUNTS_GET).coreObject()["accounts"].coreArray();load()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    Column(Modifier.fillMaxSize()){
        Text(repository?.coreText("ownerRepo")?.ifEmpty{"仓库面板"}?:"仓库面板",Modifier.padding(12.dp),style=MaterialTheme.typography.titleMedium)
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal=12.dp)){FilterChip(selected=account.isEmpty(),enabled=!busy,onClick={account=""},label={Text(stringResource(R.string.coaxi_fusion_c84f39fd7b5899fc))});accounts.forEach{row->FilterChip(selected=account==row.coreText("id"),enabled=!busy,onClick={account=row.coreText("id")},label={Text(row.coreText("provider")+" · "+row.coreText("baseUrl"))})}}
        issue?.let{Text(it,Modifier.padding(12.dp),color=CoaxiTheme.colors.danger)}
        CoaxiForgeAccounts(model){refreshRevision++}
        CoaxiForgeWriteActions(model,threadId,account,kind,repository,selected){refreshRevision++}
        val selectedRow=selected
        if(selectedRow==null){
            Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal=12.dp)){FilterChip(selected=kind=="issues",enabled=!busy,onClick={kind="issues"},label={Text(stringResource(R.string.coaxi_fusion_8c3fe87d7c8a958f))});FilterChip(selected=kind=="pulls",enabled=!busy,onClick={kind="pulls"},label={Text("PR / MR")});listOf("open" to "开放","closed" to "关闭","all" to "全部").forEach{(id,title)->FilterChip(selected=status==id,enabled=!busy,onClick={status=id},label={Text(title)})}}
            OutlinedTextField(search,{search=it},label={Text(stringResource(R.string.coaxi_fusion_e7d9341c00793441))},modifier=Modifier.fillMaxWidth().padding(horizontal=12.dp),enabled=!busy)
            Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal=12.dp)){listOf("newest" to "最新创建","oldest" to "最早创建","recently_updated" to "最近更新","least_recently_updated" to "最早更新").forEach{(id,title)->FilterChip(selected=sort==id,enabled=!busy&&repository?.coreText("provider") !in listOf("gitea","forgejo"),onClick={sort=id},label={Text(title)})}}
            Row(Modifier.padding(horizontal=12.dp)){FilterChip(selected=assigned,enabled=!busy,onClick={assigned=!assigned},label={Text(stringResource(R.string.coaxi_fusion_c4b693544d15db38))});OutlinedTextField(labelText,{labelText=it},label={Text(stringResource(R.string.coaxi_fusion_d0835da5fdb653c6))},modifier=Modifier.weight(1f),enabled=!busy);TextButton(enabled=!busy,onClick={work{load()}}){Text(stringResource(R.string.coaxi_fusion_dd0a97ab3653fa82))}}
            if(total!=null||incomplete)Text("${total?.let{"匹配 $it 项"}.orEmpty()}${reachable?.let{" · 可翻阅前 $it 项"}.orEmpty()}${if(incomplete)" · 平台返回部分结果" else ""}",Modifier.padding(horizontal=12.dp),style=MaterialTheme.typography.bodySmall,color=CoaxiTheme.colors.textSecondary)
            LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(12.dp),verticalArrangement=Arrangement.spacedBy(8.dp)){items(rows,key={it.forgeNumber()}){row->GlassRow(Modifier.fillMaxWidth()){TextButton(enabled=!busy,onClick={work{val result=call(if(kind=="pulls")"pr" else "issue",buildJsonObject{put("number",row.forgeNumber())});selected=result["data"].coreObject();loadComments(row.forgeNumber());if(kind=="pulls")detail=call("change_detail",buildJsonObject{put("number",row.forgeNumber())})["data"].coreObject()}}){Column{Text(row.coreText("title"));Text("#${row.forgeNumber()} · ${row.coreText("state")}",style=MaterialTheme.typography.bodySmall,color=CoaxiTheme.colors.textSecondary)}}}}}
            Row(Modifier.fillMaxWidth().padding(12.dp)){TextButton(enabled=!busy&&page>1,onClick={work{load(page-1)}}){Text(stringResource(R.string.coaxi_fusion_c9b9ae7a61444ab7))};Text("$page",Modifier.weight(1f));TextButton(enabled=!busy&&next!=null,onClick={next?.let{work{load(it)}}}){Text(stringResource(R.string.coaxi_fusion_8a8542f6964852dc))}}
        }else{
            TextButton(enabled=!busy,onClick={selected=null;detail=null;files=emptyList();fileNext=1}){Text(stringResource(R.string.coaxi_fusion_e9d5ca6c140665da))}
            LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(12.dp),verticalArrangement=Arrangement.spacedBy(10.dp)){
                item{Text("#${selectedRow.forgeNumber()} ${selectedRow.coreText("title")}",style=MaterialTheme.typography.titleMedium);MarkdownContent(selectedRow.forgeBody())}
                detail?.let{change->item{repository?.let{repo->CoaxiForgeMerge(model,threadId,account,selectedRow.forgeNumber(),repo,change){refreshRevision++}}};item{Text(change.coreText("head_ref")+" → "+change.coreText("base_ref"));Text(change.coreText("state")+" · "+change.coreText("head_sha").take(12),style=MaterialTheme.typography.bodySmall)};val checks=change["checks"].coreObject();item{Text(stringResource(R.string.coaxi_fusion_978e275133d14c1c),style=MaterialTheme.typography.titleSmall);if(checks["available"]!=JsonPrimitive(true))Text(stringResource(R.string.coaxi_fusion_6ba47e0816d9fecc),color=CoaxiTheme.colors.textSecondary);if(checks["partial"]==JsonPrimitive(true))Text(stringResource(R.string.coaxi_fusion_cf840691cb9f614e),color=CoaxiTheme.colors.textSecondary)};items(checks["checks"].coreArray(),key={it.coreText("id")}){check->GlassRow{Text(check.coreText("name"));Text(check.coreText("state")+(if(check["allow_failure"]==JsonPrimitive(true))" · 允许失败" else ""),color=CoaxiTheme.colors.textSecondary);if(check.coreText("summary").isNotBlank())Text(check.coreText("summary"),style=MaterialTheme.typography.bodySmall)}}
                    item{TextButton(enabled=!busy&&fileNext!=null,onClick={val current=fileNext?:return@TextButton;work{val result=call("change_files",buildJsonObject{put("number",selectedRow.forgeNumber());put("page",current)});files=if(current==1)result["data"].coreObject()["files"].coreArray()else files+result["data"].coreObject()["files"].coreArray();fileNext=(result["nextPage"]as?JsonPrimitive)?.intOrNull}}){Text(if(files.isEmpty())"读取变更文件" else "更多变更文件")}}
                }
                items(files,key={it.coreText("path")}){file->GlassRow{var expanded by remember(file){mutableStateOf(false)};TextButton(onClick={expanded=!expanded}){Text(file.coreText("path"))};if(file["binary"]==JsonPrimitive(true))Text(stringResource(R.string.coaxi_fusion_4e8c1baac23df745))else if(file.coreText("patch").isEmpty())Text(stringResource(R.string.coaxi_fusion_ad99d13c22af4bf3))else if(expanded)DiffView(UnifiedDiff.parse("diff --git a/${file.coreText("previous_path").ifEmpty{file.coreText("path")}} b/${file.coreText("path")}\n--- a/${file.coreText("previous_path").ifEmpty{file.coreText("path")}}\n+++ b/${file.coreText("path")}\n${file.coreText("patch")}")?:emptyList())}}
                item{repository?.let{repo->CoaxiForgeMission(model,threadId,account,if(kind=="pulls")"pr" else "issue",selectedRow.forgeNumber(),repo){refreshRevision++}}}
                item{repository?.let{repo->CoaxiForgeLinks(model,threadId,account,if(kind=="pulls")"pr" else "issue",selectedRow.forgeNumber(),repo.coreText("repositoryKey"))}}
                items(comments){comment->GlassRow{Text(comment["user"].coreObject().coreText("login").ifEmpty{comment["author"].coreObject().coreText("username")},style=MaterialTheme.typography.labelSmall);MarkdownContent(comment.forgeBody())}}
                commentNext?.let{current->item{TextButton(enabled=!busy,onClick={work{loadComments(selectedRow.forgeNumber(),current)}}){Text(stringResource(R.string.coaxi_fusion_6394005b5e43f108))}}}
            }
        }
    }
}
