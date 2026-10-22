package app.codeg.android.feature.main

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.codeg.android.core.designsystem.component.CodegTextField
import app.codeg.android.core.designsystem.component.GlassRow
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import java.util.UUID

@Composable
fun RovaiTaskTemplateSettings(model:RovaiWorkspaceViewModel,onClose:()->Unit){
    val scope=rememberCoroutineScope();val generation=model.connectionGeneration
    var identity by remember{mutableStateOf("")};var rows by remember{mutableStateOf<List<JsonObject>>(emptyList())};var next by remember{mutableStateOf<String?>(null)}
    var draft by remember{mutableStateOf<JsonObject?>(null)};var config by remember{mutableStateOf(buildJsonObject{})};var baseline by remember{mutableStateOf("")}
    var pending by remember{mutableStateOf<JsonObject?>(null)};var receipt by remember{mutableStateOf<JsonObject?>(null)}
    var busy by remember{mutableStateOf(false)};var ready by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)};var leaving by remember{mutableStateOf(false)};var removing by remember{mutableStateOf(false)}
    fun owner()="portable-task-templates/$identity"
    val dirty=draft!=null&&draft.toString()!=baseline
    suspend fun current(){if(identity.isEmpty()||generation!=model.connectionGeneration||model.presentationIdentity()!=identity)throw CancellationException("工作区已切换")}
    suspend fun call(input:JsonObject):JsonObject{val value=model.privateConfiguration("templates",input);current();return value}
    suspend fun remember(value:JsonObject?){model.saveBody(owner(),value?.toString().orEmpty());current();pending=value}
    suspend fun list(more:Boolean=false){val page=call(buildJsonObject{put("operation","list");put("limit",100);if(more)next?.let{put("after",it)}});val loaded=page["items"].coreArray().map{it.coreObject()};rows=if(more)(rows+loaded).distinctBy{it.coreText("id")}else loaded;next=page["next"]?.jsonPrimitive?.contentOrNull}
    suspend fun settle(value:JsonObject){require(value["commandId"]==pending?.get("commandId")){"原模板回执编号不同"};receipt=value;if(value.coreText("state")=="recorded"){
        if(value["result"].coreObject().coreText("status")=="rejected"){remember(null);error("模板已在其他位置变化，当前编辑仍保留，请重新读取")}
        if(value["result"].coreObject().coreText("status")=="applied"&&value["files"].coreArray().isNotEmpty()&&value["files"].coreArray().all{it.coreText("state")=="applied"}){remember(null);draft=null;baseline="";removing=false;list()}
    }}
    fun work(action:suspend()->Unit){if(busy)return;busy=true;issue=null;scope.launch{try{current();action()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(generation==model.connectionGeneration)issue=error.message?:"模板操作未完成"}finally{if(generation==model.connectionGeneration)busy=false}}}
    fun field(key:String,value:String){draft=draft?.let{JsonObject(it.toMutableMap().apply{put(key,JsonPrimitive(value))})}}
    suspend fun open(id:String){val value=call(buildJsonObject{put("operation","get");put("id",id)});require(value.isNotEmpty()){ "模板已被删除" };val parsed=Json.parseToJsonElement(value.coreText("config")).coreObject();require(parsed.isNotEmpty()){ "原模板需要先补齐配置，完整内容仍保留" };config=parsed;draft=buildJsonObject{put("id",value["id"]!!);put("revision",value["revision"]!!);put("name",value["name"]!!);put("title",value["title"]!!);put("body",parsed.coreText("display_text"))};baseline=draft.toString();removing=false}
    suspend fun mutate(operation:String){
        val current=draft?:return;require(pending==null&&current.coreText("name").isNotBlank()&&current.coreText("name").toByteArray().size<=512&&current.coreText("title").toByteArray().size<=4096){"请填写有效的名称和标题"}
        var written=false;val blocks=config["prompt_blocks"].coreArray().flatMap{block->if(block.coreObject().coreText("type")=="text"){if(written)emptyList()else{written=true;listOf(buildJsonObject{put("type","text");put("text",current.coreText("body"))})}}else listOf(block)}.toMutableList()
        if(!written)blocks.add(0,buildJsonObject{put("type","text");put("text",current.coreText("body"))})
        val content=JsonObject(config.toMutableMap().apply{put("display_text",JsonPrimitive(current.coreText("body")));put("prompt_blocks",JsonArray(blocks))}).toString();require(content.toByteArray().size<=1024*1024){"模板超过大小限制，请缩短说明或拆分材料"}
        val command=UUID.randomUUID().toString();remember(buildJsonObject{put("commandId",command);put("targetId",current["id"]!!)})
        settle(call(buildJsonObject{put("operation",operation);put("commandId",command);put("id",current["id"]!!);put("expectedRevision",current["revision"]?:JsonNull);if(operation=="put")put("value",buildJsonObject{put("name",current["name"]!!);put("title",current["title"]!!);put("config",content)})}))
    }
    LaunchedEffect(generation){identity="";ready=false;draft=null;pending=null;receipt=null;busy=true;try{
        identity=model.presentationIdentity();val raw=model.readBody(owner());if(raw.isNotEmpty()){val value=Json.parseToJsonElement(raw).coreObject();require(value.keys==setOf("commandId","targetId"));UUID.fromString(value.coreText("commandId"));UUID.fromString(value.coreText("targetId"));pending=value}
        list();pending?.let{settle(call(buildJsonObject{put("operation","receipt");put("commandId",it["commandId"]!!)}))};ready=true
    }catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message?:"模板恢复失败，原记录已保留"}finally{busy=false}}
    BackHandler{if(!busy){if(dirty)leaving=true else onClose()}}
    LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(12.dp),verticalArrangement=Arrangement.spacedBy(10.dp)){
        item{TextButton(enabled=!busy,onClick={if(dirty)leaving=true else onClose()}){Text("返回")};Text("任务模板",style=MaterialTheme.typography.titleLarge);Text("保存模板不会启动任务。原程序设置和材料随模板保留，使用时在本次使命里核对。")}
        item{Row{TextButton(enabled=ready&&!busy&&pending==null&&!dirty,onClick={config=buildJsonObject{put("config_values",buildJsonObject{})};draft=buildJsonObject{put("id",UUID.randomUUID().toString());put("revision",JsonNull);put("name","");put("title","");put("body","")};baseline=""}){Text("新建模板")};TextButton(enabled=!busy&&!dirty,onClick={work{list()}}){Text("刷新")}}}
        items(rows.size){index->val row=rows[index];TextButton(enabled=!busy&&pending==null&&!dirty,onClick={work{open(row.coreText("id"))}}){Text(row.coreText("name"))}}
        if(next!=null)item{TextButton(enabled=!busy,onClick={work{list(true)}}){Text("更多模板")}}
        draft?.let{value->item{GlassRow{
            CodegTextField(value.coreText("name"),{if(!busy&&pending==null)field("name",it)},"模板名称")
            CodegTextField(value.coreText("title"),{if(!busy&&pending==null)field("title",it)},"任务标题")
            CodegTextField(value.coreText("body"),{if(!busy&&pending==null)field("body",it)},"任务说明",singleLine=false)
            Text("只修改上述内容，其余原高级设置和非文字材料完整保留。",style=MaterialTheme.typography.bodySmall)
            Row{TextButton(enabled=!busy&&pending==null&&dirty,onClick={work{mutate("put")}}){Text("保存模板")};TextButton(enabled=!busy&&pending==null,onClick={draft=null;baseline="";removing=false}){Text("放弃编辑")};if(value["revision"]!=JsonNull)TextButton(enabled=!busy&&pending==null,onClick={removing=true}){Text("删除模板")}}
        }}}
        if(pending!=null)item{GlassRow{Text(if(receipt?.coreText("state")=="unknown")"尚未查到原结果，未自动重发。" else "原操作和文件结果分别保留。")
            TextButton(enabled=!busy,onClick={work{settle(call(buildJsonObject{put("operation","receipt");put("commandId",pending!!["commandId"]!!)}))}}){Text("核对原模板操作")}
            if(receipt?.coreText("state")=="recorded"&&receipt!!["files"].coreArray().any{it.coreText("state")=="pending"})TextButton(enabled=!busy,onClick={work{settle(call(buildJsonObject{put("operation","finish");put("commandId",pending!!["commandId"]!!)}))}}){Text("继续保存原模板文件")}
            if(receipt?.coreText("state")=="recorded"&&receipt!!["files"].coreArray().any{it.coreText("state") in listOf("conflict","failed")})TextButton(enabled=!busy,onClick={work{remember(null)}}){Text("保留原结果并重新编辑")}
        }}
        if(issue!=null)item{Text(issue!!,color=MaterialTheme.colorScheme.error)};if(busy)item{LinearProgressIndicator(Modifier.fillMaxWidth())}
    }
    if(removing)AlertDialog(onDismissRequest={removing=false},title={Text("删除这份模板？")},text={Text("已创建的任务、历史消息和原导入归档继续保留。")},confirmButton={TextButton(enabled=!busy,onClick={work{mutate("remove")}}){Text("确认删除")}},dismissButton={TextButton(enabled=!busy,onClick={removing=false}){Text("取消")}})
    if(leaving)AlertDialog(onDismissRequest={leaving=false},title={Text("放弃未保存的模板修改？")},confirmButton={TextButton(onClick=onClose){Text("放弃并返回")}},dismissButton={TextButton(onClick={leaving=false}){Text("继续编辑")}})
}
