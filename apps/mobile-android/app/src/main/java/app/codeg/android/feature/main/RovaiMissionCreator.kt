package app.codeg.android.feature.main

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.util.UUID

/** Mission writing plane with original Core fields; one atomic command owns all chosen files. */
@Composable fun RovaiMissionCreator(model:RovaiWorkspaceViewModel,onClose:()->Unit){
    val state=model.state.collectAsState().value;val scope=rememberCoroutineScope();val generation=model.connectionGeneration;val owner="mission-create-draft"
    var name by remember{mutableStateOf("")};var description by remember{mutableStateOf("")};var path by remember{mutableStateOf("")};var tags by remember{mutableStateOf("")}
    var ids by remember{mutableStateOf<Set<String>>(emptySet())};var mentions by remember{mutableStateOf<Set<String>>(emptySet())};var lead by remember{mutableStateOf("")};var sources by remember{mutableStateOf<List<JsonObject>>(emptyList())}
    var descriptionContent by remember{mutableStateOf(JsonArray(listOf(buildJsonObject{put("kind","text");put("text","")})))}
    var commandId by remember{mutableStateOf<String?>(null)};var ready by remember{mutableStateOf(false)};var preparing by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)}
    val locked=!ready||preparing||state.busy||state.pending!=null
    fun document()=buildJsonObject{put("version",1);put("name",name);put("description",description);put("structuredDescription",descriptionContent);put("path",path);put("tags",tags);put("ids",JsonArray(ids.sorted().map(::JsonPrimitive)));put("mentions",JsonArray(mentions.sorted().map(::JsonPrimitive)));put("lead",lead);put("sources",JsonArray(sources));put("commandId",commandId?.let(::JsonPrimitive)?:JsonNull)}.toString()
    fun rememberDraft(){if(ready)model.rememberBody(owner,document())}
    fun close(){if(preparing)return;val result=model.state.value.lastResult.coreObject();if(model.state.value.lastInputOwner==owner&&result.coreText("commandId")==commandId&&result.coreText("status")in listOf("applied","accepted")){onClose();return};val frozen=document();scope.launch{try{if(ready)model.saveBody(owner,frozen);onClose()}catch(error:Exception){issue=error.message}}}
    val picker=rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()){uris->if(uris.isNotEmpty()&&!locked){if(uris.size+sources.size>10)issue="使命最多保留10份附件"else{preparing=true;val original=sources;scope.launch{val added=mutableListOf<JsonObject>();try{for(uri in uris){added+=model.prepareMissionFile(uri);require(generation==model.connectionGeneration){"工作区已切换"}};sources=original+added;model.saveBody(owner,document())}catch(cancelled:CancellationException){added.forEach{model.removeMissionFile(it)};throw cancelled}catch(error:Exception){added.forEach{model.removeMissionFile(it)};sources=original;issue=error.message}finally{preparing=false}}}}}
    LaunchedEffect(generation){try{val raw=model.readBody(owner);if(raw.isNotBlank()){val value=Json.parseToJsonElement(raw).coreObject();require(value.coreLong("version")==1L){"原使命草稿格式无法读取，原记录保留"};name=value.coreText("name");description=value.coreText("description");path=value.coreText("path");tags=value.coreText("tags");ids=(value["ids"]as?JsonArray)?.mapNotNull{(it as?JsonPrimitive)?.contentOrNull}?.toSet().orEmpty();mentions=(value["mentions"]as?JsonArray)?.mapNotNull{(it as?JsonPrimitive)?.contentOrNull}?.toSet().orEmpty();lead=value.coreText("lead");sources=value["sources"].coreArray();commandId=(value["commandId"]as?JsonPrimitive)?.contentOrNull;descriptionContent=(value["structuredDescription"]as?JsonArray)?:JsonArray(buildList{add(buildJsonObject{put("kind","text");put("text",description)});mentions.sorted().forEach{id->add(buildJsonObject{put("kind","text");put("text","\n")});add(buildJsonObject{put("kind","member_mention");put("agentId",id)})}})};ready=true}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}
    LaunchedEffect(state.lastResult){val result=state.lastResult.coreObject();if(state.lastCompletedOperation==RovaiOperation.MISSIONS_CREATE.wire&&state.lastInputOwner==owner&&result.coreText("commandId")==commandId){if(result.coreText("status")in listOf("applied","accepted")){onClose()}else if(result.coreText("status")=="rejected"){commandId=null;issue=result.coreText("code");rememberDraft()}}}
    val members=state.members.filter{it.coreText("presence")=="present"};val available=members.map{it.coreText("agentId")}.toSet()
    val parsedTags=tags.split(Regex("[,，\\n]")).map(String::trim).filter(String::isNotEmpty).distinct()
    val valid=name.isNotBlank()&&name.codePointCount(0,name.length)<=200&&description.codePointCount(0,description.length)<=12000&&ids.isNotEmpty()&&lead in ids&&ids.all{it in available}&&mentions.all{it in ids}&&parsedTags.size<=30&&parsedTags.all{it.codePointCount(0,it.length)<=24}
    Dialog(onDismissRequest={close()},properties=DialogProperties(usePlatformDefaultWidth=false,dismissOnBackPress=!preparing,dismissOnClickOutside=false)){Surface(Modifier.fillMaxSize(),color=CodegTheme.colors.bgElevated){Column(Modifier.fillMaxSize()){
        Row(Modifier.fillMaxWidth().padding(12.dp)){Text("新建使命",Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(enabled=!preparing,onClick={close()}){Text("保存草稿并关闭")}}
        LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
            item{issue?.let{Text(it,color=CodegTheme.colors.danger)};if(state.pending!=null)Text("原提交待核对；关闭后可在工作区核对或重试同一请求。",color=CodegTheme.colors.textSecondary)}
            item{OutlinedTextField(name,{name=it;rememberDraft()},Modifier.fillMaxWidth(),label={Text("名称")},enabled=!locked,singleLine=true);RovaiMissionDescriptionInput(descriptionContent,state.members,locked){content->descriptionContent=content;description=missionDescriptionDisplay(content,state.members);mentions=missionDescriptionMemberIds(content);ids=ids+mentions;if(lead !in ids)lead=ids.sorted().firstOrNull().orEmpty();rememberDraft()};OutlinedTextField(tags,{tags=it;rememberDraft()},Modifier.fillMaxWidth(),label={Text("标签，用逗号分开")},enabled=!locked)}
            item{Text("工作目录",style=MaterialTheme.typography.titleSmall);TextButton(enabled=!locked,onClick={path="";rememberDraft()}){Text(if(path.isEmpty())"✓ 快速对话" else "快速对话")}}
            items(state.navigation["projects"].coreArray(),key={it.coreText("projectPath")}){project->TextButton(enabled=!locked,onClick={path=project.coreText("projectPath");rememberDraft()}){Text((if(path==project.coreText("projectPath"))"✓ " else "")+project.coreText("name"))}}
            if(path.isNotEmpty()&&state.navigation["projects"].coreArray().none{it.coreText("projectPath")==path})item{Text("原工程暂不可用，请重新选择工作目录。",color=CodegTheme.colors.danger)}
            item{Text("队员与负责人",style=MaterialTheme.typography.titleSmall)}
            items(members,key={it.coreText("agentId")}){member->val id=member.coreText("agentId");GlassRow{Row{Checkbox(id in ids,{selected->if(selected)ids=ids+id else{ids=ids-id;mentions=mentions-id;descriptionContent=JsonArray(descriptionContent.filter{it.coreObject().coreText("kind")!="member_mention"||it.coreObject().coreText("agentId")!=id});description=missionDescriptionDisplay(descriptionContent,state.members)};if(lead !in ids)lead=ids.firstOrNull().orEmpty();rememberDraft()},enabled=!locked);Text(member.coreText("displayName"),Modifier.weight(1f));TextButton(enabled=!locked&&id in ids,onClick={lead=id;rememberDraft()}){Text(if(lead==id)"负责人" else "设为负责人")}}}}
            (ids-available).forEach{id->item{Row{Text("草稿中的队员已不可用",Modifier.weight(1f),color=CodegTheme.colors.danger);TextButton(enabled=!locked,onClick={ids=ids-id;mentions=mentions-id;descriptionContent=JsonArray(descriptionContent.filter{it.coreObject().coreText("kind")!="member_mention"||it.coreObject().coreText("agentId")!=id});description=missionDescriptionDisplay(descriptionContent,state.members);if(lead==id)lead=ids.firstOrNull().orEmpty();rememberDraft()}){Text("移除")}}}}
            item{Row{Text("附件 · ${sources.size}/10",Modifier.weight(1f),style=MaterialTheme.typography.titleSmall);TextButton(enabled=!locked&&sources.size<10,onClick={picker.launch(arrayOf("*/*"))}){Text(if(preparing)"正在保存…" else "添加附件")}}}
            items(sources,key={it["intent"].coreObject().coreText("commandId")}){source->Row{Text(source["intent"].coreObject().coreText("displayName"),Modifier.weight(1f));TextButton(enabled=!locked,onClick={scope.launch{try{val original=sources;sources=sources.filter{it!=source};try{model.saveBody(owner,document())}catch(error:Exception){sources=original;throw error};model.removeMissionFile(source)}catch(error:Exception){issue=error.message}}}){Text("移除")}}}
        }
        Button(enabled=!locked&&valid,modifier=Modifier.fillMaxWidth().padding(12.dp),onClick={val id=commandId?:UUID.randomUUID().toString();commandId=id;val command=buildJsonObject{put("title",name.trim());put("description",description);put("descriptionContent",descriptionContent);put("projectPath",path);put("projectBindingKind",if(path.isEmpty())"quick_chat" else "directory");put("memberAgentIds",JsonArray(ids.sorted().map(::JsonPrimitive)));put("defaultLeadAgentId",lead);put("tags",JsonArray(parsedTags.map(::JsonPrimitive)))};val saved=document();scope.launch{try{model.saveBody(owner,saved);require(generation==model.connectionGeneration);model.executeMissionBatch(RovaiOperation.MISSIONS_CREATE,id,command,emptyList(),sources,owner,saved)}catch(error:Exception){issue=error.message}}}){Text("创建使命")}
    }}}
}
