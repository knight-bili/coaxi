package app.codeg.android.feature.main

import androidx.compose.foundation.Canvas
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
import androidx.compose.ui.Alignment
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.dp
import androidx.compose.ui.platform.LocalDensity
import app.codeg.android.core.designsystem.markdown.MarkdownContent
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.serialization.json.*
import kotlinx.coroutines.launch
import kotlinx.coroutines.delay

@Composable
fun RovaiNativeCanvas(model:RovaiWorkspaceViewModel,board:JsonObject){
    val density=LocalDensity.current.density
    val state by model.state.collectAsState();val id=board.coreText("id");var snapshot by remember(id){mutableStateOf<JsonObject?>(null)};var error by remember(id){mutableStateOf<String?>(null)};var selected by remember(id){mutableStateOf<JsonObject?>(null)};var creating by remember{mutableStateOf(false)};var zoom by remember(id){mutableFloatStateOf(1f)};var offset by remember(id){mutableStateOf(Offset.Zero)};val scope=rememberCoroutineScope();val revision=state.boards.firstOrNull{it.coreText("id")==id}?.coreLong("revision")
    LaunchedEffect(id,revision,state.lastResult){try{snapshot=model.query(RovaiOperation.CANVAS_NODES_SNAPSHOT,buildJsonObject{put("boardId",id)}).coreObject()}catch(failure:Exception){error=failure.message}}
    var selecting by remember(id){mutableStateOf(false)};var chosen by remember(id){mutableStateOf<Set<String>>(emptySet())};var bulk by remember(id){mutableStateOf(false)}
    val viewGeneration=model.connectionGeneration
    var viewOwner by remember(id,viewGeneration){mutableStateOf<String?>(null)};var viewReady by remember(id,viewGeneration){mutableStateOf(false)};var minimap by remember(id,viewGeneration){mutableStateOf(true)};var stage by remember{mutableStateOf(Size.Zero)};var menu by remember{mutableStateOf(false)};var packPlan by remember{mutableStateOf<JsonObject?>(null)}
    fun viewDocument()=RovaiCanvasViewport(offset.x/density,offset.y/density,zoom,minimap).document(id)
    LaunchedEffect(id,viewGeneration){viewReady=false;try{val identity=model.presentationIdentity();val owner="canvas-view:"+JsonArray(listOf(JsonPrimitive(identity),JsonPrimitive(id))).toString();val saved=RovaiCanvasViewport.parse(model.readBody(owner),id);require(viewGeneration==model.connectionGeneration);offset=Offset(saved.x*density,saved.y*density);zoom=saved.zoom;minimap=saved.minimap;viewOwner=owner;viewReady=true}catch(failure:Exception){error=failure.message;viewReady=true}}
    LaunchedEffect(id,viewOwner,viewReady,offset,zoom,minimap){val owner=viewOwner;if(viewReady&&owner!=null){delay(300);if(viewGeneration==model.connectionGeneration)try{val body=viewDocument();model.saveBody(owner,body);model.rememberBody(owner,body)}catch(failure:Exception){error=failure.message}}}
    val lastView by rememberUpdatedState(if(viewReady)viewDocument()else null)
    DisposableEffect(id,viewOwner,viewGeneration){val owner=viewOwner;onDispose{if(owner!=null&&viewGeneration==model.connectionGeneration)lastView?.let{model.rememberBody(owner,it)}}}
    var properties by remember(id){mutableStateOf<JsonObject?>(null)}
    properties?.let{node->snapshot?.let{graph->RovaiCanvasNodeProperties(model,id,node,graph){properties=null}}}
    fun mutate(mutation:JsonObject){val current=snapshot?:return;model.command(RovaiOperation.CANVAS_NODES_APPLY,buildJsonObject{put("boardId",id);put("expectedRevision",current["board"].coreObject().coreLong("revision"));put("mutation",mutation)})}
    Column(Modifier.fillMaxSize()){
        Row(Modifier.fillMaxWidth().padding(8.dp),horizontalArrangement=Arrangement.spacedBy(8.dp)){
            TextButton(onClick={creating=true},enabled=snapshot!=null&&state.pending==null){Text("添加到画布")}
            Box{TextButton(onClick={menu=true}){Text("视图")};DropdownMenu(menu,{menu=false}){
                DropdownMenuItem(text={Text("适合全部内容")},enabled=stage.width>0&&stage.height>0,onClick={val fit=canvasFit(snapshot?.get("nodes").coreArray(),stage.width/density,stage.height/density);offset=Offset(fit.x*density,fit.y*density);zoom=fit.zoom;menu=false})
                DropdownMenuItem(text={Text("恢复100%")},onClick={zoom=1f;menu=false})
                DropdownMenuItem(text={Text("缩小画布")},enabled=zoom>.1f,onClick={val next=(zoom/1.2f).coerceIn(.1f,2f);val center=Offset(stage.width/2,stage.height/2);offset=center-(center-offset)*(next/zoom);zoom=next;menu=false})
                DropdownMenuItem(text={Text("放大画布")},enabled=zoom<2f,onClick={val next=(zoom*1.2f).coerceIn(.1f,2f);val center=Offset(stage.width/2,stage.height/2);offset=center-(center-offset)*(next/zoom);zoom=next;menu=false})
                DropdownMenuItem(text={Text(if(minimap)"隐藏小地图"else"显示小地图")},onClick={minimap=!minimap;menu=false})
                DropdownMenuItem(text={Text("整理全部卡片")},enabled=snapshot!=null&&state.pending==null,onClick={snapshot?.let{graph->packPlan=buildJsonObject{put("boardId",id);put("expectedRevision",graph["board"].coreObject().coreLong("revision"));putJsonObject("mutation"){put("type","move");put("moves",canvasPackedMoves(graph["nodes"].coreArray()))}}};menu=false})
            }}
            TextButton(enabled=snapshot!=null,onClick={selecting=!selecting}){Text(if(selecting)"完成选择"else"多选")}
            Text("${(zoom*100).toInt()}%",modifier=Modifier.padding(8.dp))
        }
        if(selecting)Row(Modifier.fillMaxWidth().padding(horizontal=8.dp),horizontalArrangement=Arrangement.spacedBy(8.dp)){TextButton(onClick={chosen=snapshot?.get("nodes").coreArray().map{it.coreText("id")}.toSet()}){Text("全选")};TextButton(onClick={chosen=emptySet()}){Text("取消选择")};TextButton(enabled=chosen.isNotEmpty()&&state.pending==null,onClick={bulk=true}){Text("操作 ${chosen.size} 张卡片")}}
        error?.let{Text(it,color=CodegTheme.colors.danger,modifier=Modifier.padding(8.dp))}
        Box(Modifier.weight(1f).fillMaxWidth().clipToBounds().onSizeChanged{stage=Size(it.width.toFloat(),it.height.toFloat())}.background(CodegTheme.colors.bg).pointerInput(id,viewReady){detectTransformGestures{centroid,pan,scale,_->if(viewReady){val next=(zoom*scale).coerceIn(.1f,2f);offset=centroid-(centroid-offset)*(next/zoom)+pan;zoom=next}}}){
            val dotColor=CodegTheme.colors.textTertiary.copy(alpha=.3f)
            Canvas(Modifier.fillMaxSize()){var step=24*density*zoom;while(step<12f)step*=2;var px=((offset.x%step)+step)%step;while(px<size.width){var py=((offset.y%step)+step)%step;while(py<size.height){drawCircle(dotColor,.7f*density,Offset(px,py));py+=step};px+=step}}
            snapshot?.get("nodes").coreArray().forEach{node->val data=node["data"].coreObject();var drag by remember(node.coreText("id"),data.coreLong("x"),data.coreLong("y")){mutableStateOf(Offset.Zero)};val x=(data["x"] as? JsonPrimitive)?.floatOrNull?:0f;val y=(data["y"] as? JsonPrimitive)?.floatOrNull?:0f;val width=(data["width"] as? JsonPrimitive)?.floatOrNull?:320f;val height=(data["height"] as? JsonPrimitive)?.floatOrNull?:200f
                Column(Modifier.graphicsLayer{translationX=offset.x+(x+drag.x)*density*zoom;translationY=offset.y+(y+drag.y)*density*zoom;scaleX=zoom;scaleY=zoom;transformOrigin=androidx.compose.ui.graphics.TransformOrigin(0f,0f)}.size(width.dp,if(data["collapsed"]==JsonPrimitive(true))52.dp else height.dp).background(rovaiCanvasTint(data.coreText("color"))?.copy(alpha=.12f)?.compositeOver(CodegTheme.colors.bgElevated)?:CodegTheme.colors.bgElevated,RoundedCornerShape(16.dp)).border(if(node.coreText("id")in chosen)2.dp else 1.dp,if(node.coreText("id")in chosen)CodegTheme.colors.accent else CodegTheme.colors.surfaceStroke,RoundedCornerShape(16.dp)).combinedClickable(onClick={if(selecting){val nodeId=node.coreText("id");chosen=if(nodeId in chosen)chosen-nodeId else chosen+nodeId}else selected=node},onLongClickLabel="编辑卡片属性",onLongClick={properties=node})){
                    Text(data.coreText("title").ifEmpty{when(data.coreText("kind")){"terminal"->"终端";"note"->"便签";"file"->"文件";"folder"->"工程区域";else->"会话"}},modifier=Modifier.fillMaxWidth().pointerInput(node.coreText("id"),state.pending,zoom){detectDragGestures(onDragEnd={if(state.pending==null){val next=drag;mutate(buildJsonObject{put("type","move");putJsonArray("moves"){add(buildJsonObject{put("nodeId",node.coreText("id"));put("x",x+next.x);put("y",y+next.y)})}})};drag=Offset.Zero}){change,amount->change.consume();drag+=amount/(density*zoom)}}.padding(12.dp),style=MaterialTheme.typography.titleSmall)
                    if(data["collapsed"]!=JsonPrimitive(true)&&data.coreText("content").isNotEmpty())Text(data.coreText("content"),Modifier.padding(horizontal=12.dp),maxLines=8,color=CodegTheme.colors.textSecondary)
                    if(data["collapsed"]!=JsonPrimitive(true)&&data.coreText("path").isNotEmpty())Text(data.coreText("path"),Modifier.padding(12.dp),maxLines=2,color=CodegTheme.colors.textSecondary)
                }
            }
            if(minimap)Box(Modifier.align(Alignment.BottomEnd).padding(12.dp)){RovaiCanvasMiniMap(snapshot?.get("nodes").coreArray(),RovaiCanvasViewport(offset.x/density,offset.y/density,zoom,minimap),Size(stage.width/density,stage.height/density)){x,y->offset=Offset(stage.width/2-x*density*zoom,stage.height/2-y*density*zoom)}}
        }
    }
    if(bulk)snapshot?.let{graph->RovaiCanvasSelectionActions(model,id,graph,chosen){bulk=false}}
    packPlan?.let{plan->AlertDialog(onDismissRequest={packPlan=null},title={Text("整理全部卡片？")},text={Text("按原版卡片尺寸排布位置；不会改动会话、文件或卡片内容。")},confirmButton={TextButton(enabled=state.pending==null&&plan["expectedRevision"]==snapshot?.get("board").coreObject()["revision"],onClick={model.command(RovaiOperation.CANVAS_NODES_APPLY,plan);packPlan=null}){Text("整理位置")}},dismissButton={TextButton(onClick={packPlan=null}){Text("取消")}})}
    if(creating)snapshot?.let{graph->RovaiCanvasCreator(model,id,graph,(-offset.x/(density*zoom)+80f),(-offset.y/(density*zoom)+80f)){creating=false}}
    selected?.let{node->val data=node["data"].coreObject();if(data.coreText("kind")in listOf("folder","group","agent","custom"))RovaiCanvasRegion(model,id,node){selected=null}else if(data.coreText("kind")=="terminal")RovaiTerminalDialog(model,id,node.coreText("id"),onClose={selected=null})else if(data.coreText("kind")=="file")RovaiCanvasFileNode(model,node){selected=null}else if(data.coreText("kind")=="conversation"&&data["objectReference"] !is JsonObject)RovaiCanvasConversationNode(model,id,node){selected=null}else if(data.coreText("kind")=="conversation"&&data["objectReference"]is JsonObject)RovaiCanvasObjectNode(model,id,node){selected=null}else AlertDialog(onDismissRequest={selected=null},title={Text(data.coreText("title").ifEmpty{"画布卡片"})},text={MarkdownContent(data.coreText("content").ifEmpty{data.coreText("path")})},confirmButton={TextButton(onClick={val reference=data["objectReference"].coreObject();val conversation=data["conversation"].coreObject();val threadId=reference.coreText("threadId").ifEmpty{conversation.coreText("threadId")};if(threadId.isNotEmpty())model.openThread(threadId);selected=null}){Text("打开")}},dismissButton={TextButton(onClick={selected=null}){Text("关闭")}})}
}
