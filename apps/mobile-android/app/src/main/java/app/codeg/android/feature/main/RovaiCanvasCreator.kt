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
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.util.UUID

data class RovaiCanvasReferenceChoice(val reference:JsonObject,val label:String){val key:String get()=JsonArray(listOf(reference["threadId"]?:JsonNull,reference["conversationId"]?:JsonNull)).toString()}
fun canvasReferenceChoices(rows:List<JsonObject>):List<RovaiCanvasReferenceChoice> = rows.flatMap{thread->
    val id=thread.coreText("threadId");val title=thread.coreText("title");val members=thread["members"].coreArray().associateBy{it.coreText("agentId")}
    listOf(RovaiCanvasReferenceChoice(buildJsonObject{put("threadId",id);put("conversationId",JsonNull)},"$title · 公开对话"))+thread["privateConversations"].coreArray().map{private->
        val member=members[private.coreText("agentId")];val runtime=private["lastRunModel"].coreObject().coreText("modelId").ifEmpty{member?.coreText("runtimeKind").orEmpty()}
        RovaiCanvasReferenceChoice(buildJsonObject{put("threadId",id);put("conversationId",private.coreText("conversationId"))},title+" · "+(member?.coreText("displayName")?:"队员")+"私聊"+if(runtime.isEmpty())""else" · $runtime")
    }
}
@Composable
fun RovaiCanvasChoice(
    label: String,
    value: String,
    choices: List<Pair<String, String>>,
    enabled: Boolean = true,
    onChange: (String) -> Unit
) {
    var open by remember { mutableStateOf(false) }
    var search by remember { mutableStateOf("") }

    TextButton(
        enabled = enabled,
        onClick = { open = true }
    ) {
        val selectedLabel = choices.firstOrNull { it.first == value }?.second
            ?: if (value.isEmpty()) "请选择" else "原选择暂不可用"
        Text("$label · $selectedLabel")
    }

    if (open) {
        AlertDialog(
            onDismissRequest = { open = false },
            title = { Text(label) },
            text = {
                Column {
                    OutlinedTextField(
                        value = search,
                        onValueChange = { search = it },
                        modifier = Modifier.fillMaxWidth(),
                        label = { Text(stringResource(R.string.rovai_fusion_44ce7ae909bbb28b)) }
                    )
                    LazyColumn(Modifier.heightIn(max = 400.dp)) {
                        choices
                            .filter { search.isBlank() || it.second.contains(search, ignoreCase = true) }
                            .forEach { (id, title) ->
                                item(key = id) {
                                    TextButton(onClick = {
                                        onChange(id)
                                        open = false
                                    }) {
                                        Text((if (id == value) "✓ " else "") + title)
                                    }
                                }
                            }
                    }
                }
            },
            confirmButton = {
                TextButton(onClick = { open = false }) {
                    Text(stringResource(R.string.rovai_fusion_3fd47edce45b3603))
                }
            }
        )
    }
}

