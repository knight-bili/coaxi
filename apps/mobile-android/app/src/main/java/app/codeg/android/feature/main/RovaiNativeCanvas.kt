package app.codeg.android.feature.main

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.dp
import androidx.compose.ui.platform.LocalDensity
import app.codeg.android.core.designsystem.markdown.MarkdownContent
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.serialization.json.*
import kotlinx.coroutines.launch

@Composable
fun RovaiNativeCanvas(model:RovaiWorkspaceViewModel,board:JsonObject){
    val density=LocalDensity.current.density
    val state by model.state.collectAsState();val id=board.coreText("id");var snapshot by remember(id){mutableStateOf<JsonObject?>(null)};var error by remember(id){mutableStateOf<String?>(null)};var selected by remember(id){mutableStateOf<JsonObject?>(null)};var creating by remember{mutableStateOf(false)};var note by remember{mutableStateOf("")};var zoom by remember(id){mutableFloatStateOf(1f)};var offset by remember(id){mutableStateOf(Offset.Zero)};val scope=rememberCoroutineScope();val revision=state.boards.firstOrNull{it.coreText("id")==id}?.coreLong("revision")
    LaunchedEffect(id,revision,state.lastResult){try{snapshot=model.query(RovaiOperation.CANVAS_NODES_SNAPSHOT,buildJsonObject{put("boardId",id)}).coreObject()}catch(failure:Exception){error=failure.message}}
    var properties by remember(id){mutableStateOf<JsonObject?>(null)}
    properties?.let{node->snapshot?.let{graph->RovaiCanvasNodeProperties(model,id,node,graph){properties=null}}}
    fun mutate(mutation:JsonObject){val current=snapshot?:return;model.command(RovaiOperation.CANVAS_NODES_APPLY,buildJsonObject{put("boardId",id);put("expectedRevision",current["board"].coreObject().coreLong("revision"));put("mutation",mutation)})}
    Column(Modifier.fillMaxSize()){
        Row(Modifier.fillMaxWidth().padding(8.dp),horizontalArrangement=Arrangement.spacedBy(8.dp)){TextButton(onClick={creating=true},enabled=snapshot!=null&&state.pending==null){Text("添加便签")};TextButton(onClick={offset=Offset.Zero;zoom=1f}){Text("重置视图")};Text("${(zoom*100).toInt()}%",modifier=Modifier.padding(8.dp))}
        error?.let{Text(it,color=CodegTheme.colors.danger,modifier=Modifier.padding(8.dp))}
        Box(Modifier.weight(1f).fillMaxWidth().background(CodegTheme.colors.bg).pointerInput(id){detectTransformGestures{_,pan,scale,_->offset+=pan;zoom=(zoom*scale).coerceIn(0.1f,2f)}}){
            snapshot?.get("nodes").coreArray().forEach{node->val data=node["data"].coreObject();var drag by remember(node.coreText("id"),data.coreLong("x"),data.coreLong("y")){mutableStateOf(Offset.Zero)};val x=(data["x"] as? JsonPrimitive)?.floatOrNull?:0f;val y=(data["y"] as? JsonPrimitive)?.floatOrNull?:0f;val width=(data["width"] as? JsonPrimitive)?.floatOrNull?:320f;val height=(data["height"] as? JsonPrimitive)?.floatOrNull?:200f
                Column(Modifier.graphicsLayer{translationX=offset.x+(x+drag.x)*density*zoom;translationY=offset.y+(y+drag.y)*density*zoom;scaleX=zoom;scaleY=zoom;transformOrigin=androidx.compose.ui.graphics.TransformOrigin(0f,0f)}.size(width.dp,if(data["collapsed"]==JsonPrimitive(true))52.dp else height.dp).background(CodegTheme.colors.bgElevated,RoundedCornerShape(16.dp)).border(1.dp,CodegTheme.colors.surfaceStroke,RoundedCornerShape(16.dp)).combinedClickable(onClick={selected=node},onLongClickLabel="编辑卡片属性",onLongClick={properties=node})){
                    Text(data.coreText("title").ifEmpty{when(data.coreText("kind")){"terminal"->"终端";"note"->"便签";"file"->"文件";"folder"->"工程区域";else->"会话"}},modifier=Modifier.fillMaxWidth().pointerInput(node.coreText("id"),state.pending,zoom){detectDragGestures(onDragEnd={if(state.pending==null){val next=drag;mutate(buildJsonObject{put("type","move");putJsonArray("moves"){add(buildJsonObject{put("nodeId",node.coreText("id"));put("x",x+next.x);put("y",y+next.y)})}})};drag=Offset.Zero}){change,amount->change.consume();drag+=amount/(density*zoom)}}.padding(12.dp),style=MaterialTheme.typography.titleSmall)
                    if(data["collapsed"]!=JsonPrimitive(true)&&data.coreText("content").isNotEmpty())Text(data.coreText("content"),Modifier.padding(horizontal=12.dp),maxLines=8,color=CodegTheme.colors.textSecondary)
                    if(data["collapsed"]!=JsonPrimitive(true)&&data.coreText("path").isNotEmpty())Text(data.coreText("path"),Modifier.padding(12.dp),maxLines=2,color=CodegTheme.colors.textSecondary)
                }
            }
        }
    }
    if(creating)AlertDialog(onDismissRequest={creating=false},title={Text("添加便签")},text={OutlinedTextField(value=note,onValueChange={note=it},minLines=4)},confirmButton={TextButton(enabled=note.isNotBlank(),onClick={mutate(buildJsonObject{put("type","create");putJsonObject("node"){put("kind","note");put("title","便签");put("content",note);put("x",100);put("y",100);put("width",320);put("height",220)}});creating=false;note=""}){Text("添加")}},dismissButton={TextButton(onClick={creating=false}){Text("取消")}})
    selected?.let{node->val data=node["data"].coreObject();if(data.coreText("kind")=="terminal")RovaiTerminalDialog(model,id,node.coreText("id"),onClose={selected=null})else if(data.coreText("kind")=="file")RovaiCanvasFileNode(model,node){selected=null}else if(data.coreText("kind")=="conversation"&&data["objectReference"] !is JsonObject)RovaiCanvasConversationNode(model,id,node){selected=null}else AlertDialog(onDismissRequest={selected=null},title={Text(data.coreText("title").ifEmpty{"画布卡片"})},text={MarkdownContent(data.coreText("content").ifEmpty{data.coreText("path")})},confirmButton={TextButton(onClick={val reference=data["objectReference"].coreObject();val conversation=data["conversation"].coreObject();val threadId=reference.coreText("threadId").ifEmpty{conversation.coreText("threadId")};if(threadId.isNotEmpty())model.openThread(threadId);selected=null}){Text("打开")}},dismissButton={TextButton(onClick={selected=null}){Text("关闭")}})}
}
