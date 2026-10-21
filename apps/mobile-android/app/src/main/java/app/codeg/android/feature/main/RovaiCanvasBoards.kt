package app.codeg.android.feature.main

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
    val state=model.state.collectAsState().value;var search by remember{mutableStateOf("")};var creating by remember{mutableStateOf(false)};var editing by remember{mutableStateOf<JsonObject?>(null)};var deleting by remember{mutableStateOf<JsonObject?>(null)}
    val blocked=state.busy||state.pending!=null||!state.connected
    LazyVerticalGrid(columns=GridCells.Adaptive(180.dp),modifier=Modifier.fillMaxSize(),contentPadding=PaddingValues(16.dp),horizontalArrangement=Arrangement.spacedBy(12.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
        item(span={GridItemSpan(maxLineSpan)}){Column(verticalArrangement=Arrangement.spacedBy(10.dp)){Row{Text("画布",Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(enabled=!blocked,onClick={creating=true}){Text("新建画布")}};Text("每个画布都有自己的区域、会话卡片、便签、文件和终端。",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary);OutlinedTextField(search,{search=it},Modifier.fillMaxWidth(),label={Text("搜索画布")},singleLine=true)}}
        val rows=state.boards.filter{(it.coreText("name")+" "+it.coreText("description")).contains(search,true)}
        items(rows,key={it.coreText("id")}){board->
            var menu by remember(board.coreText("id")){mutableStateOf(false)}
            val summary=state.boardSummaries.firstOrNull{it.coreText("boardId")==board.coreText("id")&&it.coreLong("revision")==board.coreLong("revision")}
            GlassRow(Modifier.fillMaxWidth().combinedClickable(onClick={onOpen(board)},onLongClick={menu=true})){
                RovaiCanvasBoardPreview(summary?.get("preview").coreArray())
                Row{Text(board.coreText("name").ifEmpty{"未命名画布"},Modifier.weight(1f),style=MaterialTheme.typography.titleSmall);Box{TextButton(onClick={menu=true}){Text("操作")};DropdownMenu(expanded=menu,onDismissRequest={menu=false}){
                    DropdownMenuItem(text={Text("打开画布")},onClick={menu=false;onOpen(board)})
                    DropdownMenuItem(enabled=!blocked,text={Text("编辑画布")},onClick={menu=false;editing=board})
                    DropdownMenuItem(enabled=!blocked,text={Text("删除画布",color=CodegTheme.colors.danger)},onClick={menu=false;deleting=board})
                }}}
                if(board.coreText("description").isNotEmpty())Text(board.coreText("description"),maxLines=3,color=CodegTheme.colors.textSecondary,style=MaterialTheme.typography.bodySmall)
                Text(summary?.let{"${it.coreLong("nodeCount")}个节点"}?:"统计暂不可用",style=MaterialTheme.typography.labelSmall,color=CodegTheme.colors.textSecondary)
                Text(board.coreText("updatedAt"),style=MaterialTheme.typography.labelSmall,color=CodegTheme.colors.textSecondary)
            }
        }
        if(rows.isEmpty())item(span={GridItemSpan(maxLineSpan)}){Text(if(state.boards.isEmpty())"还没有画布，创建一个画布开始整理工作。" else "没有符合条件的画布",modifier=Modifier.padding(vertical=32.dp),color=CodegTheme.colors.textSecondary)}
    }
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
        rects.take(200).forEach{rect->val region=rect.coreText("kind")in listOf("folder","group","agent","custom");val position=Offset((dx+(rect.number("x")-left)*scale).toFloat(),(dy+(rect.number("y")-top)*scale).toFloat());val extent=Size((rect.number("width")*scale).toFloat(),(rect.number("height")*scale).toFloat());drawRect(if(region)ink.copy(alpha=.06f)else surface,position,extent);drawRect(ink.copy(alpha=.22f),position,extent,style=Stroke(1.dp.toPx()))}
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
        if(remove){Text("删除画布会移除其中的卡片、区域和便签，保留原对话、使命、任务及工程文件。");Text(summary?.let{"包含${it.coreLong("nodeCount")}个节点，其中${it.coreLong("terminalCount")}张终端卡；仍运行的终端会关闭。"}?:"当前数量统计暂不可用，不能假定画布为空。")}
        else{OutlinedTextField(name,{name=it;persist()},label={Text("名称")},enabled=!blocked);OutlinedTextField(description,{description=it;persist()},label={Text("说明（可选）")},enabled=!blocked,minLines=3);var picking by remember{mutableStateOf(false)};Box{TextButton(enabled=!blocked,onClick={picking=true}){Text("颜色 · "+(rovaiCanvasColors.firstOrNull{it.first==color}?.second?:color))};DropdownMenu(expanded=picking,onDismissRequest={picking=false}){rovaiCanvasColors.forEach{(key,label)->DropdownMenuItem(text={Text(label)},onClick={color=key;picking=false;persist()})}}}}
        if(stale){Text("画布已有新修改，当前输入保留。请核对后继续。",color=CodegTheme.colors.danger);if(!remove&&latest!=null)TextButton(enabled=!blocked,onClick={revision=latest.coreLong("revision");commandId=UUID.randomUUID().toString();persist()}){Text("使用最新画布版本")}}
        if(state.pending!=null)Text("原请求待核对；可关闭后从工作区核对或重试。",color=CodegTheme.colors.textSecondary)
    }},confirmButton={TextButton(enabled=!blocked&&!stale&&name.codePointCount(0,name.length)<=200&&description.codePointCount(0,description.length)<=2000,onClick={val operation=if(remove)RovaiOperation.CANVAS_BOARDS_DELETE else if(board==null)RovaiOperation.CANVAS_BOARDS_CREATE else RovaiOperation.CANVAS_BOARDS_UPDATE;val command=buildJsonObject{if(id!=null){put("boardId",id);put("expectedRevision",revision)};if(!remove){put("name",name);put("description",description);put("color",color)}};val params=buildJsonObject{put("commandId",commandId);put("command",command)};val saved=record();busy=true;scope.launch{try{model.saveBody(owner,saved);model.rememberBody(owner,saved);require(generation==model.connectionGeneration);model.executeOwned(operation,params,owner,saved)}catch(error:Exception){issue=error.message}finally{busy=false}}}){Text(if(remove)"删除画布" else "保存")}},dismissButton={TextButton(enabled=!busy,onClick={close()}){Text("关闭并保留草稿")}})
}
