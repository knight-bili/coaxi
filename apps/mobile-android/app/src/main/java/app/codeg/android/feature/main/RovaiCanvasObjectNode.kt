package app.codeg.android.feature.main

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import app.codeg.android.core.designsystem.diff.DiffView
import app.codeg.android.core.designsystem.diff.UnifiedDiff
import app.codeg.android.core.designsystem.markdown.MarkdownContent
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.util.UUID

/** CanvasObjectDialog/readCanvasObject: saved object IDs and public anchors, never a current-object fallback. */
@Composable fun RovaiCanvasObjectNode(model:RovaiWorkspaceViewModel,boardId:String,node:JsonObject,onClose:()->Unit){
    val state=model.state.collectAsState().value;val scope=rememberCoroutineScope();val generation=remember{model.connectionGeneration}
    val data=node["data"].coreObject();val reference=data["conversation"].coreObject();val target=data["objectReference"].coreObject();val threadId=reference.coreText("threadId");val kind=target.coreText("kind");val objectId=target.coreText(if(kind=="task")"taskId" else "missionId")
    var value by remember(node.coreText("id")){mutableStateOf<JsonObject?>(null)};var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)};var version by remember{mutableLongStateOf(-1)};var workspace by remember{mutableStateOf(false)};var taskProjection by remember{mutableStateOf<JsonObject?>(null)}
    suspend fun load():JsonObject{
        require(data.coreText("kind")=="conversation"&&kind in listOf("mission","task","mission_diff")&&threadId.isNotEmpty()&&(reference["conversationId"]==null||reference["conversationId"]==JsonNull)){"对象卡片的公开会话锚点无效"}
        val graph=model.query(RovaiOperation.CANVAS_NODES_SNAPSHOT,buildJsonObject{put("boardId",boardId)}).coreObject();val saved=graph["nodes"].coreArray().firstOrNull{it.coreText("id")==node.coreText("id")}?.get("data").coreObject()
        require(saved["objectReference"]==target&&saved["conversation"]==reference&&saved.coreText("kind")=="conversation"){"原卡片已删除或绑定已变化，引用保留"}
        val result=if(kind=="task")model.query(RovaiOperation.TASKS_GET,buildJsonObject{put("threadId",threadId);put("taskId",objectId)}).coreObject()else model.query(RovaiOperation.MISSIONS_GET,buildJsonObject{put("missionId",objectId)}).coreObject()
        require(result.coreText(if(kind=="task")"taskId" else "missionId")==objectId&&result.coreText("threadId")==threadId){"原对象已删除或归属已变化，卡片引用保留"}
        if(kind=="mission_diff"){val delivery=model.query(RovaiOperation.MISSIONS_DELIVERY,buildJsonObject{put("missionId",objectId)}).coreObject();val bound=delivery["workspace"]as?JsonObject;require(delivery.coreText("threadId")==threadId&&(bound==null||bound.coreText("threadId")==threadId&&bound.coreText("missionId")==objectId)){"原累计Diff工作区归属已变化"}}
        require(generation==model.connectionGeneration){"工作区已切换"};value=result;version=state.resourceVersion;issue=null;return result
    }
    fun refresh(){if(busy)return;busy=true;scope.launch{try{load()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){value=null;issue=error.message}finally{busy=false}}}
    LaunchedEffect(node.coreText("id")){busy=true;try{load()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    if(workspace)value?.let{source->if(kind=="task")RovaiTaskEditor(model,threadId,source,projection=taskProjection){workspace=false}else RovaiMissionDetail(model,source,onClose={workspace=false})}
    Dialog(onDismissRequest=onClose,properties=DialogProperties(usePlatformDefaultWidth=false)){
        Surface(Modifier.fillMaxSize(),color=CodegTheme.colors.bgElevated){Column(Modifier.fillMaxSize()){
            Row(Modifier.fillMaxWidth().padding(12.dp)){Text(when(kind){"task"->"原任务";"mission_diff"->"原使命累计文件变更";else->"原使命"},Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(onClick=onClose){Text("关闭")}}
            Row(Modifier.padding(horizontal=12.dp)){TextButton(enabled=!busy,onClick={refresh()}){Text("重新读取")};if(kind!="mission_diff")TextButton(enabled=!busy&&value!=null&&state.pending==null,onClick={busy=true;scope.launch{try{load();if(kind=="task"){val snapshot=model.query(RovaiOperation.THREADS_OPEN,buildJsonObject{put("threadId",threadId);put("traceId",UUID.randomUUID().toString())}).coreObject();require(snapshot["thread"].coreObject().coreText("id")==threadId);taskProjection=snapshot};require(generation==model.connectionGeneration);workspace=true}catch(error:Exception){issue=error.message}finally{busy=false}}}){Text("在原工作区操作")}}
            if(busy)LinearProgressIndicator(Modifier.fillMaxWidth())
            issue?.let{Text(it,color=CodegTheme.colors.danger,modifier=Modifier.padding(12.dp))}
            if(value!=null&&version!=state.resourceVersion)Text("对象可能已变化，可重新读取。",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary,modifier=Modifier.padding(horizontal=12.dp))
            if(kind=="mission_diff"&&value!=null)Box(Modifier.weight(1f)){RovaiCanvasMissionChanges(model,objectId,(target["path"]as?JsonPrimitive)?.contentOrNull,version)}
            else LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
                value?.let{source->item{Text(source.coreText("title"),style=MaterialTheme.typography.titleMedium);MarkdownContent(source.coreText("description"));Text(if(kind=="task")taskStatusName(source.coreText("status"))else missionStatusName(source.coreText("status")))}
                    if(kind=="task")listOf("blockedReason" to "阻塞原因","completionSummary" to "完成摘要","cancelReason" to "取消原因").forEach{(key,label)->if(source.coreText(key).isNotEmpty())item{Text(label,style=MaterialTheme.typography.titleSmall);Text(source.coreText(key))}}
                }
            }
        }}
    }
}

@Composable private fun RovaiCanvasMissionChanges(model:RovaiWorkspaceViewModel,missionId:String,path:String?,revision:Long){
    val scope=rememberCoroutineScope();val generation=remember{model.connectionGeneration};var changes by remember(missionId){mutableStateOf<JsonObject?>(null)};var patches by remember(missionId){mutableStateOf<Map<String,String>>(emptyMap())};var issue by remember{mutableStateOf<String?>(null)};var busy by remember{mutableStateOf(false)}
    suspend fun patch(file:JsonObject,view:JsonObject){val diff=model.query(RovaiOperation.MISSIONS_FILEDIFF,buildJsonObject{put("missionId",missionId);put("fileId",file.coreText("id"));put("viewId",view["viewId"]?:JsonNull)}).coreObject();require(generation==model.connectionGeneration);patches=patches+(file.coreText("id") to diff.coreText("patch"))}
    DisposableEffect(missionId,generation){val release=model.retainMissionDiff(missionId);onDispose{release()}}
    LaunchedEffect(missionId,path,revision){busy=true;patches=emptyMap();issue=null;try{val view=model.query(RovaiOperation.MISSIONS_CHANGES,buildJsonObject{put("missionId",missionId)}).coreObject();require(generation==model.connectionGeneration);changes=view;if(path!=null){val file=view["files"].coreArray().firstOrNull{it.coreText("path")==path};if(file==null)issue="原固定文件已不在当前累计Diff中，卡片引用保留。"else if(file["binary"]!=JsonPrimitive(true))patch(file,view)}}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
        if(busy)item{LinearProgressIndicator(Modifier.fillMaxWidth())};issue?.let{item{Text(it,color=CodegTheme.colors.danger)}}
        changes?.let{view->item{if(view.coreText("diffError").isNotEmpty())Text(view.coreText("diffError"),color=CodegTheme.colors.danger);val checkout=view["checkoutState"].coreObject();Text(checkout.coreText("kind")+" · "+checkout.coreText("branch"),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}
            view["files"].coreArray().filter{path==null||it.coreText("path")==path}.forEach{file->item(key=file.coreText("id")){
                TextButton(enabled=!busy,onClick={busy=true;scope.launch{try{patch(file,view)}catch(error:Exception){issue=error.message}finally{busy=false}}}){Text(file.coreText("path"))}
                if(file["binary"]==JsonPrimitive(true))Text("二进制文件")
                patches[file.coreText("id")]?.let{value->if(value.isEmpty())Text("没有文本行差异")else DiffView(UnifiedDiff.parse(value)?:emptyList())}
            }}
            if(path==null&&view["files"].coreArray().isEmpty())item{Text("当前没有累计文件变更",color=CodegTheme.colors.textSecondary)}
        }
    }
}
