package app.codeg.android.feature.main

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavType
import androidx.navigation.compose.*
import androidx.navigation.navArgument
import app.codeg.android.app.AppViewModel
import app.codeg.android.core.datastore.ServerProfile
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.markdown.MarkdownContent
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import app.codeg.android.feature.settings.RovaiAppearanceSettings
import app.codeg.android.feature.server.ServerEditorScreen
import app.codeg.android.feature.server.ServerListScreen
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*

@Composable
fun RovaiMainShell(appViewModel:AppViewModel,servers:List<ServerProfile>){
    val selected by appViewModel.selectedProfile.collectAsStateWithLifecycle();val nav=rememberNavController()
    NavHost(nav,startDestination="workbench"){
        composable("workbench"){RovaiWorkspaceScreen(selected?:servers.firstOrNull(),onServers={nav.navigate("servers")})}
        composable("servers"){ServerListScreen(servers,selected?.id,appViewModel::selectServer,appViewModel::deleteServer,{nav.navigate("editor")},{nav.navigate("editor?serverId=$it")},{nav.popBackStack()})}
        composable("editor?serverId={serverId}",arguments=listOf(navArgument("serverId"){type=NavType.StringType;nullable=true;defaultValue=null})){ServerEditorScreen(onDone={nav.popBackStack()})}
    }
}
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun RovaiWorkspaceScreen(profile:ServerProfile?,onServers:()->Unit,model:RovaiWorkspaceViewModel=hiltViewModel()){
    val state by model.state.collectAsStateWithLifecycle();var tab by rememberSaveable{mutableIntStateOf(0)};val lifecycle=LocalLifecycleOwner.current
    var board by remember{mutableStateOf<JsonObject?>(null)};var newConversation by remember{mutableStateOf(false)};var engineering by remember(profile){mutableStateOf(false)};var settingsPage by remember(profile){mutableStateOf<String?>(null)}
    LaunchedEffect(profile){model.bind(profile)}
    DisposableEffect(lifecycle,model){val observer=LifecycleEventObserver{_,event->if(event==Lifecycle.Event.ON_START)model.foreground(true)else if(event==Lifecycle.Event.ON_STOP)model.foreground(false)};lifecycle.lifecycle.addObserver(observer);onDispose{lifecycle.lifecycle.removeObserver(observer);model.foreground(false)}}
    BackHandler(enabled=settingsPage!=null){settingsPage=null}
    BackHandler(enabled=!engineering&&settingsPage==null&&(state.selectedThread!=null||board!=null)){if(board!=null)board=null else model.openThread(null)}
    val titles=listOf("对话","使命","队员","画布","设置");val icons=listOf(Icons.Outlined.Forum,Icons.Outlined.Flag,Icons.Outlined.Groups,Icons.Outlined.Map,Icons.Outlined.Settings)
    Scaffold(containerColor=Color.Transparent,topBar={TopAppBar(title={Text(state.thread?.get("thread").coreObject().coreText("title").ifEmpty{board?.coreText("name")?:profile?.name?:"Rovai Workbench"})},navigationIcon={if(state.selectedThread!=null||board!=null)IconButton(onClick={if(board!=null)board=null else model.openThread(null)}){Icon(Icons.AutoMirrored.Outlined.ArrowBack,null)}},actions={if(state.selectedThread!=null)IconButton(onClick={engineering=true}){Icon(Icons.Outlined.Folder,"工程")};IconButton(onClick=onServers){Icon(Icons.Outlined.Dns,"服务器")};IconButton(onClick=model::refreshNow,enabled=state.connected){Icon(Icons.Outlined.Refresh,"刷新")};if(tab==0&&state.selectedThread==null)IconButton(onClick={newConversation=true},enabled=state.connected&&state.pending==null){Icon(Icons.Outlined.Add,"新对话")}},colors=TopAppBarDefaults.topAppBarColors(containerColor=Color.Transparent))},bottomBar={if(state.selectedThread==null&&board==null)NavigationBar(containerColor=CodegTheme.colors.bgElevated){titles.forEachIndexed{index,title->NavigationBarItem(selected=tab==index,onClick={tab=index},icon={Icon(icons[index],title)},label={Text(title)})}}}){padding->Column(Modifier.fillMaxSize().padding(padding)){
        if(state.error!=null)Text(state.error!!,color=CodegTheme.colors.danger,modifier=Modifier.padding(12.dp))
        if(state.pending!=null)Row(Modifier.fillMaxWidth().padding(8.dp),horizontalArrangement=Arrangement.spacedBy(8.dp)){Text("原操作待核对",Modifier.weight(1f));TextButton(onClick=model::reconcile,enabled=!state.busy){Text("核对")};TextButton(onClick=model::retry,enabled=!state.busy){Text("重试原操作")}}
        if(!state.connected){Column(Modifier.padding(24.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){Text("连接 ${profile?.name?:"工作区"}",style=MaterialTheme.typography.titleLarge);Text("连接后使用服务器中的对话、使命、队员和工程。",color=CodegTheme.colors.textSecondary);Button(onClick=model::authenticate,enabled=!state.busy){Text(if(state.busy)"正在连接…" else "连接工作区")};TextButton(onClick=onServers){Text("管理服务器")}}}
        else if(engineering&&state.selectedThread!=null)RovaiEngineeringScreen(model,state.selectedThread!!,state.pending!=null){engineering=false}
        else if(state.selectedThread!=null)RovaiThreadScreen(model,state)
        else if(board!=null)RovaiNativeCanvas(model,board!!)
        else when(tab){
            0->LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(10.dp)){
                val quick=state.navigation["quickChat"].coreObject()["recentThreads"].coreArray();if(quick.isNotEmpty()){item{Text("快速对话",style=MaterialTheme.typography.titleSmall)};items(quick,key={"quick:"+it.coreText("id")}){RovaiThreadRow(it){model.openThread(it.coreText("id"))}}}
                state.navigation["projects"].coreArray().forEach{project->item{Text(project.coreText("name"),style=MaterialTheme.typography.titleSmall)};items(project["recentThreads"].coreArray(),key={project.coreText("projectKey")+it.coreText("id")}){RovaiThreadRow(it){model.openThread(it.coreText("id"))}};item{RovaiMoreThreads(model,project.coreText("projectPath"),project.coreLong("totalCount").toInt(),project["recentThreads"].coreArray().map{it.coreText("id")}.toSet())}}
            }
            1->LazyColumn(contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){items(state.missions,key={it.coreText("missionId")}){mission->GlassRow(Modifier.fillMaxWidth()){Text(mission.coreText("title"),style=MaterialTheme.typography.titleMedium);Text(mission.coreText("status"),color=CodegTheme.colors.textSecondary);Text(mission.coreText("description"),maxLines=3);Row{TextButton(onClick={model.openThread(mission.coreText("threadId"))}){Text("打开对话")};if((mission["startAvailable"] as? JsonPrimitive)?.booleanOrNull==true)TextButton(enabled=state.pending==null,onClick={model.command(RovaiOperation.MISSIONS_START,buildJsonObject{put("missionId",mission.coreText("missionId"))})}){Text("开始使命")}}}}}
            2->LazyColumn(contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){items(state.members,key={it.coreText("agentId")}){member->GlassRow(Modifier.fillMaxWidth()){Text(member.coreText("displayName"),style=MaterialTheme.typography.titleMedium);Text(member.coreText("teamRole"),color=CodegTheme.colors.textSecondary);Text(member.coreText("professionalResponsibilities"));val readiness=member["runtimeReadiness"].coreObject();Text(readiness.coreText("status"),style=MaterialTheme.typography.labelMedium,color=CodegTheme.colors.accent)}}}
            3->LazyColumn(contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){items(state.boards,key={it.coreText("id")}){canvas->GlassRow(Modifier.fillMaxWidth().clickable{board=canvas}){Icon(Icons.Outlined.Map,null,tint=CodegTheme.colors.textSecondary);Text(canvas.coreText("name").ifEmpty{"未命名画布"},style=MaterialTheme.typography.titleMedium);Text(canvas.coreText("description"),color=CodegTheme.colors.textSecondary)}}}
            4->if(settingsPage=="skills")RovaiSkillSettings(model)else if(settingsPage=="mcp")RovaiMcpSettings(model)else if(settingsPage=="appearance")RovaiAppearanceSettings()else Column(Modifier.padding(24.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){GlassRow(Modifier.fillMaxWidth().clickable(onClick=onServers)){Text("服务器");Text(profile?.baseUrl?:"",color=CodegTheme.colors.textSecondary)};GlassRow(Modifier.fillMaxWidth().clickable{settingsPage="appearance"}){Text("外观");Text("沿用Codeg原生主题、色彩和预览",color=CodegTheme.colors.textSecondary)};GlassRow(Modifier.fillMaxWidth().clickable{settingsPage="skills"}){Text("技能与专家库");Text("预览、安装和队员选择",color=CodegTheme.colors.textSecondary)};GlassRow(Modifier.fillMaxWidth().clickable{settingsPage="mcp"}){Text("MCP连接");Text("配置连接和队员访问权限",color=CodegTheme.colors.textSecondary)};TextButton(onClick=model::logout){Text("退出当前连接")}}
        }
    }}
    if(newConversation)RovaiNewThreadDialog(state,onDismiss={newConversation=false},onCreate={name,path,ids,lead,mode->model.execute(RovaiOperation.THREADS_CREATE,buildJsonObject{put("commandId",java.util.UUID.randomUUID().toString());put("name",name);put("workspace",path?.let{buildJsonObject{put("projectPath",it)}}?:JsonNull);put("memberAgentIds",JsonArray(ids.map(::JsonPrimitive)));put("defaultLeadAgentId",lead);put("collaborationMode",mode);put("activationState","active")});newConversation=false})
}
@Composable private fun RovaiThreadRow(thread:JsonObject,onOpen:()->Unit){GlassRow(Modifier.fillMaxWidth().clickable(onClick=onOpen)){Text(thread.coreText("title"),style=MaterialTheme.typography.titleMedium);val lead=thread["defaultLead"].coreObject().coreText("displayName");if(lead.isNotEmpty())Text(lead,color=CodegTheme.colors.textSecondary)}}
@Composable private fun RovaiMoreThreads(model:RovaiWorkspaceViewModel,path:String,total:Int,visible:Set<String>){var extra by remember(path){mutableStateOf<List<JsonObject>>(emptyList())};var next by remember(path){mutableStateOf<Int?>(0)};var busy by remember{mutableStateOf(false)};var error by remember{mutableStateOf<String?>(null)};val scope=rememberCoroutineScope();Column(verticalArrangement=Arrangement.spacedBy(8.dp)){extra.filter{it.coreText("id") !in visible}.forEach{thread->RovaiThreadRow(thread){model.openThread(thread.coreText("id"))}};error?.let{Text(it,color=CodegTheme.colors.danger)};if(next!=null&&total>visible.size)TextButton(enabled=!busy,onClick={scope.launch{busy=true;try{val value=model.query(RovaiOperation.NAVIGATION_GROUPTHREADS,buildJsonObject{put("projectPath",path);put("offset",next?:0);put("limit",50)}).coreObject();extra=extra+value["threads"].coreArray();next=(value["nextOffset"] as? JsonPrimitive)?.intOrNull}catch(failure:Exception){error=failure.message}finally{busy=false}}}){Text(if(busy)"正在读取…" else "更多对话")}}}
@Composable private fun RovaiThreadScreen(model:RovaiWorkspaceViewModel,state:RovaiWorkspaceState){var privateChat by remember(state.selectedThread){mutableStateOf(false)};if(privateChat){RovaiPrivateConversation(model,state.selectedThread!!){privateChat=false};return};val thread=state.thread;var text by rememberSaveable(state.selectedThread){mutableStateOf("")};var recipients by remember(state.selectedThread){mutableStateOf<Set<String>>(emptySet())};var submitted by remember(state.selectedThread){mutableStateOf<String?>(null)}
    LaunchedEffect(state.lastResult){if(state.lastCompletedOperation==RovaiOperation.THREAD_MESSAGES_SEND.wire&&state.lastResult.coreObject()["commandResult"].coreObject().coreText("status")=="applied"&&text==submitted){text="";submitted=null}}
    Column(Modifier.fillMaxSize()){LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){item{TextButton(onClick={privateChat=true}){Text("队员私聊")};RovaiAttachmentPicker(model,state.selectedThread!!,buildJsonObject{put("kind","camp")},0,state.pending!=null||state.busy||!state.publicSourcesReady);state.publicSources.forEach{source->Row{Text("附件 · "+source.coreText("displayName"),Modifier.weight(1f));TextButton(enabled=state.pending==null&&!state.busy,onClick={model.removePublicSource(source.coreText("id"))}){Text("移除")}};RovaiThreadControls(model,state);thread?.get("agentRuns").coreArray().forEach{run->key(run.coreText("id")){RovaiExecutionHistory(model,state.selectedThread!!,run,thread?.get("executionEvidence").coreArray())}}};items(thread?.get("messages").coreArray(),key={it.coreText("id")}){message->GlassRow(Modifier.fillMaxWidth(),selected=message.coreText("authorType")=="user"){Text(message.coreText("authorDisplayName").ifEmpty{if(message.coreText("authorType")=="user")"你" else message.coreText("authorId")},style=MaterialTheme.typography.labelMedium,color=CodegTheme.colors.textSecondary);MarkdownContent(message.coreText("body"))}};items(thread?.get("agentRuns").coreArray(),key={"run:"+it.coreText("id")}){run->if(run.coreText("status") in listOf("running","queued","waiting_approval","waiting_user"))Row{Text(run.coreText("agentId"),Modifier.weight(1f));TextButton(onClick={model.command(RovaiOperation.AGENTRUNS_CANCEL,buildJsonObject{put("threadId",state.selectedThread);put("agentRunId",run.coreText("id"));put("expectedVersion",run.coreLong("version"))})},enabled=state.pending==null){Text("停止")}}}}
        Row(Modifier.fillMaxWidth().padding(horizontal=12.dp),horizontalArrangement=Arrangement.spacedBy(6.dp)){thread?.get("members").coreArray().forEach{member->val id=member.coreText("agentId");FilterChip(selected=id in recipients,onClick={recipients=if(id in recipients)recipients-id else recipients+id},label={Text(member.coreText("displayName"))})}}
        Row(Modifier.fillMaxWidth().imePadding().padding(12.dp),horizontalArrangement=Arrangement.spacedBy(8.dp)){OutlinedTextField(value=text,onValueChange={text=it},modifier=Modifier.weight(1f),maxLines=6,placeholder={Text("发送消息…")},enabled=state.pending==null);FilledIconButton(onClick={submitted=text;model.send(text,recipients)},enabled=text.isNotBlank()&&state.pending==null&&!state.busy&&state.publicSourcesReady){Icon(Icons.Outlined.Send,"发送")}}
    }
}
@Composable private fun RovaiNewThreadDialog(state:RovaiWorkspaceState,onDismiss:()->Unit,onCreate:(String,String?,Set<String>,String,String)->Unit){var name by remember{mutableStateOf("")};var path by remember{mutableStateOf<String?>(null)};var ids by remember{mutableStateOf<Set<String>>(emptySet())};var lead by remember{mutableStateOf("")};var coordinated by remember{mutableStateOf(false)};AlertDialog(onDismissRequest=onDismiss,title={Text("新建对话")},text={LazyColumn{item{OutlinedTextField(value=name,onValueChange={name=it},label={Text("名称")})};item{TextButton(onClick={path=null}){Text(if(path==null)"✓ 快速对话" else "快速对话")}};items(state.navigation["projects"].coreArray()){project->TextButton(onClick={path=project.coreText("projectPath")}){Text((if(path==project.coreText("projectPath"))"✓ " else "")+project.coreText("name"))}};items(state.members.filter{it.coreText("presence")=="present"}){member->val id=member.coreText("agentId");Row{Checkbox(checked=id in ids,onCheckedChange={checked->ids=if(checked)ids+id else ids-id;if(lead !in ids)lead=ids.firstOrNull()?:""});TextButton(onClick={if(id in ids)lead=id}){Text(member.coreText("displayName")+(if(lead==id)" · 负责人" else ""))}}};item{Row{Switch(checked=coordinated,onCheckedChange={coordinated=it});Text("由负责人协调")}}}},confirmButton={TextButton(enabled=ids.isNotEmpty()&&lead in ids,onClick={onCreate(name,path,ids,lead,if(coordinated)"lead_coordinated" else "peer")}){Text("创建")}},dismissButton={TextButton(onClick=onDismiss){Text("取消")}})}
