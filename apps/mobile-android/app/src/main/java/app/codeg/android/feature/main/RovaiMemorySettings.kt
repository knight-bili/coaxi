package app.codeg.android.feature.main

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.markdown.MarkdownContent
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.util.UUID

private val memoryScopes=listOf("hearth" to "共同记忆","companion" to "队员记忆","relationship" to "队员间记忆")
private val memoryKinds=listOf("preference" to "偏好","agreement" to "约定","lesson" to "经验")
private fun JsonObject.stringList(key:String)=(this[key] as? JsonArray)?.mapNotNull{(it as? JsonPrimitive)?.contentOrNull}?:emptyList()
@Composable
fun RovaiMemorySettings(model:RovaiWorkspaceViewModel){
    val state by model.state.collectAsStateWithLifecycle();val coroutine=rememberCoroutineScope();val context=LocalContext.current
    var library by remember{mutableStateOf(buildJsonObject{})};var reviews by remember{mutableStateOf<List<JsonObject>>(emptyList())};var selected by remember{mutableStateOf<JsonObject?>(null)}
    var scope by remember{mutableStateOf("hearth")};var filter by remember{mutableStateOf("all")};var search by remember{mutableStateOf("")};var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)};var reviewMode by remember{mutableStateOf(false)}
    var editor by remember{mutableStateOf<Pair<String,JsonObject?>?>(null)};var confirmation by remember{mutableStateOf<Triple<RovaiOperation,JsonObject,String>?>(null)};var export by remember{mutableStateOf<String?>(null)}
    val exportPicker=rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/json")){uri->val bytes=export;export=null;if(uri!=null&&bytes!=null)coroutine.launch{try{withContext(Dispatchers.IO){context.contentResolver.openOutputStream(uri,"wt")?.use{it.write(bytes.toByteArray(Charsets.UTF_8))}?:error("文件无法写入")}}catch(error:Exception){issue=error.message}}}
    suspend fun load(){if(busy)return;busy=true;try{val value=model.query(RovaiOperation.MEMORY_LIST).coreObject();val candidates=model.query(RovaiOperation.MEMORY_HEARTHREVIEWITEMS_LIST).coreArray();library=value;reviews=candidates;selected=selected?.let{old->value["memories"].coreArray().firstOrNull{it.coreText("id")==old.coreText("id")}};issue=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    LaunchedEffect(state.lastResult){load()}
    if(editor!=null){RovaiMemoryEditor(model,editor!!.first,editor!!.second,{editor=null},{editor=null});return}
    val blocked=state.pending!=null||state.busy
    val memories=library["memories"].coreArray()
    Column(Modifier.fillMaxSize().padding(16.dp),verticalArrangement=Arrangement.spacedBy(10.dp)){
        Row{Text("记忆",Modifier.weight(1f),style=MaterialTheme.typography.titleLarge);TextButton(enabled=!blocked,onClick={editor="create" to null}){Text("新增")};TextButton(enabled=!busy,onClick={coroutine.launch{load()}}){Text("刷新")}}
        issue?.let{Text(it,color=CodegTheme.colors.danger)};if(busy)LinearProgressIndicator(Modifier.fillMaxWidth())
        Row{TextButton(onClick={reviewMode=!reviewMode;selected=null}){Text(if(reviewMode)"返回记忆库"else"待审核 · "+reviews.count{it.coreText("status")=="pending"})};TextButton(enabled=!busy,onClick={coroutine.launch{try{export=Json{prettyPrint=true}.encodeToString(JsonElement.serializer(),model.query(RovaiOperation.MEMORY_EXPORT));exportPicker.launch("Rovai-memory.json")}catch(error:Exception){issue=error.message}}}){Text("导出记忆")}}
        if(reviewMode){LazyColumn(Modifier.weight(1f),verticalArrangement=Arrangement.spacedBy(8.dp)){items(reviews,key={it.coreText("reviewItemId")}){item->GlassRow{Text(if(item.coreText("requestedAction")=="revise")"修订建议"else"新增建议",style=MaterialTheme.typography.titleMedium);Text(item.coreText("status"),color=CodegTheme.colors.textSecondary);if(item.coreText("status")=="pending"){MarkdownContent(item.coreText("candidateBody"));Text("候选内容尚未成为共同记忆。",style=MaterialTheme.typography.labelSmall);if(item["stale"]==JsonPrimitive(true))Text("目标版本已经变化，只能拒绝此建议。",color=CodegTheme.colors.textSecondary);Row{if(item["stale"]!=JsonPrimitive(true))TextButton(enabled=!blocked,onClick={editor="review" to item}){Text("核对内容后接受")};TextButton(enabled=!blocked,onClick={confirmation=Triple(RovaiOperation.MEMORY_HEARTHREVIEWITEMS_REJECT,buildJsonObject{put("reviewItemId",item.coreText("reviewItemId"));put("expectedReviewItemVersion",item.coreLong("version"))},"拒绝这条建议？")}){Text("拒绝")}}else Text("候选正文已清除。",color=CodegTheme.colors.textSecondary)}}}}
        else if(selected!=null){val item=selected!!;LazyColumn(Modifier.weight(1f),verticalArrangement=Arrangement.spacedBy(8.dp)){
            item{TextButton(onClick={selected=null}){Text("返回记忆列表")};Text(memoryScopes.firstOrNull{it.first==item.coreText("scope")}?.second?:"记忆");SelectionContainer{MarkdownContent(item.coreText("currentBody").ifEmpty{"正文已清除"})};Text("检索词 · "+item.stringList("currentRetrievalKeys").joinToString("、"),style=MaterialTheme.typography.labelSmall);Text("更新 · "+item.coreText("updatedAt"),style=MaterialTheme.typography.labelSmall)}
            if(item.coreText("lifecycle")!="forgotten")item{Row(Modifier.horizontalScroll(rememberScrollState())){TextButton(enabled=!blocked,onClick={editor="revise" to item}){Text("修订")};TextButton(enabled=!blocked,onClick={editor="schedule" to item}){Text("复核提醒")};TextButton(enabled=!blocked,onClick={confirmation=Triple(if(item.coreText("lifecycle")=="active")RovaiOperation.MEMORY_RETIRE else RovaiOperation.MEMORY_REACTIVATE,buildJsonObject{put("memoryId",item.coreText("id"));put("expectedVersion",item.coreLong("version"))},if(item.coreText("lifecycle")=="active")"停止沿用这条记忆？"else"重新沿用这条记忆？")}){Text(if(item.coreText("lifecycle")=="active")"停止沿用"else"重新沿用")};TextButton(enabled=!blocked,onClick={confirmation=Triple(RovaiOperation.MEMORY_FORGET,buildJsonObject{put("memoryId",item.coreText("id"));put("expectedVersion",item.coreLong("version"))},"永久清除正文和版本内容？")}){Text("彻底忘记")}}}
            item{Text("版本记录",style=MaterialTheme.typography.titleSmall)};items(item["revisions"].coreArray(),key={it.coreText("id")}){revision->GlassRow{Text(revision.coreText("createdAt"),style=MaterialTheme.typography.labelSmall);MarkdownContent(revision.coreText("body").ifEmpty{"正文已清除"})}}
        }}else{
            Row(Modifier.horizontalScroll(rememberScrollState())){memoryScopes.forEach{(id,label)->FilterChip(selected=scope==id,onClick={scope=id},label={Text(label)})}}
            Row(Modifier.horizontalScroll(rememberScrollState())){listOf("all" to "全部","agent" to "队员形成","review" to "待复核","stopped" to "已停止沿用").forEach{(id,label)->FilterChip(selected=filter==id,onClick={filter=id},label={Text(label)})}}
            OutlinedTextField(search,{search=it},label={Text("搜索正文、检索词或队员")},modifier=Modifier.fillMaxWidth())
            library["capacities"].coreArray().filter{it.coreText("scope")==scope}.forEach{capacity->Text("沿用 ${capacity.coreLong("activeCount")} / ${capacity.coreLong("maxCount")} 条 · ${capacity.coreLong("activeBodyBytes")} 字节",style=MaterialTheme.typography.labelSmall,color=CodegTheme.colors.textSecondary)}
            val visible=memories.filter{it.coreText("scope")==scope&&when(filter){"agent"->it.coreText("creationOrigin")=="agent";"review"->it["reviewDue"]==JsonPrimitive(true);"stopped"->it.coreText("lifecycle")!="active";else->true}&&(it.coreText("currentBody")+it.stringList("currentRetrievalKeys").joinToString(" ")+it.coreText("companionAgentId")).contains(search,true)}
            LazyColumn(Modifier.weight(1f),verticalArrangement=Arrangement.spacedBy(8.dp)){if(visible.isEmpty())item{Text("当前筛选下没有记忆。",color=CodegTheme.colors.textSecondary)};items(visible,key={it.coreText("id")}){item->GlassRow{Text(memoryKinds.firstOrNull{it.first==item.coreText("kind")}?.second?:"记忆",style=MaterialTheme.typography.labelSmall);Text(item.coreText("currentBody").ifEmpty{"正文已清除"},maxLines=4);TextButton(onClick={selected=item}){Text("查看完整内容")}}}}
        }
    }
    confirmation?.let{value->AlertDialog(onDismissRequest={confirmation=null},title={Text(value.third)},text={Text(if(value.first==RovaiOperation.MEMORY_FORGET)"正文、历史版本和关联候选会按Core规则清除。此操作无法通过重新沿用恢复。"else"按所选内容和当前版本执行。")},confirmButton={TextButton(enabled=!blocked,onClick={model.command(value.first,value.second);confirmation=null}){Text("确认")}},dismissButton={TextButton(onClick={confirmation=null}){Text("取消")}})}
}
@Composable
private fun RovaiMemoryEditor(model:RovaiWorkspaceViewModel,mode:String,original:JsonObject?,onSaved:()->Unit,onClose:()->Unit){
    val state by model.state.collectAsStateWithLifecycle();val coroutine=rememberCoroutineScope();var authority by remember{mutableStateOf(original)}
    var scope by remember{mutableStateOf(original?.coreText("scope").orEmpty().ifEmpty{"hearth"})};var kind by remember{mutableStateOf(original?.coreText("kind").orEmpty().ifEmpty{"preference"})};var body by remember{mutableStateOf(original?.coreText(if(mode=="review")"candidateBody"else"currentBody")?:"")};var keys by remember{mutableStateOf(original?.stringList(if(mode=="review")"candidateRetrievalKeys"else"currentRetrievalKeys")?.joinToString("
")?:"")}
    var first by remember{mutableStateOf(original?.coreText("companionAgentId")?:"")};var second by remember{mutableStateOf("")};var directed by remember{mutableStateOf(false)};var actor by remember{mutableStateOf("")};var reviewAt by remember{mutableStateOf(original?.coreText("reviewAfter")?:"")};var sent by remember{mutableStateOf<String?>(null)};var issue by remember{mutableStateOf<String?>(null)}
    val blocked=state.pending!=null||state.busy
    LaunchedEffect(state.lastResult){val receipt=state.lastResult.coreObject()["commandResult"].coreObject().ifEmpty{state.lastResult.coreObject()};if(sent!=null&&receipt.coreText("commandId")==sent&&receipt.coreText("status")=="applied")onSaved()}
    LazyColumn(Modifier.fillMaxSize().padding(16.dp),verticalArrangement=Arrangement.spacedBy(10.dp)){
        item{Row{Text(when(mode){"create"->"新增记忆";"review"->"审核共同记忆建议";"schedule"->"复核提醒";else->"修订记忆"},Modifier.weight(1f),style=MaterialTheme.typography.titleLarge);TextButton(onClick=onClose){Text("返回")}};issue?.let{Text(it,color=CodegTheme.colors.danger)}}
        if(mode=="create")item{Row(Modifier.horizontalScroll(rememberScrollState())){memoryScopes.forEach{(id,label)->FilterChip(selected=scope==id,enabled=!blocked,onClick={scope=id},label={Text(label)})}};Row{memoryKinds.forEach{(id,label)->FilterChip(selected=kind==id,enabled=!blocked,onClick={kind=id},label={Text(label)})}};if(scope!="hearth"){Text("选择队员");state.members.forEach{member->Row{RadioButton(selected=first==member.coreText("agentId"),enabled=!blocked,onClick={first=member.coreText("agentId")});Text(member.coreText("displayName"))}}};if(scope=="relationship"){Text("另一位队员");state.members.filter{it.coreText("agentId")!=first}.forEach{member->Row{RadioButton(selected=second==member.coreText("agentId"),enabled=!blocked,onClick={second=member.coreText("agentId")});Text(member.coreText("displayName"))}};Row{Checkbox(checked=directed,enabled=!blocked,onCheckedChange={directed=it});Text("定向约定")};if(directed)listOf(first,second).filter{it.isNotEmpty()}.forEach{id->Row{RadioButton(selected=actor==id,enabled=!blocked,onClick={actor=id});Text(state.members.firstOrNull{it.coreText("agentId")==id}?.coreText("displayName")?:id)}}}}
        if(mode!="schedule")item{OutlinedTextField(body,{body=it},enabled=!blocked,label={Text("完整正文")},minLines=6,modifier=Modifier.fillMaxWidth());OutlinedTextField(keys,{keys=it},enabled=!blocked,label={Text("检索词，每行一项")},minLines=2,modifier=Modifier.fillMaxWidth())}
        if(mode!="review")item{OutlinedTextField(reviewAt,{reviewAt=it},enabled=!blocked,label={Text("复核时间，例如2026-12-01T09:00:00+08:00；留空清除")},modifier=Modifier.fillMaxWidth())}
        if(mode in listOf("revise","schedule"))item{TextButton(enabled=!blocked,onClick={coroutine.launch{try{val value=model.query(RovaiOperation.MEMORY_GET,buildJsonObject{put("memoryId",authority!!.coreText("id"))}).coreObject();authority=value;issue="已读取最新版本；当前编辑保留，请对照下面的最新正文后保存。"}catch(error:Exception){issue=error.message}}}){Text("核对最新版本")};authority?.let{Text("最新正文");MarkdownContent(it.coreText("currentBody"))}}
        item{Button(enabled=!blocked&&(mode=="schedule"||body.isNotBlank())&&!(mode=="review"&&(authority?.get("stale")==JsonPrimitive(true)||authority?.coreText("status")!="pending")),onClick={try{val current=authority;if(reviewAt.isNotBlank()&&mode!="review")java.time.OffsetDateTime.parse(reviewAt);val operation=when(mode){"create"->RovaiOperation.MEMORY_CREATE;"review"->RovaiOperation.MEMORY_HEARTHREVIEWITEMS_ACCEPT;"schedule"->RovaiOperation.MEMORY_REVIEW_SCHEDULE;else->RovaiOperation.MEMORY_REVISE};val command=buildJsonObject{when(mode){"create"->{put("scope",scope);put("kind",kind);put("companionAgentId",if(scope=="companion")JsonPrimitive(first)else JsonNull);put("relationshipAgentIds",if(scope=="relationship")JsonArray(listOf(first,second).map(::JsonPrimitive))else JsonArray(emptyList()));put("direction",if(scope=="relationship")JsonPrimitive(if(directed)"directed"else"mutual")else JsonNull);put("directedActorAgentId",if(scope=="relationship"&&directed)JsonPrimitive(actor)else JsonNull)};"review"->{put("reviewItemId",current!!.coreText("reviewItemId"));put("expectedReviewItemVersion",current.coreLong("version"))};else->{put("memoryId",current!!.coreText("id"));put("expectedVersion",current.coreLong("version"));if(mode=="revise")put("baseRevisionId",current.coreText("currentRevisionId"))}};if(mode!="schedule"){put(if(mode=="review")"finalBody"else"body",body.trim());put(if(mode=="review")"finalRetrievalKeys"else"retrievalKeys",JsonArray(keys.lines().map{it.trim()}.filter{it.isNotEmpty()}.map(::JsonPrimitive)))};if(mode!="review")put("reviewAfter",reviewAt.takeIf{it.isNotBlank()}?.let(::JsonPrimitive)?:JsonNull)};val id=UUID.randomUUID().toString();sent=id;model.execute(operation,buildJsonObject{put("commandId",id);put("command",command)})}catch(error:Exception){issue=error.message}}){Text(if(mode=="review")"按最终内容接受"else"保存")}}
    }
}
