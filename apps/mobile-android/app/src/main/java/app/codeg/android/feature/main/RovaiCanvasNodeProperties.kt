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

val rovaiCanvasKindNames=mapOf("folder" to "工程区域","group" to "项目分组","agent" to "队员区域","conversation" to "对话","custom" to "自定义分组","note" to "便签","file" to "文件","terminal" to "终端")

/** The original CanvasNodeEditor property patch never rewrites a binding or a concurrent move. */
@Composable fun RovaiCanvasNodeProperties(model:RovaiWorkspaceViewModel,boardId:String,node:JsonObject,graph:JsonObject,onClose:()->Unit){
    val state=model.state.collectAsState().value;val scope=rememberCoroutineScope();val generation=remember{model.connectionGeneration};val id=node.coreText("id");val data=node["data"].coreObject();val kind=data.coreText("kind");val owner="canvas-node:$boardId:$id"
    var title by remember{mutableStateOf(data.coreText("title"))};var content by remember{mutableStateOf(data.coreText("content"))};var color by remember{mutableStateOf(data.coreText("color"))};var columns by remember{mutableIntStateOf(data.coreLong("gridColumns").toInt())};var rows by remember{mutableIntStateOf(data.coreLong("gridRows").toInt())};var collapsed by remember{mutableStateOf(data["collapsed"]==JsonPrimitive(true))}
    var revision by remember{mutableLongStateOf(graph["board"].coreObject().coreLong("revision"))};var commandId by remember{mutableStateOf(UUID.randomUUID().toString())};var ready by remember{mutableStateOf(false)};var busy by remember{mutableStateOf(false)};var removal by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)};var latest by remember{mutableStateOf<JsonObject?>(null)}
    val region=kind in listOf("folder","group","agent","custom");val current=state.boards.firstOrNull{it.coreText("id")==boardId};val stale=current==null||current.coreLong("revision")!=revision;val blocked=!ready||busy||state.busy||state.pending!=null||generation!=model.connectionGeneration
    fun record()=buildJsonObject{put("version",1);put("boardId",boardId);put("nodeId",id);put("title",title);put("content",content);put("color",color);put("gridColumns",columns);put("gridRows",rows);put("collapsed",collapsed);put("revision",revision);put("commandId",commandId)}.toString()
    fun persist(){if(ready&&generation==model.connectionGeneration)model.rememberBody(owner,record())}
    fun close(){if(busy)return;if(!ready||generation!=model.connectionGeneration){onClose();return};val saved=record();scope.launch{try{model.saveBody(owner,saved);model.rememberBody(owner,saved);onClose()}catch(error:Exception){issue=error.message}}}
    fun submit(remove:Boolean){if(blocked||stale)return;val mutation=buildJsonObject{if(remove){put("type","delete");put("nodeIds",JsonArray(listOf(JsonPrimitive(id))))}else{put("type","update");put("nodeId",id);putJsonObject("patch"){put("title",title);put("color",color);put("collapsed",collapsed);if(kind=="note")put("content",content);if(region){put("gridColumns",columns);put("gridRows",rows)}}}};val params=buildJsonObject{put("commandId",commandId);putJsonObject("command"){put("boardId",boardId);put("expectedRevision",revision);put("mutation",mutation)}};val saved=record();busy=true;scope.launch{try{model.saveBody(owner,saved);model.rememberBody(owner,saved);require(generation==model.connectionGeneration);model.executeOwned(RovaiOperation.CANVAS_NODES_APPLY,params,owner,saved)}catch(error:Exception){issue=error.message}finally{busy=false}}}
    LaunchedEffect(owner){try{val raw=model.readBody(owner);if(raw.isNotEmpty()){val value=Json.parseToJsonElement(raw).coreObject();require(value.coreLong("version")==1L&&value.coreText("boardId")==boardId&&value.coreText("nodeId")==id){"原卡片草稿无法读取，原记录保留"};title=value.coreText("title");content=value.coreText("content");color=value.coreText("color");columns=value.coreLong("gridColumns").toInt();rows=value.coreLong("gridRows").toInt();collapsed=value["collapsed"]==JsonPrimitive(true);revision=value.coreLong("revision");commandId=value.coreText("commandId").ifEmpty{UUID.randomUUID().toString()}};ready=true}catch(error:Exception){issue=error.message}}
    LaunchedEffect(state.lastResult){val receipt=state.lastResult.coreObject()["commandResult"].coreObject().ifEmpty{state.lastResult.coreObject()};if(state.lastInputOwner==owner&&receipt.coreText("commandId")==commandId){if(receipt.coreText("status")in listOf("applied","accepted")){ready=false;onClose()}else if(receipt.coreText("status")=="rejected"){commandId=UUID.randomUUID().toString();issue=receipt.coreText("code");persist()}}}
    Dialog(onDismissRequest={close()},properties=DialogProperties(usePlatformDefaultWidth=false,dismissOnClickOutside=false)){
        Surface(Modifier.fillMaxSize(),color=CodegTheme.colors.bgElevated){Column(Modifier.fillMaxSize()){
            Row(Modifier.fillMaxWidth().padding(12.dp)){Text((rovaiCanvasKindNames[kind]?:kind)+"属性",Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(enabled=!busy,onClick={close()}){Text("保留草稿并关闭")}}
            LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
                item{issue?.let{Text(it,color=CodegTheme.colors.danger)};if(stale)Text("画布版本已变化，当前草稿保留。需读取最新属性后重新编辑。",color=CodegTheme.colors.danger);if(state.pending!=null)Text("原请求待核对，可关闭后从工作区核对或重试。",color=CodegTheme.colors.textSecondary)}
                item{OutlinedTextField(title,{title=it;persist()},Modifier.fillMaxWidth(),label={Text("名称")},enabled=!blocked);if(kind=="note")OutlinedTextField(content,{content=it;persist()},Modifier.fillMaxWidth(),label={Text("便签内容（支持Markdown）")},enabled=!blocked,minLines=8)}
                item{var picking by remember{mutableStateOf(false)};Box{TextButton(enabled=!blocked,onClick={picking=true}){Text("颜色 · "+(rovaiCanvasColors.firstOrNull{it.first==color}?.second?:color))};DropdownMenu(expanded=picking,onDismissRequest={picking=false}){rovaiCanvasColors.forEach{(key,label)->DropdownMenuItem(text={Text(label)},onClick={color=key;picking=false;persist()})}}}}
                item{Row{Text("折叠卡片",Modifier.weight(1f));Switch(checked=collapsed,enabled=!blocked,onCheckedChange={collapsed=it;persist()})}}
                if(region)item{Text("网格列数 · "+if(columns==0)"自动" else columns.toString());Slider(columns.toFloat(),{columns=it.toInt();persist()},valueRange=0f..12f,steps=11,enabled=!blocked);Text("网格行数 · "+if(rows==0)"自动" else rows.toString());Slider(rows.toFloat(),{rows=it.toInt();persist()},valueRange=0f..12f,steps=11,enabled=!blocked)}
                item{Text("原绑定",style=MaterialTheme.typography.titleSmall);listOf("folderPath","folderGroupId","agentId","runtimeKind","path").forEach{key->if(data.coreText(key).isNotEmpty())Text(data.coreText(key),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)};if(data["conversation"]is JsonObject)Text("会话引用保留",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary);Text("属性保存不改绑会话、文件，不重写其他端移动后的位置。",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}
                item{TextButton(enabled=!blocked,onClick={busy=true;scope.launch{try{val value=model.query(RovaiOperation.CANVAS_NODES_SNAPSHOT,buildJsonObject{put("boardId",boardId)}).coreObject();require(generation==model.connectionGeneration);require(value["nodes"].coreArray().any{it.coreText("id")==id}){"卡片已删除；当前草稿保留"};latest=value}catch(error:Exception){issue=error.message}finally{busy=false}}}){Text("读取最新属性")};TextButton(enabled=!blocked&&!stale,onClick={removal=true}){Text("删除这张卡片",color=CodegTheme.colors.danger)}}
            }
            Button(enabled=!blocked&&!stale,modifier=Modifier.fillMaxWidth().padding(12.dp),onClick={submit(false)}){Text("保存属性")}
        }}
    }
    if(removal)AlertDialog(onDismissRequest={if(!busy)removal=false},title={Text("删除这张卡片？")},text={Text(if(kind=="terminal")"此终端卡片会移除，仍运行的终端会关闭。工程文件保留。" else "只从画布移除此卡片；原会话、使命、任务和文件保留。便签正文仅属于此卡片，删除后无法恢复。")},confirmButton={TextButton(enabled=!blocked&&!stale,onClick={submit(true)}){Text("删除卡片")}},dismissButton={TextButton(onClick={removal=false}){Text("保留卡片")}})
    latest?.let{value->val next=value["nodes"].coreArray().firstOrNull{it.coreText("id")==id}?.get("data").coreObject();AlertDialog(onDismissRequest={latest=null},title={Text("放弃草稿，载入最新属性？")},text={Column{Text(next.coreText("title"));if(kind=="note")Text(next.coreText("content").take(1200));Text("当前未保存的属性草稿将被替换；此操作不提交画布修改。")}},confirmButton={TextButton(enabled=!blocked,onClick={title=next.coreText("title");content=next.coreText("content");color=next.coreText("color");columns=next.coreLong("gridColumns").toInt();rows=next.coreLong("gridRows").toInt();collapsed=next["collapsed"]==JsonPrimitive(true);revision=value["board"].coreObject().coreLong("revision");commandId=UUID.randomUUID().toString();latest=null;persist()}){Text("载入最新属性")}},dismissButton={TextButton(onClick={latest=null}){Text("保留当前草稿")}})}
}
