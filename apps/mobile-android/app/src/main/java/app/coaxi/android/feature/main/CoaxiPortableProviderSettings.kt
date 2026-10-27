package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.coaxi.android.core.designsystem.component.CoaxiTextField
import app.coaxi.android.core.designsystem.component.GlassRow
import app.coaxi.android.core.designsystem.component.SecretField
import app.coaxi.android.core.designsystem.theme.CoaxiTheme
import app.coaxi.android.core.network.CoaxiOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import java.util.UUID

private val portableTypes=listOf("claude_code" to "Claude Code","codex" to "Codex","gemini" to "Gemini")
private val claudeFields=listOf("main" to "主模型","reasoning" to "推理模型","haiku" to "Haiku","sonnet" to "Sonnet","opus" to "Opus","fable" to "Fable","customOption" to "自定义选项","customOptionName" to "选项名称","customOptionDescription" to "选项说明")
private fun JsonObject.withField(key:String,value:JsonElement)=JsonObject(toMutableMap().apply{put(key,value)})
private fun portableModel(type:String):JsonObject=when(type){
    "claude_code"->buildJsonObject{put("kind","claude");claudeFields.forEach{put(it.first,"")}}
    "codex"->buildJsonObject{put("kind","codex");put("customs",JsonArray(emptyList()));put("excludedOfficials",JsonArray(emptyList()));put("default",JsonNull)}
    else->buildJsonObject{put("kind","gemini");put("model",JsonNull)}
}
private fun newPortableProvider()=buildJsonObject{put("schemaVersion",1);put("id",UUID.randomUUID().toString());put("revision",0);put("name","");put("agentType","claude_code");put("apiUrl","");put("credential",JsonNull);put("rawAgentTypesJson","");put("rawModel",JsonNull);put("model",portableModel("claude_code"));put("enabled",true)}

