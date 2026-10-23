package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.diff.DiffView
import app.codeg.android.core.designsystem.diff.UnifiedDiff
import app.codeg.android.core.designsystem.markdown.MarkdownContent
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*

val rovaiMissionStatuses=listOf("needs_you" to "需要你","not_started" to "未开始","in_progress" to "进行中","completed" to "已完成")
fun missionStatusName(value:String)=rovaiMissionStatuses.firstOrNull{it.first==value}?.second?:value
@Composable fun RovaiMissionBoard(model:RovaiWorkspaceViewModel,state:RovaiWorkspaceState){
    var creating by remember{mutableStateOf(false)}
    if(creating)RovaiMissionCreator(model){creating=false}
    var status by remember{mutableStateOf("")};var search by remember{mutableStateOf("")};var selected by remember{mutableStateOf<JsonObject?>(null)}
    val rows=state.missions.filter{(status.isEmpty()||it.coreText("status")==status)&&(it.coreText("title")+" "+it.coreText("description")+" "+it["tags"].toString()+" "+it.coreText("projectPath")).contains(search,ignoreCase=true)}
    Column(Modifier.fillMaxSize()){
        Row(Modifier.fillMaxWidth().padding(horizontal=12.dp)){Text(stringResource(R.string.rovai_fusion_33aed1edf80b039d),Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(enabled=state.connected&&!state.busy,onClick={creating=true}){Text(stringResource(R.string.rovai_fusion_b4675bdfbf24074a))}}
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal=12.dp),horizontalArrangement=Arrangement.spacedBy(6.dp)){(listOf("" to "全部")+rovaiMissionStatuses).forEach{(id,title)->FilterChip(selected=status==id,onClick={status=id},label={Text("$title ${state.missions.count{ id.isEmpty()||it.coreText("status")==id }}")})}}
        OutlinedTextField(search,{search=it},label={Text(stringResource(R.string.rovai_fusion_82a2b6a7d1acaa9a))},modifier=Modifier.fillMaxWidth().padding(12.dp))
        LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(12.dp),verticalArrangement=Arrangement.spacedBy(10.dp)){items(rows,key={it.coreText("missionId")}){mission->GlassRow(Modifier.fillMaxWidth().combinedClickable(onClick={model.openThread(mission.coreText("threadId"))},onLongClick={selected=mission})){
            Row{Text("M-${mission.coreLong("number").toString().padStart(3,'0')}",Modifier.weight(1f),style=MaterialTheme.typography.labelSmall,color=CodegTheme.colors.textSecondary);Text(missionStatusName(mission.coreText("status")),style=MaterialTheme.typography.labelSmall);if((mission["runningAgentIds"]as?JsonArray)?.isNotEmpty()==true)Text(stringResource(R.string.rovai_fusion_b698eeb238aa3e9f),style=MaterialTheme.typography.labelSmall)}
            Text(mission.coreText("title"),style=MaterialTheme.typography.titleMedium);Text(mission.coreText("description"),maxLines=3,color=CodegTheme.colors.textSecondary)
            val lead=mission.coreText("defaultLeadAgentId");val members=(mission["memberAgentIds"]as?JsonArray)?.mapNotNull{(it as?JsonPrimitive)?.contentOrNull}.orEmpty().let{list->listOfNotNull(lead.takeIf{it.isNotEmpty()})+list.filter{it!=lead}}
            Text(members.take(5).joinToString(" · "){id->state.members.firstOrNull{it.coreText("agentId")==id}?.coreText("displayName")?:"不可用队员"}+(if(members.size>5)" +${members.size-5}" else ""),style=MaterialTheme.typography.bodySmall)
            Text(mission.coreText("projectPath").ifEmpty{"快速对话"}+" · "+(mission["tags"]as?JsonArray)?.joinToString(" · "){(it as?JsonPrimitive)?.contentOrNull.orEmpty()}.orEmpty(),style=MaterialTheme.typography.labelSmall,color=CodegTheme.colors.textSecondary)
            Row{TextButton(onClick={selected=mission}){Text(stringResource(R.string.rovai_fusion_d8679bb020c27b3d))};if(mission["startAvailable"]==JsonPrimitive(true))TextButton(enabled=state.pending==null&&!state.busy,onClick={model.command(RovaiOperation.MISSIONS_START,buildJsonObject{put("missionId",mission.coreText("missionId"))})}){Text(stringResource(R.string.rovai_fusion_68c7e61d6b97009a))};if(mission["hasUnread"]==JsonPrimitive(true))Text(stringResource(R.string.rovai_fusion_c7acdde1a7ad0daf),color=CodegTheme.colors.accent,style=MaterialTheme.typography.labelSmall)}
        }};if(rows.isEmpty())item{Text(stringResource(R.string.rovai_fusion_8a63274bbc740fec),color=CodegTheme.colors.textSecondary)}}
    }
    selected?.let{mission->RovaiMissionDetail(model,mission,onClose={selected=null})}
}

