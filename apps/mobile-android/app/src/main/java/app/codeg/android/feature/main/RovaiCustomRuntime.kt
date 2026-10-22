package app.codeg.android.feature.main

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
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import java.util.UUID

internal fun customRuntimeRef(value:JsonObject)=buildJsonObject{put("id",value.coreText("id"));put("definitionRevision",value.coreLong("revision"));put("definitionDigest",value.coreText("definitionDigest"))}
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
private fun customRuntimeMasked(value:JsonObject):JsonObject=buildJsonObject{value.forEach{(key,entry)->put(key,if(key!="spec")entry else buildJsonObject{entry.coreObject().forEach{(channel,spec)->
    fun masked(item:JsonElement)=if(item !is JsonObject) item else buildJsonObject{item.forEach{(name,field)->put(name,if(name!="env")field else buildJsonObject{field.coreObject().forEach{(variable,secret)->put(variable,if(secret is JsonPrimitive&&secret.isString)JsonPrimitive("<本次新输入，保存后仅保留秘密引用>")else secret)}})}}
    put(channel,if(channel=="binary")buildJsonObject{spec.coreObject().forEach{(platform,item)->put(platform,masked(item))}}else masked(spec))}})}}

/** 原 AgentsContent 的单列表和 GlassRow；业务只读取连接的 Rovai Core。 */
@Composable fun RovaiCustomRuntimeSettings(model:RovaiWorkspaceViewModel,onClose:()->Unit){
    val coroutine=rememberCoroutineScope();val state=model.state.collectAsState().value;val generation=remember{model.connectionGeneration};val owner="custom-runtime-commands"
    var rows by remember{mutableStateOf<List<JsonObject>>(emptyList())};var next by remember{mutableStateOf<String?>(null)};var search by remember{mutableStateOf("")}
    var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)};var original by remember{mutableStateOf<JsonObject?>(null)}
    var editing by remember{mutableStateOf<JsonObject?>(null)};var removing by remember{mutableStateOf<JsonObject?>(null)};var inspection by remember{mutableStateOf<JsonObject?>(null)};var receipt by remember{mutableStateOf<JsonObject?>(null)};var registry by remember{mutableStateOf(false)}
    val disabled=busy||state.busy||state.pending!=null||generation!=model.connectionGeneration
    suspend fun call(operation:RovaiOperation,input:JsonObject=buildJsonObject{}):JsonObject{require(generation==model.connectionGeneration){"工作区已切换，请重新打开"};val result=model.query(operation,input).coreObject();require(generation==model.connectionGeneration){"工作区已切换，保留原记录"};return result}
    suspend fun load(more:Boolean=false){val page=call(RovaiOperation.RUNTIME_CUSTOM_LIST,buildJsonObject{put("after",if(more)next?.let(::JsonPrimitive)?:JsonNull else JsonNull);put("limit",100)});rows=if(more)(rows+page["items"].coreArray()).distinctBy{it.coreText("id")}else page["items"].coreArray();next=(page["next"] as? JsonPrimitive)?.contentOrNull}
    fun work(block:suspend()->Unit){if(disabled)return;busy=true;coroutine.launch{try{block();issue=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(generation==model.connectionGeneration)issue=error.message}finally{busy=false}}}
    suspend fun accept(pending:JsonObject,result:JsonObject){receipt=result
        if(pending.coreText("operation")==RovaiOperation.RUNTIME_CUSTOM_INSTALL.wire){if(result.coreText("state") in setOf("running","unknown"))return;require(result.coreText("state") in setOf("completed","failed")){"安装结果尚未确定"}}
        else require(result.coreText("status") in setOf("applied","rejected")){"保存结果尚未确定"}
        model.clearBodyIfMatches(owner,pending.toString(),generation);original=null
        if(result.coreText("state")=="failed")error(result.coreText("error").ifEmpty{"安装失败"})
        if(result.coreText("status")=="rejected")error(result.coreText("code"))
        if(pending.coreText("operation")==RovaiOperation.RUNTIME_CUSTOM_UPSERT.wire)editing=null
        removing=null;load();model.refreshNow()
    }
    suspend fun write(operation:RovaiOperation,id:String,payload:JsonObject){require(original==null){"请先核对原请求"};val commandId=UUID.randomUUID().toString();val pending=buildJsonObject{put("commandId",commandId);put("definitionId",id);put("operation",operation.wire)}
        model.saveBody(owner,pending.toString());original=pending;val result=call(operation,buildJsonObject{payload.forEach{(key,value)->put(key,value)};put("commandId",commandId)});accept(pending,result)}
    suspend fun reconcile(){val pending=original?:return;val result=call(RovaiOperation.COMMANDS_RECONCILE,buildJsonObject{put("operation",pending.coreText("operation"));put("params",buildJsonObject{put("commandId",pending.coreText("commandId"))})});require(result.coreText("state")=="recorded"){"原请求尚无确定回执，未重复发送"};val value=customObject(result["result"]);val id=if(pending.coreText("operation")==RovaiOperation.RUNTIME_CUSTOM_INSTALL.wire)value["reference"].coreObject().coreText("id")else value["payload"].coreObject().coreText("id");require(id.isEmpty()||id==pending.coreText("definitionId")){"原回执归属不同，请保留编号核对"};accept(pending,value)}
    LaunchedEffect(Unit){busy=true;try{val raw=model.readBody(owner);if(raw.isNotEmpty())original=customObject(Json.parseToJsonElement(raw));load()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    Dialog(onDismissRequest=onClose,properties=DialogProperties(usePlatformDefaultWidth=false)){Surface(Modifier.fillMaxSize().imePadding(),color=CodegTheme.colors.bg){LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
        item{Row{TextButton(onClick=onClose){Text("返回")};TextButton(enabled=!disabled,onClick={work{load()}}){Text("刷新列表")}};Text("自定义智能体",style=MaterialTheme.typography.titleLarge);Text("沿用原版分发配置。保存后再明确安装；已有任务保留原版本。",color=CodegTheme.colors.textSecondary)}
        item{Row{TextButton(enabled=!disabled&&original==null,onClick={registry=true}){Text("从注册表添加")};TextButton(enabled=!disabled&&original==null,onClick={editing=customRuntimeBlank()}){Text("手动添加")}};OutlinedTextField(search,{search=it},label={Text("搜索名称或标识")},modifier=Modifier.fillMaxWidth())}
        original?.let{pending->item{GlassRow(Modifier.fillMaxWidth()){Text("原请求待核对");Text(pending.coreText("commandId"));Text("只保存编号和程序归属，环境明文仅在当前表单中。",style=MaterialTheme.typography.bodySmall);TextButton(enabled=!disabled,onClick={work{reconcile()}}){Text("读取原请求回执")}}}}
        issue?.let{item{Text(it,color=CodegTheme.colors.danger)}}
        items(rows.filter{listOf(it.coreText("name"),it.coreText("registryId"),it.coreText("description")).any{field->field.contains(search,true)}},key={it.coreText("id")}){definition->GlassRow(Modifier.fillMaxWidth()){
            Text(definition.coreText("name"),style=MaterialTheme.typography.titleMedium);Text("${definition.coreText("registryId")} · ${definition.coreText("version")} · r${definition.coreLong("revision")}",color=CodegTheme.colors.textSecondary);Text(definition.coreText("description"));Text(if(definition["enabled"]==JsonPrimitive(false))"已停用" else "尚未实测运行",style=MaterialTheme.typography.bodySmall)
            Row{TextButton(enabled=!disabled,onClick={work{inspection=call(RovaiOperation.RUNTIME_CUSTOM_INSPECT,buildJsonObject{put("reference",customRuntimeRef(definition))})}}){Text("状态")};TextButton(enabled=!disabled&&original==null,onClick={editing=definition}){Text("编辑")}}
            Row{TextButton(enabled=!disabled&&original==null&&definition["enabled"]==JsonPrimitive(true),onClick={work{val info=call(RovaiOperation.RUNTIME_CUSTOM_INSPECT,buildJsonObject{put("reference",customRuntimeRef(definition))});write(RovaiOperation.RUNTIME_CUSTOM_INSTALL,definition.coreText("id"),buildJsonObject{put("reference",customRuntimeRef(definition));put("platform",info.coreText("platform"))})}}){Text("安装此版本")};TextButton(enabled=!disabled&&original==null,onClick={removing=definition}){Text("移除")}}
        }}
        if(!busy&&rows.isEmpty())item{Text("还没有自定义智能体。添加已有 ACP 程序后，可在队员设置中选择。")}
        if(next!=null)item{TextButton(enabled=!disabled,onClick={work{load(true)}}){Text("更多程序")}}
        inspection?.let{item{Text("安装与启动来源");Text(it.toString(),style=MaterialTheme.typography.bodySmall);TextButton(onClick={inspection=null}){Text("收起")}}}
        receipt?.let{item{Text("最近原请求回执");Text(it.toString(),style=MaterialTheme.typography.bodySmall)}}
    }}}
    removing?.let{value->AlertDialog(onDismissRequest={if(!busy)removing=null},title={Text("移除 ${value.coreText("name")}")},text={Text("之后的任务不能再选择此定义，原任务仍保留原安装、记录和秘密引用。")},confirmButton={TextButton(enabled=!disabled&&original==null,onClick={work{write(RovaiOperation.RUNTIME_CUSTOM_REMOVE,value.coreText("id"),buildJsonObject{put("id",value.coreText("id"));put("expectedRevision",value.coreLong("revision"))})}}){Text("移除")}},dismissButton={TextButton(enabled=!busy,onClick={removing=null}){Text("取消")}})}
    editing?.let{value->RovaiCustomRuntimeEditor(value,disabled,original!=null,issue,onClose={editing=null},onReconcile={work{reconcile()}},onSave={definition->work{write(RovaiOperation.RUNTIME_CUSTOM_UPSERT,definition.coreText("id"),buildJsonObject{put("expectedRevision",value.coreLong("revision"));put("definition",definition)})}})}
    if(registry)RovaiCustomRuntimeRegistry(model,onClose={registry=false},onChoose={registry=false;editing=it})
}

