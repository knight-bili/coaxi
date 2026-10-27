package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import app.coaxi.android.core.designsystem.component.GlassRow
import app.coaxi.android.core.designsystem.theme.CoaxiTheme
import app.coaxi.android.core.network.CoaxiOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import java.util.UUID

internal fun customRuntimeRef(value:JsonObject)=buildJsonObject{put("id",value.coreText("id"));put("definitionRevision",value.coreLong("revision"));put("definitionDigest",value.coreText("definitionDigest"))}
private fun customRuntimeEnvironmentText(value:JsonObject):String{
    val environment=(value["installation"] as? JsonObject)?.get("environment") as? JsonObject
    return when(environment?.coreText("status")){
        "needs_environment"->"本机程序或关联文件需要修复。保留旧任务，在本机重新安装此版本，再为之后的任务选择本机安装。"
        "unreadable"->"暂时无法核对恢复记录，请先检查主机数据目录，原任务和安装记录已保留。"
        "artifacts_current"->"本机程序文件与安装记录一致，运行资格仍待实际验证。"
        "not_installed"->"当前主机尚未安装此版本，可选择安装此版本。"
        else->"当前主机没有提供恢复环境检查结果。"
    }
}
private fun customObject(value:JsonElement?):JsonObject=value as? JsonObject?:error("配置必须是 JSON 对象")
private fun customKeys(value:JsonObject,names:Set<String>){require(value.keys.all{it in names}){"配置含有未知字段，请核对原版格式"}}
private fun customPackage(raw:JsonObject,kind:String):JsonObject{
    customKeys(raw,setOf("package","args","env","cmd")+if(kind=="npx")setOf("nodeRequired","node_required")else setOf("uvRequired","uv_required","python"))
    require(!(("nodeRequired" in raw&&"node_required" in raw)||("uvRequired" in raw&&"uv_required" in raw))){"版本要求不能重复填写"}
    return buildJsonObject{put("package",raw["package"]?:JsonNull);put("args",raw["args"]?:JsonArray(emptyList()));put("env",raw["env"]?:buildJsonObject{});put("cmd",raw["cmd"]?:JsonNull)
        if(kind=="npx")put("nodeRequired",raw["nodeRequired"]?:raw["node_required"]?:JsonNull)
        else{put("uvRequired",raw["uvRequired"]?:raw["uv_required"]?:JsonNull);put("python",raw["python"]?:JsonNull)}}
}
/** 原版手动表单接受 distribution 或整条注册表记录；保留全部渠道，启动只选指定渠道。 */
internal fun customRuntimeSpec(text:String,kind:String):JsonObject{
    require(text.toByteArray(Charsets.UTF_8).size<=2*1024*1024){"配置超过2MB"}
    val raw=customObject(Json.parseToJsonElement(text));if("distribution" in raw)customKeys(raw,setOf("id","name","description","version","website","repository","license","icon","distribution"))
    val channels=customObject(raw["distribution"]?:raw);customKeys(channels,setOf("npx","uvx","binary"))
    require(kind in setOf("npx","uvx","binary")){"请选择分发方式"}
    require(channels[kind] is JsonObject&&(kind!="binary"||channels[kind].coreObject().isNotEmpty())){"所选分发方式不在这份配置中"}
    return buildJsonObject{put("npx",if(channels["npx"] is JsonObject)customPackage(customObject(channels["npx"]),"npx")else JsonNull);put("uvx",if(channels["uvx"] is JsonObject)customPackage(customObject(channels["uvx"]),"uvx")else JsonNull)
        put("binary",if(channels["binary"]==null)buildJsonObject{} else buildJsonObject{val binaries=customObject(channels["binary"]);binaries.forEach{(platform,value)->val item=customObject(value);customKeys(item,setOf("archive","url","cmd","args","env","sha256"));require(!("archive" in item&&"url" in item)){"下载地址不能重复填写"};put(platform,buildJsonObject{put("archive",item["archive"]?:item["url"]?:JsonNull);put("cmd",item["cmd"]?:JsonPrimitive(""));put("args",item["args"]?:JsonArray(emptyList()));put("env",item["env"]?:buildJsonObject{});put("sha256",item["sha256"]?:JsonNull)})}})}
}
internal fun customRuntimeBlank()=buildJsonObject{
    put("schemaVersion",1);put("id",UUID.randomUUID().toString());put("registryId","");put("name","");put("description","");put("version","1.0.0");put("distributionKind","npx")
    put("spec",buildJsonObject{put("npx",buildJsonObject{put("package","");put("args",JsonArray(listOf(JsonPrimitive("--acp"))));put("env",buildJsonObject{});put("cmd",JsonNull);put("nodeRequired",JsonNull)});put("uvx",JsonNull);put("binary",buildJsonObject{})})
    put("iconUrl",JsonNull);put("skillsSharedStore",false);put("skillsDir",JsonNull);put("source","manual");put("sourceRef",JsonNull);put("versionProbe",JsonNull);put("supportsMcp",true);put("enabled",true)
}
private fun customPreferredDistribution(entry:JsonObject,platform:String):String{
    val aliases=when(platform){"windows-x64"->listOf("windows-x86_64","x86_64-pc-windows-msvc");"windows-arm64"->listOf("windows-aarch64","aarch64-pc-windows-msvc");"macos-x64"->listOf("darwin-x86_64","x86_64-apple-darwin");"macos-arm64"->listOf("darwin-aarch64","aarch64-apple-darwin");"linux-x64"->listOf("linux-x86_64","x86_64-unknown-linux-gnu");"linux-arm64"->listOf("linux-aarch64","aarch64-unknown-linux-gnu");else->emptyList()}
    val source=entry["distribution"].coreObject();if(aliases.any{source["binary"].coreObject()[it] is JsonObject})return "binary";if(source["npx"] is JsonObject)return "npx";if(source["uvx"] is JsonObject)return "uvx";require(source["binary"].coreObject().isNotEmpty()){"这条记录没有可用的分发方式"};return "binary"
}
private fun customRuntimeMasked(value:JsonObject):JsonObject=buildJsonObject{value.forEach{(key,entry)->put(key,if(key!="spec")entry else buildJsonObject{entry.coreObject().forEach{(channel,spec)->
    fun masked(item:JsonElement)=if(item !is JsonObject) item else buildJsonObject{item.forEach{(name,field)->put(name,if(name!="env")field else buildJsonObject{field.coreObject().forEach{(variable,secret)->put(variable,if(secret is JsonPrimitive&&secret.isString)JsonPrimitive("<本次新输入，保存后仅保留秘密引用>")else secret)}})}}
    put(channel,if(channel=="binary")buildJsonObject{spec.coreObject().forEach{(platform,item)->put(platform,masked(item))}}else masked(spec))}})}}