/** CanvasNodeEditor + CanvasObjectPicker, using original bindings, sizes and captured board revision. */
@Composable fun RovaiCanvasCreator(model:RovaiWorkspaceViewModel,boardId:String,graph:JsonObject,x:Float,y:Float,bindingNode:JsonObject?=null,onClose:()->Unit){
    val state=model.state.collectAsState().value;val scope=rememberCoroutineScope();val generation=remember{model.connectionGeneration};val owner=if(bindingNode==null)"canvas-create:$boardId"else"canvas-file-binding:$boardId:${bindingNode.coreText("id")}"
    var form by remember{mutableStateOf(buildJsonObject{put("kind",if(bindingNode==null)"note"else"file");put("path",bindingNode?.get("data").coreObject().coreText("path"));put("title","");put("content","");put("color","");put("columns",0);put("rows",0);put("x",x);put("y",y)})}
    var revision by remember{mutableLongStateOf(graph["board"].coreObject().coreLong("revision"))};var commandId by remember{mutableStateOf(UUID.randomUUID().toString())};var ready by remember{mutableStateOf(false)};var busy by remember{mutableStateOf(false)};var reading by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)}
    var managingGroups by remember{mutableStateOf(false)}
    var references by remember{mutableStateOf<List<JsonObject>>(emptyList())};var cursor by remember{mutableStateOf<RovaiCanvasReferenceCursor?>(null)};var catalogLoaded by remember{mutableStateOf(false)};var groups by remember{mutableStateOf<List<JsonObject>>(emptyList())}
    var tasks by remember{mutableStateOf<List<JsonObject>>(emptyList())};var taskCursor by remember{mutableStateOf<String?>(null)};var mission by remember{mutableStateOf<JsonObject?>(null)};var diffFiles by remember{mutableStateOf<List<JsonObject>?>(null)};var objectReady by remember{mutableStateOf(false)};var objectBusy by remember{mutableStateOf(false)}
    val kind=form.coreText("kind");val objectKind=kind in listOf("task","mission","mission_diff");val region=kind in listOf("folder","group","agent","custom");val choices=canvasReferenceChoices(references)
    val stale=state.boards.firstOrNull{it.coreText("id")==boardId}?.coreLong("revision")!=revision
    val blocked=!ready||busy||state.busy||state.pending!=null||generation!=model.connectionGeneration
    fun document()=buildJsonObject{put("version",1);put("boardId",boardId);put("bindingNodeId",bindingNode?.get("id")?:JsonNull);put("form",form);put("revision",revision);put("commandId",commandId)}.toString()
    fun persist(){if(ready&&generation==model.connectionGeneration)model.rememberBody(owner,document())}
    fun field(key:String,value:JsonElement){form=JsonObject(form+mapOf(key to value));persist()}
    fun text(key:String,value:String){if(key=="kind"||key=="objectThreadId")form=JsonObject(form+mapOf("objectReference" to JsonNull));field(key,JsonPrimitive(value))}
    fun close(){if(busy)return;if(!ready||generation!=model.connectionGeneration){onClose();return};scope.launch{try{val body=document();model.saveBody(owner,body);model.rememberBody(owner,body);onClose()}catch(error:Exception){issue=error.message}}}
    suspend fun page(reset:Boolean){if(reading)return;reading=true;try{if(reset){references=emptyList();cursor=null;catalogLoaded=false};val next=readCanvasReferencePage(model,if(reset)null else cursor);require(generation==model.connectionGeneration);require(next.items.none{row->references.any{it.coreText("threadId")==row.coreText("threadId")}}){"对话目录出现重复页，请重新读取"};references=references+next.items;cursor=next.next;catalogLoaded=true}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{reading=false}}
    suspend fun taskPage(after:String?){val thread=form.coreText("objectThreadId");val result=model.query(RovaiOperation.TASKS_LIST,buildJsonObject{put("threadId",thread);put("cursor",after?.let(::JsonPrimitive)?:JsonNull);put("limit",50)}).coreObject();require(generation==model.connectionGeneration);val rows=result["tasks"]as?JsonArray?:error("任务列表无法读取");val next=(result["nextCursor"]as?JsonPrimitive)?.contentOrNull;require(rows.size<=50&&rows.all{it is JsonObject&&it.coreText("taskId").isNotEmpty()}&&(next==null||next.isNotEmpty()&&next!=after&&rows.isNotEmpty())&&(result["truncated"]!=JsonPrimitive(true)||next!=null)){"任务分页未前进"};val values=rows.map{it as JsonObject};require(values.map{it.coreText("taskId")}.toSet().size==values.size&&values.none{row->tasks.any{it.coreText("taskId")==row.coreText("taskId")}}){"任务目录有重复项"};tasks=tasks+values;taskCursor=next}
    LaunchedEffect(owner){try{val raw=model.readBody(owner);if(raw.isNotEmpty()){val value=Json.parseToJsonElement(raw).coreObject();require(value.coreLong("version")==1L&&value.coreText("boardId")==boardId&&value["form"]is JsonObject&&(bindingNode==null||value["bindingNodeId"]==bindingNode["id"]&&value["form"].coreObject().coreText("path")==bindingNode["data"].coreObject().coreText("path"))){"原创建草稿无法读取，原记录保留"};form=value["form"].coreObject();revision=value.coreLong("revision");commandId=value.coreText("commandId").ifEmpty{UUID.randomUUID().toString()}};require(generation==model.connectionGeneration);ready=true;groups=model.query(RovaiOperation.NAVIGATION_PROJECTGROUPS,buildJsonObject{put("action","get")}).coreObject()["projectGroups"].coreArray();page(true)}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}
    LaunchedEffect(kind,form.coreText("objectThreadId"),references.firstOrNull{it.coreText("threadId")==form.coreText("objectThreadId")}?.get("missionId")){
        tasks=emptyList();taskCursor=null;mission=null;diffFiles=null;objectReady=false
        val thread=references.firstOrNull{it.coreText("threadId")==form.coreText("objectThreadId")};if(!objectKind||thread==null)return@LaunchedEffect
        objectBusy=true;try{if(kind=="task")taskPage(null)else{val id=thread.coreText("missionId");require(id.isNotEmpty()){"此对话没有使命"};val result=model.query(RovaiOperation.MISSIONS_GET,buildJsonObject{put("missionId",id)}).coreObject();require(result.coreText("missionId")==id&&result.coreText("threadId")==thread.coreText("threadId")){"使命归属已变化"};mission=result};require(generation==model.connectionGeneration);objectReady=true}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{objectBusy=false}
    }
    LaunchedEffect(state.lastResult){val receipt=state.lastResult.coreObject()["commandResult"].coreObject().ifEmpty{state.lastResult.coreObject()};if(state.lastInputOwner==owner&&receipt.coreText("commandId")==commandId){if(receipt.coreText("status")in listOf("applied","accepted")){ready=false;onClose()}else if(receipt.coreText("status")=="rejected"){commandId=UUID.randomUUID().toString();issue=receipt.coreText("code");persist()}}}
    val selectedReference=choices.firstOrNull{it.key==form.coreText("referenceKey")};val memberKeys=(form["members"]as?JsonArray)?.mapNotNull{(it as?JsonPrimitive)?.contentOrNull}.orEmpty();val selectedObject=form["objectReference"].coreObject()
    val valid=(bindingNode==null||kind=="file"&&graph["fileReferenceVersion"]==JsonPrimitive(1))&&when(kind){"note"->true;"custom"->memberKeys.size<=200&&memberKeys.all{key->choices.any{it.key==key}};"folder","terminal"->form.coreText("path").isNotBlank();"file"->form.coreText("path").isNotEmpty()&&references.any{it.coreText("threadId")==form.coreText("fileThreadId")&&it.coreText("projectBindingKind")=="directory"};"group"->groups.any{it.coreText("id")==form.coreText("groupId")};"agent"->state.members.any{it.coreText("agentId")==form.coreText("agentId")&&(it["removedAt"]==null||it["removedAt"]==JsonNull)};"conversation"->selectedReference!=null;"task"->objectReady&&tasks.any{it.coreText("taskId")==selectedObject.coreText("taskId")};"mission","mission_diff"->objectReady&&mission!=null&&(selectedObject.coreText("missionId")==mission?.coreText("missionId"))&&(selectedObject.coreText("path").isEmpty()||diffFiles?.any{it.coreText("path")==selectedObject.coreText("path")}==true);else->false}
    fun submit(){if(blocked||stale||!valid)return;val columns=form.coreLong("columns").toInt().coerceIn(0,12);val rows=form.coreLong("rows").toInt().coerceIn(0,12);val size=when{kind=="conversation"||objectKind->224 to 132;kind=="file"->460 to 360;kind=="terminal"->480 to 300;region->(24+(if(columns==0)2 else columns)*224+((if(columns==0)2 else columns)-1)*12) to (64+(if(rows==0)1 else rows)*132+((if(rows==0)1 else rows)-1)*12);else->320 to 220}
        val node=buildJsonObject{put("kind",if(objectKind)"conversation"else kind);put("title",form.coreText("title"));put("color",form.coreText("color"));put("x",form["x"]?:JsonPrimitive(x));put("y",form["y"]?:JsonPrimitive(y));put("width",size.first);put("height",size.second);if(region){put("gridColumns",columns);put("gridRows",rows)};when(kind){"note"->put("content",form.coreText("content"));"folder"->put("folderPath",form.coreText("path"));"group"->put("folderGroupId",form.coreText("groupId"));"agent"->put("agentId",form.coreText("agentId"));"file"->{put("path",form.coreText("path"));putJsonObject("fileReference"){put("threadId",form.coreText("fileThreadId"))}};"terminal"->put("path",form.coreText("path"));"conversation"->put("conversation",selectedReference!!.reference);"custom"->put("conversations",JsonArray(choices.filter{it.key in memberKeys}.map{it.reference}));else->if(objectKind){put("objectReference",selectedObject);putJsonObject("conversation"){put("threadId",form.coreText("objectThreadId"));put("conversationId",JsonNull)}}}}
        val saved=document();busy=true;scope.launch{try{model.saveBody(owner,saved);model.rememberBody(owner,saved);require(generation==model.connectionGeneration);model.executeOwned(RovaiOperation.CANVAS_NODES_APPLY,buildJsonObject{put("commandId",commandId);putJsonObject("command"){put("boardId",boardId);put("expectedRevision",revision);put("mutation",if(bindingNode==null)buildJsonObject{put("type","create");put("node",node)}else buildJsonObject{put("type","bind_file");put("nodeId",bindingNode.coreText("id"));put("threadId",form.coreText("fileThreadId"))})}},owner,saved)}catch(error:Exception){issue=error.message}finally{busy=false}}
    }
    if(managingGroups)RovaiProjectGroups(model){managingGroups=false;scope.launch{try{groups=readProjectGroups(model)["projectGroups"].coreArray()}catch(error:Exception){issue=error.message}}}
    Dialog(onDismissRequest={close()},properties=DialogProperties(usePlatformDefaultWidth=false,dismissOnClickOutside=false)){Surface(Modifier.fillMaxSize(),color=CodegTheme.colors.bgElevated){Column{
        Row(Modifier.fillMaxWidth().padding(12.dp)){Text(if(bindingNode==null)"添加到画布"else"绑定文件来源",Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(enabled=!busy,onClick={close()}){Text(stringResource(R.string.rovai_fusion_92f27bce8ccbae8b))}}
        LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
            item{Text(stringResource(R.string.rovai_fusion_54280a5e18874b51),color=CodegTheme.colors.textSecondary);issue?.let{Text(it,color=CodegTheme.colors.danger)};if(stale){Text(stringResource(R.string.rovai_fusion_39d13d77fdfb7fdc),color=CodegTheme.colors.danger);TextButton(enabled=!blocked,onClick={busy=true;scope.launch{try{val latest=model.query(RovaiOperation.CANVAS_NODES_SNAPSHOT,buildJsonObject{put("boardId",boardId)}).coreObject();require(generation==model.connectionGeneration);revision=latest["board"].coreObject().coreLong("revision");commandId=UUID.randomUUID().toString();persist()}catch(error:Exception){issue=error.message}finally{busy=false}}}){Text(stringResource(R.string.rovai_fusion_5ad505c4123ff798))}}}
            if(bindingNode==null)item{RovaiCanvasChoice("类型",kind,(rovaiCanvasKindNames+mapOf("mission" to "使命","task" to "任务","mission_diff" to "累计文件变更")).toList(),!blocked){text("kind",it)};OutlinedTextField(form.coreText("title"),{text("title",it)},Modifier.fillMaxWidth(),label={Text(stringResource(R.string.rovai_fusion_c3405f8c7d9d392a))},enabled=!blocked);RovaiCanvasChoice("标记颜色",form.coreText("color"),rovaiCanvasColors,!blocked){text("color",it)}}
            if(kind=="note")item{OutlinedTextField(form.coreText("content"),{text("content",it)},Modifier.fillMaxWidth(),label={Text(stringResource(R.string.rovai_fusion_05ae41f8623f98ee))},minLines=8,enabled=!blocked)}
            if(kind in listOf("folder","file","terminal"))item{if(bindingNode!=null)Text(form.coreText("path"))else OutlinedTextField(form.coreText("path"),{text("path",it)},Modifier.fillMaxWidth(),label={Text(if(kind=="file")"文件绝对路径"else"工作目录绝对路径")},enabled=!blocked)}
            if(kind=="agent")item{RovaiCanvasChoice("队员",form.coreText("agentId"),state.members.filter{it["removedAt"]==null||it["removedAt"]==JsonNull}.map{it.coreText("agentId") to it.coreText("displayName")},!blocked){text("agentId",it)}}
            if(kind=="group")item{RovaiCanvasChoice("项目分组",form.coreText("groupId"),groups.map{it.coreText("id") to it.coreText("name")},!blocked){text("groupId",it)};TextButton(enabled=!blocked,onClick={managingGroups=true}){Text(stringResource(R.string.rovai_fusion_6bdf4cfcd44175f2))};TextButton(enabled=!blocked&&!reading,onClick={scope.launch{try{groups=model.query(RovaiOperation.NAVIGATION_PROJECTGROUPS,buildJsonObject{put("action","get")}).coreObject()["projectGroups"].coreArray()}catch(error:Exception){issue=error.message}}}){Text(stringResource(R.string.rovai_fusion_d927b5365b2a079e))}}
            if(kind=="conversation")item{RovaiCanvasChoice("对话",form.coreText("referenceKey"),choices.map{it.key to it.label},!blocked){text("referenceKey",it)}}
            if(kind=="custom")choices.forEach{choice->item(key=choice.key){Row{Checkbox(choice.key in memberKeys,enabled=!blocked&&(choice.key in memberKeys||memberKeys.size<200),onCheckedChange={checked->field("members",JsonArray((if(checked)memberKeys+choice.key else memberKeys-choice.key).map(::JsonPrimitive)))});Text(choice.label,Modifier.weight(1f))}}}
            if(kind=="file")item{RovaiCanvasChoice("所属工程对话",form.coreText("fileThreadId"),references.filter{it.coreText("projectBindingKind")=="directory"}.map{it.coreText("threadId") to it.coreText("title")},!blocked){text("fileThreadId",it)};Text(stringResource(R.string.rovai_fusion_b3d53f7f5c161ec6),color=CodegTheme.colors.textSecondary)}
            if(objectKind){item{RovaiCanvasChoice("所属对话",form.coreText("objectThreadId"),references.filter{kind=="task"||it.coreText("missionId").isNotEmpty()}.map{it.coreText("threadId") to it.coreText("title")},!blocked&&!objectBusy){text("objectThreadId",it);field("objectReference",JsonNull)}}
                if(kind=="task")item{RovaiCanvasChoice("任务",selectedObject.coreText("taskId"),tasks.map{it.coreText("taskId") to it.coreText("title")},!blocked&&!objectBusy){field("objectReference",buildJsonObject{put("kind","task");put("taskId",it)})};if(taskCursor!=null)TextButton(enabled=!blocked&&!objectBusy,onClick={objectBusy=true;scope.launch{try{taskPage(taskCursor)}catch(error:Exception){issue=error.message}finally{objectBusy=false}}}){Text(stringResource(R.string.rovai_fusion_4970096be645ae34))}}
                mission?.let{value->item{Text(value.coreText("title"));TextButton(enabled=!blocked&&!objectBusy,onClick={field("objectReference",buildJsonObject{put("kind",kind);put("missionId",value.coreText("missionId"))})}){Text(stringResource(R.string.rovai_fusion_4d43e3ce1da71e06))};if(kind=="mission_diff")TextButton(enabled=!blocked&&!objectBusy,onClick={objectBusy=true;scope.launch{try{val view=model.query(RovaiOperation.MISSIONS_CHANGES,buildJsonObject{put("missionId",value.coreText("missionId"))}).coreObject();require(generation==model.connectionGeneration&&view.coreText("viewId").isNotEmpty()&&view.coreText("diffError").isEmpty()){view.coreText("diffError").ifEmpty{"当前变更清单暂不可用"}};diffFiles=view["files"].coreArray()}catch(error:Exception){issue=error.message}finally{objectBusy=false}}}){Text(stringResource(R.string.rovai_fusion_d4b8b7bfee148498))}}
                    if(kind=="mission_diff"&&diffFiles!=null)item{RovaiCanvasChoice("定位文件",selectedObject.coreText("path"),listOf("" to "整个使命的累计变更")+diffFiles.orEmpty().map{it.coreText("path") to it.coreText("path")},!blocked&&!objectBusy){path->field("objectReference",buildJsonObject{put("kind","mission_diff");put("missionId",value.coreText("missionId"));if(path.isNotEmpty())put("path",path)})}}
                }
            }
            if(region)item{Text(stringResource(R.string.rovai_fusion_fec59aa9c20f4e1c));Slider(form.coreLong("columns").toFloat(),{field("columns",JsonPrimitive(it.toInt()))},valueRange=0f..12f,steps=11,enabled=!blocked);Text(stringResource(R.string.rovai_fusion_d2a3fffc1c1b0092));Slider(form.coreLong("rows").toFloat(),{field("rows",JsonPrimitive(it.toInt()))},valueRange=0f..12f,steps=11,enabled=!blocked)}
            item{if(reading||objectBusy)LinearProgressIndicator(Modifier.fillMaxWidth());if(!catalogLoaded||cursor!=null)Text(stringResource(R.string.rovai_fusion_70c8062a10338b15),color=CodegTheme.colors.textSecondary);Row{TextButton(enabled=!blocked&&!reading,onClick={scope.launch{page(true)}}){Text(stringResource(R.string.rovai_fusion_9cbaf45e3b229146))};if(cursor!=null)TextButton(enabled=!blocked&&!reading,onClick={scope.launch{page(false)}}){Text(stringResource(R.string.rovai_fusion_14336047193ed3da))}}}
        }
        Button(enabled=!blocked&&!stale&&valid&&!objectBusy,modifier=Modifier.fillMaxWidth().padding(12.dp),onClick={submit()}){Text(if(bindingNode==null)"保存到画布"else"绑定来源")}
    }}}
}