/** 沿原 ModelProvidersContent 的分组列表与编辑弹窗，只替换为融合 Core 的真实权威。 */
@Composable
fun CoaxiPortableProviderSettings(model:CoaxiWorkspaceViewModel,onClose:()->Unit){
    val state by model.state.collectAsStateWithLifecycle();val scope=rememberCoroutineScope()
    var providers by remember{mutableStateOf<List<JsonObject>>(emptyList())};var next by remember{mutableStateOf<String?>(null)}
    var filter by remember{mutableStateOf("")};var editor by remember{mutableStateOf<JsonObject?>(null)};var removal by remember{mutableStateOf<JsonObject?>(null)}
    var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)};var identity by remember{mutableStateOf("")};var pending by remember{mutableStateOf<JsonObject?>(null)}
    fun owner()="portable-providers/$identity"
    suspend fun current(){if(identity.isEmpty()||model.presentationIdentity()!=identity)throw CancellationException("工作区已切换")}
    suspend fun load(more:Boolean=false){current();val page=model.query(CoaxiOperation.CONFIGURATION_PROVIDERS_LIST,buildJsonObject{put("schemaVersion",1);put("after",if(more)next?.let(::JsonPrimitive)?:JsonNull else JsonNull);put("limit",100)}).coreObject();current();providers=if(more)providers+page["items"].coreArray() else page["items"].coreArray();next=(page["next"] as? JsonPrimitive)?.contentOrNull}
    suspend fun rememberPending(value:JsonObject?){current();model.saveBody(owner(),value?.toString()?:"");current();pending=value}
    suspend fun accept(result:JsonObject){
        val original=pending?:error("缺少原请求编号")
        require(result.coreText("commandId")==original.coreText("commandId")&&result.coreText("commandType")==original.coreText("operation")){"原请求归属不同"}
        require(result.coreText("status") in listOf("applied","rejected")){"原请求尚无确定回执，未重复发送"}
        result["payload"].coreObject().coreText("id").takeIf{it.isNotEmpty()}?.let{require(it==original.coreText("targetId")){"目标已变化"}}
        val rejected=result.coreText("status")=="rejected";rememberPending(null)
        if(rejected)error("保存被拒绝，请刷新后核对设置")
        editor=null;removal=null;load()
    }
    fun work(action:suspend ()->Unit){if(busy)return;busy=true;issue=null;scope.launch{try{action()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message?:"操作未完成"}finally{busy=false}}}
    LaunchedEffect(state.connected){
        if(!state.connected){identity="";providers=emptyList();editor=null;removal=null;pending=null;return@LaunchedEffect}
        busy=true
        try{identity=model.presentationIdentity();val raw=model.readBody(owner());pending=raw.takeIf{it.isNotBlank()}?.let{Json.parseToJsonElement(it).coreObject().also{value->require(value.keys==setOf("operation","commandId","targetId")&&value.coreText("operation") in listOf("configuration.providers.upsert","configuration.providers.remove"));UUID.fromString(value.coreText("commandId"));UUID.fromString(value.coreText("targetId"))}};load()}
        catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue="配置列表或原请求恢复失败"}finally{busy=false}
    }
    BackHandler(enabled=editor==null){onClose()}
    LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(12.dp),verticalArrangement=Arrangement.spacedBy(6.dp)){
        item{Row{TextButton(onClick=onClose,enabled=!busy){Text(stringResource(R.string.coaxi_fusion_572cf45ba43634b3))};TextButton(onClick={work{load()}},enabled=!busy){Text(stringResource(R.string.coaxi_fusion_aee88743413144a2))};TextButton(onClick={editor=newPortableProvider()},enabled=!busy&&pending==null&&identity.isNotEmpty()){Text(stringResource(R.string.coaxi_fusion_08328eb33ebae59c))}}}
        item{Row(horizontalArrangement=Arrangement.spacedBy(6.dp)){FilterChip(selected=filter.isEmpty(),onClick={filter=""},label={Text(stringResource(R.string.coaxi_fusion_5c55a67935af8f45))});portableTypes.forEach{(type,name)->FilterChip(selected=filter==type,onClick={filter=type},label={Text(name)})}}}
        if(pending!=null)item{GlassRow{Text(stringResource(R.string.coaxi_fusion_916efa5a50693c4f));TextButton(onClick={work{val original=pending?:return@work;val result=model.query(CoaxiOperation.COMMANDS_RECONCILE,buildJsonObject{put("operation",original["operation"]!!);put("params",buildJsonObject{put("commandId",original["commandId"]!!)})}).coreObject();current();if(result.coreText("state")!="recorded")error("原请求尚无确定回执，请稍后继续核对");accept(result["result"].coreObject())}},enabled=!busy){Text(stringResource(R.string.coaxi_fusion_d027c9aa232e6375))}}}
        if(issue!=null)item{Text(issue!!,color=MaterialTheme.colorScheme.error)}
        if(providers.isEmpty()&&!busy)item{Text(stringResource(R.string.coaxi_fusion_09692c252256f72a),modifier=Modifier.padding(24.dp),color=CoaxiTheme.colors.textSecondary)}
        items(providers.filter{filter.isEmpty()||it.coreText("agentType")==filter},key={it.coreText("id")}){provider->GlassRow{
            Text(provider.coreText("name"),style=MaterialTheme.typography.titleSmall)
            Text(portableTypes.find{it.first==provider.coreText("agentType")}?.second?:provider.coreText("agentType"),color=CoaxiTheme.colors.textSecondary)
            Text(provider.coreText("apiUrl"),maxLines=2,color=CoaxiTheme.colors.textTertiary)
            Row{TextButton(onClick={work{val original=model.privateConfiguration("provider",buildJsonObject{put("id",provider["id"]!!);put("revision",provider["revision"]!!);put("digest",provider["digest"]!!)});current();editor=original}},enabled=!busy&&pending==null){Text(stringResource(R.string.coaxi_fusion_051836569928a9f9))};TextButton(onClick={removal=provider},enabled=!busy&&pending==null){Text(stringResource(R.string.coaxi_fusion_6135d4159e892541))}}
        }}
        if(next!=null)item{TextButton(onClick={work{load(true)}},enabled=!busy){Text(stringResource(R.string.coaxi_fusion_af90a08fec8ee28d))}}
        if(busy)item{LinearProgressIndicator(Modifier.fillMaxWidth())}
    }
    editor?.let{original->PortableProviderEditor(original,busy||pending!=null,onClose={if(!busy)editor=null},onSave={definition,keyMode,key,secretOperation->work{
        current();var candidate=definition
        if(keyMode=="replace"){
            val secret=model.privateConfiguration("secrets",buildJsonObject{put("schemaVersion",1);put("operationId",secretOperation);put("purpose","provider_key");put("value",key)});current();candidate=candidate.withField("credential",secret["reference"]?:error("密钥引用缺失"))
        }else if(keyMode=="clear")candidate=candidate.withField("credential",JsonNull)
        val command=UUID.randomUUID().toString();rememberPending(buildJsonObject{put("operation","configuration.providers.upsert");put("commandId",command);put("targetId",candidate["id"]!!)})
        val result=model.query(CoaxiOperation.CONFIGURATION_PROVIDERS_UPSERT,buildJsonObject{put("schemaVersion",1);put("commandId",command);put("expectedRevision",original.coreLong("revision"));put("definition",candidate)}).coreObject();current();accept(result)
    }})}
    removal?.let{provider->AlertDialog(onDismissRequest={if(!busy)removal=null},title={Text(stringResource(R.string.coaxi_fusion_4fe918315cb93f73))},text={Text("移除 ${provider.coreText("name")} 后，不再用于新的选择；历史任务仍保留原版本。")},dismissButton={TextButton(onClick={removal=null},enabled=!busy){Text(stringResource(R.string.coaxi_fusion_2cd0f3be8738a86c))}},confirmButton={TextButton(enabled=!busy&&pending==null,onClick={work{val command=UUID.randomUUID().toString();rememberPending(buildJsonObject{put("operation","configuration.providers.remove");put("commandId",command);put("targetId",provider["id"]!!)});val result=model.query(CoaxiOperation.CONFIGURATION_PROVIDERS_REMOVE,buildJsonObject{put("schemaVersion",1);put("commandId",command);put("id",provider["id"]!!);put("expectedRevision",provider["revision"]!!)}).coreObject();current();accept(result)}}){Text(stringResource(R.string.coaxi_fusion_6135d4159e892541))}})}
}