/** 原 AgentsContent 的单列表和 GlassRow；业务只读取连接的 Coaxi Core。 */
@Composable fun CoaxiCustomRuntimeSettings(model:CoaxiWorkspaceViewModel,onClose:()->Unit){
    val coroutine=rememberCoroutineScope();val state=model.state.collectAsState().value;val generation=remember{model.connectionGeneration};val owner="custom-runtime-commands"
    var rows by remember{mutableStateOf<List<JsonObject>>(emptyList())};var next by remember{mutableStateOf<String?>(null)};var search by remember{mutableStateOf("")}
    var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)};var original by remember{mutableStateOf<JsonObject?>(null)}
    var editing by remember{mutableStateOf<JsonObject?>(null)};var removing by remember{mutableStateOf<JsonObject?>(null)};var inspection by remember{mutableStateOf<JsonObject?>(null)};var receipt by remember{mutableStateOf<JsonObject?>(null)};var registry by remember{mutableStateOf(false)}
    val disabled=busy||state.busy||state.pending!=null||generation!=model.connectionGeneration
    suspend fun call(operation:CoaxiOperation,input:JsonObject=buildJsonObject{}):JsonObject{require(generation==model.connectionGeneration){"工作区已切换，请重新打开"};val result=model.query(operation,input).coreObject();require(generation==model.connectionGeneration){"工作区已切换，保留原记录"};return result}
    suspend fun load(more:Boolean=false){val page=call(CoaxiOperation.RUNTIME_CUSTOM_LIST,buildJsonObject{put("after",if(more)next?.let(::JsonPrimitive)?:JsonNull else JsonNull);put("limit",100)});rows=if(more)(rows+page["items"].coreArray()).distinctBy{it.coreText("id")}else page["items"].coreArray();next=(page["next"] as? JsonPrimitive)?.contentOrNull}
    fun work(block:suspend()->Unit){if(disabled)return;busy=true;coroutine.launch{try{block();issue=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(generation==model.connectionGeneration)issue=error.message}finally{busy=false}}}
    suspend fun accept(pending:JsonObject,result:JsonObject){receipt=result
        if(pending.coreText("operation")==CoaxiOperation.RUNTIME_CUSTOM_INSTALL.wire){if(result.coreText("state") in setOf("running","unknown"))return;require(result.coreText("state") in setOf("completed","failed")){"安装结果尚未确定"}}
        else require(result.coreText("status") in setOf("applied","rejected")){"保存结果尚未确定"}
        model.clearBodyIfMatches(owner,pending.toString(),generation);original=null
        if(result.coreText("state")=="failed")error(result.coreText("error").ifEmpty{"安装失败"})
        if(result.coreText("status")=="rejected")error(result.coreText("code"))
        if(pending.coreText("operation")==CoaxiOperation.RUNTIME_CUSTOM_UPSERT.wire)editing=null
        removing=null;load();model.refreshNow()
    }
    suspend fun write(operation:CoaxiOperation,id:String,payload:JsonObject){require(original==null){"请先核对原请求"};val commandId=UUID.randomUUID().toString();val pending=buildJsonObject{put("commandId",commandId);put("definitionId",id);put("operation",operation.wire)}
        model.saveBody(owner,pending.toString());original=pending;val result=call(operation,buildJsonObject{payload.forEach{(key,value)->put(key,value)};put("commandId",commandId)});accept(pending,result)}
    suspend fun reconcile(){val pending=original?:return;val result=call(CoaxiOperation.COMMANDS_RECONCILE,buildJsonObject{put("operation",pending.coreText("operation"));put("params",buildJsonObject{put("commandId",pending.coreText("commandId"))})});require(result.coreText("state")=="recorded"){"原请求尚无确定回执，未重复发送"};val value=customObject(result["result"]);val id=if(pending.coreText("operation")==CoaxiOperation.RUNTIME_CUSTOM_INSTALL.wire)value["reference"].coreObject().coreText("id")else value["payload"].coreObject().coreText("id");require(id.isEmpty()||id==pending.coreText("definitionId")){"原回执归属不同，请保留编号核对"};accept(pending,value)}
    LaunchedEffect(Unit){busy=true;try{val raw=model.readBody(owner);if(raw.isNotEmpty())original=customObject(Json.parseToJsonElement(raw));load()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    Dialog(onDismissRequest=onClose,properties=DialogProperties(usePlatformDefaultWidth=false)){Surface(Modifier.fillMaxSize().imePadding(),color=CoaxiTheme.colors.bg){LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
        item{Row{TextButton(onClick=onClose){Text(stringResource(R.string.coaxi_fusion_572cf45ba43634b3))};TextButton(enabled=!disabled,onClick={work{load()}}){Text(stringResource(R.string.coaxi_fusion_e28be7098db113cb))}};Text(stringResource(R.string.coaxi_fusion_e5c2d4add8ccd108),style=MaterialTheme.typography.titleLarge);Text(stringResource(R.string.coaxi_fusion_fef09e916e39c4eb),color=CoaxiTheme.colors.textSecondary)}
        item{Row{TextButton(enabled=!disabled&&original==null,onClick={registry=true}){Text(stringResource(R.string.coaxi_fusion_7189b3ec85e3f4b0))};TextButton(enabled=!disabled&&original==null,onClick={editing=customRuntimeBlank()}){Text(stringResource(R.string.coaxi_fusion_ddfe2c796c7eb71b))}};OutlinedTextField(search,{search=it},label={Text(stringResource(R.string.coaxi_fusion_b7e405d999a62df5))},modifier=Modifier.fillMaxWidth())}
        original?.let{pending->item{GlassRow(Modifier.fillMaxWidth()){Text(stringResource(R.string.coaxi_fusion_9b7a11fad050210d));Text(pending.coreText("commandId"));Text(stringResource(R.string.coaxi_fusion_95d3e5056a6c904c),style=MaterialTheme.typography.bodySmall);TextButton(enabled=!disabled,onClick={work{reconcile()}}){Text(stringResource(R.string.coaxi_fusion_962e813f935aee23))}}}}
        issue?.let{item{Text(it,color=CoaxiTheme.colors.danger)}}
        items(rows.filter{listOf(it.coreText("name"),it.coreText("registryId"),it.coreText("description")).any{field->field.contains(search,true)}},key={it.coreText("id")}){definition->GlassRow(Modifier.fillMaxWidth()){
            Text(definition.coreText("name"),style=MaterialTheme.typography.titleMedium);Text("${definition.coreText("registryId")} · ${definition.coreText("version")} · r${definition.coreLong("revision")}",color=CoaxiTheme.colors.textSecondary);Text(definition.coreText("description"));Text(if(definition["enabled"]==JsonPrimitive(false))"已停用" else "尚未实测运行",style=MaterialTheme.typography.bodySmall)
            Row{TextButton(enabled=!disabled,onClick={work{inspection=call(CoaxiOperation.RUNTIME_CUSTOM_INSPECT,buildJsonObject{put("reference",customRuntimeRef(definition))})}}){Text(stringResource(R.string.coaxi_fusion_6320b4a8722a851f))};TextButton(enabled=!disabled&&original==null,onClick={editing=definition}){Text(stringResource(R.string.coaxi_fusion_051836569928a9f9))}}
            Row{TextButton(enabled=!disabled&&original==null&&definition["enabled"]==JsonPrimitive(true),onClick={work{val info=call(CoaxiOperation.RUNTIME_CUSTOM_INSPECT,buildJsonObject{put("reference",customRuntimeRef(definition))});write(CoaxiOperation.RUNTIME_CUSTOM_INSTALL,definition.coreText("id"),buildJsonObject{put("reference",customRuntimeRef(definition));put("platform",info.coreText("platform"))})}}){Text(stringResource(R.string.coaxi_fusion_627308143ea2a9c6))};TextButton(enabled=!disabled&&original==null,onClick={removing=definition}){Text(stringResource(R.string.coaxi_fusion_6135d4159e892541))}}
        }}
        if(!busy&&rows.isEmpty())item{Text(stringResource(R.string.coaxi_fusion_ce3dbc5c2dc9f50a))}
        if(next!=null)item{TextButton(enabled=!disabled,onClick={work{load(true)}}){Text(stringResource(R.string.coaxi_fusion_ac09edc833c947fe))}}
        inspection?.let{value->item{GlassRow(Modifier.fillMaxWidth()){Text(customRuntimeEnvironmentText(value));var expanded by remember(value){mutableStateOf(false)};TextButton(onClick={expanded=!expanded}){Text(if(expanded)"收起安装与启动来源" else "安装与启动来源")};if(expanded)Text(value.toString(),style=MaterialTheme.typography.bodySmall);TextButton(onClick={inspection=null}){Text(stringResource(R.string.coaxi_fusion_d4c9f9c3a1a8a4f5))}}}}
        receipt?.let{item{Text(stringResource(R.string.coaxi_fusion_5907f7cce3e5ba2c));Text(it.toString(),style=MaterialTheme.typography.bodySmall)}}
    }}}
    removing?.let{value->AlertDialog(onDismissRequest={if(!busy)removing=null},title={Text("移除 ${value.coreText("name")}")},text={Text(stringResource(R.string.coaxi_fusion_14ccfd4056f01454))},confirmButton={TextButton(enabled=!disabled&&original==null,onClick={work{write(CoaxiOperation.RUNTIME_CUSTOM_REMOVE,value.coreText("id"),buildJsonObject{put("id",value.coreText("id"));put("expectedRevision",value.coreLong("revision"))})}}){Text(stringResource(R.string.coaxi_fusion_6135d4159e892541))}},dismissButton={TextButton(enabled=!busy,onClick={removing=null}){Text(stringResource(R.string.coaxi_fusion_2cd0f3be8738a86c))}})}
    editing?.let{value->CoaxiCustomRuntimeEditor(value,disabled,original!=null,issue,onClose={editing=null},onReconcile={work{reconcile()}},onSave={definition->work{write(CoaxiOperation.RUNTIME_CUSTOM_UPSERT,definition.coreText("id"),buildJsonObject{put("expectedRevision",value.coreLong("revision"));put("definition",definition)})}})}
    if(registry)CoaxiCustomRuntimeRegistry(model,onClose={registry=false},onChoose={registry=false;editing=it})
}