/** 原 Codeg 添加程序的字段分组；敏感的新环境值不写入本机草稿。 */
@Composable private fun RovaiCustomRuntimeEditor(value:JsonObject,disabled:Boolean,pending:Boolean,issue:String?,onClose:()->Unit,onReconcile:()->Unit,onSave:(JsonObject)->Unit){
    var name by remember(value){mutableStateOf(value.coreText("name"))};var registryId by remember(value){mutableStateOf(value.coreText("registryId"))};var description by remember(value){mutableStateOf(value.coreText("description"))};var version by remember(value){mutableStateOf(value.coreText("version"))}
    var kind by remember(value){mutableStateOf(value.coreText("distributionKind"))};var distribution by remember(value){mutableStateOf(value["spec"].toString())};var icon by remember(value){mutableStateOf(value.coreText("iconUrl"))};var skillsDir by remember(value){mutableStateOf(value.coreText("skillsDir"))};var versionProbe by remember(value){mutableStateOf(value.coreText("versionProbe"))}
    var shared by remember(value){mutableStateOf(value["skillsSharedStore"]==JsonPrimitive(true))};var mcp by remember(value){mutableStateOf(value["supportsMcp"]==JsonPrimitive(true))};var enabled by remember(value){mutableStateOf(value["enabled"]==JsonPrimitive(true))};var preview by remember{mutableStateOf<JsonObject?>(null)};var error by remember{mutableStateOf<String?>(null)};var discard by remember{mutableStateOf(false)}
    val inputEnabled=!disabled&&!pending
    fun definition()=buildJsonObject{value.filterKeys{it !in setOf("revision","definitionDigest","createdAt","updatedAt")}.forEach{(key,item)->put(key,item)};put("name",name);put("registryId",registryId);put("description",description);put("version",version);put("distributionKind",kind);put("spec",customRuntimeSpec(distribution,kind));put("iconUrl",icon.takeIf{it.isNotBlank()}?.let(::JsonPrimitive)?:JsonNull);put("skillsDir",skillsDir.takeIf{it.isNotBlank()}?.let(::JsonPrimitive)?:JsonNull);put("versionProbe",versionProbe.takeIf{it.isNotBlank()}?.let(::JsonPrimitive)?:JsonNull);put("skillsSharedStore",shared);put("supportsMcp",mcp);put("enabled",enabled)}
    Dialog(onDismissRequest={discard=true},properties=DialogProperties(usePlatformDefaultWidth=false,dismissOnClickOutside=false)){Surface(Modifier.fillMaxSize().imePadding(),color=CodegTheme.colors.bg){LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
        item{Text(if(value.coreLong("revision")>0)"编辑自定义智能体" else "添加自定义智能体",style=MaterialTheme.typography.titleLarge);TextButton(enabled=!disabled,onClick={discard=true}){Text("取消")}}
        item{OutlinedTextField(name,{name=it;preview=null},label={Text("名称")},enabled=inputEnabled,modifier=Modifier.fillMaxWidth());OutlinedTextField(registryId,{registryId=it;preview=null},label={Text("注册标识")},enabled=inputEnabled,modifier=Modifier.fillMaxWidth());OutlinedTextField(description,{description=it;preview=null},label={Text("描述")},enabled=inputEnabled,modifier=Modifier.fillMaxWidth());OutlinedTextField(version,{version=it;preview=null},label={Text("版本")},enabled=inputEnabled,modifier=Modifier.fillMaxWidth())}
        item{Text("分发方式");Row{listOf("npx","uvx","binary").forEach{channel->FilterChip(selected=kind==channel,onClick={kind=channel;preview=null},enabled=inputEnabled,label={Text(channel)})}};OutlinedTextField(distribution,{distribution=it;preview=null},label={Text("原版 distribution 配置 JSON")},enabled=inputEnabled,minLines=6,modifier=Modifier.fillMaxWidth());Text("环境新值仅保留在当前页面；保存后回显秘密引用。",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}
        item{OutlinedTextField(icon,{icon=it;preview=null},label={Text("图标 URL 或内嵌图片")},enabled=inputEnabled,modifier=Modifier.fillMaxWidth());Row{Text("声明支持 MCP",Modifier.weight(1f));Switch(mcp,{mcp=it;preview=null},enabled=inputEnabled)};Row{Text("共享技能目录",Modifier.weight(1f));Switch(shared,{shared=it;preview=null},enabled=inputEnabled)};OutlinedTextField(skillsDir,{skillsDir=it;preview=null},label={Text("技能绝对目录（可选）")},enabled=inputEnabled,modifier=Modifier.fillMaxWidth());OutlinedTextField(versionProbe,{versionProbe=it;preview=null},label={Text("版本检查命令（可选）")},enabled=inputEnabled,modifier=Modifier.fillMaxWidth());Row{Text("允许新任务使用",Modifier.weight(1f));Switch(enabled,{enabled=it;preview=null},enabled=inputEnabled)}}
        error?.let{item{Text(it,color=CodegTheme.colors.danger)}};issue?.let{item{Text(it,color=CodegTheme.colors.danger)}}
        item{TextButton(enabled=inputEnabled,onClick={try{preview=customRuntimeMasked(definition());error=null}catch(problem:Exception){error=problem.message}}){Text("核对完整配置")}}
        preview?.let{item{Text(it.toString(),style=MaterialTheme.typography.bodySmall);TextButton(enabled=inputEnabled,onClick={try{onSave(definition())}catch(problem:Exception){error=problem.message}}){Text("保存配置")}}}
        if(pending)item{Text("原请求待核对；不重复保存。 ");TextButton(enabled=!disabled,onClick=onReconcile){Text("读取原请求回执")}}
    }}}
    if(discard)AlertDialog(onDismissRequest={discard=false},title={Text("离开当前表单？")},text={Text("当前输入未保存；新环境明文不会写入草稿。已发送的请求仍保留原编号供核对。")},confirmButton={TextButton(onClick=onClose){Text("离开")}},dismissButton={TextButton(onClick={discard=false}){Text("继续编辑")}})
}

