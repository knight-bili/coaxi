package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*

private data class McpConfirmation(val operation:RovaiOperation,val params:JsonObject,val title:String,val detail:String)
@Composable
fun RovaiMcpSettings(model:RovaiWorkspaceViewModel){
    val state by model.state.collectAsStateWithLifecycle();val scope=rememberCoroutineScope()
    var config by remember{mutableStateOf(buildJsonObject{})};var selected by remember{mutableStateOf<String?>(null)};var filter by remember{mutableStateOf("")}
    var draft by remember{mutableStateOf<String?>(null)};var baseline by remember{mutableStateOf<String?>(null)};var baseDigest by remember{mutableStateOf("")}
    var issue by remember{mutableStateOf<String?>(null)};var loading by remember{mutableStateOf(false)};var confirmation by remember{mutableStateOf<McpConfirmation?>(null)};var discard by remember{mutableStateOf(false)}
    val disabled=loading||state.pending!=null||state.busy;val dirty=draft!=null&&draft!=baseline
    suspend fun load(){loading=true;try{config=model.query(RovaiOperation.MCP_CONFIG_GET).coreObject();issue=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{loading=false}}
    fun choose(id:String?){selected=id;draft=if(id=="new")"{\n  \"mcpServers\": {\n    \"my-server\": {\n      \"command\": \"npx\",\n      \"args\": [\"-y\", \"@example/mcp-server\"]\n    }\n  }\n}" else null;baseline=draft;baseDigest=config.coreText("configDigest")}
    fun mutate(operation:RovaiOperation,params:JsonObject,title:String,highRisk:Boolean=false){if(highRisk)confirmation=McpConfirmation(operation,params,title,"该连接可以执行外部命令或访问外部服务。确认后按当前选择启用。")else model.execute(operation,params)}
    LaunchedEffect(Unit){load()}
    LaunchedEffect(state.lastResult){if(state.lastCompletedOperation?.startsWith("mcp.")==true){val result=state.lastResult.coreObject();if(result.coreText("status")=="ok"){config=result["config"].coreObject();baseline=draft;baseDigest=config.coreText("configDigest")}else{issue=if(result.coreText("status")=="conflict")"配置已在其他位置修改。草稿已保留，请重新读取后编辑。" else result["issues"].coreArray().firstOrNull()?.coreText("message")?:"配置尚未保存，请刷新后核对。"}}}
    BackHandler(enabled=selected!=null){if(dirty)discard=true else choose(null)}
    val server=config["servers"].coreArray().find{it.coreText("serverId")==selected}
    LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
        item{Row(horizontalArrangement=Arrangement.spacedBy(8.dp)){if(selected!=null)TextButton(onClick={if(dirty)discard=true else choose(null)}){Text(stringResource(R.string.rovai_fusion_fbd8834534ebb64c))};TextButton(enabled=!disabled,onClick={scope.launch{load()}}){Text(stringResource(R.string.rovai_fusion_aee88743413144a2))};if(selected==null)TextButton(enabled=!disabled,onClick={choose("new")}){Text(stringResource(R.string.rovai_fusion_ef8009c397a3a1f7))}}}
        issue?.let{item{Text(it,color=CodegTheme.colors.danger)}}
        config["fileIssue"]?.takeIf{it!=JsonNull}?.let{item{Text(stringResource(R.string.rovai_fusion_7ef7e730a800bdda)+it.coreObject().coreText("message"),color=CodegTheme.colors.danger)}}
        if(selected==null){
            item{OutlinedTextField(value=filter,onValueChange={filter=it},label={Text(stringResource(R.string.rovai_fusion_5faadb2dfff8ec56))},modifier=Modifier.fillMaxWidth())}
            items(config["servers"].coreArray().filter{"${it.coreText("name")} ${it.coreText("endpoint")}".contains(filter,true)},key={it.coreText("serverId")}){entry->GlassRow(Modifier.fillMaxWidth().clickable{choose(entry.coreText("serverId"))}){Text(entry.coreText("name"),style=MaterialTheme.typography.titleMedium);Text(entry.coreText("transport")+" · "+entry.coreText("endpoint"),color=CodegTheme.colors.textSecondary);Text(if((entry["enabled"] as? JsonPrimitive)?.booleanOrNull==true)"已启用" else "已停用",style=MaterialTheme.typography.labelSmall)}}
        }else{
            item{Text(server?.coreText("name")?:"添加MCP",style=MaterialTheme.typography.titleLarge)}
            if(server!=null){
                item{GlassRow(Modifier.fillMaxWidth()){
                    val enabled=(server["enabled"] as? JsonPrimitive)?.booleanOrNull==true
                    Row{Text(stringResource(R.string.rovai_fusion_b697b0265d63a322),Modifier.weight(1f));Switch(checked=enabled,enabled=!disabled,onCheckedChange={value->mutate(RovaiOperation.MCP_SERVERS_SETENABLED,buildJsonObject{put("serverId",selected);put("expectedConfigDigest",config.coreText("configDigest"));put("enabled",value);put("acknowledgeHighRisk",value&&server.coreText("riskLevel")=="high")},"启用高风险MCP",value&&server.coreText("riskLevel")=="high")})}
                    TextButton(enabled=!disabled,onClick={confirmation=McpConfirmation(RovaiOperation.MCP_SERVERS_DELETE,buildJsonObject{put("serverId",selected);put("expectedConfigDigest",config.coreText("configDigest"))},"删除MCP","删除连接及队员授权，已产生的会话历史仍保留。")}){Text(stringResource(R.string.rovai_fusion_92e11347ed70b031))}
                }}
                item{Text(stringResource(R.string.rovai_fusion_85f5366ea87cc439),style=MaterialTheme.typography.titleMedium)}
                items(state.members.filter{it.coreText("presence")!="removed"},key={it.coreText("agentId")}){member->val picked=(server["assignedAgentIds"] as? JsonArray)?.mapNotNull{(it as? JsonPrimitive)?.contentOrNull}?.toSet()?:emptySet();val id=member.coreText("agentId");Row(Modifier.fillMaxWidth()){Text(member.coreText("displayName"),Modifier.weight(1f));Checkbox(checked=id in picked,enabled=!disabled,onCheckedChange={value->mutate(RovaiOperation.MCP_SERVERS_SETMEMBERS,buildJsonObject{put("serverId",selected);put("expectedConfigDigest",config.coreText("configDigest"));put("agentIds",JsonArray((if(value)picked+id else picked-id).sorted().map(::JsonPrimitive)));put("acknowledgeHighRisk",value&&server.coreText("riskLevel")=="high")},"授权高风险MCP",value&&server.coreText("riskLevel")=="high")})}}
            }
            item{GlassRow(Modifier.fillMaxWidth()){
                Text(stringResource(R.string.rovai_fusion_148d195e21b05db5),style=MaterialTheme.typography.titleMedium)
                if(draft==null){Text(server?.coreText("definitionJson")?:"",fontFamily=FontFamily.Monospace,color=CodegTheme.colors.textSecondary);TextButton(enabled=!disabled,onClick={scope.launch{loading=true;try{val revealed=model.query(RovaiOperation.MCP_SERVERS_REVEAL,buildJsonObject{put("serverId",selected);put("expectedConfigDigest",config.coreText("configDigest"))}).coreObject();if(revealed.coreText("status")!="ok")throw IllegalStateException("配置已变化，请刷新后重新查看。");draft=revealed.coreText("definitionJson");baseline=draft;baseDigest=revealed.coreText("configDigest")}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{loading=false}}}){Text(stringResource(R.string.rovai_fusion_08672ce76c4017b3))}}
                else{OutlinedTextField(value=draft?:"",onValueChange={draft=it},modifier=Modifier.fillMaxWidth(),minLines=8,maxLines=22,textStyle=LocalTextStyle.current.copy(fontFamily=FontFamily.Monospace),enabled=!disabled);Row{TextButton(enabled=!disabled,onClick={draft=null;baseline=null}){Text(stringResource(R.string.rovai_fusion_17f1ff1b24ca2d7d))};Button(enabled=!disabled&&dirty,onClick={try{Json.parseToJsonElement(draft?:"");model.execute(if(selected=="new")RovaiOperation.MCP_SERVERS_CREATE else RovaiOperation.MCP_SERVERS_UPDATE,buildJsonObject{put("expectedConfigDigest",baseDigest);if(selected!="new")put("serverId",selected);put("definitionJson",draft)})}catch(error:Exception){issue="JSON格式不正确，请检查括号、引号和逗号。"}}){Text(stringResource(R.string.rovai_fusion_a3030bf8f16dc63c))}}}
            }}
        }
    }
    confirmation?.let{value->AlertDialog(onDismissRequest={if(!disabled)confirmation=null},title={Text(value.title)},text={Text(value.detail)},confirmButton={TextButton(enabled=!disabled,onClick={model.execute(value.operation,value.params);confirmation=null}){Text(stringResource(R.string.rovai_fusion_36f33adaf0942634))}},dismissButton={TextButton(enabled=!disabled,onClick={confirmation=null}){Text(stringResource(R.string.rovai_fusion_2cd0f3be8738a86c))}})}
    if(discard)AlertDialog(onDismissRequest={discard=false},title={Text(stringResource(R.string.rovai_fusion_164190f797b468f4))},confirmButton={TextButton(onClick={discard=false;choose(null)}){Text(stringResource(R.string.rovai_fusion_e7f51b070effdbe9))}},dismissButton={TextButton(onClick={discard=false}){Text(stringResource(R.string.rovai_fusion_fd4b9e3b6c685bae))}})
}