/** 原 Coaxi 添加程序的字段分组；敏感的新环境值不写入本机草稿。 */
@Composable private fun CoaxiCustomRuntimeEditor(value:JsonObject,disabled:Boolean,pending:Boolean,issue:String?,onClose:()->Unit,onReconcile:()->Unit,onSave:(JsonObject)->Unit){
    var name by remember(value){mutableStateOf(value.coreText("name"))};var registryId by remember(value){mutableStateOf(value.coreText("registryId"))};var description by remember(value){mutableStateOf(value.coreText("description"))};var version by remember(value){mutableStateOf(value.coreText("version"))}
    var kind by remember(value){mutableStateOf(value.coreText("distributionKind"))};var distribution by remember(value){mutableStateOf(value["spec"].toString())};var icon by remember(value){mutableStateOf(value.coreText("iconUrl"))};var skillsDir by remember(value){mutableStateOf(value.coreText("skillsDir"))};var versionProbe by remember(value){mutableStateOf(value.coreText("versionProbe"))}
    var shared by remember(value){mutableStateOf(value["skillsSharedStore"]==JsonPrimitive(true))};var mcp by remember(value){mutableStateOf(value["supportsMcp"]==JsonPrimitive(true))};var enabled by remember(value){mutableStateOf(value["enabled"]==JsonPrimitive(true))};var preview by remember{mutableStateOf<JsonObject?>(null)};var error by remember{mutableStateOf<String?>(null)};var discard by remember{mutableStateOf(false)}
    val inputEnabled=!disabled&&!pending
    fun definition()=buildJsonObject{value.filterKeys{it !in setOf("revision","definitionDigest","createdAt","updatedAt")}.forEach{(key,item)->put(key,item)};put("name",name);put("registryId",registryId);put("description",description);put("version",version);put("distributionKind",kind);put("spec",customRuntimeSpec(distribution,kind));put("iconUrl",icon.takeIf{it.isNotBlank()}?.let(::JsonPrimitive)?:JsonNull);put("skillsDir",skillsDir.takeIf{it.isNotBlank()}?.let(::JsonPrimitive)?:JsonNull);put("versionProbe",versionProbe.takeIf{it.isNotBlank()}?.let(::JsonPrimitive)?:JsonNull);put("skillsSharedStore",shared);put("supportsMcp",mcp);put("enabled",enabled)}
    Dialog(onDismissRequest={discard=true},properties=DialogProperties(usePlatformDefaultWidth=false,dismissOnClickOutside=false)){Surface(Modifier.fillMaxSize().imePadding(),color=CoaxiTheme.colors.bg){LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
        item{Text(if(value.coreLong("revision")>0)"编辑自定义智能体" else "添加自定义智能体",style=MaterialTheme.typography.titleLarge);TextButton(enabled=!disabled,onClick={discard=true}){Text(stringResource(R.string.coaxi_fusion_2cd0f3be8738a86c))}}
        item{OutlinedTextField(name,{name=it;preview=null},label={Text(stringResource(R.string.coaxi_fusion_d44e9b3d3b31d37b))},enabled=inputEnabled,modifier=Modifier.fillMaxWidth());OutlinedTextField(registryId,{registryId=it;preview=null},label={Text(stringResource(R.string.coaxi_fusion_7ad84238e3e14068))},enabled=inputEnabled,modifier=Modifier.fillMaxWidth());OutlinedTextField(description,{description=it;preview=null},label={Text(stringResource(R.string.coaxi_fusion_dc2ba467fc7ac962))},enabled=inputEnabled,modifier=Modifier.fillMaxWidth());OutlinedTextField(version,{version=it;preview=null},label={Text(stringResource(R.string.coaxi_fusion_5f76b2bf82dd2c5d))},enabled=inputEnabled,modifier=Modifier.fillMaxWidth())}
        item{Text(stringResource(R.string.coaxi_fusion_3a802dcbcf2485de));Row{listOf("npx","uvx","binary").forEach{channel->FilterChip(selected=kind==channel,onClick={kind=channel;preview=null},enabled=inputEnabled,label={Text(channel)})}};OutlinedTextField(distribution,{distribution=it;preview=null},label={Text(stringResource(R.string.coaxi_fusion_1b935c5b97f5046e))},enabled=inputEnabled,minLines=6,modifier=Modifier.fillMaxWidth());Text(stringResource(R.string.coaxi_fusion_a7c388f5f69ff634),style=MaterialTheme.typography.bodySmall,color=CoaxiTheme.colors.textSecondary)}
        item{OutlinedTextField(icon,{icon=it;preview=null},label={Text(stringResource(R.string.coaxi_fusion_eca50b454e629814))},enabled=inputEnabled,modifier=Modifier.fillMaxWidth());Row{Text(stringResource(R.string.coaxi_fusion_e612ada52db168e4),Modifier.weight(1f));Switch(mcp,{mcp=it;preview=null},enabled=inputEnabled)};Row{Text(stringResource(R.string.coaxi_fusion_d972aa558510ee08),Modifier.weight(1f));Switch(shared,{shared=it;preview=null},enabled=inputEnabled)};OutlinedTextField(skillsDir,{skillsDir=it;preview=null},label={Text(stringResource(R.string.coaxi_fusion_72335f8e0c1c0a8f))},enabled=inputEnabled,modifier=Modifier.fillMaxWidth());OutlinedTextField(versionProbe,{versionProbe=it;preview=null},label={Text(stringResource(R.string.coaxi_fusion_844b904173195d57))},enabled=inputEnabled,modifier=Modifier.fillMaxWidth());Row{Text(stringResource(R.string.coaxi_fusion_80317564c9cbc1db),Modifier.weight(1f));Switch(enabled,{enabled=it;preview=null},enabled=inputEnabled)}}
        error?.let{item{Text(it,color=CoaxiTheme.colors.danger)}};issue?.let{item{Text(it,color=CoaxiTheme.colors.danger)}}
        item{TextButton(enabled=inputEnabled,onClick={try{preview=customRuntimeMasked(definition());error=null}catch(problem:Exception){error=problem.message}}){Text(stringResource(R.string.coaxi_fusion_337de94a5e93de1d))}}
        preview?.let{item{Text(it.toString(),style=MaterialTheme.typography.bodySmall);TextButton(enabled=inputEnabled,onClick={try{onSave(definition())}catch(problem:Exception){error=problem.message}}){Text(stringResource(R.string.coaxi_fusion_6e584e3d5ce64aa0))}}}
        if(pending)item{Text(stringResource(R.string.coaxi_fusion_4e84abe85daa28a2));TextButton(enabled=!disabled,onClick=onReconcile){Text(stringResource(R.string.coaxi_fusion_962e813f935aee23))}}
    }}}
    if(discard)AlertDialog(onDismissRequest={discard=false},title={Text(stringResource(R.string.coaxi_fusion_c1b218ff4e8e2041))},text={Text(stringResource(R.string.coaxi_fusion_4b089eea9554d1b4))},confirmButton={TextButton(onClick=onClose){Text(stringResource(R.string.coaxi_fusion_7345ed45ce936fff))}},dismissButton={TextButton(onClick={discard=false}){Text(stringResource(R.string.coaxi_fusion_fd4b9e3b6c685bae))}})
}

