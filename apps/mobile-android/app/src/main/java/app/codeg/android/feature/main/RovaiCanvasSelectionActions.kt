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
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import java.util.UUID

/** Original selection dock: only ordinary conversation pins may become conversation members. */
@Composable fun RovaiCanvasSelectionActions(model:RovaiWorkspaceViewModel,boardId:String,graph:JsonObject,nodeIds:Set<String>,onClose:()->Unit){
    val state=model.state.collectAsState().value;val scope=rememberCoroutineScope();val generation=remember{model.connectionGeneration};val owner="canvas-bulk:$boardId";val nodes=graph["nodes"].coreArray().filter{it.coreText("id")in nodeIds};val revision=graph["board"].coreObject().coreLong("revision")
    var target by remember{mutableStateOf("")};var confirm by remember{mutableStateOf<JsonObject?>(null)};var issue by remember{mutableStateOf<String?>(null)};var busy by remember{mutableStateOf(false)};var sentId by remember{mutableStateOf<String?>(null)}
    val stale=state.boards.firstOrNull{it.coreText("id")==boardId}?.coreLong("revision")!=revision;val blocked=busy||state.busy||state.pending!=null||generation!=model.connectionGeneration||nodes.size!=nodeIds.size
    val groupable=nodes.isNotEmpty()&&nodes.size<=200&&nodes.all{it["data"].coreObject().let{data->data.coreText("kind")=="conversation"&&data["conversation"]is JsonObject&&data["objectReference"] !is JsonObject}}
    fun submit(mutation:JsonObject){if(blocked||stale)return;val commandId=UUID.randomUUID().toString();val body=mutation.toString();sentId=commandId;busy=true;scope.launch{try{model.saveBody(owner,body);model.rememberBody(owner,body);require(generation==model.connectionGeneration);model.executeOwned(RovaiOperation.CANVAS_NODES_APPLY,buildJsonObject{put("commandId",commandId);putJsonObject("command"){put("boardId",boardId);put("expectedRevision",revision);put("mutation",mutation)}},owner,body)}catch(error:Exception){issue=error.message}finally{busy=false}}}
    LaunchedEffect(state.lastResult){val receipt=state.lastResult.coreObject()["commandResult"].coreObject().ifEmpty{state.lastResult.coreObject()};if(state.lastInputOwner==owner&&receipt.coreText("commandId")==sentId){if(receipt.coreText("status")in listOf("applied","accepted"))onClose()else if(receipt.coreText("status")=="rejected")issue=receipt.coreText("code")}}
    Dialog(onDismissRequest={if(!busy)onClose()},properties=DialogProperties(usePlatformDefaultWidth=false,dismissOnClickOutside=false)){Surface(Modifier.fillMaxSize(),color=CodegTheme.colors.bgElevated){Column{
        Row(Modifier.fillMaxWidth().padding(12.dp)){Text("已选 ${nodes.size} 张卡片",Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(enabled=!busy,onClick=onClose){Text("关闭")}}
        LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
            item{issue?.let{Text(it,color=CodegTheme.colors.danger)};if(stale)Text("画布已更新；关闭后核对最新卡片，再重新操作。",color=CodegTheme.colors.danger);if(state.pending!=null)Text("原请求待核对，当前选择保留。",color=CodegTheme.colors.textSecondary)}
            nodes.forEach{node->item(key=node.coreText("id")){val data=node["data"].coreObject();Text(data.coreText("title").ifEmpty{rovaiCanvasKindNames[data.coreText("kind")]?:"卡片"})}}
            item{RovaiCanvasChoice("目标分组",target,listOf("" to "新建自定义分组")+graph["nodes"].coreArray().filter{it["data"].coreObject().coreText("kind")=="custom"}.map{it.coreText("id") to it["data"].coreObject().coreText("title").ifEmpty{"自定义分组"}},!blocked){target=it}
                Button(enabled=!blocked&&!stale&&groupable,onClick={val references=nodes.map{it["data"].coreObject()["conversation"].coreObject()}.distinctBy{RovaiCanvasReferenceChoice(it,"").key};fun n(node:JsonObject,key:String)=(node["data"].coreObject()[key]as?JsonPrimitive)?.doubleOrNull?:0.0;val count=(references.size+1)/2;submit(buildJsonObject{put("type","group");put("targetNodeId",if(target.isEmpty())JsonNull else JsonPrimitive(target));put("newRegion",if(target.isNotEmpty())JsonNull else buildJsonObject{put("kind","custom");put("x",nodes.minOf{n(it,"x")}-12);put("y",nodes.minOf{n(it,"y")}-52);put("width",484);put("height",64+count*132+(count-1).coerceAtLeast(0)*12)});put("conversations",JsonArray(references));put("consumeNodeIds",JsonArray(nodes.map{JsonPrimitive(it.coreText("id"))}))})}){Text(if(target.isEmpty())"归为新分组"else"归入已有分组")}
                if(!groupable)Text("归组请选择最多200张普通会话卡；使命、任务、文件、终端和便签保留各自的对象类型。",color=CodegTheme.colors.textSecondary)
            }
            item{TextButton(enabled=!blocked&&!stale&&nodes.isNotEmpty(),onClick={confirm=buildJsonObject{put("type","move");put("moves",canvasPackedMoves(nodes))}}){Text("整理所选卡片")};TextButton(enabled=!blocked&&!stale&&nodes.isNotEmpty(),onClick={confirm=buildJsonObject{put("type","delete");put("nodeIds",JsonArray(nodes.map{JsonPrimitive(it.coreText("id"))}))}}){Text("删除所选卡片",color=CodegTheme.colors.danger)}}
        }
    }}}
    confirm?.let{mutation->val deletion=mutation.coreText("type")=="delete";AlertDialog(onDismissRequest={confirm=null},title={Text(if(deletion)"删除所选卡片？"else"整理所选卡片？")},text={Text(if(deletion)"仅移除画布引用，原会话、使命、任务和文件保留。所选便签的正文会删除，所选运行中终端会关闭。"else"按原版排布规则调整所选卡片位置，其他卡片位置保留。")},confirmButton={TextButton(enabled=!blocked&&!stale,onClick={submit(mutation);confirm=null}){Text(if(deletion)"删除卡片"else"整理位置")}},dismissButton={TextButton(onClick={confirm=null}){Text("取消")}})}
}