/** 原版注册表搜索：带入表单不会安装或启动程序。 */
@Composable private fun RovaiCustomRuntimeRegistry(model:RovaiWorkspaceViewModel,onClose:()->Unit,onChoose:(JsonObject)->Unit){
    val coroutine=rememberCoroutineScope();val generation=remember{model.connectionGeneration};var catalog by remember{mutableStateOf<JsonObject?>(null)};var search by remember{mutableStateOf("")};var limit by remember{mutableStateOf(100)};var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)}
    suspend fun load(refresh:Boolean){busy=true;try{val value=model.query(RovaiOperation.RUNTIME_CUSTOM_REGISTRY,buildJsonObject{put("refresh",refresh)}).coreObject();require(generation==model.connectionGeneration);catalog=value;issue=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    LaunchedEffect(Unit){load(false)}
    val entries=catalog?.get("entries").coreArray().filter{listOf(it.coreText("name"),it.coreText("id"),it.coreText("description")).any{field->field.contains(search,true)}}
    val registered=(catalog?.get("registeredIds") as? JsonArray)?.mapNotNull{(it as? JsonPrimitive)?.contentOrNull}.orEmpty()
    Dialog(onDismissRequest=onClose,properties=DialogProperties(usePlatformDefaultWidth=false)){Surface(Modifier.fillMaxSize().imePadding(),color=CodegTheme.colors.bg){LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
        item{Text("从 ACP 注册表添加",style=MaterialTheme.typography.titleLarge);Row{TextButton(onClick=onClose){Text("返回")};TextButton(enabled=!busy&&generation==model.connectionGeneration,onClick={coroutine.launch{load(true)}}){Text("刷新来源")}};OutlinedTextField(search,{search=it;limit=100},label={Text("搜索注册表")},modifier=Modifier.fillMaxWidth());Text(catalog?.coreText("registryUrl").orEmpty(),style=MaterialTheme.typography.bodySmall);Text(catalog?.coreText("fetchedAt").orEmpty(),style=MaterialTheme.typography.bodySmall)}
        issue?.let{item{Text(it,color=CodegTheme.colors.danger)}}
        items(entries.take(limit),key={it.coreText("id")}){entry->GlassRow(Modifier.fillMaxWidth()){Text(entry.coreText("name"),style=MaterialTheme.typography.titleMedium);Text(entry.coreText("description"));Text(entry.coreText("id"),color=CodegTheme.colors.textSecondary);TextButton(enabled=!busy&&entry.coreText("id") !in registered&&generation==model.connectionGeneration,onClick={try{val source=entry["distribution"].coreObject();val kind=if(source["npx"] is JsonObject)"npx" else if(source["uvx"] is JsonObject)"uvx" else "binary";val spec=customRuntimeSpec(entry.toString(),kind);onChoose(buildJsonObject{customRuntimeBlank().forEach{(key,item)->put(key,item)};put("registryId",entry.coreText("id"));put("name",entry.coreText("name"));put("description",entry.coreText("description"));put("version",entry.coreText("version").ifEmpty{"1.0.0"});put("iconUrl",entry["icon"]?:JsonNull);put("distributionKind",kind);put("spec",spec);put("source","registry");put("sourceRef",catalog!!.coreText("registryUrl")+"#"+java.net.URLEncoder.encode(entry.coreText("id"),"UTF-8"))})}catch(error:Exception){issue=error.message}}){Text(if(entry.coreText("id") in registered)"已添加" else "带入表单")}}}
        if(entries.size>limit)item{TextButton(onClick={limit+=100}){Text("更多程序")}}
        if(!busy&&catalog!=null&&entries.isEmpty())item{Text("没有符合搜索条件的程序")}
    }}}
}