@Composable
private fun PortableProviderEditor(original:JsonObject,disabled:Boolean,onClose:()->Unit,onSave:(JsonObject,String,String,String)->Unit){
    var value by remember(original.coreText("id")){mutableStateOf(original)};var key by remember{mutableStateOf("")};var keyMode by remember{mutableStateOf("keep")};var secretOperation by remember{mutableStateOf(UUID.randomUUID().toString())}
    var discard by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)}
    val dirty=value!=original||key.isNotEmpty()||keyMode!="keep"
    fun close(){if(dirty)discard=true else onClose()}
    val type=value.coreText("agentType");val settings=value["model"].coreObject()
    fun field(name:String,text:String){value=value.withField(name,JsonPrimitive(text))}
    fun modelField(name:String,item:JsonElement){value=value.withField("model",settings.withField(name,item))}
    fun custom(index:Int,name:String,item:JsonElement){val rows=settings["customs"].coreArray().toMutableList();rows[index]=rows[index].withField(name,item);modelField("customs",JsonArray(rows))}
    BackHandler{close()}
    AlertDialog(onDismissRequest={if(!disabled)close()},title={Text(if(original.coreLong("revision")==0L)"新增服务商" else "编辑服务商")},text={Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()),verticalArrangement=Arrangement.spacedBy(10.dp)){
        CoaxiTextField(value.coreText("name"),{field("name",it)},"名称")
        if(original.coreLong("revision")==0L)Row{portableTypes.forEach{(kind,name)->FilterChip(selected=type==kind,onClick={value=value.withField("agentType",JsonPrimitive(kind)).withField("model",portableModel(kind))},enabled=!disabled,label={Text(name)})}}
        CoaxiTextField(value.coreText("apiUrl"),{field("apiUrl",it)},"接口地址",mono=true)
        Row{listOf("keep" to "保留密钥","replace" to "替换","clear" to "清空").forEach{(mode,name)->FilterChip(selected=keyMode==mode,onClick={keyMode=mode},enabled=!disabled,label={Text(name)})}}
        if(keyMode=="replace")SecretField(key,{key=it;secretOperation=UUID.randomUUID().toString()},"API密钥")
        Row{Text(stringResource(R.string.coaxi_fusion_b17021e56396dfcc),modifier=Modifier.weight(1f));Switch(checked=value["enabled"]==JsonPrimitive(true),onCheckedChange={value=value.withField("enabled",JsonPrimitive(it))},enabled=!disabled)}
        when(type){
            "claude_code"->claudeFields.forEach{(name,label)->CoaxiTextField(settings.coreText(name),{modelField(name,JsonPrimitive(it))},label)}
            "gemini"->CoaxiTextField(settings.coreText("model"),{modelField("model",it.takeIf(String::isNotBlank)?.let(::JsonPrimitive)?:JsonNull)},"模型（可选）")
            "codex"->{
                settings["customs"].coreArray().forEachIndexed{index,row->
                    HorizontalDivider();Text("自定义模型 ${index+1}",style=MaterialTheme.typography.titleSmall)
                    CoaxiTextField(row.coreText("slug"),{custom(index,"slug",JsonPrimitive(it))},"模型编号")
                    CoaxiTextField(row.coreText("displayName"),{custom(index,"displayName",JsonPrimitive(it))},"显示名称")
                    CoaxiTextField(row.coreText("contextWindow"),{custom(index,"contextWindow",if(it.isBlank())JsonNull else it.toLongOrNull()?.let(::JsonPrimitive)?:JsonPrimitive(it))},"上下文窗口（可选）")
                    CoaxiTextField(row.coreText("base"),{custom(index,"base",JsonPrimitive(it))},"基于官方模型")
                    CoaxiPortableCodexOverrides(row["overrides"].coreObject(),disabled){custom(index,"overrides",it)}
                    TextButton(onClick={modelField("customs",JsonArray(settings["customs"].coreArray().filterIndexed{i,_->i!=index}))},enabled=!disabled){Text(stringResource(R.string.coaxi_fusion_bca0038ea705f998))}
                }
                TextButton(onClick={modelField("customs",JsonArray(settings["customs"].coreArray()+buildJsonObject{put("slug","");put("displayName","");put("contextWindow",JsonNull);put("base","");put("overrides",buildJsonObject{})}))},enabled=!disabled){Text(stringResource(R.string.coaxi_fusion_97f48676e143269c))}
                CoaxiTextField(settings["excludedOfficials"]?.jsonArray?.joinToString("\n"){it.jsonPrimitive.content}?:"",{modelField("excludedOfficials",JsonArray(it.lines().map(String::trim).filter(String::isNotEmpty).map(::JsonPrimitive)))},"隐藏的官方模型（每行一个）",singleLine=false)
                CoaxiTextField(settings.coreText("default"),{modelField("default",it.takeIf(String::isNotBlank)?.let(::JsonPrimitive)?:JsonNull)},"默认模型（可选）")
            }
        }
        if(issue!=null)Text(issue!!,color=MaterialTheme.colorScheme.error)
    }},dismissButton={TextButton(onClick={close()},enabled=!disabled){Text(stringResource(R.string.coaxi_fusion_2cd0f3be8738a86c))}},confirmButton={TextButton(enabled=!disabled&&value.coreText("name").isNotBlank()&&value.coreText("apiUrl").isNotBlank()&&(keyMode!="replace"||key.isNotBlank()),onClick={
        try{var candidate=value.filterKeys{it !in setOf("revision","digest","createdAt","updatedAt")}.let(::JsonObject);issue=null;onSave(candidate,keyMode,key,secretOperation)}catch(error:Exception){issue="请检查高级模型目录与数字字段"}
    }){Text(stringResource(R.string.coaxi_fusion_a3030bf8f16dc63c))}})
    if(discard)AlertDialog(onDismissRequest={discard=false},title={Text(stringResource(R.string.coaxi_fusion_85a9990cca9a9fa0))},text={Text(stringResource(R.string.coaxi_fusion_a46a02c27a7e7adc))},dismissButton={TextButton(onClick={discard=false}){Text(stringResource(R.string.coaxi_fusion_fd4b9e3b6c685bae))}},confirmButton={TextButton(onClick=onClose){Text(stringResource(R.string.coaxi_fusion_17f1ff1b24ca2d7d))}})
}