/** Original Mission delivery/activity/changes projections; imports and paths are not grants. */
@Composable fun RovaiMissionDetail(model:RovaiWorkspaceViewModel,initial:JsonObject,onClose:()->Unit){
    val state=model.state.collectAsState().value;val id=initial.coreText("missionId");val generation=model.connectionGeneration;val coroutine=rememberCoroutineScope()
    val mission=state.missions.find{it.coreText("missionId")==id}?:initial
    var tab by remember(id){mutableIntStateOf(0)};var delivery by remember(id){mutableStateOf<JsonObject?>(null)};var history by remember(id){mutableStateOf<List<JsonObject>>(emptyList())};var more by remember{mutableStateOf(false)};var changes by remember(id){mutableStateOf<JsonObject?>(null)};var patches by remember{mutableStateOf<Map<String,String>>(emptyMap())};var source by remember{mutableStateOf<JsonObject?>(null)}
    var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)}
    fun work(action:suspend ()->Unit){if(busy)return;busy=true;issue=null;coroutine.launch{try{action();require(generation==model.connectionGeneration){"工作区已切换"}}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}
    LaunchedEffect(id,state.resourceVersion){try{delivery=model.query(RovaiOperation.MISSIONS_DELIVERY,buildJsonObject{put("missionId",id)}).coreObject();history=model.query(RovaiOperation.MISSIONS_ACTIVITY,buildJsonObject{put("missionId",id)}).coreArray();more=history.size==100}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}
    DisposableEffect(id,generation){val release=model.retainMissionDiff(id);onDispose{release()}}
    source?.let{RovaiFilePreview(model,it){source=null}}
    Dialog(onDismissRequest=onClose,properties=DialogProperties(usePlatformDefaultWidth=false)){Surface(Modifier.fillMaxSize(),color=CodegTheme.colors.bgElevated){Column(Modifier.fillMaxSize().padding(12.dp),verticalArrangement=Arrangement.spacedBy(8.dp)){
        Row{Text(mission.coreText("title"),Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(onClick=onClose){Text(stringResource(R.string.rovai_fusion_3fd47edce45b3603))}}
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState())){listOf("详情","交付","活动","累计Diff").forEachIndexed{index,title->FilterChip(selected=tab==index,onClick={tab=index;if(index==3)work{changes=model.query(RovaiOperation.MISSIONS_CHANGES,buildJsonObject{put("missionId",id)}).coreObject();patches=emptyMap()}},label={Text(title)})}}
        issue?.let{Text(it,color=CodegTheme.colors.danger)}
        LazyColumn(Modifier.weight(1f),verticalArrangement=Arrangement.spacedBy(10.dp)){
            when (tab) {
                0 -> {
                    item { RovaiMissionActions(model, mission, onClose) }
                    item {
                        RovaiAttachmentPicker(
                            model,
                            mission.coreText("threadId"),
                            buildJsonObject {
                                put("kind", "mission")
                                put("missionId", id)
                            },
                            mission.coreLong("detailsVersion"),
                            state.pending != null || state.busy
                        )
                    }
                    item {
                        MarkdownContent(mission.coreText("description"))
                        Text(stringResource(R.string.rovai_fusion_316eb58edffd8bdf) + missionStatusName(mission.coreText("status")))
                        Text(
                            mission.coreText("projectPath").ifEmpty { "快速对话" },
                            style = MaterialTheme.typography.bodySmall,
                            color = CodegTheme.colors.textSecondary
                        )
                    }
                    items(mission["attachments"].coreArray(), key = { it.coreText("id") }) { attachment ->
                        RovaiMissionAttachmentRow(model, mission, attachment, onOpen = {
                            source = buildJsonObject {
                                put("kind", "attachment")
                                put("threadId", mission.coreText("threadId"))
                                putJsonObject("locator") {
                                    put("owner", "mission")
                                    put("threadId", mission.coreText("threadId"))
                                    put("missionId", id)
                                    put("attachmentRefId", attachment.coreText("id"))
                                }
                            }
                        })
                    }
                }
                1 -> {
                    delivery?.let { value ->
                        item {
                            val workspace = value["workspace"].coreObject()
                            Text(stringResource(R.string.rovai_fusion_3db7b06b5f6de0e0), style = MaterialTheme.typography.titleSmall)
                            Text(value.coreText("workingDirectory"), style = MaterialTheme.typography.bodySmall)
                            if (workspace.isNotEmpty()) {
                                Text(workspace.coreText("managedBranch"))
                                Text(workspace.coreText("state"), color = CodegTheme.colors.textSecondary)
                                if (workspace.coreText("diagnostic").isNotEmpty()) {
                                    Text(workspace.coreText("diagnostic"), color = CodegTheme.colors.danger)
                                }
                            }
                        }
                        items(value["pullRequests"].coreArray()) { pr ->
                            GlassRow {
                                Text(pr.coreText("title"))
                                Text(pr.coreText("url"), style = MaterialTheme.typography.bodySmall)
                            }
                        }
                        items(value["files"].coreArray()) { file ->
                            TextButton(onClick = {
                                source = buildJsonObject {
                                    put("kind", "attachment")
                                    put("threadId", mission.coreText("threadId"))
                                    putJsonObject("locator") {
                                        put("owner", "message")
                                        put("threadId", mission.coreText("threadId"))
                                        put("messageId", file.coreText("messageId"))
                                        put("attachmentRefId", file.coreText("attachmentId"))
                                    }
                                }
                            }) { Text(file.coreText("displayName")) }
                        }
                    }
                }
                2 -> {
                    items(history, key = { it.coreLong("id") }) { activity ->
                        GlassRow {
                            Text(missionActivityName(activity))
                            Text(activity.coreText("createdAt"), style = MaterialTheme.typography.bodySmall, color = CodegTheme.colors.textSecondary)
                        }
                    }
                    if (more) {
                        item {
                            TextButton(enabled = !busy, onClick = {
                                work {
                                    val next = model.query(RovaiOperation.MISSIONS_ACTIVITY, buildJsonObject {
                                        put("missionId", id)
                                        history.lastOrNull()?.let { put("before", it.coreLong("id")) }
                                    }).coreArray()
                                    history = (history + next).distinctBy { it.coreLong("id") }
                                    more = next.size == 100
                                }
                            }) { Text(stringResource(R.string.rovai_fusion_3fe6c8a5fdfb7ba7)) }
                        }
                    }
                }
                3 -> {
                    changes?.let { view ->
                        item {
                            val checkout = view["checkoutState"].coreObject()
                            Text(checkout.coreText("kind") + " · " + checkout.coreText("branch"), style = MaterialTheme.typography.bodySmall)
                            if (view.coreText("diffError").isNotEmpty()) {
                                Text(view.coreText("diffError"), color = CodegTheme.colors.danger)
                            }
                        }
                        items(view["files"].coreArray(), key = { it.coreText("id") }) { file ->
                            GlassRow {
                                TextButton(enabled = !busy, onClick = {
                                    work {
                                        val diff = model.query(RovaiOperation.MISSIONS_FILEDIFF, buildJsonObject {
                                            put("missionId", id)
                                            put("fileId", file.coreText("id"))
                                            put("viewId", view["viewId"] ?: JsonNull)
                                        }).coreObject()
                                        patches = patches + (file.coreText("id") to diff.coreText("patch"))
                                    }
                                }) { Text(file.coreText("path")) }
                                if (file["binary"] == JsonPrimitive(true)) {
                                    Text(stringResource(R.string.rovai_fusion_4e8c1baac23df745))
                                }
                                patches[file.coreText("id")]?.let { patch ->
                                    if (patch.isEmpty()) {
                                        Text(stringResource(R.string.rovai_fusion_467b109e0d14f60c))
                                    } else {
                                        DiffView(UnifiedDiff.parse(patch) ?: emptyList())
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }}}
}
fun missionActivityName(value:JsonObject)=when(value.coreText("kind")){"created"->"创建了使命";"started"->"开始了使命";"status"->"状态改为“${missionStatusName(value["changes"].coreObject().coreText("status"))}”";"pull_request"->if(value["changes"].coreObject()["removed"]==JsonPrimitive(true))"移除了PR关联" else "关联了PR";"members"->"更新了队员";"lead"->"调整了队长";else->"更新了使命定义"}
