package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.dp
import androidx.compose.ui.platform.LocalDensity
import app.coaxi.android.core.designsystem.markdown.MarkdownContent
import app.coaxi.android.core.designsystem.theme.CoaxiTheme
import app.coaxi.android.core.network.CoaxiOperation
import kotlinx.serialization.json.*
import kotlinx.coroutines.launch
import kotlinx.coroutines.delay
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner

@Composable
fun CoaxiNativeCanvas(model:CoaxiWorkspaceViewModel,board:JsonObject){
    val density=LocalDensity.current.density
    val state by model.state.collectAsState();val id=board.coreText("id");var snapshot by remember(id){mutableStateOf<JsonObject?>(null)};var error by remember(id){mutableStateOf<String?>(null)};var selected by remember(id){mutableStateOf<JsonObject?>(null)};var creating by remember{mutableStateOf(false)};var zoom by remember(id){mutableFloatStateOf(1f)};var offset by remember(id){mutableStateOf(Offset.Zero)};val scope=rememberCoroutineScope();val revision=state.boards.firstOrNull{it.coreText("id")==id}?.coreLong("revision")
    LaunchedEffect(id,revision,state.lastResult){try{snapshot=model.query(CoaxiOperation.CANVAS_NODES_SNAPSHOT,buildJsonObject{put("boardId",id)}).coreObject()}catch(failure:Exception){error=failure.message}}
    var selecting by remember(id){mutableStateOf(false)};var chosen by remember(id){mutableStateOf<Set<String>>(emptySet())};var bulk by remember(id){mutableStateOf(false)};var bulkGraph by remember(id){mutableStateOf<JsonObject?>(null)}
    val viewGeneration=model.connectionGeneration
    var viewOwner by remember(id,viewGeneration){mutableStateOf<String?>(null)};var viewReady by remember(id,viewGeneration){mutableStateOf(false)};var minimap by remember(id,viewGeneration){mutableStateOf(true)};var expandedRegions by remember(id,viewGeneration){mutableStateOf<Set<String>>(emptySet())};var stage by remember{mutableStateOf(Size.Zero)};var menu by remember{mutableStateOf(false)};var packPlan by remember{mutableStateOf<JsonObject?>(null)}
    fun viewDocument()=CoaxiCanvasViewport(offset.x/density,offset.y/density,zoom,minimap,expandedRegions.toList().sorted()).document(id)
    LaunchedEffect(id,viewGeneration){viewReady=false;try{val identity=model.presentationIdentity();val owner="canvas-view:"+JsonArray(listOf(JsonPrimitive(identity),JsonPrimitive(id))).toString();val saved=CoaxiCanvasViewport.parse(model.readBody(owner),id);require(viewGeneration==model.connectionGeneration);offset=Offset(saved.x*density,saved.y*density);zoom=saved.zoom;minimap=saved.minimap;expandedRegions=saved.expandedRegions.toSet();viewOwner=owner;viewReady=true}catch(failure:Exception){error=failure.message;viewReady=true}}
    LaunchedEffect(id,viewOwner,viewReady,offset,zoom,minimap,expandedRegions){val owner=viewOwner;if(viewReady&&owner!=null){delay(300);if(viewGeneration==model.connectionGeneration)try{val body=viewDocument();model.saveBody(owner,body);model.rememberBody(owner,body)}catch(failure:Exception){error=failure.message}}}
    val lastView by rememberUpdatedState(if(viewReady)viewDocument()else null)
    DisposableEffect(id,viewOwner,viewGeneration){val owner=viewOwner;onDispose{if(owner!=null&&viewGeneration==model.connectionGeneration)lastView?.let{model.rememberBody(owner,it)}}}
    val catalog=remember(id,viewGeneration){CoaxiCanvasCatalog()};val currentNodes by rememberUpdatedState(snapshot?.get("nodes").coreArray());val lifecycle=LocalLifecycleOwner.current
    var active by remember(lifecycle){mutableStateOf(lifecycle.lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED))};var memberOpening by remember(id){mutableStateOf<Pair<JsonObject,JsonObject>?>(null)}
    DisposableEffect(lifecycle){val observer=LifecycleEventObserver{_,event->if(event==Lifecycle.Event.ON_STOP)active=false else if(event==Lifecycle.Event.ON_START)active=true};lifecycle.lifecycle.addObserver(observer);onDispose{lifecycle.lifecycle.removeObserver(observer)}}
    LaunchedEffect(id,viewGeneration,active,snapshot!=null){catalog.configure("$viewGeneration:$id");if(active&&snapshot!=null)while(true){catalog.load(model,currentNodes);delay(20_000)}}
    val bindings=JsonArray(currentNodes.map{node->buildJsonObject{put("id",node["id"]?:JsonNull);val data=node["data"].coreObject();listOf("kind","folderPath","folderGroupId","agentId","runtimeKind","conversation","conversations").forEach{key->data[key]?.let{put(key,it)}}}}).toString()
    LaunchedEffect(bindings,active){if(active&&snapshot!=null){delay(300);catalog.load(model,currentNodes)}}
    var properties by remember(id){mutableStateOf<JsonObject?>(null)}
    properties?.let{node->snapshot?.let{graph->CoaxiCanvasNodeProperties(model,id,node,graph){properties=null}}}
    val currentGraphRevision by rememberUpdatedState(snapshot?.get("board").coreObject().coreLong("revision"))
    val presentationNodes=canvasPresentationNodes(currentNodes,catalog.snapshot,expandedRegions)
    fun mutate(mutation:JsonObject,expectedRevision:Long?=null){val current=snapshot?:return;model.command(CoaxiOperation.CANVAS_NODES_APPLY,buildJsonObject{put("boardId",id);put("expectedRevision",expectedRevision?:current["board"].coreObject().coreLong("revision"));put("mutation",mutation)})}
    fun nodeMutation(node:JsonObject,mutation:JsonObject,captured:Long){if(captured!=currentGraphRevision){error="画布已变化，请核对当前位置后重新拖动";return};try{val moves=mutation["moves"].coreArray();val actual=if(mutation.coreText("type")=="move"&&moves.size==1){val move=moves.first();canvasDropMutation(node,null,(move["x"]as?JsonPrimitive)?.doubleOrNull?:error("拖动位置无效"),(move["y"]as?JsonPrimitive)?.doubleOrNull?:error("拖动位置无效"),presentationNodes,snapshot?.coreLong("memberTransferVersion")?:0)}else mutation;if(actual!=null)mutate(actual,captured)}catch(failure:Exception){error=failure.message}}
    Column(Modifier.fillMaxSize()){
        Row(Modifier.fillMaxWidth().padding(8.dp),horizontalArrangement=Arrangement.spacedBy(8.dp)){
            TextButton(onClick={creating=true},enabled=snapshot!=null&&state.pending==null){Text(stringResource(R.string.coaxi_fusion_675d681353241631))}
            Box{TextButton(onClick={menu=true}){Text(stringResource(R.string.coaxi_fusion_1c5c067138704dda))};DropdownMenu(menu,{menu=false}){
                DropdownMenuItem(text={Text(stringResource(R.string.coaxi_fusion_8c8cb4673951aa99))},enabled=stage.width>0&&stage.height>0,onClick={val fit=canvasFit(presentationNodes,stage.width/density,stage.height/density);offset=Offset(fit.x*density,fit.y*density);zoom=fit.zoom;menu=false})
                DropdownMenuItem(text={Text(stringResource(R.string.coaxi_fusion_b4e8c17a58854f46))},onClick={zoom=1f;menu=false})
                DropdownMenuItem(text={Text(stringResource(R.string.coaxi_fusion_a131556214329bb0))},enabled=zoom>.1f,onClick={val next=(zoom/1.2f).coerceIn(.1f,2f);val center=Offset(stage.width/2,stage.height/2);offset=center-(center-offset)*(next/zoom);zoom=next;menu=false})
                DropdownMenuItem(text={Text(stringResource(R.string.coaxi_fusion_c6c87ee566a0111e))},enabled=zoom<2f,onClick={val next=(zoom*1.2f).coerceIn(.1f,2f);val center=Offset(stage.width/2,stage.height/2);offset=center-(center-offset)*(next/zoom);zoom=next;menu=false})
                DropdownMenuItem(text={Text(if(minimap)"隐藏小地图"else"显示小地图")},onClick={minimap=!minimap;menu=false})
                DropdownMenuItem(text={Text(stringResource(R.string.coaxi_fusion_51e631e743f4b84c))},enabled=snapshot!=null&&state.pending==null,onClick={snapshot?.let{graph->packPlan=buildJsonObject{put("boardId",id);put("expectedRevision",graph["board"].coreObject().coreLong("revision"));putJsonObject("mutation"){put("type","move");put("moves",canvasPackedMoves(presentationNodes))}}};menu=false})
            }}
            TextButton(enabled=snapshot!=null,onClick={selecting=!selecting}){Text(if(selecting)"完成选择"else"多选")}
            Text("${(zoom*100).toInt()}%",modifier=Modifier.padding(8.dp))
        }
        if(selecting)Row(Modifier.fillMaxWidth().padding(horizontal=8.dp),horizontalArrangement=Arrangement.spacedBy(8.dp)){TextButton(onClick={chosen=snapshot?.get("nodes").coreArray().map{it.coreText("id")}.toSet()}){Text(stringResource(R.string.coaxi_fusion_3a5040b68abf75f9))};TextButton(onClick={chosen=emptySet()}){Text(stringResource(R.string.coaxi_fusion_74966e20df2ea9fd))};TextButton(enabled=chosen.isNotEmpty()&&state.pending==null,onClick={bulkGraph=snapshot;bulk=true}){Text("操作 ${chosen.size} 张卡片")}}
        error?.let{Text(it,color=CoaxiTheme.colors.danger,modifier=Modifier.padding(8.dp))}
        Box(Modifier.weight(1f).fillMaxWidth().clipToBounds().onSizeChanged{stage=Size(it.width.toFloat(),it.height.toFloat())}.background(CoaxiTheme.colors.bg).pointerInput(id,viewReady){detectTransformGestures{centroid,pan,scale,_->if(viewReady){val next=(zoom*scale).coerceIn(.1f,2f);offset=centroid-(centroid-offset)*(next/zoom)+pan;zoom=next}}}){
            val dotColor=CoaxiTheme.colors.textTertiary.copy(alpha=.3f)
            Canvas(Modifier.fillMaxSize()){var step=24*density*zoom;while(step<12f)step*=2;var px=((offset.x%step)+step)%step;while(px<size.width){var py=((offset.y%step)+step)%step;while(py<size.height){drawCircle(dotColor,.7f*density,Offset(px,py));py+=step};px+=step}}
            snapshot?.get("nodes").coreArray().forEach{node->
                CoaxiCanvasCard(node,zoom,offset,currentGraphRevision,state.pending!=null||selecting,node.coreText("id")in chosen,catalog.snapshot,state.members,node.coreText("id")in expandedRegions,onExpand={val nodeId=node.coreText("id");expandedRegions=if(nodeId in expandedRegions)expandedRegions-nodeId else expandedRegions+nodeId},onOpen={if(selecting){val nodeId=node.coreText("id");chosen=if(nodeId in chosen)chosen-nodeId else chosen+nodeId}else selected=node},onEdit={properties=node},onMember={reference->memberOpening=node to reference},onDetach={reference->val data=node["data"].coreObject();val x=(data["x"]as?JsonPrimitive)?.doubleOrNull?:0.0;val y=(data["y"]as?JsonPrimitive)?.doubleOrNull?:0.0;val width=(data["width"]as?JsonPrimitive)?.doubleOrNull?:484.0;mutate(buildJsonObject{put("type","detach");put("regionId",node.coreText("id"));put("conversation",reference);put("x",x+width+48);put("y",y)})},onMutation={mutation,captured->nodeMutation(node,mutation,captured)},onMemberMove={reference,x,y,captured->if(captured!=currentGraphRevision)error="画布已变化，请核对后重新拖动"else try{canvasDropMutation(node,reference,x.toDouble(),y.toDouble(),presentationNodes,snapshot?.coreLong("memberTransferVersion")?:0)?.let{mutate(it,captured)}}catch(failure:Exception){error=failure.message}})
            }
            if(minimap)Box(Modifier.align(Alignment.BottomEnd).padding(12.dp)){CoaxiCanvasMiniMap(presentationNodes,CoaxiCanvasViewport(offset.x/density,offset.y/density,zoom,minimap),Size(stage.width/density,stage.height/density)){x,y->offset=Offset(stage.width/2-x*density*zoom,stage.height/2-y*density*zoom)}}
        }
    }
    memberOpening?.let{(region,reference)->CoaxiCanvasConversationNode(model,id,region,memberReference=reference){memberOpening=null}}
    if(bulk)bulkGraph?.let{graph->CoaxiCanvasSelectionActions(model,id,graph,chosen){bulk=false}}
    packPlan?.let{plan->AlertDialog(onDismissRequest={packPlan=null},title={Text(stringResource(R.string.coaxi_fusion_93a9d736b4fac883))},text={Text(stringResource(R.string.coaxi_fusion_a91d8bb1eac17add))},confirmButton={TextButton(enabled=state.pending==null&&plan["expectedRevision"]==snapshot?.get("board").coreObject()["revision"],onClick={model.command(CoaxiOperation.CANVAS_NODES_APPLY,plan);packPlan=null}){Text(stringResource(R.string.coaxi_fusion_f42b4e892ec6acbd))}},dismissButton={TextButton(onClick={packPlan=null}){Text(stringResource(R.string.coaxi_fusion_2cd0f3be8738a86c))}})}
    if(creating)snapshot?.let{graph->CoaxiCanvasCreator(model,id,graph,(-offset.x/(density*zoom)+80f),(-offset.y/(density*zoom)+80f)){creating=false}}
    selected?.let{node->val data=node["data"].coreObject();if(data.coreText("kind")in listOf("folder","group","agent","custom"))CoaxiCanvasRegion(model,id,node){selected=null}else if(data.coreText("kind")=="terminal")CoaxiTerminalDialog(model,id,node.coreText("id"),onClose={selected=null})else if(data.coreText("kind")=="file"&&data["fileReference"] !is JsonObject)snapshot?.let{graph->CoaxiCanvasCreator(model,id,graph,0f,0f,bindingNode=node){selected=null}}else if(data.coreText("kind")=="file")CoaxiCanvasFileNode(model,node){selected=null}else if(data.coreText("kind")=="conversation"&&data["objectReference"] !is JsonObject)CoaxiCanvasConversationNode(model,id,node){selected=null}else if(data.coreText("kind")=="conversation"&&data["objectReference"]is JsonObject)CoaxiCanvasObjectNode(model,id,node){selected=null}else AlertDialog(onDismissRequest={selected=null},title={Text(data.coreText("title").ifEmpty{"画布卡片"})},text={MarkdownContent(data.coreText("content").ifEmpty{data.coreText("path")})},confirmButton={TextButton(onClick={val reference=data["objectReference"].coreObject();val conversation=data["conversation"].coreObject();val threadId=reference.coreText("threadId").ifEmpty{conversation.coreText("threadId")};if(threadId.isNotEmpty())model.openThread(threadId);selected=null}){Text(stringResource(R.string.coaxi_fusion_c771248e511fbf93))}},dismissButton={TextButton(onClick={selected=null}){Text(stringResource(R.string.coaxi_fusion_3fd47edce45b3603))}})}
}