/** 原版注册表搜索：带入表单不会安装或启动程序。 */
@Composable private fun CoaxiCustomRuntimeRegistry(model:CoaxiWorkspaceViewModel,onClose:()->Unit,onChoose:(JsonObject)->Unit){
    val coroutine=rememberCoroutineScope();val generation=remember{model.connectionGeneration};var catalog by remember{mutableStateOf<JsonObject?>(null)};var search by remember{mutableStateOf("")};var limit by remember{mutableStateOf(100)};var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)}
    suspend fun load(refresh:Boolean){busy=true;try{val value=model.query(CoaxiOperation.RUNTIME_CUSTOM_REGISTRY,buildJsonObject{put("refresh",refresh)}).coreObject();require(generation==model.connectionGeneration);catalog=value;issue=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    LaunchedEffect(Unit){load(false)}
    val entries=catalog?.get("entries").coreArray().filter{listOf(it.coreText("name"),it.coreText("id"),it.coreText("description")).any{field->field.contains(search,true)}}
    val registered=(catalog?.get("registeredIds") as? JsonArray)?.mapNotNull{(it as? JsonPrimitive)?.contentOrNull}.orEmpty()
    Dialog(onDismissRequest=onClose,properties=DialogProperties(usePlatformDefaultWidth=false)){Surface(Modifier.fillMaxSize().imePadding(),color=CoaxiTheme.colors.bg){LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
        item{Text(stringResource(R.string.coaxi_fusion_5b7755d5afec5f92),style=MaterialTheme.typography.titleLarge);Row{TextButton(onClick=onClose){Text(stringResource(R.string.coaxi_fusion_572cf45ba43634b3))};TextButton(enabled=!busy&&generation==model.connectionGeneration,onClick={coroutine.launch{load(true)}}){Text(stringResource(R.string.coaxi_fusion_2b6272bb04b3a5a3))}};OutlinedTextField(search,{search=it;limit=100},label={Text(stringResource(R.string.coaxi_fusion_6c3f5055f189e63f))},modifier=Modifier.fillMaxWidth());Text(catalog?.coreText("registryUrl").orEmpty(),style=MaterialTheme.typography.bodySmall);Text(catalog?.coreText("fetchedAt").orEmpty(),style=MaterialTheme.typography.bodySmall)}
        issue?.let{item{Text(it,color=CoaxiTheme.colors.danger)}}
        items(entries.take(limit),key={it.coreText("id")}){entry->GlassRow(Modifier.fillMaxWidth()){Text(entry.coreText("name"),style=MaterialTheme.typography.titleMedium);Text(entry.coreText("description"));Text(entry.coreText("id"),color=CoaxiTheme.colors.textSecondary);TextButton(enabled=!busy&&entry.coreText("id") !in registered&&generation==model.connectionGeneration,onClick={try{val kind=customPreferredDistribution(entry,catalog!!.coreText("platform"));val spec=customRuntimeSpec(entry.toString(),kind);onChoose(buildJsonObject{customRuntimeBlank().forEach{(key,item)->put(key,item)};put("registryId",entry.coreText("id"));put("name",entry.coreText("name"));put("description",entry.coreText("description"));put("version",entry.coreText("version").ifEmpty{"1.0.0"});put("iconUrl",entry["icon"]?:JsonNull);put("distributionKind",kind);put("spec",spec);put("source","registry");put("sourceRef",catalog!!.coreText("registryUrl")+"#"+java.net.URLEncoder.encode(entry.coreText("id"),"UTF-8"))})}catch(error:Exception){issue=error.message}}){Text(if(entry.coreText("id") in registered)"已添加" else "带入表单")}}}
        if(entries.size>limit)item{TextButton(onClick={limit+=100}){Text(stringResource(R.string.coaxi_fusion_ac09edc833c947fe))}}
        if(!busy&&catalog!=null&&entries.isEmpty())item{Text(stringResource(R.string.coaxi_fusion_f8623b1653f6de21))}
    }}}
}
