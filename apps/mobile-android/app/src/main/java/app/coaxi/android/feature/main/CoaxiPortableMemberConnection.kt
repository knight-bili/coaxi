package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.coaxi.android.core.network.CoaxiOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import java.util.UUID

/** 原队员运行表单中的连接选择；读取实际程序与队员版本，只影响之后的新任务。 */
@Composable
fun CoaxiPortableMemberConnection(model:CoaxiWorkspaceViewModel,agentId:String,disabled:Boolean){
    val scope=rememberCoroutineScope();var binding by remember(agentId){mutableStateOf(buildJsonObject{})}
    var providers by remember(agentId){mutableStateOf<List<JsonObject>>(emptyList())};var next by remember(agentId){mutableStateOf<String?>(null)}
    var identity by remember(agentId){mutableStateOf("")};var choice by remember(agentId){mutableStateOf<JsonElement>(JsonNull)}
    var pending by remember(agentId){mutableStateOf<JsonObject?>(null)};var busy by remember(agentId){mutableStateOf(false)};var issue by remember(agentId){mutableStateOf<String?>(null)};var menu by remember{mutableStateOf(false)}
    val original=binding["binding"].coreObject()["connection"]?:JsonNull
    val adapter=binding["runtime"].coreObject().coreText("adapterKind")
    val kind=when(adapter){"claude-code-cli"->"claude_code";"codex-cli"->"codex";"gemini-cli"->"gemini";else->""}
    val unavailable=disabled||busy||identity.isEmpty()||binding["runtime"]==JsonNull||kind.isEmpty()
    fun owner()="portable-member/$agentId/$identity"
    suspend fun current(){if(identity.isEmpty()||model.presentationIdentity()!=identity)throw CancellationException("工作区已切换")}
    suspend fun remember(value:JsonObject?){current();model.saveBody(owner(),value?.toString()?:"");pending=value}
    suspend fun load(more:Boolean=false){
        current()
        if(!more){binding=model.query(CoaxiOperation.CONFIGURATION_MEMBERCONNECTION_GET,buildJsonObject{put("schemaVersion",1);put("agentId",agentId)}).coreObject();current();choice=binding["binding"].coreObject()["connection"]?:JsonNull}
        val page=model.query(CoaxiOperation.CONFIGURATION_PROVIDERS_LIST,buildJsonObject{put("schemaVersion",1);put("after",if(more)next?.let(::JsonPrimitive)?:JsonNull else JsonNull);put("limit",100)}).coreObject();current();providers=if(more)providers+page["items"].coreArray() else page["items"].coreArray();next=(page["next"] as? JsonPrimitive)?.contentOrNull
    }
    suspend fun accept(result:JsonObject){val original=pending?:error("缺少原请求");require(result.coreText("commandId")==original.coreText("commandId")&&result.coreText("commandType")=="configuration.memberConnection.set"){"原请求归属不同"};require(result.coreText("status") in listOf("applied","rejected")){"原请求尚待核对"};result["payload"].coreObject().coreText("agentId").takeIf{it.isNotEmpty()}?.let{require(it==agentId){"队员不同"}};val rejected=result.coreText("status")=="rejected";remember(null);if(rejected)error("设置被拒绝，请刷新后再选择");load();model.refresh()}
    fun work(action:suspend()->Unit){if(busy)return;busy=true;issue=null;scope.launch{try{action()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message?:"连接设置未完成"}finally{busy=false}}}
    LaunchedEffect(agentId){busy=true;try{identity=model.presentationIdentity();val raw=model.readBody(owner());if(raw.isNotBlank()){val value=Json.parseToJsonElement(raw).coreObject();require(value.keys==setOf("operation","commandId","targetId")&&value.coreText("operation")=="configuration.memberConnection.set"&&value.coreText("targetId")==agentId);UUID.fromString(value.coreText("commandId"));pending=value};load()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue="队员连接或原请求恢复失败"}finally{busy=false}}
    Column(verticalArrangement=Arrangement.spacedBy(8.dp)){
        Text(stringResource(R.string.coaxi_fusion_e0e9874451748a8d),style=MaterialTheme.typography.titleSmall)
        Text(stringResource(R.string.coaxi_fusion_c1ef3207a40d928c),style=MaterialTheme.typography.bodySmall)
        if(binding["runtime"]==JsonNull)Text(stringResource(R.string.coaxi_fusion_bd2be4a7549b7531),style=MaterialTheme.typography.bodySmall)
        Box{
            val selected=providers.find{it.coreText("id")==choice.coreObject().coreText("id")&&it["revision"]==choice.coreObject()["revision"]}
            OutlinedButton(onClick={menu=true},enabled=!unavailable&&pending==null){Text(if(choice is JsonNull)"使用程序原有配置" else selected?.coreText("name")?:"保留已选历史版本")}
            DropdownMenu(expanded=menu,onDismissRequest={menu=false}){
                DropdownMenuItem(text={Text(stringResource(R.string.coaxi_fusion_db17c81000e2049b))},onClick={choice=JsonNull;menu=false})
                if(original !is JsonNull)DropdownMenuItem(text={Text(stringResource(R.string.coaxi_fusion_284251c01f0d666d))},onClick={choice=original;menu=false})
                providers.filter{it.coreText("agentType")==kind&&it["enabled"]==JsonPrimitive(true)}.forEach{provider->DropdownMenuItem(text={Text(provider.coreText("name"))},onClick={choice=buildJsonObject{put("id",provider["id"]!!);put("revision",provider["revision"]!!);put("digest",provider["digest"]!!)};menu=false})}
            }
        }
        Row{
            TextButton(onClick={work{load()}},enabled=!unavailable&&pending==null){Text(stringResource(R.string.coaxi_fusion_aee88743413144a2))}
            if(next!=null)TextButton(onClick={work{load(true)}},enabled=!unavailable&&pending==null){Text(stringResource(R.string.coaxi_fusion_d91f4b6fa02fd05c))}
            TextButton(enabled=!unavailable&&pending==null&&choice!=original,onClick={work{current();val command=UUID.randomUUID().toString();remember(buildJsonObject{put("operation","configuration.memberConnection.set");put("commandId",command);put("targetId",agentId)});val result=model.query(CoaxiOperation.CONFIGURATION_MEMBERCONNECTION_SET,buildJsonObject{put("schemaVersion",1);put("commandId",command);put("agentId",agentId);put("expectedVersion",binding["agentVersion"]?:JsonNull);put("connection",choice)}).coreObject();current();accept(result)}}){Text(stringResource(R.string.coaxi_fusion_f5dede740ee08b61))}
        }
        if(pending!=null){Text(stringResource(R.string.coaxi_fusion_4a94d16197812038));TextButton(enabled=!busy,onClick={work{val original=pending?:return@work;val value=model.query(CoaxiOperation.COMMANDS_RECONCILE,buildJsonObject{put("operation","configuration.memberConnection.set");put("params",buildJsonObject{put("commandId",original["commandId"]!!)})}).coreObject();current();if(value.coreText("state")!="recorded")error("原请求尚无确定回执");accept(value["result"].coreObject())}}){Text(stringResource(R.string.coaxi_fusion_d027c9aa232e6375))}}
        if(disabled)Text(stringResource(R.string.coaxi_fusion_ad10aebe4e5fee5b),style=MaterialTheme.typography.bodySmall)
        if(issue!=null)Text(issue!!,color=MaterialTheme.colorScheme.error)
        if(busy)LinearProgressIndicator(Modifier.fillMaxWidth())
    }
}
