package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.coaxi.android.core.designsystem.theme.CoaxiTheme
import kotlinx.coroutines.*
import kotlinx.serialization.json.*

@Composable
fun CoaxiProjectNavigation(model:CoaxiWorkspaceViewModel){
    val state by model.state.collectAsStateWithLifecycle();val generation=model.connectionGeneration;val lifecycle=LocalLifecycleOwner.current;val scope=rememberCoroutineScope()
    var snapshot by remember(generation){mutableStateOf<JsonObject>(buildJsonObject{put("projectGroups",JsonArray(emptyList()));put("projectOrder",JsonNull)})}
    var folders by remember(generation){mutableStateOf<Map<String,CoaxiCanvasFolderRelation>>(emptyMap())};var collapsed by remember(generation){mutableStateOf<Set<String>>(emptySet())};var ready by remember(generation){mutableStateOf(false)};var issue by remember(generation){mutableStateOf<String?>(null)};var managing by remember(generation){mutableStateOf(false)};var refresh by remember(generation){mutableIntStateOf(0)}
    val owner="project-navigation:groups"
    LaunchedEffect(generation){try{val raw=model.readBody(owner);if(raw.isNotBlank()){val value=Json.parseToJsonElement(raw).coreObject();require(value["version"]==JsonPrimitive(1));val values=value["collapsed"]as?JsonArray?:error("项目分组显示偏好无法读取");collapsed=values.map{(it as?JsonPrimitive)?.takeIf{it.isString}?.content?:error("项目分组显示偏好无法读取")}.toSet()};ready=true}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}
    LaunchedEffect(generation,state.connected,refresh){if(!state.connected)return@LaunchedEffect;while(isActive){
        if(lifecycle.lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED))try{
            val next=readProjectGroups(model);val projects=model.state.value.navigation["projects"].coreArray();val roots=projects.map{it.coreText("projectPath")}+next["projectGroups"].coreArray().flatMap{group->(group["projectPaths"]as?JsonArray)?.mapNotNull{(it as?JsonPrimitive)?.contentOrNull}.orEmpty()}
            val relations=readCanvasFolderRelations(model,roots);if(generation!=model.connectionGeneration)return@LaunchedEffect
            folders=relations.mapValues{(path,value)->if(value.complete)value else folders[path]?.let{it.copy(complete=false,issue=value.issue)}?:value};snapshot=next;issue=null
        }catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(generation==model.connectionGeneration)issue=error.message}
        delay(20_000)
    }}
    fun toggle(id:String){if(!ready)return;val next=if(id in collapsed)collapsed-id else collapsed+id;val body=buildJsonObject{put("version",1);put("collapsed",JsonArray(next.sorted().map(::JsonPrimitive)))}.toString();scope.launch{try{model.saveBody(owner,body);if(generation!=model.connectionGeneration)return@launch;model.rememberBody(owner,body);collapsed=next}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}}
    if(managing)CoaxiProjectGroups(model){managing=false;refresh++}
    val sections=projectNavigationSections(snapshot,state.navigation["projects"].coreArray(),folders)
    LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(10.dp)){
        item{Row(Modifier.fillMaxWidth(),horizontalArrangement=Arrangement.SpaceBetween){Text(stringResource(R.string.coaxi_fusion_797eba09d6beb579),style=MaterialTheme.typography.titleSmall);TextButton(onClick={managing=true}){Text(stringResource(R.string.coaxi_fusion_25c3aad45894871b))}};issue?.let{Text(it,color=CoaxiTheme.colors.danger);TextButton(onClick={refresh++}){Text(stringResource(R.string.coaxi_fusion_c4dab4b4f78a8fdf))}}}
        val quick=state.navigation["quickChat"].coreObject()["recentThreads"].coreArray();if(quick.isNotEmpty()){item{Text(stringResource(R.string.coaxi_fusion_7b0df27853e6f545),style=MaterialTheme.typography.titleSmall)};items(quick,key={"quick:"+it.coreText("id")}){CoaxiThreadRow(it){model.openThread(it.coreText("id"))}}}
        sections.forEach{section->val groupId=section.groupId
            if(groupId!=null)item(key=section.id){Row(Modifier.fillMaxWidth().clickable(enabled=ready){toggle(groupId)}.padding(vertical=8.dp),horizontalArrangement=Arrangement.spacedBy(8.dp)){Icon(if(groupId in collapsed)Icons.Outlined.ChevronRight else Icons.Outlined.ExpandMore,if(groupId in collapsed)"展开分组"else"收起分组");Icon(Icons.Outlined.Folder,null);Text(section.name,Modifier.weight(1f),style=MaterialTheme.typography.titleSmall);Text(section.projects.size.toString(),color=CoaxiTheme.colors.textSecondary)}}
            if(groupId==null||groupId !in collapsed){
                if(groupId!=null&&section.projects.isEmpty())item{Text(stringResource(R.string.coaxi_fusion_6fdc62b6d1f2b2cf),color=CoaxiTheme.colors.textSecondary,modifier=Modifier.padding(start=24.dp))}
                section.projects.forEach{project->val path=project.coreText("projectPath");val inset=if(groupId!=null)24.dp else 0.dp
                    item(key="project:$path"){Column(Modifier.padding(start=inset)){Text(project.coreText("name"),style=MaterialTheme.typography.titleSmall);if(folders[path]?.complete==false)Text(stringResource(R.string.coaxi_fusion_f9c44816dfbff147),style=MaterialTheme.typography.labelSmall,color=CoaxiTheme.colors.textSecondary)}}
                    items(project["recentThreads"].coreArray(),key={"$path:"+it.coreText("id")}){thread->Box(Modifier.padding(start=inset)){CoaxiThreadRow(thread){model.openThread(thread.coreText("id"))}}}
                    item(key="more:$path"){Box(Modifier.padding(start=inset)){CoaxiMoreThreads(model,path,project.coreLong("totalCount").toInt(),project["recentThreads"].coreArray().map{it.coreText("id")}.toSet())}}
                }
            }
        }
    }
}
