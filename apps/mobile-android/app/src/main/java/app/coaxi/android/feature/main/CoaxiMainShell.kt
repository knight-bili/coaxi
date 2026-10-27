package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
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
import app.coaxi.android.app.AppViewModel
import app.coaxi.android.core.datastore.ServerProfile
import app.coaxi.android.core.designsystem.component.GlassRow
import app.coaxi.android.core.designsystem.markdown.MarkdownContent
import app.coaxi.android.core.designsystem.theme.CoaxiTheme
import app.coaxi.android.core.network.CoaxiOperation
import app.coaxi.android.feature.settings.CoaxiAppearanceSettings
import app.coaxi.android.feature.server.ServerEditorScreen
import app.coaxi.android.feature.server.ServerListScreen
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*

@Composable
fun CoaxiMainShell(appViewModel:AppViewModel,servers:List<ServerProfile>){
    val selected by appViewModel.selectedProfile.collectAsStateWithLifecycle();val nav=rememberNavController()
    NavHost(nav,startDestination="workbench"){
        composable("workbench"){CoaxiWorkspaceScreen(selected?:servers.firstOrNull(),onServers={nav.navigate("servers")})}
        composable("servers"){ServerListScreen(servers,selected?.id,appViewModel::selectServer,appViewModel::deleteServer,{nav.navigate("editor")},{nav.navigate("editor?serverId=$it")},{nav.popBackStack()})}
        composable("editor?serverId={serverId}",arguments=listOf(navArgument("serverId"){type=NavType.StringType;nullable=true;defaultValue=null})){ServerEditorScreen(onDone={nav.popBackStack()})}
    }
}
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun CoaxiWorkspaceScreen(profile:ServerProfile?,onServers:()->Unit,model:CoaxiWorkspaceViewModel=hiltViewModel()){
    val state by model.state.collectAsStateWithLifecycle();var tab by rememberSaveable{mutableIntStateOf(0)};val lifecycle=LocalLifecycleOwner.current
    var board by remember{mutableStateOf<JsonObject?>(null)};var newConversation by remember{mutableStateOf(false)};var engineering by remember(profile){mutableStateOf(false)};var settingsPage by remember(profile){mutableStateOf<String?>(null)};var editingMember by remember(profile){mutableStateOf<JsonObject?>(null)};var creatingMember by remember(profile){mutableStateOf(false)};var notifications by remember(profile){mutableStateOf(false)}
    val incomingLink by app.coaxi.android.core.common.CoaxiDeepLinkBus.pending.collectAsStateWithLifecycle()
    var linkIssue by remember(profile){mutableStateOf<String?>(null)}
    LaunchedEffect(profile){model.bind(profile)}
    LaunchedEffect(incomingLink,state.connected,state.busy,state.pending){
        val link=incomingLink?:return@LaunchedEffect;if(!state.connected||state.busy||state.pending!=null)return@LaunchedEffect
        val generation=model.connectionGeneration
        try {
            var threadTarget:String?=null;var canvasTarget:JsonObject?=null;var nextTab:Int?=null;var pane:String?=null
            when(link.kind){
                "thread"->{threadTarget=link.value;nextTab=0}
                "mission"->{val mission=model.query(CoaxiOperation.MISSIONS_GET,buildJsonObject{put("missionId",link.value)}).coreObject();threadTarget=mission.coreText("threadId");require(!threadTarget.isNullOrBlank()){"使命对话不存在"};nextTab=1}
                "canvas"->{canvasTarget=model.query(CoaxiOperation.CANVAS_BOARDS_GET,buildJsonObject{put("boardId",link.value)}).coreObject()["board"] as? JsonObject;require(canvasTarget!=null){"画布不存在"};nextTab=3}
                "tab"->{nextTab=when(link.value.lowercase()){ "chats","threads"->0;"missions"->1;"members"->2;"canvas","canvases"->3;"settings"->4;else->throw IllegalArgumentException("此旧版入口请从当前工作区导航打开")}}
                "settings"->{nextTab=4;pane=when(link.value.lowercase()){ "appearance"->"appearance";"agents","runtime"->"runtime";"skills","experts"->"skills";"mcp"->"mcp";"notifications"->"notifications";"quickmessages"->"quickMessages";else->throw IllegalArgumentException("该设置入口尚未映射，请从设置页打开")}}
                else->throw IllegalArgumentException("这是原Coaxi对象链接，请先导入原会话或从工程列表打开；不会将旧编号当作新对话。")
            }
            if(model.connectionGeneration!=generation||app.coaxi.android.core.common.CoaxiDeepLinkBus.pending.value!=link)return@LaunchedEffect
            notifications=false;engineering=false;settingsPage=pane;board=canvasTarget;nextTab?.let{tab=it};model.openThread(threadTarget);linkIssue=null
        }catch(cancelled:kotlinx.coroutines.CancellationException){throw cancelled}catch(error:Exception){if(model.connectionGeneration==generation)linkIssue=error.message}
        app.coaxi.android.core.common.CoaxiDeepLinkBus.consume(link)
    }
    DisposableEffect(lifecycle,model){val observer=LifecycleEventObserver{_,event->if(event==Lifecycle.Event.ON_START)model.foreground(true)else if(event==Lifecycle.Event.ON_STOP)model.foreground(false)};lifecycle.lifecycle.addObserver(observer);onDispose{lifecycle.lifecycle.removeObserver(observer);model.foreground(false)}}
    BackHandler(enabled=settingsPage!=null){settingsPage=null}
    BackHandler(enabled=!engineering&&settingsPage==null&&(state.selectedThread!=null||board!=null)){if(board!=null)board=null else model.openThread(null)}
    val titles=listOf(stringResource(R.string.coaxi_fusion_6bf6d3f3d6261b75),stringResource(R.string.coaxi_fusion_33aed1edf80b039d),stringResource(R.string.coaxi_fusion_6f3a9f7421bc2fe6),stringResource(R.string.coaxi_fusion_3906276fd1bda417),stringResource(R.string.coaxi_fusion_df3d58c7d84b85f2));val icons=listOf(Icons.Outlined.Forum,Icons.Outlined.Flag,Icons.Outlined.Groups,Icons.Outlined.Map,Icons.Outlined.Settings)
    Scaffold(containerColor=Color.Transparent,topBar={TopAppBar(title={Text(state.thread?.get("thread").coreObject().coreText("title").ifEmpty{board?.coreText("name")?:profile?.name?:"Coaxi"})},navigationIcon={if(state.selectedThread!=null||board!=null)IconButton(onClick={if(board!=null)board=null else model.openThread(null)}){Icon(Icons.AutoMirrored.Outlined.ArrowBack,null)}},actions={IconButton(enabled=state.connected,onClick={notifications=true}){Icon(Icons.Outlined.Notifications,stringResource(R.string.coaxi_fusion_8f29b3132ccf2182))};if(tab==2&&state.selectedThread==null)IconButton(enabled=state.pending==null&&state.connected,onClick={creatingMember=true}){Icon(Icons.Outlined.PersonAdd,stringResource(R.string.coaxi_fusion_2940344fab55863b))};if(state.selectedThread!=null)IconButton(onClick={engineering=true}){Icon(Icons.Outlined.Folder,stringResource(R.string.coaxi_fusion_85d56bf4d4229ac8))};IconButton(onClick=onServers){Icon(Icons.Outlined.Dns,stringResource(R.string.coaxi_fusion_2388cbc6a486af9f))};IconButton(onClick=model::refreshNow,enabled=state.connected){Icon(Icons.Outlined.Refresh,stringResource(R.string.coaxi_fusion_aee88743413144a2))};if(tab==0&&state.selectedThread==null)IconButton(onClick={newConversation=true},enabled=state.connected&&state.pending==null){Icon(Icons.Outlined.Add,stringResource(R.string.coaxi_fusion_ea4b29f58f879802))}},colors=TopAppBarDefaults.topAppBarColors(containerColor=Color.Transparent))},bottomBar={if(state.selectedThread==null&&board==null)NavigationBar(containerColor=CoaxiTheme.colors.bgElevated){titles.forEachIndexed{index,title->NavigationBarItem(selected=tab==index,onClick={tab=index},icon={Icon(icons[index],title)},label={Text(title)})}}}){padding->Column(Modifier.fillMaxSize().padding(padding)){
        linkIssue?.let{Text(it,color=CoaxiTheme.colors.danger,modifier=Modifier.padding(12.dp))}
        if(state.error!=null)Text(state.error!!,color=CoaxiTheme.colors.danger,modifier=Modifier.padding(12.dp))
        if(state.pending!=null)Row(Modifier.fillMaxWidth().padding(8.dp),horizontalArrangement=Arrangement.spacedBy(8.dp)){Text(stringResource(R.string.coaxi_fusion_5a692cd92a0632c9),Modifier.weight(1f));TextButton(onClick=model::reconcile,enabled=!state.busy){Text(stringResource(R.string.coaxi_fusion_2e81b3dff5809023))};TextButton(onClick=model::retry,enabled=!state.busy){Text(stringResource(R.string.coaxi_fusion_04a02c897ac919f0))}}
        if(!state.connected){Column(Modifier.padding(24.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){Text("连接 ${profile?.name?:"工作区"}",style=MaterialTheme.typography.titleLarge);Text(stringResource(R.string.coaxi_fusion_d8b94d743e1e9697),color=CoaxiTheme.colors.textSecondary);Button(onClick=model::authenticate,enabled=!state.busy){Text(if(state.busy)"正在连接…" else "连接工作区")};TextButton(onClick=onServers){Text(stringResource(R.string.coaxi_fusion_50cfabf193e1464e))}}}
        else if(engineering&&state.selectedThread!=null)CoaxiEngineeringScreen(model,state.selectedThread!!,state.pending!=null){engineering=false}
        else if(state.selectedThread!=null)CoaxiThreadScreen(model,state)
        else if(board!=null)CoaxiNativeCanvas(model,board!!)
        else when(tab){
            0->CoaxiProjectNavigation(model)
            1->CoaxiMissionBoard(model,state)
            2->LazyColumn(contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){items(state.members,key={it.coreText("agentId")}){member->GlassRow(Modifier.fillMaxWidth().clickable{editingMember=member}){Text(member.coreText("displayName"),style=MaterialTheme.typography.titleMedium);Text(member.coreText("teamRole"),color=CoaxiTheme.colors.textSecondary);Text(member.coreText("professionalResponsibilities"));val readiness=member["runtimeReadiness"].coreObject();Text(readiness.coreText("status"),style=MaterialTheme.typography.labelMedium,color=CoaxiTheme.colors.accent);CoaxiMemberLifecycle(model,member)}}}
            3->CoaxiCanvasBoards(model){board=it}
            4->if(settingsPage=="diagnostics")CoaxiDiagnostics(model)else if(settingsPage=="monitoring")CoaxiMonitoring(model)else if(settingsPage=="webdav")CoaxiWebdavSettings(model){settingsPage=null}else if(settingsPage=="memory")CoaxiMemorySettings(model)else if(settingsPage=="automations")CoaxiAutomationSettings(model){id->model.openThread(id)}else if(settingsPage=="quickMessages")CoaxiMessageLibrary(model)else if(settingsPage=="skills")CoaxiSkillSettings(model)else if(settingsPage=="mcp")CoaxiMcpSettings(model)else if(settingsPage=="appearance")CoaxiAppearanceSettings()else if(settingsPage=="runtime")CoaxiRuntimeSettings(model)else if(settingsPage=="notifications")CoaxiNotificationSettings(model)else Column(Modifier.padding(24.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){GlassRow(Modifier.fillMaxWidth().clickable{settingsPage="memory"}){Text(stringResource(R.string.coaxi_fusion_7a6335b379b9a161));Text(stringResource(R.string.coaxi_fusion_70103bbdb08dfe5d),color=CoaxiTheme.colors.textSecondary)};GlassRow(Modifier.fillMaxWidth().clickable{settingsPage="automations"}){Text(stringResource(R.string.coaxi_fusion_3c39e9bde120ddb5));Text(stringResource(R.string.coaxi_fusion_0b3f8dede80791c5),color=CoaxiTheme.colors.textSecondary)};GlassRow(Modifier.fillMaxWidth().clickable(onClick=onServers)){Text(stringResource(R.string.coaxi_fusion_2388cbc6a486af9f));Text(profile?.baseUrl?:"",color=CoaxiTheme.colors.textSecondary)};GlassRow(Modifier.fillMaxWidth().clickable{settingsPage="notifications"}){Text(stringResource(R.string.coaxi_fusion_8f29b3132ccf2182));Text(stringResource(R.string.coaxi_fusion_dfc73598766c5592),color=CoaxiTheme.colors.textSecondary)};GlassRow(Modifier.fillMaxWidth().clickable{settingsPage="diagnostics"}){Text(stringResource(R.string.coaxi_fusion_00e884d505858e4c));Text(stringResource(R.string.coaxi_fusion_274412dc667137e4),color=CoaxiTheme.colors.textSecondary)};GlassRow(Modifier.fillMaxWidth().clickable{settingsPage="monitoring"}){Text(stringResource(R.string.coaxi_fusion_65e09e4dfa428480));Text(stringResource(R.string.coaxi_fusion_88a9d9c742375f49),color=CoaxiTheme.colors.textSecondary)};GlassRow(Modifier.fillMaxWidth().clickable{settingsPage="runtime"}){Text(stringResource(R.string.coaxi_fusion_b43bf713d5da85e3));Text(stringResource(R.string.coaxi_fusion_2dc02f1857eb757d),color=CoaxiTheme.colors.textSecondary)};GlassRow(Modifier.fillMaxWidth().clickable{settingsPage="appearance"}){Text(stringResource(R.string.coaxi_fusion_86a63f23a076b11e));Text(stringResource(R.string.coaxi_fusion_e692645667d2d5a2),color=CoaxiTheme.colors.textSecondary)};GlassRow(Modifier.fillMaxWidth().clickable{settingsPage="skills"}){Text(stringResource(R.string.coaxi_fusion_daf860ab8b6be09b));Text(stringResource(R.string.coaxi_fusion_6dac025f48ffb9d1),color=CoaxiTheme.colors.textSecondary)};GlassRow(Modifier.fillMaxWidth().clickable{settingsPage="quickMessages"}){Text(stringResource(R.string.coaxi_fusion_22194bde79e3a689));Text(stringResource(R.string.coaxi_fusion_2c18b930ef7174b4),color=CoaxiTheme.colors.textSecondary)};GlassRow(Modifier.fillMaxWidth().clickable{settingsPage="webdav"}){Text(stringResource(R.string.coaxi_fusion_acde8c3e5ad0a8e3));Text(stringResource(R.string.coaxi_fusion_27cd6e139e9b10d2),color=CoaxiTheme.colors.textSecondary)};GlassRow(Modifier.fillMaxWidth().clickable{settingsPage="mcp"}){Text(stringResource(R.string.coaxi_fusion_aff92ea64f3abcf4));Text(stringResource(R.string.coaxi_fusion_f696a19c0b55fbb0),color=CoaxiTheme.colors.textSecondary)};TextButton(onClick=model::logout){Text(stringResource(R.string.coaxi_fusion_359a568156c041b4))}}
        }
    }}
    if(notifications)Dialog(onDismissRequest={notifications=false},properties=DialogProperties(usePlatformDefaultWidth=false,dismissOnBackPress=false,dismissOnClickOutside=false)){Surface(Modifier.fillMaxSize(),color=CoaxiTheme.colors.bgElevated){CoaxiNotificationInbox(model){notifications=false}}}
    if(creatingMember||editingMember!=null)CoaxiMemberEditor(model,editingMember){creatingMember=false;editingMember=null}
    if(newConversation)CoaxiNewThreadDialog(state,onDismiss={newConversation=false},onCreate={name,path,ids,lead,mode->model.execute(CoaxiOperation.THREADS_CREATE,buildJsonObject{put("commandId",java.util.UUID.randomUUID().toString());put("name",name);put("workspace",path?.let{buildJsonObject{put("projectPath",it)}}?:JsonNull);put("memberAgentIds",JsonArray(ids.map(::JsonPrimitive)));put("defaultLeadAgentId",lead);put("collaborationMode",mode);put("activationState","active")});newConversation=false})
}
@Composable internal fun CoaxiThreadRow(thread:JsonObject,onOpen:()->Unit){GlassRow(Modifier.fillMaxWidth().clickable(onClick=onOpen)){Text(thread.coreText("title"),style=MaterialTheme.typography.titleMedium);val lead=thread["defaultLead"].coreObject().coreText("displayName");if(lead.isNotEmpty())Text(lead,color=CoaxiTheme.colors.textSecondary)}}
@Composable internal fun CoaxiMoreThreads(model:CoaxiWorkspaceViewModel,path:String,total:Int,visible:Set<String>){var extra by remember(path){mutableStateOf<List<JsonObject>>(emptyList())};var next by remember(path){mutableStateOf<Int?>(0)};var busy by remember{mutableStateOf(false)};var error by remember{mutableStateOf<String?>(null)};val scope=rememberCoroutineScope();Column(verticalArrangement=Arrangement.spacedBy(8.dp)){extra.filter{it.coreText("id") !in visible}.forEach{thread->CoaxiThreadRow(thread){model.openThread(thread.coreText("id"))}};error?.let{Text(it,color=CoaxiTheme.colors.danger)};if(next!=null&&total>visible.size)TextButton(enabled=!busy,onClick={scope.launch{busy=true;try{val value=model.query(CoaxiOperation.NAVIGATION_GROUPTHREADS,buildJsonObject{put("projectPath",path);put("offset",next?:0);put("limit",50)}).coreObject();extra=extra+value["threads"].coreArray();next=(value["nextOffset"] as? JsonPrimitive)?.intOrNull}catch(failure:Exception){error=failure.message}finally{busy=false}}}){Text(if(busy)"正在读取…" else "更多对话")}}}
@Composable private fun CoaxiThreadScreen(model:CoaxiWorkspaceViewModel,state:CoaxiWorkspaceState){var feedback by remember(state.selectedThread){mutableStateOf(false)};if(feedback&&state.selectedThread!=null)CoaxiUserFeedback(model,state.selectedThread!!){feedback=false};var messageHistory by remember(state.selectedThread){mutableStateOf(false)};if(messageHistory)CoaxiThreadHistory(model,state.selectedThread!!){messageHistory=false};var importedHistory by remember(state.selectedThread){mutableStateOf(false)};if(importedHistory){CoaxiHistoryLibrary(model,state.selectedThread!!){importedHistory=false};return};var library by remember(state.selectedThread){mutableStateOf(false)};var privateChat by remember(state.selectedThread){mutableStateOf(false)};if(privateChat){CoaxiPrivateConversation(model,state.selectedThread!!){privateChat=false};return};val thread=state.thread;var text by rememberSaveable(state.selectedThread){mutableStateOf("")};var recipients by remember(state.selectedThread){mutableStateOf<Set<String>>(emptySet())};var reply by remember(state.selectedThread){mutableStateOf<JsonObject?>(null)};var quotes by remember(state.selectedThread){mutableStateOf<List<JsonObject>>(emptyList())};var removedQuote by remember(state.selectedThread){mutableStateOf<Pair<Int,JsonObject>?>(null)};var bodyReady by remember(state.selectedThread){mutableStateOf(false)};var inputRevision by remember(state.selectedThread){mutableLongStateOf(0L)};var inputIssue by remember(state.selectedThread){mutableStateOf<String?>(null)};val coroutine=rememberCoroutineScope();val owner="public:${state.selectedThread}"
    LaunchedEffect(state.selectedThread){try{val raw=model.readBody(owner);val input=if(raw.isBlank())buildJsonObject{}else Json.parseToJsonElement(raw).coreObject();text=input.coreText("text");recipients=(input["recipients"] as? JsonArray)?.mapNotNull{(it as? JsonPrimitive)?.contentOrNull}?.toSet()?:emptySet();inputRevision=input.coreLong("revision");reply=input["reply"] as? JsonObject;quotes=input["quotes"].coreArray();bodyReady=true}catch(cancelled:kotlinx.coroutines.CancellationException){throw cancelled}catch(error:Exception){inputIssue=error.message}}
    LaunchedEffect(state.lastResult){try{val expected=state.lastInputBody;val receipt=state.lastResult.coreObject()["commandResult"].coreObject().ifEmpty{state.lastResult.coreObject()};if(receipt.coreText("status") in listOf("applied","accepted")&&state.lastCompletedOperation==CoaxiOperation.THREAD_MESSAGES_SEND.wire&&state.lastInputOwner==owner&&bodyReady&&expected!=null&&model.publicBody(text,recipients,inputRevision,reply,quotes)==expected){val raw=model.readBody(owner);if(model.publicBody(text,recipients,inputRevision,reply,quotes)==expected){val input=if(raw.isBlank())buildJsonObject{}else Json.parseToJsonElement(raw).coreObject();text=input.coreText("text");recipients=(input["recipients"] as? JsonArray)?.mapNotNull{(it as? JsonPrimitive)?.contentOrNull}?.toSet()?:emptySet();inputRevision=if(raw.isBlank())inputRevision+1 else input.coreLong("revision");reply=input["reply"] as? JsonObject;quotes=input["quotes"].coreArray()}}}catch(cancelled:kotlinx.coroutines.CancellationException){throw cancelled}catch(error:Exception){inputIssue=error.message}}

    if(library)Dialog(onDismissRequest={library=false},properties=DialogProperties(usePlatformDefaultWidth=false)){Surface(Modifier.fillMaxSize()){Column{TextButton(onClick={library=false}){Text(stringResource(R.string.coaxi_fusion_0d83851ba3dbd802))};CoaxiMessageLibrary(model,state.selectedThread){body->if(bodyReady&&model.state.value.pending==null&&!model.state.value.busy){text=if(text.isBlank())body else text+"\n"+body;inputRevision++;model.rememberBody(owner,model.publicBody(text,recipients,inputRevision,reply,quotes));library=false;true}else false}}}}
    Column(Modifier.fillMaxSize()) {
        inputIssue?.let { Text(it, color = CoaxiTheme.colors.danger, modifier = Modifier.padding(12.dp)) }

        LazyColumn(
            Modifier.weight(1f),
            contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            item {
                TextButton(
                    enabled = bodyReady && state.pending == null && !state.busy,
                    onClick = { library = true }
                ) { Text(stringResource(R.string.coaxi_fusion_22194bde79e3a689)) }
                TextButton(onClick = { feedback = true }) {
                    Text(stringResource(R.string.coaxi_fusion_d223a7cd3f8dbb4b))
                }
                TextButton(onClick = { messageHistory = true }) {
                    Text(stringResource(R.string.coaxi_fusion_cf116362723d291d))
                }
                TextButton(onClick = { importedHistory = true }) {
                    Text(stringResource(R.string.coaxi_fusion_e7f6bd860c215c56))
                }
                TextButton(
                    onClick = {
                        coroutine.launch {
                            try {
                                if (bodyReady) {
                                    model.saveBody(owner, model.publicBody(text, recipients, inputRevision, reply, quotes))
                                }
                                privateChat = true
                            } catch (error: Exception) {
                                inputIssue = error.message
                            }
                        }
                    }
                ) { Text(stringResource(R.string.coaxi_fusion_a5295a243877625b)) }
                CoaxiAttachmentPicker(
                    model,
                    state.selectedThread!!,
                    buildJsonObject { put("kind", "camp") },
                    0,
                    state.pending != null || state.busy || !state.publicSourcesReady
                )
                state.publicSources.forEach { source ->
                    Row {
                        Text(
                            stringResource(R.string.coaxi_fusion_f90da1272c11a23f) + source.coreText("displayName"),
                            Modifier.weight(1f)
                        )
                        TextButton(
                            enabled = state.pending == null && !state.busy,
                            onClick = { model.removePublicSource(source.coreText("id")) }
                        ) { Text(stringResource(R.string.coaxi_fusion_6135d4159e892541)) }
                    }
                }
                CoaxiConversationSources(
                    model,
                    state.selectedThread!!,
                    enabled = bodyReady && state.pending == null && !state.busy
                ) { reference ->
                    if (!bodyReady || model.state.value.pending != null || model.state.value.busy) {
                        false
                    } else {
                        if (!text.contains(reference)) {
                            text = if (text.isEmpty()) reference else text + "\n" + reference
                            inputRevision++
                            model.rememberBody(owner, model.publicBody(text, recipients, inputRevision, reply, quotes))
                        }
                        true
                    }
                }
                CoaxiThreadControls(model, state)
                key(state.selectedThread, model.connectionGeneration) {
                    CoaxiConversationQuestions(model, state.selectedThread!!)
                }
                thread?.get("agentRuns").coreArray().forEach { run ->
                    key(run.coreText("id")) {
                        CoaxiExecutionHistory(
                            model,
                            state.selectedThread!!,
                            run,
                            thread?.get("executionEvidence").coreArray()
                        )
                    }
                }
            }

            items(thread?.get("messages").coreArray(), key = { it.coreText("id") }) { message ->
                GlassRow(
                    Modifier.fillMaxWidth(),
                    selected = message.coreText("authorType") == "user"
                ) {
                    Text(
                        message.coreText("authorDisplayName").ifEmpty {
                            if (message.coreText("authorType") == "user") "你" else message.coreText("authorId")
                        },
                        style = MaterialTheme.typography.labelMedium,
                        color = CoaxiTheme.colors.textSecondary
                    )
                    CoaxiMessageBody(model, state.selectedThread!!, message)
                    CoaxiRuntimeImageGallery(
                        model,
                        state.selectedThread!!,
                        coaxiImageGroups(thread ?: buildJsonObject {}, message)
                    )
                    CoaxiMessageActions(
                        model,
                        state.selectedThread!!,
                        message,
                        bodyReady && state.pending == null && !state.busy,
                        onQuote = { quote ->
                            if (!bodyReady || model.state.value.pending != null || model.state.value.busy) {
                                false
                            } else {
                                quotes = quotes + quote
                                inputRevision++
                                model.rememberBody(owner, model.publicBody(text, recipients, inputRevision, reply, quotes))
                                true
                            }
                        },
                        onReply = { chosen ->
                            reply = chosen
                            val author = chosen.coreText("authorId")
                            if (thread?.get("members").coreArray().any { it.coreText("agentId") == author }) {
                                recipients = recipients + author
                            }
                            inputRevision++
                            model.rememberBody(owner, model.publicBody(text, recipients, inputRevision, reply, quotes))
                        }
                    )
                }
            }
            items(
                coaxiOrphanImages(thread ?: buildJsonObject {}),
                key = { "images:" + it.coreText("agentRunId") + ":" + it.coreLong("executionEpoch") }
            ) { group ->
                GlassRow {
                    CoaxiRuntimeImageGallery(model, state.selectedThread!!, listOf(group))
                }
            }
            items(thread?.get("agentRuns").coreArray(), key = { "run:" + it.coreText("id") }) { run ->
                if (run.coreText("status") in listOf("running", "queued", "waiting_approval", "waiting_user")) {
                    Row {
                        Text(run.coreText("agentId"), Modifier.weight(1f))
                        TextButton(
                            onClick = {
                                model.command(
                                    CoaxiOperation.AGENTRUNS_CANCEL,
                                    buildJsonObject {
                                        put("threadId", state.selectedThread)
                                        put("agentRunId", run.coreText("id"))
                                        put("expectedVersion", run.coreLong("version"))
                                    }
                                )
                            },
                            enabled = state.pending == null
                        ) {
                            Text(stringResource(R.string.coaxi_fusion_ca4d973c0b006b75))
                        }
                    }
                }
            }
        }

        CoaxiComposerQuotes(quotes, bodyReady && state.pending == null && !state.busy) { quoteId ->
            val index = quotes.indexOfFirst { it.coreText("quoteId") == quoteId }
            if (index >= 0) {
                removedQuote = index to quotes[index]
                quotes = quotes.filterIndexed { i, _ -> i != index }
                inputRevision++
                model.rememberBody(owner, model.publicBody(text, recipients, inputRevision, reply, quotes))
            }
        }
        removedQuote?.let { (index, quote) ->
            TextButton(
                enabled = bodyReady && state.pending == null && !state.busy,
                onClick = {
                    quotes = quotes.toMutableList().apply { add(index.coerceAtMost(size), quote) }
                    removedQuote = null
                    inputRevision++
                    model.rememberBody(owner, model.publicBody(text, recipients, inputRevision, reply, quotes))
                }
            ) { Text(stringResource(R.string.coaxi_fusion_e80b755ef87b7d65)) }
        }
        reply?.let { chosen ->
            CoaxiReplyPreview(chosen, bodyReady && state.pending == null && !state.busy) {
                reply = null
                inputRevision++
                model.rememberBody(owner, model.publicBody(text, recipients, inputRevision, reply, quotes))
            }
        }
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 12.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            thread?.get("members").coreArray().forEach { member ->
                val id = member.coreText("agentId")
                FilterChip(
                    selected = id in recipients,
                    enabled = bodyReady && state.pending == null && !state.busy,
                    onClick = {
                        if (model.state.value.pending == null && !model.state.value.busy) {
                            recipients = if (id in recipients) recipients - id else recipients + id
                            inputRevision++
                            model.rememberBody(owner, model.publicBody(text, recipients, inputRevision, reply, quotes))
                        }
                    },
                    label = { Text(member.coreText("displayName")) }
                )
            }
            (
                recipients - thread?.get("members").coreArray().map { it.coreText("agentId") }.toSet()
            ).forEach { id ->
                InputChip(
                    selected = true,
                    onClick = {
                        if (model.state.value.pending == null && !model.state.value.busy) {
                            recipients = recipients - id
                            inputRevision++
                            model.rememberBody(owner, model.publicBody(text, recipients, inputRevision, reply, quotes))
                        }
                    },
                    enabled = bodyReady && state.pending == null && !state.busy,
                    label = { Text(stringResource(R.string.coaxi_fusion_f8f8ea223e4962a7)) },
                    trailingIcon = { Icon(Icons.Outlined.Close, "移除收件人") }
                )
            }
        }
        Row(
            Modifier.fillMaxWidth().imePadding().padding(12.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            OutlinedTextField(
                value = text,
                onValueChange = { value ->
                    if (model.state.value.pending == null && !model.state.value.busy && text != value) {
                        text = value
                        inputRevision++
                        model.rememberBody(owner, model.publicBody(text, recipients, inputRevision, reply, quotes))
                    }
                },
                modifier = Modifier.weight(1f),
                maxLines = 6,
                placeholder = { Text(stringResource(R.string.coaxi_fusion_ae2f86f0c4099981)) },
                enabled = bodyReady && state.pending == null && !state.busy
            )
            FilledIconButton(
                onClick = {
                    val frozenText = text
                    val frozenRecipients = recipients
                    val frozenThread = state.selectedThread!!
                    val document = model.publicBody(text, recipients, inputRevision, reply, quotes)
                    coroutine.launch {
                        try {
                            model.saveBody(owner, document)
                            model.send(frozenText, frozenRecipients, document, frozenThread)
                        } catch (error: Exception) {
                            inputIssue = error.message
                        }
                    }
                },
                enabled = text.isNotBlank() && state.pending == null && !state.busy &&
                    state.publicSourcesReady && bodyReady && inputIssue == null
            ) {
                Icon(Icons.Outlined.Send, "发送")
            }
        }
    }
}
@Composable
private fun CoaxiNewThreadDialog(
    state: CoaxiWorkspaceState,
    onDismiss: () -> Unit,
    onCreate: (String, String?, Set<String>, String, String) -> Unit
) {
    var name by remember { mutableStateOf("") }
    var path by remember { mutableStateOf<String?>(null) }
    var ids by remember { mutableStateOf<Set<String>>(emptySet()) }
    var lead by remember { mutableStateOf("") }
    var coordinated by remember { mutableStateOf(false) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.coaxi_fusion_6897a8ef070bd2c3)) },
        text = {
            LazyColumn {
                item {
                    OutlinedTextField(
                        value = name,
                        onValueChange = { name = it },
                        label = { Text(stringResource(R.string.coaxi_fusion_d44e9b3d3b31d37b)) }
                    )
                }
                item {
                    TextButton(onClick = { path = null }) {
                        Text(if (path == null) "✓ 快速对话" else "快速对话")
                    }
                }
                items(state.navigation["projects"].coreArray()) { project ->
                    TextButton(onClick = { path = project.coreText("projectPath") }) {
                        val selected = path == project.coreText("projectPath")
                        Text((if (selected) "✓ " else "") + project.coreText("name"))
                    }
                }
                items(state.members.filter { it.coreText("presence") == "present" }) { member ->
                    val id = member.coreText("agentId")
                    Row {
                        Checkbox(
                            checked = id in ids,
                            onCheckedChange = { checked ->
                                ids = if (checked) ids + id else ids - id
                                if (lead !in ids) lead = ids.firstOrNull() ?: ""
                            }
                        )
                        TextButton(onClick = { if (id in ids) lead = id }) {
                            Text(member.coreText("displayName") + (if (lead == id) " · 负责人" else ""))
                        }
                    }
                }
                item {
                    Row {
                        Switch(
                            checked = coordinated,
                            onCheckedChange = { coordinated = it }
                        )
                        Text(stringResource(R.string.coaxi_fusion_cafbd22ba74866fd))
                    }
                }
            }
        },
        confirmButton = {
            TextButton(
                enabled = ids.isNotEmpty() && lead in ids,
                onClick = {
                    onCreate(name, path, ids, lead, if (coordinated) "lead_coordinated" else "peer")
                }
            ) {
                Text(stringResource(R.string.coaxi_fusion_cde2cd071d25bbab))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.coaxi_fusion_2cd0f3be8738a86c))
            }
        }
    )
}
