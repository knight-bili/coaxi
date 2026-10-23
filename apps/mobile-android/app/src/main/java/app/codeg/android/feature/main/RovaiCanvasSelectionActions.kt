package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

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
        Row(Modifier.fillMaxWidth().padding(12.dp)){Text("已选 ${nodes.size} 张卡片",Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(enabled=!busy,onClick=onClose){Text(stringResource(R.string.rovai_fusion_3fd47edce45b3603))}}
        LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
            item{issue?.let{Text(it,color=CodegTheme.colors.danger)};if(stale)Text(stringResource(R.string.rovai_fusion_7ee7ed047066bed9),color=CodegTheme.colors.danger);if(state.pending!=null)Text(stringResource(R.string.rovai_fusion_bf9578664719ce2f),color=CodegTheme.colors.textSecondary)}
            nodes.forEach{node->item(key=node.coreText("id")){val data=node["data"].coreObject();Text(data.coreText("title").ifEmpty{rovaiCanvasKindNames[data.coreText("kind")]?:"卡片"})}}
            item{RovaiCanvasChoice("目标分组",target,listOf("" to "新建自定义分组")+graph["nodes"].coreArray().filter{it["data"].coreObject().coreText("kind")=="custom"}.map{it.coreText("id") to it["data"].coreObject().coreText("title").ifEmpty{"自定义分组"}},!blocked){target=it}
                Button(
                    enabled = !blocked && !stale && groupable,
                    onClick = {
                        val references = nodes
                            .map { it["data"].coreObject()["conversation"].coreObject() }
                            .distinctBy { RovaiCanvasReferenceChoice(it, "").key }
                        val count = (references.size + 1) / 2
                        val newRegion = if (target.isNotEmpty()) {
                            JsonNull
                        } else {
                            val coordinate: (JsonObject, String) -> Double = { node, key ->
                                (node["data"].coreObject()[key] as? JsonPrimitive)?.doubleOrNull ?: 0.0
                            }

                            buildJsonObject {
                                put("kind", "custom")
                                put("x", nodes.minOf { coordinate(it, "x") } - 12)
                                put("y", nodes.minOf { coordinate(it, "y") } - 52)
                                put("width", 484)
                                put("height", 64 + count * 132 + (count - 1).coerceAtLeast(0) * 12)
                            }
                        }
                        submit(
                            buildJsonObject {
                                put("type", "group")
                                put("targetNodeId", if (target.isEmpty()) JsonNull else JsonPrimitive(target))
                                put("newRegion", newRegion)
                                put("conversations", JsonArray(references))
                                put("consumeNodeIds", JsonArray(nodes.map { JsonPrimitive(it.coreText("id")) }))
                            }
                        )
                    }
                ) {
                    Text(if (target.isEmpty()) "归为新分组" else "归入已有分组")
                }
                if(!groupable)Text(stringResource(R.string.rovai_fusion_bdb39e069e23d7b3),color=CodegTheme.colors.textSecondary)
            }
            item{TextButton(enabled=!blocked&&!stale&&nodes.isNotEmpty(),onClick={confirm=buildJsonObject{put("type","move");put("moves",canvasPackedMoves(nodes))}}){Text(stringResource(R.string.rovai_fusion_c8b6fc5c52191014))};TextButton(enabled=!blocked&&!stale&&nodes.isNotEmpty(),onClick={confirm=buildJsonObject{put("type","delete");put("nodeIds",JsonArray(nodes.map{JsonPrimitive(it.coreText("id"))}))}}){Text(stringResource(R.string.rovai_fusion_e6d7c2665e984459),color=CodegTheme.colors.danger)}}
        }
    }}}
    confirm?.let{mutation->val deletion=mutation.coreText("type")=="delete";AlertDialog(onDismissRequest={confirm=null},title={Text(if(deletion)"删除所选卡片？"else"整理所选卡片？")},text={Text(if(deletion)"仅移除画布引用，原会话、使命、任务和文件保留。所选便签的正文会删除，所选运行中终端会关闭。"else"按原版排布规则调整所选卡片位置，其他卡片位置保留。")},confirmButton={TextButton(enabled=!blocked&&!stale,onClick={submit(mutation);confirm=null}){Text(if(deletion)"删除卡片"else"整理位置")}},dismissButton={TextButton(onClick={confirm=null}){Text(stringResource(R.string.rovai_fusion_2cd0f3be8738a86c))}})}
}
