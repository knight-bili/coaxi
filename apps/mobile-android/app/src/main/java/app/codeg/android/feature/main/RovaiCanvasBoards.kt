package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.grid.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.util.UUID

val rovaiCanvasColors=listOf("" to "默认","neutral" to "中性","zinc" to "锌灰","slate" to "蓝灰","stone" to "石灰","gray" to "灰色","red" to "红色","rose" to "玫瑰","orange" to "橙色","green" to "绿色","blue" to "蓝色","yellow" to "黄色","violet" to "紫色")

/** Codeg BoardCard's footprint, copy and separate action menu; summaries must match the board revision. */
@Composable fun RovaiCanvasBoards(model:RovaiWorkspaceViewModel,onOpen:(JsonObject)->Unit){
    val state=model.state.collectAsState().value;var search by remember{mutableStateOf("")};var creating by remember{mutableStateOf(false)};var editing by remember{mutableStateOf<JsonObject?>(null)};var deleting by remember{mutableStateOf<JsonObject?>(null)};var managingGroups by remember{mutableStateOf(false)}
    val blocked=state.busy||state.pending!=null||!state.connected
    LazyVerticalGrid(columns=GridCells.Adaptive(180.dp),modifier=Modifier.fillMaxSize(),contentPadding=PaddingValues(16.dp),horizontalArrangement=Arrangement.spacedBy(12.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
        item(span={GridItemSpan(maxLineSpan)}){Column(verticalArrangement=Arrangement.spacedBy(10.dp)){Row{Text(stringResource(R.string.rovai_fusion_3906276fd1bda417),Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(enabled=!blocked,onClick={managingGroups=true}){Text(stringResource(R.string.rovai_fusion_d03a6f7d0c79ae20))};TextButton(enabled=!blocked,onClick={creating=true}){Text(stringResource(R.string.rovai_fusion_37a11f07f5c819ab))}};Text(stringResource(R.string.rovai_fusion_02426d55dd8e5b1f),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary);OutlinedTextField(search,{search=it},Modifier.fillMaxWidth(),label={Text(stringResource(R.string.rovai_fusion_d08f46ece5352994))},singleLine=true)}}
        val rows=state.boards.filter{(it.coreText("name")+" "+it.coreText("description")).contains(search,true)}
        items(rows,key={it.coreText("id")}){board->
            var menu by remember(board.coreText("id")){mutableStateOf(false)}
            val summary=state.boardSummaries.firstOrNull{it.coreText("boardId")==board.coreText("id")&&it.coreLong("revision")==board.coreLong("revision")}
            GlassRow(Modifier.fillMaxWidth().combinedClickable(onClick={onOpen(board)},onLongClick={menu=true})){
                RovaiCanvasBoardPreview(summary?.get("preview").coreArray())
                Row{Text(board.coreText("name").ifEmpty{"未命名画布"},Modifier.weight(1f),style=MaterialTheme.typography.titleSmall);Box{TextButton(onClick={menu=true}){Text(stringResource(R.string.rovai_fusion_ed31fbb483ee1b0a))};DropdownMenu(expanded=menu,onDismissRequest={menu=false}){
                    DropdownMenuItem(text={Text(stringResource(R.string.rovai_fusion_860a53c4a5473854))},onClick={menu=false;onOpen(board)})
                    DropdownMenuItem(enabled=!blocked,text={Text(stringResource(R.string.rovai_fusion_bddfc9becf063a32))},onClick={menu=false;editing=board})
                    DropdownMenuItem(enabled=!blocked,text={Text(stringResource(R.string.rovai_fusion_9f025ebf8b256707),color=CodegTheme.colors.danger)},onClick={menu=false;deleting=board})
                }}}
                if(board.coreText("description").isNotEmpty())Text(board.coreText("description"),maxLines=3,color=CodegTheme.colors.textSecondary,style=MaterialTheme.typography.bodySmall)
                Text(summary?.let{"${it.coreLong("nodeCount")}个节点"}?:"统计暂不可用",style=MaterialTheme.typography.labelSmall,color=CodegTheme.colors.textSecondary)
                Text(board.coreText("updatedAt"),style=MaterialTheme.typography.labelSmall,color=CodegTheme.colors.textSecondary)
            }
        }
        if(rows.isEmpty())item(span={GridItemSpan(maxLineSpan)}){Text(if(state.boards.isEmpty())"还没有画布，创建一个画布开始整理工作。" else "没有符合条件的画布",modifier=Modifier.padding(vertical=32.dp),color=CodegTheme.colors.textSecondary)}
    }
    if(managingGroups)RovaiProjectGroups(model){managingGroups=false}
    if(creating||editing!=null||deleting!=null)RovaiCanvasBoardEditor(model,editing?:deleting,deleting!=null){creating=false;editing=null;deleting=null}
}

@Composable fun RovaiCanvasBoardPreview(rects:List<JsonObject>){
    val ink=CodegTheme.colors.textSecondary;val surface=CodegTheme.colors.bgElevated
    Canvas(Modifier.fillMaxWidth().height(92.dp)){
        if(rects.isEmpty()){drawRect(ink.copy(alpha=.18f),topLeft=Offset(size.width*.38f,size.height*.2f),size=Size(size.width*.24f,size.height*.6f),style=Stroke(1.dp.toPx()));return@Canvas}
        fun JsonObject.number(key:String)=(this[key]as?JsonPrimitive)?.doubleOrNull?:0.0
        val minX=rects.minOf{it.number("x")};val minY=rects.minOf{it.number("y")};val maxX=rects.maxOf{it.number("x")+it.number("width")};val maxY=rects.maxOf{it.number("y")+it.number("height")}
        val width=(maxX-minX).coerceAtLeast(1200.0);val height=(maxY-minY).coerceAtLeast(720.0);val left=(minX+maxX)/2-width*.58;val top=(minY+maxY)/2-height*.58
        val scale=minOf(size.width/(width*1.16),size.height/(height*1.16));val dx=(size.width-width*1.16*scale)/2;val dy=(size.height-height*1.16*scale)/2
        rects.take(200).forEach{rect->val region=rect.coreText("kind")in listOf("folder","group","agent","custom");val tint=rovaiCanvasTint(rect.coreText("color"));val terminal=rect.coreText("kind")=="terminal";val position=Offset((dx+(rect.number("x")-left)*scale).toFloat(),(dy+(rect.number("y")-top)*scale).toFloat());val extent=Size((rect.number("width")*scale).toFloat(),(rect.number("height")*scale).toFloat());drawRect(when{terminal->ink.copy(alpha=.55f);region->(tint?:ink).copy(alpha=if(tint!=null).14f else .06f);tint!=null->tint.copy(alpha=.35f);else->surface},position,extent);drawRect((if(region)tint?:ink else ink).copy(alpha=.22f),position,extent,style=Stroke(1.dp.toPx()))}
    }
}

@Composable fun RovaiCanvasBoardEditor(model:RovaiWorkspaceViewModel,board:JsonObject?,remove:Boolean,onClose:()->Unit){
    val state=model.state.collectAsState().value;val scope=rememberCoroutineScope();val generation=remember{model.connectionGeneration};val id=board?.coreText("id");val owner="canvas-board:${id?:"new"}:${if(remove)"remove" else "edit"}"
    var name by remember{mutableStateOf(board?.coreText("name").orEmpty())};var description by remember{mutableStateOf(board?.coreText("description").orEmpty())};var color by remember{mutableStateOf(board?.coreText("color").orEmpty())};var revision by remember{mutableLongStateOf(board?.coreLong("revision")?:0)}
    var ready by remember{mutableStateOf(false)};var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)};var commandId by remember{mutableStateOf(UUID.randomUUID().toString())}
    val latest=state.boards.firstOrNull{it.coreText("id")==id};val stale=id!=null&&(latest==null||latest.coreLong("revision")!=revision);val blocked=!ready||busy||state.pending!=null||state.busy||generation!=model.connectionGeneration
    val summary=state.boardSummaries.firstOrNull{it.coreText("boardId")==id&&it.coreLong("revision")==revision&&!stale}
    fun record()=buildJsonObject{put("version",1);put("boardId",id?.let(::JsonPrimitive)?:JsonNull);put("name",name);put("description",description);put("color",color);put("revision",revision);put("commandId",commandId)}.toString()
    fun persist(){if(ready&&generation==model.connectionGeneration)model.rememberBody(owner,record())}
    fun close(){if(busy)return;if(!ready||generation!=model.connectionGeneration){onClose();return};val saved=record();scope.launch{try{model.saveBody(owner,saved);model.rememberBody(owner,saved);onClose()}catch(error:Exception){issue=error.message}}}
    LaunchedEffect(owner){try{val raw=model.readBody(owner);if(raw.isNotEmpty()){val value=Json.parseToJsonElement(raw).coreObject();require(value.coreLong("version")==1L&&(value["boardId"]as?JsonPrimitive)?.contentOrNull==id){"原画布草稿无法读取，原记录保留"};name=value.coreText("name");description=value.coreText("description");color=value.coreText("color");revision=value.coreLong("revision");commandId=value.coreText("commandId").ifEmpty{UUID.randomUUID().toString()}};ready=true}catch(error:Exception){issue=error.message}}
    LaunchedEffect(state.lastResult){val receipt=state.lastResult.coreObject()["commandResult"].coreObject().ifEmpty{state.lastResult.coreObject()};if(state.lastInputOwner==owner&&receipt.coreText("commandId")==commandId){if(receipt.coreText("status")in listOf("applied","accepted")){ready=false;onClose()}else if(receipt.coreText("status")=="rejected"){commandId=UUID.randomUUID().toString();issue=receipt.coreText("code");persist()}}}
    AlertDialog(onDismissRequest={close()},title={Text(if(remove)"删除画布？" else if(board==null)"新建画布" else "编辑画布")},text={Column(verticalArrangement=Arrangement.spacedBy(10.dp)){
        issue?.let{Text(it,color=CodegTheme.colors.danger)}
        if(remove){Text(stringResource(R.string.rovai_fusion_672997ee74decd14));Text(summary?.let{"包含${it.coreLong("nodeCount")}个节点，其中${it.coreLong("terminalCount")}张终端卡；仍运行的终端会关闭。"}?:"当前数量统计暂不可用，不能假定画布为空。")}
        else{OutlinedTextField(name,{name=it;persist()},label={Text(stringResource(R.string.rovai_fusion_d44e9b3d3b31d37b))},enabled=!blocked);OutlinedTextField(description,{description=it;persist()},label={Text(stringResource(R.string.rovai_fusion_bd5b18f0c13903b4))},enabled=!blocked,minLines=3);var picking by remember{mutableStateOf(false)};Box{TextButton(enabled=!blocked,onClick={picking=true}){Text(stringResource(R.string.rovai_fusion_545908ae60436702)+(rovaiCanvasColors.firstOrNull{it.first==color}?.second?:color))};DropdownMenu(expanded=picking,onDismissRequest={picking=false}){rovaiCanvasColors.forEach{(key,label)->DropdownMenuItem(text={RovaiCanvasColorLabel(key,label)},onClick={color=key;picking=false;persist()})}}}}
        if(stale){Text(stringResource(R.string.rovai_fusion_e5f805e104218967),color=CodegTheme.colors.danger);if(!remove&&latest!=null)TextButton(enabled=!blocked,onClick={revision=latest.coreLong("revision");commandId=UUID.randomUUID().toString();persist()}){Text(stringResource(R.string.rovai_fusion_9ded9d0b711d6373))}}
        if(state.pending!=null)Text(stringResource(R.string.rovai_fusion_a416feafa697a13c),color=CodegTheme.colors.textSecondary)
    }},confirmButton={TextButton(enabled=!blocked&&!stale&&name.codePointCount(0,name.length)<=200&&description.codePointCount(0,description.length)<=2000,onClick={val operation=if(remove)RovaiOperation.CANVAS_BOARDS_DELETE else if(board==null)RovaiOperation.CANVAS_BOARDS_CREATE else RovaiOperation.CANVAS_BOARDS_UPDATE;val command=buildJsonObject{if(id!=null){put("boardId",id);put("expectedRevision",revision)};if(!remove){put("name",name);put("description",description);put("color",color)}};val params=buildJsonObject{put("commandId",commandId);put("command",command)};val saved=record();busy=true;scope.launch{try{model.saveBody(owner,saved);model.rememberBody(owner,saved);require(generation==model.connectionGeneration);model.executeOwned(operation,params,owner,saved)}catch(error:Exception){issue=error.message}finally{busy=false}}}){Text(if(remove)"删除画布" else "保存")}},dismissButton={TextButton(enabled=!busy,onClick={close()}){Text(stringResource(R.string.rovai_fusion_f9c624d9f11fbb23))}})
}

@Composable fun RovaiCanvasColorLabel(value:String,label:String){
    val tint=rovaiCanvasTint(value);val ink=CodegTheme.colors.textSecondary
    Row(horizontalArrangement=Arrangement.spacedBy(8.dp)){
        Canvas(Modifier.size(12.dp)){if(tint==null)drawCircle(ink,style=Stroke(1.dp.toPx()))else drawCircle(tint)}
        Text(label)
    }
}
