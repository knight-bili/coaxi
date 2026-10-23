package app.codeg.android.feature.main

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import java.util.UUID

@Composable
fun RovaiRuntimeSettings(model:RovaiWorkspaceViewModel){
    val state by model.state.collectAsStateWithLifecycle();val coroutine=rememberCoroutineScope()
    var portable by remember{mutableStateOf(false)}
    var migration by remember{mutableStateOf(false)}
    var templates by remember{mutableStateOf(false)}
    if(templates){RovaiTaskTemplateSettings(model){templates=false};return}
    var logging by remember{mutableStateOf(false)}
    if(logging){RovaiHostLoggingSettings(model){logging=false};return}
    var export by remember{mutableStateOf(false)}
    if(export){RovaiPortableConfigurationExport(model){export=false};return}
    if(migration){RovaiPortableConfigurationMigration(model){migration=false};return}
    if(portable){RovaiPortableProviderSettings(model){portable=false};return}
    var installations by remember{mutableStateOf<List<JsonObject>>(emptyList())};var agent by remember{mutableStateOf<JsonObject?>(null)}
    var kind by remember{mutableStateOf("")};var selection by remember{mutableStateOf(buildJsonObject{put("mode","runtime_default")})}
    var permissions by remember{mutableStateOf(buildJsonObject{})};var catalog by remember{mutableStateOf(buildJsonObject{})}
    var advanced by remember{mutableStateOf<Map<String,String>>(emptyMap())};var baseline by remember{mutableStateOf("")};var submitted by remember{mutableStateOf<String?>(null)}
    var issue by remember{mutableStateOf<String?>(null)};var loading by remember{mutableStateOf(false)};var picker by remember{mutableStateOf(false)};var startup by remember{mutableStateOf(false)};var discard by remember{mutableStateOf(false)};var custom by remember{mutableStateOf(false)}
    val installation=installations.find{it.coreText("adapterKind")==kind&&it.coreText("installationClass")=="managed_default"&&it.coreText("authScope")=="default"}
    val snapshot=installation?.get("snapshot").coreObject();val defaults=installation?.get("memberRuntimeDefaults").coreObject()
    val disabled=loading||state.busy||state.pending!=null
    val dirty=agent!=null&&baseline!="${kind}:${selection}:${permissions}:${advanced}"
    val changed=agent?.let{current->state.members.find{it.coreText("agentId")==current.coreText("agentId")}?.coreLong("version")!=current.coreLong("version")}?:false
    suspend fun load(){loading=true;try{installations=model.query(RovaiOperation.RUNTIME_INSTALLATIONS_LIST).coreArray();issue=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{loading=false}}
    fun chooseRuntime(runtime:String,member:JsonObject?=agent){val source=member?.get("runtimeConfiguration").coreObject().takeIf{it.coreText("adapterKind")==runtime}?:installations.find{it.coreText("adapterKind")==runtime&&it.coreText("installationClass")=="managed_default"&&it.coreText("authScope")=="default"}?.get("memberRuntimeDefaults").coreObject();kind=runtime;selection=source["model"].coreObject().ifEmpty{buildJsonObject{put("mode","runtime_default")}};permissions=source["permissions"].coreObject();advanced=emptyMap();catalog=buildJsonObject{};picker=false}
    fun chooseMember(member:JsonObject){agent=member;chooseRuntime(member["runtimeConfiguration"].coreObject().coreText("adapterKind"));baseline="${kind}:${selection}:${permissions}:${advanced}"}
    fun permission(key:String,value:JsonElement){permissions=buildJsonObject{permissions.forEach{(name,item)->put(name,item)};put("values",buildJsonObject{permissions["values"].coreObject().forEach{(name,item)->put(name,item)};put(key,value)})}}
    LaunchedEffect(Unit){load()}
    LaunchedEffect(state.lastResult){val receipt=state.lastResult.coreObject()["commandResult"].coreObject().ifEmpty{state.lastResult.coreObject()};if(receipt.coreText("commandId")==submitted){if(receipt.coreText("status")=="applied"){agent=null;submitted=null;load()}else issue=receipt.coreText("code")};if(state.lastCompletedOperation==RovaiOperation.RUNTIME_PRODUCT_CHECK.wire)load()}
    BackHandler(enabled=agent!=null){if(dirty)discard=true else agent=null}
    if(custom)RovaiCustomRuntimeSettings(model,onClose={custom=false})
    val models=catalog["models"].coreArray().ifEmpty{snapshot["models"].coreArray()};val currentModel=models.find{it.coreText("id")==selection.coreText("modelId")}
    LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
        item{TextButton(enabled=!disabled&&!dirty,onClick={portable=true}){Text("模型服务商")}}
        item{TextButton(enabled=!disabled&&!dirty,onClick={migration=true}){Text("搬入原来的配置")}}
        item{TextButton(enabled=!disabled&&!dirty,onClick={export=true}){Text("导出完整配置")}}
        item{TextButton(enabled=!disabled&&!dirty,onClick={logging=true}){Text("主机日志输出")}}
        item{TextButton(enabled=!disabled&&!dirty,onClick={templates=true}){Text("任务模板")}}
        if(agent!=null)item{RovaiPortableMemberConnection(model,agent!!.coreText("agentId"),disabled||dirty||changed)}
        item{Row{if(agent!=null)TextButton(onClick={if(dirty)discard=true else agent=null}){Text("返回队员列表")};TextButton(enabled=!disabled,onClick={coroutine.launch{load()}}){Text("刷新")};TextButton(enabled=!disabled,onClick={custom=true}){Text("自定义智能体")}}}
        issue?.let{item{Text(it,color=CodegTheme.colors.danger)}}
        if(agent==null){items(state.members.filter{it.coreText("presence")!="removed"},key={it.coreText("agentId")}){member->GlassRow(Modifier.fillMaxWidth()){Text(member.coreText("displayName"),style=MaterialTheme.typography.titleMedium);Text(member["runtimeReadiness"].coreObject().coreText("status"),color=CodegTheme.colors.textSecondary);RovaiMemberFeedbackSwitch(model,member.coreText("agentId"));RovaiConversationToolSettings(model,member.coreText("agentId"));RovaiMemberNativeSteerSwitch(model,member.coreText("agentId"));RovaiMemberCustomRuntime(model,member);TextButton(enabled=!disabled,onClick={chooseMember(member)}){Text("模型和权限")}}}}
        else{
            item{Text(agent!!.coreText("displayName"),style=MaterialTheme.typography.titleLarge);if(changed)Text("队员信息已变化，当前更改保留；请返回后重新打开。",color=CodegTheme.colors.danger)}
            item{GlassRow(Modifier.fillMaxWidth()){Text("智能体",style=MaterialTheme.typography.titleMedium);TextButton(enabled=!disabled,onClick={picker=true}){Text(kind.ifEmpty{"选择智能体"})};if(installation!=null){Text(snapshot.coreText("probeStatus").ifEmpty{"尚未检查"},color=CodegTheme.colors.textSecondary);Text("认证："+snapshot.coreText("authenticationStatus"),style=MaterialTheme.typography.bodySmall);TextButton(enabled=!disabled,onClick={startup=true}){Text("启动程序与环境变量")};TextButton(enabled=!disabled,onClick={model.execute(RovaiOperation.RUNTIME_PRODUCT_CHECK,buildJsonObject{put("runtimeKind",kind)})}){Text("检查运行环境")}}}}
            if(installation!=null&&defaults.isNotEmpty()){
                item{GlassRow(Modifier.fillMaxWidth()){Text("模型",style=MaterialTheme.typography.titleMedium);Row{FilterChip(selected=selection.coreText("mode")=="runtime_default",onClick={selection=buildJsonObject{put("mode","runtime_default")}},enabled=!disabled,label={Text("使用智能体默认模型")});TextButton(enabled=!disabled,onClick={coroutine.launch{loading=true;try{catalog=model.query(RovaiOperation.RUNTIME_MODELCATALOG_OPEN,buildJsonObject{put("runtimeKind",kind);put("waitForRefresh",true)}).coreObject()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{loading=false}}}){Text("更新模型列表")}}
                    models.filter{it["hidden"]!=JsonPrimitive(true)}.forEach{descriptor->FilterChip(selected=selection.coreText("modelId")==descriptor.coreText("id")&&selection.coreText("mode")=="explicit",onClick={selection=buildJsonObject{put("mode","explicit");put("modelId",descriptor.coreText("id"));put("options",buildJsonObject{})}},enabled=!disabled,label={Text(descriptor.coreText("displayName").ifEmpty{descriptor.coreText("id")})})}
                }}
                items(currentModel?.get("options").coreArray(),key={"model:"+it.coreText("key")}){option->GlassRow(Modifier.fillMaxWidth()){Text(option.coreText("label"));option["values"].coreArray().forEach{choice->FilterChip(selected=selection["options"].coreObject().coreText(option.coreText("key"))==choice.coreText("value"),onClick={selection=buildJsonObject{selection.forEach{(key,value)->put(key,value)};put("options",buildJsonObject{selection["options"].coreObject().forEach{(key,value)->put(key,value)};put(option.coreText("key"),choice.coreText("value"))})}},enabled=!disabled,label={Text(choice.coreText("label"))})}}}
                items(installation["permissionOptions"].coreArray().ifEmpty{snapshot["permissionOptions"].coreArray()},key={"permission:"+it.coreText("key")}){option->val key=option.coreText("key");val value=permissions["values"].coreObject()[key]?:option["recommendedValue"]?:JsonNull;val enabled=!disabled&&option["supported"]!=JsonPrimitive(false)
                    GlassRow(Modifier.fillMaxWidth()){Text(option.coreText("label"),style=MaterialTheme.typography.titleSmall);Text(option.coreText("description"),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary);when(option.coreText("valueType")){
                        "boolean"->Switch(checked=value==JsonPrimitive(true),onCheckedChange={permission(key,JsonPrimitive(it))},enabled=enabled)
                        "enum"->option["choices"].coreArray().forEach{choice->FilterChip(selected=(value as? JsonPrimitive)?.contentOrNull==choice.coreText("value"),onClick={permission(key,JsonPrimitive(choice.coreText("value")))},enabled=enabled,label={Text(choice.coreText("label"))})}
                        "string_list"->OutlinedTextField(value=advanced[key]?:((value as? JsonArray)?.mapNotNull{(it as? JsonPrimitive)?.contentOrNull}?.joinToString("\n")?:""),onValueChange={text->advanced=advanced+(key to text);permission(key,JsonArray(text.lines().map{it.trim()}.filter{it.isNotEmpty()}.map(::JsonPrimitive)))},label={Text("每行一项")},maxLines=8,enabled=enabled)
                        "rule_list"->OutlinedTextField(value=advanced[key]?:value.toString(),onValueChange={advanced=advanced+(key to it)},label={Text("原生规则（JSON）")},maxLines=12,enabled=enabled)
                    };if(!enabled&&option.coreText("unsupportedReason").isNotEmpty())Text(option.coreText("unsupportedReason"),color=CodegTheme.colors.textSecondary)}
                }
                item{Button(enabled=!disabled&&!changed&&dirty,onClick={try{val options=installation["permissionOptions"].coreArray().ifEmpty{snapshot["permissionOptions"].coreArray()};options.filter{it.coreText("valueType")=="rule_list"&&advanced.containsKey(it.coreText("key"))}.forEach{permission(it.coreText("key"),Json.parseToJsonElement(advanced[it.coreText("key")]!!))};val id=UUID.randomUUID().toString();submitted=id;model.execute(RovaiOperation.MEMBERS_RUNTIME_SET,buildJsonObject{put("commandId",id);putJsonObject("command"){put("agentId",agent!!.coreText("agentId"));put("expectedVersion",agent!!.coreLong("version"));put("adapterKind",kind);put("model",selection);put("permissions",permissions)}})}catch(error:Exception){issue="原生权限规则格式不正确，请检查后保存。"}}){Text("保存模型和权限")}}
            }else item{Text("请先选择已配置的智能体；模型和权限以Host实际支持的选项为准。",color=CodegTheme.colors.textSecondary)}
        }
    }
    if(startup)RovaiRuntimeStartup(model,kind){startup=false}
    if(picker)AlertDialog(onDismissRequest={picker=false},title={Text("选择智能体")},text={LazyColumn{items(installations.filter{it.coreText("installationClass")=="managed_default"&&it.coreText("authScope")=="default"},key={it.coreText("id")}){item->TextButton(enabled=!disabled,onClick={chooseRuntime(item.coreText("adapterKind"))}){Text(item.coreText("adapterKind"))}}}},confirmButton={TextButton(onClick={picker=false}){Text("关闭")}})
    if(discard)AlertDialog(onDismissRequest={discard=false},title={Text("放弃未保存的运行参数？")},confirmButton={TextButton(onClick={discard=false;agent=null}){Text("放弃")}},dismissButton={TextButton(onClick={discard=false}){Text("继续编辑")}})
}
