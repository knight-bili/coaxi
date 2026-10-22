package app.codeg.android.feature.main

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import java.util.UUID

/** 沿用原模型表单的默认/指定切换，保留原请求和具体程序修订。 */
@Composable fun RovaiMemberCustomRuntime(model:RovaiWorkspaceViewModel,member:JsonObject){
    val id=member.coreText("agentId");val config=member["runtimeConfiguration"].coreObject();val saved=config["customRuntimeRef"].coreObject();val coroutine=rememberCoroutineScope()
    val generation=remember(id){model.connectionGeneration};val owner="custom-member-runtime:$id";val state=model.state.collectAsState().value
    var rows by remember(id){mutableStateOf<List<JsonObject>>(emptyList())};var next by remember(id){mutableStateOf<String?>(null)};var selected by remember(id){mutableStateOf(saved.coreText("id"))}
    var explicit by remember(id){mutableStateOf(config["model"].coreObject().coreText("mode")=="explicit")};var modelId by remember(id){mutableStateOf(config["model"].coreObject().coreText("modelId"))};var options by remember(id){mutableStateOf((config["model"].coreObject()["options"]?:buildJsonObject{}).toString())}
    var original by remember(id){mutableStateOf<JsonObject?>(null)};var busy by remember(id){mutableStateOf(false)};var issue by remember(id){mutableStateOf<String?>(null)}
    val current=rows.find{it.coreText("id")==selected};val disabled=busy||state.busy||state.pending!=null||generation!=model.connectionGeneration
    suspend fun load(more:Boolean=false){val result=model.query(RovaiOperation.RUNTIME_CUSTOM_LIST,buildJsonObject{put("after",if(more)next?.let(::JsonPrimitive)?:JsonNull else JsonNull);put("limit",100)}).coreObject();require(generation==model.connectionGeneration);rows=if(more)(rows+result["items"].coreArray()).distinctBy{it.coreText("id")}else result["items"].coreArray();next=(result["next"] as? JsonPrimitive)?.contentOrNull}
    fun work(block:suspend()->Unit){if(disabled)return;busy=true;coroutine.launch{try{block();issue=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(generation==model.connectionGeneration)issue=error.message}finally{busy=false}}}
    suspend fun apply(reconcile:Boolean){
        require(generation==model.connectionGeneration);val request=original?:run{val definition=current?:error("请选择自定义智能体");require(definition["enabled"]==JsonPrimitive(true)){"所选定义已停用"}
            val selection=if(!explicit)buildJsonObject{put("mode","runtime_default")}else{require(modelId.isNotBlank()){ "请填写模型标识" };require(options.toByteArray(Charsets.UTF_8).size<=65536){"模型参数超过64KB"};val parsed=Json.parseToJsonElement(options);require(parsed is JsonObject){"模型参数必须是 JSON 对象"};buildJsonObject{put("mode","explicit");put("modelId",modelId);put("options",parsed)}}
            buildJsonObject{put("commandId",UUID.randomUUID().toString());put("command",buildJsonObject{put("agentId",id);put("expectedVersion",member.coreLong("version"));put("adapterKind","custom-acp");put("model",selection);put("permissions",buildJsonObject{put("adapterKind","custom-acp");put("schemaVersion",1);put("values",buildJsonObject{})});put("customRuntimeRef",customRuntimeRef(definition))})}}
        model.saveBody(owner,request.toString());original=request
        val receipt=if(reconcile){val result=model.query(RovaiOperation.COMMANDS_RECONCILE,buildJsonObject{put("operation",RovaiOperation.MEMBERS_RUNTIME_SET.wire);put("params",request)}).coreObject();require(generation==model.connectionGeneration);require(result.coreText("state")=="recorded"){"原配置尚无确定回执，没有重复保存"};result["result"].coreObject()}else model.query(RovaiOperation.MEMBERS_RUNTIME_SET,request).coreObject()
        require(generation==model.connectionGeneration);require(receipt.coreText("status") in setOf("applied","rejected")){"原配置结果尚未确定"};model.clearBodyIfMatches(owner,request.toString(),generation);original=null
        require(receipt.coreText("status")=="applied"){receipt.coreText("code")};model.refreshNow();load()
    }
    LaunchedEffect(id){busy=true;try{val raw=model.readBody(owner);if(raw.isNotEmpty()){val parsed=Json.parseToJsonElement(raw).coreObject();require(parsed["command"].coreObject().coreText("agentId")==id){"原请求队员归属不同"};original=parsed};load()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    Column(verticalArrangement=Arrangement.spacedBy(6.dp)){
        Text("自定义智能体程序",style=MaterialTheme.typography.titleSmall);Text("从已添加的具体版本中选择。先在智能体设置中安装此版本。",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)
        rows.forEach{definition->FilterChip(selected=selected==definition.coreText("id"),onClick={selected=definition.coreText("id")},enabled=!disabled&&original==null&&definition["enabled"]==JsonPrimitive(true),label={Text("${definition.coreText("name")} · ${definition.coreText("version")} · r${definition.coreLong("revision")}")})}
        if(next!=null)TextButton(enabled=!disabled,onClick={work{load(true)}}){Text("更多程序")}
        if(saved.isNotEmpty())Text("当前保存：${saved.coreText("id")} · r${saved.coreLong("definitionRevision")}",style=MaterialTheme.typography.bodySmall)
        if(current!=null){Row{Text("指定模型",Modifier.weight(1f));Switch(explicit,{explicit=it},enabled=!disabled&&original==null)}
            if(explicit){OutlinedTextField(modelId,{modelId=it},label={Text("原程序模型标识")},enabled=!disabled&&original==null,modifier=Modifier.fillMaxWidth());OutlinedTextField(options,{options=it},label={Text("模型参数 JSON")},minLines=3,enabled=!disabled&&original==null,modifier=Modifier.fillMaxWidth())}
            Text("保存只影响之后的任务。模型和参数由真实智能体会话核对，失败会保留原因，不自动换模型。",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}
        issue?.let{Text(it,color=CodegTheme.colors.danger)}
        if(original!=null){Text(original!!.coreText("commandId"),style=MaterialTheme.typography.bodySmall);TextButton(enabled=!disabled,onClick={work{apply(true)}}){Text("核对原配置请求")}}
        else TextButton(enabled=!disabled&&current?.get("enabled")==JsonPrimitive(true),onClick={work{apply(false)}}){Text("使用所选程序")}
    }
}
